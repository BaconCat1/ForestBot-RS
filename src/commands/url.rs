use std::collections::HashSet;

use crate::{
    commands::{utils::flag_content_if_needed, CommandContext, CommandDefinition, CommandFuture},
    config::load_word_list,
    structure::mineflayer::{url_blocklist::is_blocked, utils::profanity_filter::censor_bad_words},
};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["url", "preview", "www"],
    description: "Preview a URL's description. Usage: {prefix}url <link>",
    whitelisted: false,
    execute: execute,
};

const BAD_WORDS_PATH: &str = "./json/bad_words.json";
const WORD_WHITELIST_PATH: &str = "./json/word_whitelist.json";

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        if ctx.args.is_empty() {
            ctx.whisper(format!("Usage: {}url <link>", ctx.runtime.prefix));
            return Ok(());
        }

        let url = ctx.args[0];

        // Blocklist check — refuse if still loading; clone so guard drops before awaits
        let blocklist = {
            let guard = ctx.state.url_blocklist.read().expect("url_blocklist read");
            match guard.as_ref() {
                None => {
                    ctx.whisper("Blocklist still loading, try again shortly.");
                    return Ok(());
                }
                Some(bl) => {
                    let domain = extract_domain(url);
                    if is_blocked(&domain, bl) {
                        ctx.whisper("Link blocked.");
                        return Ok(());
                    }
                    bl.clone()
                }
            }
        };

        let key = ctx.runtime.google_safe_browsing_key.clone();

        if key.is_empty() {
            ctx.whisper("URL preview unavailable: api_keys.google_safe_browsing not configured.");
            return Ok(());
        }

        match safe_browsing_check(url, &key).await {
            Ok(true) => {
                ctx.whisper("Link blocked.");
                return Ok(());
            }
            Ok(false) => {}
            Err(e) => {
                ctx.whisper(format!("Safe Browsing check failed: {e}"));
                return Ok(());
            }
        }

        match fetch_preview(url, &blocklist).await {
            None => ctx.whisper("No preview available."),
            Some((title, description)) => {
                let bad_words = load_word_list(BAD_WORDS_PATH).await.unwrap_or_default();
                let word_whitelist = load_word_list(WORD_WHITELIST_PATH).await.unwrap_or_default();
                let censored = censor_bad_words(&description, &bad_words, &word_whitelist);
                if censored != description {
                    flag_content_if_needed(ctx.state, ctx.sender, "url", &format!("{url}\n{description}"));
                }
                let truncated = truncate_word_boundary(&censored, 180);
                ctx.chat(format!("[{title}] {truncated}"));
            }
        }

        Ok(())
    })
}

async fn safe_browsing_check(url: &str, key: &str) -> anyhow::Result<bool> {
    let client = reqwest::Client::new();
    let endpoint = format!(
        "https://safebrowsing.googleapis.com/v4/threatMatches:find?key={key}"
    );
    let body = serde_json::json!({
        "client": { "clientId": "forestbot", "clientVersion": "1.0" },
        "threatInfo": {
            "threatTypes": ["MALWARE", "SOCIAL_ENGINEERING", "UNWANTED_SOFTWARE", "POTENTIALLY_HARMFUL_APPLICATION"],
            "platformTypes": ["ANY_PLATFORM"],
            "threatEntryTypes": ["URL"],
            "threatEntries": [{ "url": url }]
        }
    });

    let resp = client.post(&endpoint).json(&body).send().await?;

    if resp.status() == 429 {
        anyhow::bail!("Safe Browsing rate limit reached");
    }
    if !resp.status().is_success() {
        anyhow::bail!("Safe Browsing API error: {}", resp.status());
    }

    let json: serde_json::Value = resp.json().await?;
    Ok(json.get("matches").is_some())
}

const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024; // 2 MB hard ceiling

