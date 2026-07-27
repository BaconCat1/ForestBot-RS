use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use serde::Deserialize;
use serde_json::json;
use std::sync::atomic::Ordering;
use translators::{GoogleTranslator, Translator};
use whatlang::{detect, Lang};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["translate", "tr", "tl"],
    description: "Translate text or a player's last message. Defaults to English. Usage: {prefix}translate [lang] <text|player>",
    whitelisted: false,
    execute,
};

#[derive(Deserialize)]
struct TranslateResponse {
    translations: Vec<Translation>,
    #[serde(rename = "detectedLanguage")]
    detected_language: Option<DetectedLanguage>,
}

#[derive(Deserialize)]
struct Translation {
    text: String,
}

#[derive(Deserialize)]
struct DetectedLanguage {
    language: String,
}

#[derive(Deserialize)]
struct GoogleCloudResponse {
    data: GoogleCloudData,
}

#[derive(Deserialize)]
struct GoogleCloudData {
    translations: Vec<GoogleCloudTranslation>,
}

#[derive(Deserialize)]
struct GoogleCloudTranslation {
    #[serde(rename = "translatedText")]
    translated_text: String,
    #[serde(rename = "detectedSourceLanguage")]
    detected_source_language: Option<String>,
}

// Google Translate's standard supported-language set (2-letter ISO 639-1, mostly). An
// allow-list, not a shape check -- ISO 639-3 alone assigns ~7000 three-letter codes, so a
// bare "2-3 lowercase letters" shape heuristic collides with ordinary short words in every
// source language (e.g. German "was" = a real code for Washo, silently ate the whole
// sentence as a bogus target lang instead of translating it -- 2026-07-24 prod report).
const KNOWN_LANG_CODES: &[&str] = &[
    "af", "am", "ar", "az", "be", "bg", "bn", "bs", "ca", "ceb", "co", "cs", "cy", "da", "de",
    "el", "en", "eo", "es", "et", "eu", "fa", "fi", "fr", "fy", "ga", "gd", "gl", "gu", "ha",
    "haw", "he", "hi", "hmn", "hr", "ht", "hu", "hy", "id", "ig", "is", "it", "iw", "ja", "jw",
    "ka", "kk", "km", "kn", "ko", "ku", "ky", "la", "lb", "lo", "lt", "lv", "mg", "mi", "mk",
    "ml", "mn", "mr", "ms", "mt", "my", "ne", "nl", "no", "ny", "pa", "pl", "ps", "pt", "ro",
    "ru", "rw", "sd", "si", "sk", "sl", "sm", "sn", "so", "sq", "sr", "st", "su", "sv", "sw",
    "ta", "te", "tg", "th", "tl", "tr", "uk", "ur", "uz", "vi", "xh", "yi", "yo", "zh", "zu",
];

