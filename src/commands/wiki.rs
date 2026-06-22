use crate::commands::{CommandContext, CommandDefinition, CommandFuture};

pub const WIKI_COMMAND: CommandDefinition = CommandDefinition {
    names: &["wiki", "wikipedia"],
    description: "Search Wikipedia. Usage: {prefix}wiki <query> | {prefix}wiki random",
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
            ctx.whisper(format!(
                "Usage: {0}wiki <query> | {0}wiki random",
                ctx.runtime.prefix
            ));
            return Ok(());
        }

        let result = if ctx.args.len() == 1 && ctx.args[0].eq_ignore_ascii_case("random") {
            wiki_random().await
        } else {
            wiki_search_and_fetch(&ctx.args.join(" ")).await
        };

        match result {
            Some((text, url)) => {
                ctx.chat(text);
                ctx.whisper(url);
            }
            None => ctx.chat(format!("No Wikipedia article found for: {}", ctx.args.join(" "))),
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
        match minewiki_summary(&query).await {
            Some((text, url)) => {
                ctx.chat(text);
                ctx.whisper(url);
            }
            None => ctx.chat(format!("No Minecraft Wiki article found for: {query}")),
        }
        Ok(())
    })
}

async fn wiki_search_and_fetch(query: &str) -> Option<(String, String)> {
    let client = reqwest::Client::new();

    let search_json: serde_json::Value = client
        .get(format!(
            "https://en.wikipedia.org/w/api.php?action=query&list=search&srsearch={}&format=json&srlimit=1&utf8=1",
            percent_encode(query)
        ))
        .header("User-Agent", "ForestBot/1.0")
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    let title = search_json
        .pointer("/query/search/0/title")
        .and_then(|v| v.as_str())?
        .to_owned();

    fetch_wiki_rest(&client, &title).await
}

async fn wiki_random() -> Option<(String, String)> {
    let client = reqwest::Client::new();
    let json: serde_json::Value = client
        .get("https://en.wikipedia.org/api/rest_v1/page/random/summary")
        .header("User-Agent", "ForestBot/1.0")
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    format_rest_summary(&json)
}

async fn fetch_wiki_rest(client: &reqwest::Client, title: &str) -> Option<(String, String)> {
    let json: serde_json::Value = client
        .get(format!(
            "https://en.wikipedia.org/api/rest_v1/page/summary/{}",
            percent_encode(title)
        ))
        .header("User-Agent", "ForestBot/1.0")
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    if json.get("type").and_then(|v| v.as_str()) == Some("disambiguation") {
        let url = json
            .pointer("/content_urls/desktop/page")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();
        return Some((
            format!("[{title}] Disambiguation page — be more specific."),
            url,
        ));
    }

    format_rest_summary(&json)
}

fn format_rest_summary(json: &serde_json::Value) -> Option<(String, String)> {
    let title = json.get("title").and_then(|v| v.as_str())?;
    let extract = json.get("extract").and_then(|v| v.as_str())?;
    let url = json
        .pointer("/content_urls/desktop/page")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_owned();

    let first_line = extract
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())?;

    Some((format!("[{title}] {}", truncate(first_line, 200)), url))
}

async fn minewiki_summary(query: &str) -> Option<(String, String)> {
    let api_url = "https://minecraft.wiki/api.php";
    let client = reqwest::Client::new();

    let search_json: serde_json::Value = client
        .get(format!(
            "{}?action=query&list=search&srsearch={}&format=json&srlimit=1&utf8=1",
            api_url,
            percent_encode(query)
        ))
        .header("User-Agent", "ForestBot/1.0")
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    let title = search_json
        .pointer("/query/search/0/title")
        .and_then(|v| v.as_str())?
        .to_owned();

    let extract_json: serde_json::Value = client
        .get(format!(
            "{}?action=query&prop=extracts&exintro=1&explaintext=1&titles={}&format=json&utf8=1",
            api_url,
            percent_encode(&title)
        ))
        .header("User-Agent", "ForestBot/1.0")
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    let pages = extract_json.pointer("/query/pages")?.as_object()?;
    let page = pages.values().next()?;
    let extract = page.get("extract").and_then(|v| v.as_str())?;

    let first_line = extract
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())?;

    let url = format!("https://minecraft.wiki/w/{}", percent_encode(&title));
    Some((format!("[{title}] {}", truncate(first_line, 200)), url))
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        format!("{}...", s.chars().take(max - 3).collect::<String>())
    } else {
        s.to_owned()
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
