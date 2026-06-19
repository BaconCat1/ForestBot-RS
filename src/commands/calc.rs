use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::logger;

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["calc", "wa", "wolframalpha"],
    description: "Query Wolfram|Alpha. Usage: {prefix}calc <query>",
    whitelisted: false,
    execute: execute,
};

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        if ctx.args.is_empty() {
            ctx.whisper(format!("Usage: {}calc <query>", ctx.runtime.prefix));
            return Ok(());
        }
        let app_id = ctx.runtime.wolfram_app_id.clone();
        if app_id.is_empty() {
            ctx.whisper("Wolfram|Alpha is not configured.".to_owned());
            return Ok(());
        }
        let query = ctx.args.join(" ");
        match wolfram_query(&app_id, &query).await {
            Some(result) => ctx.chat(result),
            None => ctx.chat(format!("No result for: {query}")),
        }
        Ok(())
    })
}

async fn wolfram_query(app_id: &str, query: &str) -> Option<String> {
    let url = format!(
        "https://www.wolframalpha.com/api/v1/llm-api?input={}&appid={}&maxchars=500",
        percent_encode(query),
        app_id,
    );

    let text = reqwest::Client::new()
        .get(&url)
        .header("User-Agent", "ForestBot/1.0")
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;

    if text.trim().is_empty() {
        return None;
    }

    logger::debug(format!("[calc] WA raw response:\n{text}"));

    // WA response: each section is a bare label line ("Result:") followed by value on next line
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let answer = ["Result:", "Input:", "Answer:"]
        .iter()
        .find_map(|label| {
            lines
                .iter()
                .position(|l| l.eq_ignore_ascii_case(label))
                .and_then(|pos| lines.get(pos + 1).copied())
        })?;

    let display = format!("{query} = {answer}");
    if display.chars().count() > 220 {
        Some(format!("{}...", display.chars().take(217).collect::<String>()))
    } else {
        Some(display)
    }
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