fn looks_like_lang_code(s: &str) -> bool {
    // BCP-47 subtag: known primary code, optionally followed by hyphen + 2-6 chars (e.g. zh-Hans)
    let mut parts = s.split('-');
    let primary = parts.next().unwrap_or("").to_ascii_lowercase();
    if !KNOWN_LANG_CODES.contains(&primary.as_str()) {
        return false;
    }
    parts.all(|sub| sub.len() >= 2 && sub.len() <= 6 && sub.chars().all(|c| c.is_ascii_alphanumeric()))
}

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        if ctx.args.is_empty() {
            ctx.whisper(format!(
                "Usage: {}translate [lang] <text|player>  (e.g. {}translate Bonjour  or  {}translate es hello)",
                ctx.runtime.prefix, ctx.runtime.prefix, ctx.runtime.prefix
            ));
            return Ok(());
        }

        // Each fallback tier below self-guards on its own missing config (empty key /
        // disabled toggle) -- unlike the old single-provider version, an unconfigured or
        // dead tier no longer aborts the whole command, it just falls through to the next.
        let azure_key = ctx.runtime.azure_translator_key.clone();
        let azure_region = ctx.runtime.azure_translator_region.clone();
        let gcloud_key = ctx.runtime.google_cloud_translate_key.clone();
        let scrape_enabled = ctx.runtime.google_scrape_enabled;
        let scrape_min_interval_ms = ctx.runtime.google_scrape_min_interval_ms;

        // If first arg looks like a lang code and there are more args, treat it as target lang.
        // Otherwise default to English and treat all args as input.
        let (lang, input_args) = if ctx.args.len() >= 2 && looks_like_lang_code(ctx.args[0]) {
            (ctx.args[0], &ctx.args[1..])
        } else {
            ("en", &ctx.args[..])
        };

        // Single word — check if it's an online player
        let text = if input_args.len() == 1 {
            let candidate = input_args[0];
            let is_online = {
                let players = ctx.state.players.read().expect("player cache lock poisoned");
                players.contains_key(candidate)
            };
            if is_online {
                match ctx.state.api.get_messages(candidate, &ctx.state.mc_server, 1, "DESC", 0).await
                    .and_then(|mut rows| rows.pop())
                {
                    Some(row) => row.message,
                    None => {
                        ctx.whisper(format!("{candidate} has no recorded messages."));
                        return Ok(());
                    }
                }
            } else {
                candidate.to_owned()
            }
        } else {
            input_args.join(" ")
        };

        let source_is_english = detect(&text).is_some_and(|info| {
            info.lang() == Lang::Eng
                && info.is_reliable()
                && text.split_whitespace().count() >= 4
        });
        if source_is_english {
            ctx.whisper("Translate is for non-English messages.".to_owned());
            return Ok(());
        }
        // Disabled: allow FROM-English (e.g. for personal use). Re-enable by removing the block above.
        // if lang == "en" { ... }

        match translate_chain(
            ctx.state,
            &azure_key,
            &azure_region,
            &gcloud_key,
            scrape_enabled,
            scrape_min_interval_ms,
            &text,
            lang,
        )
        .await
        {
            Some((translated, detected)) => {
                let from = detected.as_deref().unwrap_or("?");
                ctx.chat_success(format!("[{from}→{lang}] {translated}"));
            }
            None => ctx.whisper("Translation failed on all providers. Try again later.".to_owned()),
        }

        Ok(())
    })
}

// Tries each translation provider in order, cheapest/best-quality first, falling through
// on any failure (bad/missing key, dead subscription, quota, network error) rather than
// aborting the command -- see REFERENCE_MATERIAL/translation_tests/comparison.md for the
// quality/cost comparison behind this ordering.
async fn translate_chain(
    state: &crate::structure::mineflayer::bot::AzaleaState,
    azure_key: &str,
    azure_region: &str,
    gcloud_key: &str,
    scrape_enabled: bool,
    scrape_min_interval_ms: u64,
    text: &str,
    lang: &str,
) -> Option<(String, Option<String>)> {
    if let Some(r) = azure_translate(azure_key, azure_region, text, lang).await {
        eprintln!("[Translate] served by tier=azure");
        return Some(r);
    }
    if let Some(r) = google_cloud_translate(gcloud_key, text, lang).await {
        eprintln!("[Translate] served by tier=google_cloud");
        return Some(r);
    }
    if scrape_enabled {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let last_ms = state.google_scrape_last_call_ms.load(Ordering::Relaxed);
        let on_cooldown = now_ms.saturating_sub(last_ms) < scrape_min_interval_ms;
        if !on_cooldown {
            // Reserve the slot before the request fires (not after success) -- a failed
            // attempt still counts against the cooldown, so a block/error doesn't just get
            // retried instantly.
            state.google_scrape_last_call_ms.store(now_ms, Ordering::Relaxed);
            if let Some(r) = google_scrape_translate(text, lang).await {
                eprintln!("[Translate] served by tier=google_scrape");
                return Some(r);
            }
        }
    }
    let result = llm_translate(state, text, lang).await;
    if result.is_some() {
        eprintln!("[Translate] served by tier=llm_chain");
    }
    result
}

