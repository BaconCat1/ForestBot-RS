use crate::commands::{CommandContext, CommandDefinition, CommandFuture};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["urbandictionary", "ud"],
    description: "Search Urban Dictionary. Usage: {prefix}ud <query>",
    whitelisted: false,
    execute: execute,
};

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        if ctx.args.is_empty() {
            ctx.whisper(format!("Usage: {}ud <query>", ctx.runtime.prefix));
            return Ok(());
        }
        let query = ctx.args.join(" ");
        match ud_lookup(&query).await {
            Some(result) => ctx.chat(result),
            None => ctx.chat(format!("No Urban Dictionary entry found for: {query}")),
        }
        Ok(())
    })
}

async fn ud_lookup(query: &str) -> Option<String> {
    let url = format!(
        "https://api.urbandictionary.com/v0/define?term={}",
        percent_encode(query)
    );

    let resp = reqwest::Client::new()
        .get(&url)
        .header("User-Agent", "ForestBot/1.0")
        .send()
        .await
        .ok()?;

    let json: serde_json::Value = resp.json().await.ok()?;
    let entry = json.pointer("/list/0")?;

    let word = entry.get("word").and_then(|v| v.as_str())?;
    let definition = entry.get("definition").and_then(|v| v.as_str())?;
    let thumbs_up = entry.get("thumbs_up").and_then(|v| v.as_u64()).unwrap_or(0);
    let thumbs_down = entry.get("thumbs_down").and_then(|v| v.as_u64()).unwrap_or(0);

    // Strip [bracket] link syntax UD uses inside definitions
    let clean = definition.replace('[', "").replace(']', "");
    let clean = clean.replace('\r', "").replace('\n', " ");
    let clean = clean.trim();

    let truncated = if clean.chars().count() > 180 {
        format!("{}...", clean.chars().take(177).collect::<String>())
    } else {
        clean.to_owned()
    };

    Some(format!("[{word}] {truncated} (+{thumbs_up}/-{thumbs_down})"))
}

fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}
