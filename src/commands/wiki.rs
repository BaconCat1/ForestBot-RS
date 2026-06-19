use crate::commands::{CommandContext, CommandDefinition, CommandFuture};

pub const WIKI_COMMAND: CommandDefinition = CommandDefinition {
    names: &["wiki", "wikipedia"],
    description: "Search Wikipedia. Usage: {prefix}wiki <query>",
    whitelisted: false,
    execute: execute_wiki,
};

pub const MINEWIKI_COMMAND: CommandDefinition = CommandDefinition {
    names: &["minewiki", "mcwiki"],
    description: "Search the Minecraft wiki. Usage: {prefix}minewiki <query>",
    whitelisted: false,
    execute: execute_minewiki,
};

fn execute_wiki(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        if ctx.args.is_empty() {
            ctx.whisper(format!("Usage: {}wiki <query>", ctx.runtime.prefix));
            return Ok(());
        }
        let query = ctx.args.join(" ");
        match wiki_summary("https://en.wikipedia.org/w/api.php", &query).await {
            Some(result) => ctx.chat(result),
            None => ctx.chat(format!("No Wikipedia article found for: {query}")),
        }
        Ok(())
    })
}

fn execute_minewiki(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        if ctx.args.is_empty() {
            ctx.whisper(format!("Usage: {}minewiki <query>", ctx.runtime.prefix));
            return Ok(());
        }
        let query = ctx.args.join(" ");
        match wiki_summary("https://minecraft.wiki/api.php", &query).await {
            Some(result) => ctx.chat(result),
            None => ctx.chat(format!("No Minecraft Wiki article found for: {query}")),
        }
        Ok(())
    })
}

async fn wiki_summary(api_url: &str, query: &str) -> Option<String> {
    // Step 1: search for the best matching title
    let search_url = format!(
        "{}?action=query&list=search&srsearch={}&format=json&srlimit=1&utf8=1",
        api_url,
        percent_encode(query)
    );

    let search_resp = reqwest::Client::new()
        .get(&search_url)
        .header("User-Agent", "ForestBot/1.0")
        .send()
        .await
        .ok()?;

    let search_json: serde_json::Value = search_resp.json().await.ok()?;
    let title = search_json
        .pointer("/query/search/0/title")
        .and_then(|v| v.as_str())?
        .to_owned();

    // Step 2: fetch the intro extract for that title
    let extract_url = format!(
        "{}?action=query&prop=extracts&exintro=1&explaintext=1&titles={}&format=json&utf8=1",
        api_url,
        percent_encode(&title)
    );

    let extract_resp = reqwest::Client::new()
        .get(&extract_url)
        .header("User-Agent", "ForestBot/1.0")
        .send()
        .await
        .ok()?;

    let extract_json: serde_json::Value = extract_resp.json().await.ok()?;

    // Pages are keyed by page ID (unknown at this point), grab first value
    let pages = extract_json.pointer("/query/pages")?.as_object()?;
    let page = pages.values().next()?;
    let extract = page.get("extract").and_then(|v| v.as_str())?;

    // Take first non-empty line, trim to 200 chars
    let first_line = extract
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())?;

    let truncated = if first_line.chars().count() > 200 {
        format!("{}...", first_line.chars().take(197).collect::<String>())
    } else {
        first_line.to_owned()
    };

    Some(format!("[{title}] {truncated}"))
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
