use std::sync::OnceLock;

use crate::commands::{enqueue_chat, CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::AllPlayerStats;

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["roast"],
    description: "AI roast of a player or server based on stats. Requires together_api_key. !roast <username|server>",
    whitelisted: false,
    execute,
};

// Serverless models confirmed working — Together serverless availability is account-tier
// specific and does not correlate with model name patterns. Ordered smallest/cheapest first.
const MODEL_PRIORITY: &[&str] = &[
    "meta-llama/Llama-3.3-70B-Instruct-Turbo",
    "Qwen/Qwen2.5-7B-Instruct-Turbo",
];
const FALLBACK_MODEL: &str = "meta-llama/Llama-3.3-70B-Instruct-Turbo";

static SELECTED_MODEL: OnceLock<String> = OnceLock::new();

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(roast_run(ctx))
}

async fn roast_run(ctx: CommandContext<'_>) -> anyhow::Result<()> {
    let api_key = ctx.runtime.together_api_key.trim().to_owned();
    if api_key.is_empty() {
        ctx.whisper("Roast is not configured on this server.");
        return Ok(());
    }

    let Some(&target) = ctx.args.first() else {
        ctx.chat(format!("{}roast <username|server>", ctx.runtime.prefix));
        return Ok(());
    };
    let target = target.to_owned();

    let prompt = if let Some(uuid) = ctx.state.api.convert_username_to_uuid(&target).await {
        if let Some(stats) = ctx.state.api.get_stats_by_uuid(&uuid, &ctx.state.mc_server).await {
            build_player_prompt(&target, &stats)
        } else if let Some(data) = ctx.state.api.get_server_summary(&target).await {
            build_server_prompt(&target, &data)
        } else {
            ctx.chat(format!("{target} not found."));
            return Ok(());
        }
    } else if let Some(data) = ctx.state.api.get_server_summary(&target).await {
        build_server_prompt(&target, &data)
    } else {
        ctx.chat(format!("{target} not found."));
        return Ok(());
    };

    let state = ctx.state.clone();
    let timeout_ms = ctx.runtime.roast_timeout_ms;

    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let model = pick_model(&client, &api_key).await;
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(timeout_ms),
            call_together(&client, &api_key, &prompt, model),
        )
        .await;

        let msg = match result {
            Ok(Some(roast)) => roast,
            _ => format!("Failed to roast {target}."),
        };

        enqueue_chat(&state, msg);
    });

    Ok(())
}

async fn pick_model(client: &reqwest::Client, api_key: &str) -> &'static str {
    if let Some(model) = SELECTED_MODEL.get() {
        return model.as_str();
    }

    let available = fetch_chat_model_ids(client, api_key).await;
    eprintln!("[roast] fetched {} chat models", available.len());
    let chosen = MODEL_PRIORITY
        .iter()
        .find(|&&m| available.iter().any(|a| a == m))
        .copied()
        .unwrap_or(FALLBACK_MODEL);

    eprintln!("[roast] selected model: {chosen}");
    let _ = SELECTED_MODEL.set(chosen.to_owned());
    SELECTED_MODEL.get().map(|s| s.as_str()).unwrap_or(FALLBACK_MODEL)
}

async fn fetch_chat_model_ids(client: &reqwest::Client, api_key: &str) -> Vec<String> {
    let Ok(resp) = client
        .get("https://api.together.ai/v1/models")
        .bearer_auth(api_key)
        .send()
        .await
    else {
        return vec![];
    };

    let Ok(body) = resp.json::<serde_json::Value>().await else {
        return vec![];
    };

    body.as_array()
        .map(|arr| {
            arr.iter()
                .filter(|m| m["type"].as_str() == Some("chat"))
                .filter_map(|m| m["id"].as_str().map(|s| s.to_owned()))
                .collect()
        })
        .unwrap_or_default()
}

fn build_player_prompt(username: &str, stats: &AllPlayerStats) -> String {
    let kills = stats.kills.unwrap_or(0);
    let deaths = stats.deaths.unwrap_or(0);
    let playtime_hrs = stats.playtime.map(|p| p / 1000 / 3600).unwrap_or(0);
    let joins = stats.joins.unwrap_or(0);
    let join_date = stats.join_date.as_deref().unwrap_or("unknown");
    let last_death = stats.last_death_string.as_deref().unwrap_or("unknown");

    let facts = [
        format!("{kills} kills"),
        format!("{deaths} deaths"),
        format!("{playtime_hrs}h playtime"),
        format!("{joins} server joins"),
        format!("first joined {join_date}"),
        format!("last death: \"{last_death}\""),
    ];
    let focus = &facts[rand_index(facts.len())];

    format!(
        "Write a single savage roast (1 sentence, max 180 characters, no quotation marks, \
         no emojis, plain text only) of a Minecraft player named {username}. \
         Base the roast on this one fact: {focus}. \
         Do not use the phrase \"more X than Y\". Output only the roast, nothing else."
    )
}

fn build_server_prompt(server: &str, data: &serde_json::Value) -> String {
    let players = data["total_players"].as_u64().unwrap_or(0);
    let msgs = data["total_messages"].as_u64().unwrap_or(0);
    let kills = data["total_kills"].as_u64().unwrap_or(0);
    let deaths = data["total_deaths"].as_u64().unwrap_or(0);
    let hrs = data["total_playtime_ms"].as_u64().unwrap_or(0) / 3_600_000;
    let top_chatter = data["top_chatter"]["name"].as_str().unwrap_or("unknown");

    let facts = [
        format!("{players} total players"),
        format!("{msgs} messages sent"),
        format!("{kills} kills and {deaths} deaths"),
        format!("{hrs}h total playtime"),
        format!("top chatter is {top_chatter}"),
    ];
    let focus = &facts[rand_index(facts.len())];

    format!(
        "Write a single savage roast (1 sentence, max 180 characters, no quotation marks, \
         no emojis, plain text only) of a Minecraft server named {server}. \
         Base the roast on this one fact: {focus}. \
         Do not use the phrase \"more X than Y\". Output only the roast, nothing else."
    )
}

fn rand_index(len: usize) -> usize {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    COUNTER.fetch_add(1, Ordering::Relaxed) % len
}

async fn call_together(client: &reqwest::Client, api_key: &str, prompt: &str, model: &str) -> Option<String> {
    eprintln!("[roast] calling Together with model: {model}");
    let resp = match client
        .post("https://api.together.ai/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&serde_json::json!({
            "model": model,
            "messages": [
                {
                    "role": "system",
                    "content": "You write funny, witty roasts suitable for public Minecraft chat. \
                                Never use slurs, hate speech, explicit or sexual language, or threats. \
                                Keep it PG-13. One sentence only, no quotation marks, no emojis."
                },
                {"role": "user", "content": prompt}
            ],
            "max_tokens": 80,
            "temperature": 0.9,
        }))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => { eprintln!("[roast] request failed: {e}"); return None; }
    };

    let status = resp.status();
    let text = match resp.text().await {
        Ok(t) => t,
        Err(e) => { eprintln!("[roast] failed to read body: {e}"); return None; }
    };

    if !status.is_success() {
        eprintln!("[roast] Together error {status}: {text}");
        return None;
    }

    let body: serde_json::Value = serde_json::from_str(&text).ok()?;
    body["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}