async fn fetch_preview(url: &str, blocklist: &HashSet<String>) -> Option<(String, String)> {
    // 1. Scheme allowlist
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return None;
    }

    // 2. SSRF: resolve host, reject private/reserved IP ranges before fetching
    let host = extract_domain(url);

    if let Ok(mut addrs) = tokio::net::lookup_host(format!("{host}:80")).await {
        if let Some(addr) = addrs.next() {
            if is_private_ip(addr.ip()) {
                return None;
            }
        }
    }

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .user_agent("ForestBot/1.0")
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            crate::structure::logger::warn(format!("url: client build failed: {e}"));
            return None;
        }
    };

    let resp = match client.get(url).send().await {
        Ok(r) => r,
        Err(e) => {
            crate::structure::logger::warn(format!("url: fetch failed for {url}: {e}"));
            return None;
        }
    };

    // Check final URL after redirect resolution
    let final_domain = resp.url().host_str().unwrap_or("").to_owned();
    if is_blocked(&final_domain, blocklist) {
        crate::structure::logger::warn(format!("url: post-redirect domain blocked: {final_domain}"));
        return None;
    }

    // SSRF: also check final IP after any redirects
    if let Ok(mut addrs) = tokio::net::lookup_host(format!("{final_domain}:80")).await {
        if let Some(addr) = addrs.next() {
            if is_private_ip(addr.ip()) {
                crate::structure::logger::warn(format!("url: SSRF block on final domain {final_domain}"));
                return None;
            }
        }
    }

    let status = resp.status();
    if !status.is_success() {
        crate::structure::logger::warn(format!("url: HTTP {status} for {url}"));
        return None;
    }

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    if !content_type.contains("text/html") {
        crate::structure::logger::warn(format!("url: non-html content-type '{content_type}' for {url}"));
        return None;
    }

    // 3. Size cap — read bytes up to limit, don't pull unbounded bodies
    let mut body = Vec::with_capacity(65536);
    let mut stream = resp.bytes_stream();
    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(b) => b,
            Err(e) => {
                crate::structure::logger::warn(format!("url: stream error for {url}: {e}"));
                return None;
            }
        };
        body.extend_from_slice(&chunk);
        if body.len() >= MAX_RESPONSE_BYTES {
            break;
        }
        // Stop at </head> only if we already have a description meta tag —
        // otherwise keep reading so we can fall back to <p> body content
        if body.windows(7).any(|w| w.eq_ignore_ascii_case(b"</head>")) {
            let partial = String::from_utf8_lossy(&body);
            if extract_meta_description(&partial).is_some() {
                break;
            }
        }
    }
    let html = String::from_utf8_lossy(&body).into_owned();

    if crate::structure::logger::debug_cat_enabled("url") {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let safe_host = host.replace('.', "_");
        let path = format!("./url_fetch_cache/{safe_host}_{ts}.html");
        let _ = std::fs::create_dir_all("./url_fetch_cache");
        if let Err(e) = std::fs::write(&path, &body) {
            crate::structure::logger::warn(format!("url: cache write failed: {e}"));
        } else {
            crate::structure::logger::warn(format!("url: cached {host} -> {path}"));
        }
    }

    let title = extract_title(&html).unwrap_or_else(|| extract_domain(url));

    let description = extract_meta_description(&html)
        .or_else(|| extract_first_paragraph(&html))
        .or_else(|| extract_title_as_description(&html));

    if description.is_none() {
        crate::structure::logger::warn(format!("url: no description found for {url} (body {} bytes)", body.len()));
    }

    let description = description?;

    Some((title, description))
}

fn is_private_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_unspecified()
        }
        std::net::IpAddr::V6(v6) => v6.is_loopback() || v6.is_unspecified(),
    }
}

