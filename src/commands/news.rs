use rss::Channel;

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["news"],
    description: "Browse BBC News. Usage: {prefix}news | {prefix}news <category> | {prefix}news [category] <N>",
    whitelisted: false,
    execute: execute,
};

const BASE: &str = "https://feeds.bbci.co.uk";

const FEEDS: &[(&str, &str)] = &[
    ("top",           "/news/rss.xml"),
    ("world",         "/news/world/rss.xml"),
    ("europe",        "/news/world/europe/rss.xml"),
    ("us",            "/news/world/us_and_canada/rss.xml"),
    ("tech",          "/news/technology/rss.xml"),
    ("science",       "/news/science_and_environment/rss.xml"),
    ("health",        "/news/health/rss.xml"),
    ("business",      "/news/business/rss.xml"),
    ("entertainment", "/news/entertainment_and_arts/rss.xml"),
    ("politics",      "/news/politics/rss.xml"),
];

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let args = &ctx.args;
        let last_is_index = args.last().and_then(|a| a.parse::<usize>().ok());

        match (args.as_slice(), last_is_index) {
            // !news
            ([], _) => {
                let cats = FEEDS.iter().map(|(k, _)| *k).collect::<Vec<_>>().join(", ");
                ctx.whisper(format!("Categories: {cats}"));
                match fetch_headlines("top").await {
                    Some(lines) => ctx.whisper(format!("Top stories: {lines}")),
                    None => ctx.whisper("Could not fetch top stories."),
                }
            }

            // !news <N>  →  article N from top stories
            ([_], Some(n)) => post_article(&ctx, "top", n).await,

            // !news <category>
            ([cat], None) => {
                let cat = cat.to_ascii_lowercase();
                if !known_category(&cat) {
                    ctx.whisper(format!("Unknown category '{cat}'. Use !news to see categories."));
                    return Ok(());
                }
                match fetch_headlines(&cat).await {
                    Some(lines) => ctx.whisper(format!("{cat}: {lines}")),
                    None => ctx.whisper(format!("Could not fetch {cat} news.")),
                }
            }

            // !news <category> <N>
            ([cat, ..], Some(n)) => {
                let cat = cat.to_ascii_lowercase();
                if !known_category(&cat) {
                    ctx.whisper(format!("Unknown category '{cat}'. Use !news to see categories."));
                    return Ok(());
                }
                post_article(&ctx, &cat, n).await;
            }

            _ => ctx.whisper(format!(
                "Usage: {p}news | {p}news <category> | {p}news [category] <N>",
                p = ctx.runtime.prefix
            )),
        }

        Ok(())
    })
}

async fn post_article(ctx: &CommandContext<'_>, category: &str, n: usize) {
    if n == 0 {
        ctx.whisper("Article numbers start at 1.");
        return;
    }
    let Some(channel) = fetch_channel(category).await else {
        ctx.whisper(format!("Could not fetch {category} news."));
        return;
    };
    let Some(item) = channel.items().get(n - 1) else {
        ctx.whisper(format!("No article {n} in {category} (feed has {} items).", channel.items().len()));
        return;
    };
    let title = item.title().unwrap_or("?");
    let desc = item.description().unwrap_or("").trim();
    let link = item.link().unwrap_or("").split('?').next().unwrap_or("");

    let desc = if desc.chars().count() > 160 {
        format!("{}...", desc.chars().take(157).collect::<String>())
    } else {
        desc.to_owned()
    };

    ctx.chat(format!("[{title}] {desc} | {link}"));
}

async fn fetch_headlines(category: &str) -> Option<String> {
    let channel = fetch_channel(category).await?;
    let lines = channel
        .items()
        .iter()
        .take(5)
        .enumerate()
        .map(|(i, item)| {
            let t = truncate(item.title().unwrap_or("?"), 40);
            format!("{}. {t}", i + 1)
        })
        .collect::<Vec<_>>()
        .join(" | ");
    Some(lines)
}

async fn fetch_channel(category: &str) -> Option<Channel> {
    let path = FEEDS.iter().find(|(k, _)| *k == category).map(|(_, p)| *p)?;
    let url = format!("{BASE}{path}");
    let bytes = reqwest::Client::new()
        .get(&url)
        .header("User-Agent", "ForestBot/1.0")
        .send()
        .await
        .ok()?
        .bytes()
        .await
        .ok()?;
    Channel::read_from(bytes.as_ref()).ok()
}

fn known_category(cat: &str) -> bool {
    FEEDS.iter().any(|(k, _)| *k == cat)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        format!("{}...", s.chars().take(max - 3).collect::<String>())
    } else {
        s.to_owned()
    }
}
