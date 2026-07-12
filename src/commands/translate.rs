use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use serde::Deserialize;
use serde_json::json;
use whatlang::{detect, Lang};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["translate", "tr", "tl"],
    description: "Translate text or a player's last message. Defaults to English. Usage: {prefix}translate [lang] <text|player>",
    whitelisted: false,
    bridge_ok: true,
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

fn looks_like_lang_code(s: &str) -> bool {
    // BCP-47 subtag: 2-3 lowercase letters, optionally followed by hyphen + 2-6 chars
    let mut parts = s.split('-');
    let primary = parts.next().unwrap_or("");
    if primary.len() < 2 || primary.len() > 3 || !primary.chars().all(|c| c.is_ascii_lowercase()) {
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

        let key = ctx.runtime.azure_translator_key.clone();
        let region = ctx.runtime.azure_translator_region.clone();
        if key.is_empty() {
            ctx.whisper("Azure Translator is not configured.".to_owned());
            return Ok(());
        }

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

        match azure_translate(&key, &region, &text, lang).await {
            Some((translated, detected)) => {
                let from = detected.as_deref().unwrap_or("?");
                ctx.chat(format!("[{from}→{lang}] {translated}"));
            }
            None => ctx.whisper("Translation failed. Check lang code (e.g. es, fr, ja, zh-Hans).".to_owned()),
        }

        Ok(())
    })
}

async fn azure_translate(
    key: &str,
    region: &str,
    text: &str,
    lang: &str,
) -> Option<(String, Option<String>)> {
    let url = format!(
        "https://api.cognitive.microsofttranslator.com/translate?api-version=3.0&to={}",
        lang
    );

    let body = json!([{ "Text": text }]);

    let resp = reqwest::Client::new()
        .post(&url)
        .header("Ocp-Apim-Subscription-Key", key)
        .header("Ocp-Apim-Subscription-Region", region)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let mut results: Vec<TranslateResponse> = resp.json().await.ok()?;
    let result = results.pop()?;
    let translated = result.translations.into_iter().next()?.text;
    let detected = result.detected_language.map(|d| d.language);

    Some((translated, detected))
}