fn extract_meta_description(html: &str) -> Option<String> {
    let head = html.split("</head>").next().unwrap_or(html);
    let head_lower = head.to_lowercase();

    for property in &[
        r#"property="og:description""#,
        r#"name="description""#,
        r#"name="twitter:description""#,
        r#"itemprop="description""#,
        r#"name="DC.description""#,
        r#"name="sailthru.description""#,
        r#"name="abstract""#,
        r#"name="summary""#,
        r#"property='og:description'"#,
        r#"name='description'"#,
        r#"itemprop='description'"#,
    ] {
        if let Some(val) = find_meta_content(head, &head_lower, property) {
            let trimmed = val.trim().to_owned();
            if !trimmed.is_empty() {
                return Some(clean_text(&trimmed));
            }
        }
    }
    None
}

fn find_meta_content(html: &str, html_lower: &str, property: &str) -> Option<String> {
    let start = html_lower.find(property)?;
    let tag_start = html_lower[..start].rfind("<meta")?;
    let tag_end = html_lower[tag_start..].find('>')? + tag_start;
    let tag = &html[tag_start..=tag_end];
    extract_attr(tag, "content")
}

fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let tag_lower = tag.to_lowercase();

    let double_pat = format!(r#"{}=""#, attr);
    if let Some(pos) = tag_lower.find(&double_pat) {
        let after = &tag[pos + double_pat.len()..];
        let end = after.find('"')?;
        return Some(after[..end].to_owned());
    }

    let single_pat = format!("{}='", attr);
    if let Some(pos) = tag_lower.find(&single_pat) {
        let after = &tag[pos + single_pat.len()..];
        let end = after.find('\'')?;
        return Some(after[..end].to_owned());
    }

    None
}

fn extract_first_paragraph(html: &str) -> Option<String> {
    let html_lower = html.to_lowercase();
    let mut search: &str = html;
    let mut search_lower: &str = &html_lower;
    loop {
        let p_start = search_lower.find("<p")?;
        search = &search[p_start..];
        search_lower = &search_lower[p_start..];
        let tag_end = search_lower.find('>')?;
        let after_tag = &search[tag_end + 1..];
        let after_lower = &search_lower[tag_end + 1..];
        let close = after_lower.find("</p>")?;
        let raw = &after_tag[..close];
        // Strip inner tags
        let text = strip_tags(raw);
        let trimmed = text.trim().to_owned();
        if trimmed.len() >= 40 {
            return Some(clean_text(&trimmed));
        }
        search = &search[tag_end + 1..];
        search_lower = &search_lower[tag_end + 1..];
    }
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

fn extract_title_as_description(html: &str) -> Option<String> {
    let raw = extract_title(html)?;
    // Strip common " - Site" / " | Site" / " — Site" suffixes
    let stripped = raw
        .rsplit_once(" - ")
        .or_else(|| raw.rsplit_once(" | "))
        .or_else(|| raw.rsplit_once(" — "))
        .map(|(left, _)| left.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(raw.as_str());
    if stripped == raw.as_str() {
        return None; // title has no suffix to strip, same as what's already shown
    }
    Some(stripped.to_owned())
}

fn extract_title(html: &str) -> Option<String> {
    let head = html.split("</head>").next().unwrap_or(html);
    let head_lower = head.to_lowercase();

    for property in &[r#"property="og:title""#, r#"property='og:title'"#] {
        if let Some(val) = find_meta_content(head, &head_lower, property) {
            let trimmed = val.trim().to_owned();
            if !trimmed.is_empty() {
                return Some(clean_text(&trimmed));
            }
        }
    }

    let title_start = head_lower.find("<title>")?;
    let after = &head[title_start + 7..];
    let end = after.to_lowercase().find("</title>")?;
    let raw = after[..end].trim();
    if raw.is_empty() {
        return None;
    }
    Some(clean_text(raw))
}

fn extract_domain(url: &str) -> String {
    let stripped = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    stripped.split('/').next().unwrap_or(url).to_owned()
}

fn clean_text(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace('\n', " ")
        .replace('\r', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn truncate_word_boundary(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let cut: String = s.chars().take(max - 3).collect();
    let trimmed = match cut.rfind(' ') {
        Some(pos) => &cut[..pos],
        None => &cut,
    };
    format!("{trimmed}...")
}