async fn azure_translate(
    key: &str,
    region: &str,
    text: &str,
    lang: &str,
) -> Option<(String, Option<String>)> {
    if key.is_empty() {
        return None;
    }
    let url = format!(
        "https://api.cognitive.microsofttranslator.com/translate?api-version=3.0&to={}",
        lang
    );

    let body = json!([{ "Text": text }]);

    let resp = match reqwest::Client::new()
        .post(&url)
        .header("Ocp-Apim-Subscription-Key", key)
        .header("Ocp-Apim-Subscription-Region", region)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[Translate] request failed: {e:?}");
            return None;
        }
    };

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        eprintln!("[Translate] Azure returned {status}: {body}");
        return None;
    }

    let body = match resp.text().await {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[Translate] failed to read response body: {e:?}");
            return None;
        }
    };
    let mut results: Vec<TranslateResponse> = match serde_json::from_str(&body) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[Translate] failed to parse response: {e:?} body={body}");
            return None;
        }
    };
    let result = results.pop()?;
    let translated = result.translations.into_iter().next()?.text;
    let detected = result.detected_language.map(|d| d.language);

    Some((translated, detected))
}

// Google Cloud Translation Basic (v2) -- permanent, recurring 500K-chars/month free tier
// (separate from the 90-day/$300 GCP trial credit), not a one-shot trial like Azure's.
// Recommended setup: hard-cap the project's quota at/under the free allowance in the
// Cloud Console (IAM & Admin -> Quotas) so overage fails clean with a 403 instead of
// billing -- see REFERENCE_MATERIAL/translation_tests/comparison.md for the writeup.
async fn google_cloud_translate(key: &str, text: &str, lang: &str) -> Option<(String, Option<String>)> {
    if key.is_empty() {
        return None;
    }
    let url = format!("https://translation.googleapis.com/language/translate/v2?key={key}");
    let body = json!({ "q": text, "target": lang, "format": "text" });

    let resp = match reqwest::Client::new().post(&url).json(&body).send().await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[Translate] Google Cloud request failed: {e:?}");
            return None;
        }
    };

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        eprintln!("[Translate] Google Cloud returned {status}: {body}");
        return None;
    }

    let body = match resp.text().await {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[Translate] Google Cloud failed to read response body: {e:?}");
            return None;
        }
    };
    let parsed: GoogleCloudResponse = match serde_json::from_str(&body) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[Translate] Google Cloud failed to parse response: {e:?} body={body}");
            return None;
        }
    };
    let translation = parsed.data.translations.into_iter().next()?;
    Some((translation.translated_text, translation.detected_source_language))
}

// Unofficial Google Translate web scrape (translators crate) -- no key, no official quota,
// but real IP-ban risk if hammered (bans reported at ~40-50 requests/burst with no delay,
// self-expire after a few hours; see comparison.md). Gated two ways: `scrape_enabled` lets
// this tier be killed instantly without a recompile if it ever gets flagged, and the
// interval/cooldown check lives in `translate_chain` (the orchestrator) rather than here --
// a call inside the cooldown window doesn't block/delay the chat response, it just falls
// through to the LLM tier immediately. Doesn't return a detected source language; the crate
// only gives back the translated text.
async fn google_scrape_translate(text: &str, lang: &str) -> Option<(String, Option<String>)> {
    let translator = GoogleTranslator::builder().timeout(10usize).delay(0usize).build();
    match translator.translate_async(text, "", lang).await {
        Ok(translated) => Some((translated, None)),
        Err(e) => {
            eprintln!("[Translate] Google scrape failed: {e}");
            None
        }
    }
}

// Last-resort tier: free LLM provider chain, reusing !ai's own fallback infra
// (ai::run_provider_chain) with a translation-flavored system prompt instead of the Q&A
// one. No ban-risk category at all -- worst case here is a clean quota/auth error per
// provider, same as !ai's own failure mode.
async fn llm_translate(
    state: &crate::structure::mineflayer::bot::AzaleaState,
    text: &str,
    lang: &str,
) -> Option<(String, Option<String>)> {
    let providers = state.ai_providers.read().expect("ai_providers lock").clone();
    let system_prompt = format!(
        "You are a translation engine. Translate the user's message to the language with \
         BCP-47/ISO code \"{lang}\". Reply with ONLY the translation, nothing else -- no \
         quotes, no explanation, no additional commentary."
    );
    let (provider_name, response) = crate::commands::ai::run_provider_chain(
        &state.http,
        &state.ai_model_cache,
        &providers,
        &system_prompt,
        text,
    )
    .await?;
    eprintln!("[Translate] LLM chain succeeded via provider={provider_name}");
    Some((response.trim().to_owned(), None))
}
