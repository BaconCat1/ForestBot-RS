use crate::commands::{enqueue_chat, utils::flag_content_if_needed, CommandContext, CommandDefinition, CommandFuture};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["ai"],
    description: "Ask an AI a question. Tries a chain of free providers in priority order.",
    whitelisted: false,
    bridge_ok: true,
    execute,
};

const SYSTEM_PROMPT: &str =
    "You are a helpful assistant in a Minecraft server chat. Reply in under 200 characters. Your scope is not limited to Minecraft, that is merely the context in which you are answering questions. Answer as helpfully as possible at all times.";
const SERVER_WIDE_COOLDOWN_SECS: u64 = 10;
const MAX_RESPONSE_CHARS: usize = 250;

#[derive(Debug, Clone, Deserialize)]
struct AiProviderSpec {
    name: String,
    base_url: String,
    #[serde(default)]
    key_field: String,
    preferred_models: Vec<String>,
    #[serde(default)]
    notes: String,
}

#[derive(Debug, Clone)]
pub struct AiProviderEntry {
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub preferred_models: Vec<String>,
}

pub async fn load_ai_providers(
    path: &str,
    api_keys: &crate::config::ApiKeys,
) -> Vec<AiProviderEntry> {
    let json = match tokio::fs::read_to_string(path).await {
        Ok(s) => s,
        Err(e) => {
            crate::structure::logger::warn(format!(
                "Could not load AI providers from {path}: {e}"
            ));
            return Vec::new();
        }
    };
    let specs: Vec<AiProviderSpec> = match serde_json::from_str(&json) {
        Ok(v) => v,
        Err(e) => {
            crate::structure::logger::warn(format!("Bad AI providers JSON: {e}"));
            return Vec::new();
        }
    };

    let mut entries = Vec::new();
    for spec in specs {
        let key = if spec.key_field.is_empty() {
            String::new()
        } else {
            let k = api_keys.get_ai_key(&spec.key_field).to_owned();
            if k.is_empty() {
                continue;
            }
            k
        };
        let base_url = spec
            .base_url
            .replace("{cloudflare_account_id}", &api_keys.ai_cloudflare_account_id);
        entries.push(AiProviderEntry {
            name: spec.name,
            base_url,
            api_key: key,
            preferred_models: spec.preferred_models,
        });
    }
    entries
}

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let prompt = ctx.args.join(" ");
        if prompt.is_empty() {
            ctx.whisper("Usage: !ai <question>");
            return Ok(());
        }

        {
            let mut last = ctx.state.last_ai_at.lock().expect("last_ai_at lock");
            if let Some(t) = *last {
                if t.elapsed() < Duration::from_secs(SERVER_WIDE_COOLDOWN_SECS) {
                    ctx.whisper("AI busy. Try again shortly.");
                    return Ok(());
                }
            }
            *last = Some(Instant::now());
        }

        let providers = ctx
            .state
            .ai_providers
            .read()
            .expect("ai_providers lock")
            .clone();
        let model_cache = &ctx.state.ai_model_cache;
        let http = &ctx.state.http;

        for provider in &providers {
            if let Some(response) = try_provider(http, model_cache, provider, &prompt).await {
                crate::structure::logger::info(format!("[AI] provider={}", provider.name));
                let response = response.trim();
                flag_content_if_needed(
                    ctx.state,
                    ctx.sender,
                    "ai",
                    &format!("Q: {prompt}\nA: {response}"),
                );
                let text = format!("[AI] {}", truncate(response, MAX_RESPONSE_CHARS));
                enqueue_chat(ctx.state, text);
                return Ok(());
            }
        }

        enqueue_chat(
            ctx.state,
            "No AI providers available, most likely due to usage exhaustion. Try again later.",
        );
        Ok(())
    })
}

async fn try_provider(
    http: &reqwest::Client,
    model_cache: &Arc<Mutex<HashMap<String, String>>>,
    provider: &AiProviderEntry,
    prompt: &str,
) -> Option<String> {
    let model = resolve_model(http, model_cache, provider).await;
    call_provider(http, provider, &model, prompt).await
}

async fn resolve_model(
    http: &reqwest::Client,
    model_cache: &Arc<Mutex<HashMap<String, String>>>,
    provider: &AiProviderEntry,
) -> String {
    {
        let cache = model_cache.lock().expect("ai_model_cache lock");
        if let Some(m) = cache.get(&provider.name) {
            return m.clone();
        }
    }

    let live = fetch_models(http, provider).await;
    let selected = select_model(&live, &provider.preferred_models);

    model_cache
        .lock()
        .expect("ai_model_cache lock")
        .insert(provider.name.clone(), selected.clone());

    selected
}

async fn fetch_models(http: &reqwest::Client, provider: &AiProviderEntry) -> Vec<String> {
    #[derive(Deserialize)]
    struct ModelsResponse {
        data: Vec<ModelEntry>,
    }
    #[derive(Deserialize)]
    struct ModelEntry {
        id: String,
    }

    let url = format!("{}/models", provider.base_url);
    let mut req = http.get(&url);
    if !provider.api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", provider.api_key));
    }
    let resp = req.send().await;

    match resp {
        Ok(r) if r.status().is_success() => r
            .json::<ModelsResponse>()
            .await
            .map(|m| m.data.into_iter().map(|e| e.id).collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn select_model(live: &[String], preferred: &[String]) -> String {
    for p in preferred {
        if live.contains(p) {
            return p.clone();
        }
    }
    live.first()
        .or_else(|| preferred.first())
        .cloned()
        .unwrap_or_default()
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    max_tokens: u32,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: MessageContent,
}

#[derive(Deserialize)]
struct MessageContent {
    content: String,
}

async fn call_provider(
    http: &reqwest::Client,
    provider: &AiProviderEntry,
    model: &str,
    prompt: &str,
) -> Option<String> {
    if model.is_empty() {
        return None;
    }

    let body = ChatRequest {
        model,
        messages: vec![
            ChatMessage {
                role: "system",
                content: SYSTEM_PROMPT,
            },
            ChatMessage {
                role: "user",
                content: prompt,
            },
        ],
        max_tokens: 150,
    };

    let url = format!("{}/chat/completions", provider.base_url);
    let mut req = http.post(&url).json(&body);
    if !provider.api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", provider.api_key));
    }
    let resp = req.send().await.ok()?;

    if !resp.status().is_success() {
        crate::structure::logger::debug(format!(
            "AI provider {} returned {} — skipping",
            provider.name,
            resp.status()
        ));
        return None;
    }

    let chat: ChatResponse = resp.json().await.ok()?;
    let content = chat.choices.into_iter().next()?.message.content;
    let content = content.trim();
    if content.is_empty() {
        crate::structure::logger::debug(format!(
            "AI provider {} returned empty content — skipping",
            provider.name
        ));
        return None;
    }
    Some(content.to_owned())
}

fn truncate(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        return s.to_owned();
    }
    let cut: String = chars[..max].iter().collect();
    match cut.rfind(|c: char| c.is_whitespace()) {
        Some(i) => format!("{}...", &cut[..i]),
        None => format!("{}...", cut),
    }
}
