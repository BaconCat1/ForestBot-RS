use std::collections::HashSet;
use std::path::Path;

use tokio::fs;

const CACHE_DIR: &str = "./json/url_blocklist_cache";

pub async fn build_blocklist(sources: &[String], whitelist_file: &str, timeout_ms: u64) -> HashSet<String> {
    let _ = fs::create_dir_all(CACHE_DIR).await;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(timeout_ms))
        .user_agent("ForestBot/1.0")
        .build()
        .unwrap_or_default();

    let mut all = HashSet::new();

    for source in sources {
        let content = if source.starts_with("http://") || source.starts_with("https://") {
            fetch_with_etag(&client, source).await
        } else {
            read_local(source).await
        };

        if let Some(text) = content {
            for domain in parse_hosts(&text) {
                all.insert(domain);
            }
        } else {
            crate::structure::logger::warn(format!(
                "url_blocklist: failed to load source: {source}"
            ));
        }
    }

    // Remove whitelisted domains (parent-domain-aware: whitelist "youtube.com" removes "www.youtube.com" too)
    if let Some(text) = read_local(whitelist_file).await {
        let whitelist: HashSet<String> = parse_hosts(&text).collect();
        let before = all.len();
        all.retain(|d| !is_whitelisted(d, &whitelist));
        let removed = before - all.len();
        if removed > 0 {
            crate::structure::logger::info(format!(
                "url_blocklist: {removed} domain(s) whitelisted"
            ));
        }
    }

    crate::structure::logger::info(format!(
        "url_blocklist: {} domains loaded from {} source(s)",
        all.len(),
        sources.len()
    ));

    all
}

async fn fetch_with_etag(client: &reqwest::Client, url: &str) -> Option<String> {
    let key = url_to_cache_key(url);
    let cache_file = format!("{CACHE_DIR}/{key}.hosts");
    let etag_file = format!("{CACHE_DIR}/{key}.etag");

    let cached_etag = fs::read_to_string(&etag_file).await.ok();

    let mut req = client.get(url);
    if let Some(ref etag) = cached_etag {
        req = req.header("If-None-Match", etag.trim());
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            crate::structure::logger::warn(format!("url_blocklist: fetch failed for {url}: {e}"));
            // Fall back to cache if available
            return fs::read_to_string(&cache_file).await.ok();
        }
    };

    if resp.status() == 304 {
        // Not modified — use cache
        return fs::read_to_string(&cache_file).await.ok();
    }

    if !resp.status().is_success() {
        crate::structure::logger::warn(format!(
            "url_blocklist: HTTP {} for {url}",
            resp.status()
        ));
        return fs::read_to_string(&cache_file).await.ok();
    }

    // Save new ETag if present
    if let Some(etag) = resp.headers().get("etag").and_then(|v| v.to_str().ok()) {
        let _ = fs::write(&etag_file, etag).await;
    }

    let text = resp.text().await.ok()?;
    let _ = fs::write(&cache_file, &text).await;
    Some(text)
}

async fn read_local(path: &str) -> Option<String> {
    if !Path::new(path).exists() {
        return None;
    }
    fs::read_to_string(path).await.ok()
}

fn parse_hosts(content: &str) -> impl Iterator<Item = String> + '_ {
    content.lines().filter_map(|line| {
        let line = line.trim();
        // Skip comments and blank lines
        if line.is_empty() || line.starts_with('#') {
            return None;
        }
        // Strip inline comments
        let line = line.split('#').next().unwrap_or("").trim();
        // Split on whitespace: "0.0.0.0 domain.com" or just "domain.com"
        let mut parts = line.split_whitespace();
        let first = parts.next()?;
        let domain = if let Some(second) = parts.next() {
            // "0.0.0.0 domain.com" format — skip localhost entries
            if second == "localhost" || second == "localhost.localdomain" || second == "local" {
                return None;
            }
            second
        } else {
            // Bare domain format
            first
        };
        // Skip IP addresses
        if domain.parse::<std::net::IpAddr>().is_ok() {
            return None;
        }
        Some(domain.to_lowercase())
    })
}

fn url_to_cache_key(url: &str) -> String {
    // Simple deterministic hash for filename — no crypto crate needed
    let hash = url
        .bytes()
        .fold(0u64, |h, b| h.wrapping_mul(31).wrapping_add(b as u64));
    format!("{hash:016x}")
}

fn is_whitelisted(domain: &str, whitelist: &HashSet<String>) -> bool {
    if whitelist.contains(domain) {
        return true;
    }
    let mut rest = domain;
    while let Some(pos) = rest.find('.') {
        rest = &rest[pos + 1..];
        if whitelist.contains(rest) {
            return true;
        }
    }
    false
}

pub fn is_blocked(domain: &str, blocklist: &HashSet<String>) -> bool {
    let domain = domain.to_lowercase();
    // Check exact match and all parent domains
    if blocklist.contains(&domain) {
        return true;
    }
    // Check parent domains (e.g. "sub.xvideos.com" → "xvideos.com")
    let mut rest = domain.as_str();
    while let Some(pos) = rest.find('.') {
        rest = &rest[pos + 1..];
        if blocklist.contains(rest) {
            return true;
        }
    }
    false
}
