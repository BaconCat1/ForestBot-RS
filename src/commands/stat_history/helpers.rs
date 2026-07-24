//! Pure cross-command utilities. Shared *command implementations* (used by
//! more than one command's fn, e.g. death_or_kill) live in patterns.rs instead --
//! keeping this file to genuinely generic helpers that aren't command bodies.

use crate::commands::{CommandContext, utils::stats_target::parse_stats_target_or_reply};
use std::collections::HashSet;
use std::sync::atomic::AtomicBool;

pub static BOT_SLEEPING: AtomicBool = AtomicBool::new(false);

pub const BACKFILL_CONCURRENCY: usize = 12;
pub const ONE_DAY_MS: u64 = 24 * 60 * 60 * 1000;

pub fn whisper(ctx: &CommandContext<'_>, message: &str) {
    ctx.whisper(message);
}

pub fn whisper_no_record(ctx: &CommandContext<'_>, search: &str, thing: &str) {
    if search.eq_ignore_ascii_case(ctx.sender) {
        whisper(
            ctx,
            &format!(" You have no {thing}, or unexpected error occurred."),
        );
    } else {
        whisper(
            ctx,
            &format!(" {search} has no {thing}, or unexpected error occurred."),
        );
    }
}

pub fn player_uuid(ctx: &CommandContext<'_>, username: &str) -> Option<String> {
    ctx.state
        .players
        .read()
        .expect("player cache lock poisoned")
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(username))
        .map(|(_, player)| player.uuid.clone())
}

/// Shared by joindate/jdpt -- formats a raw join-date value (epoch string or
/// already-formatted) into a display string.
pub fn format_date_value(value: &str) -> String {
    if let Some(ms) = epoch_ms_from_string(value) {
        crate::functions::utils::time::convert_unix_timestamp(ms / 1000)
    } else {
        value.to_owned()
    }
}

pub fn epoch_ms_from_string(value: &str) -> Option<u64> {
    let raw = value.parse::<u64>().ok()?;
    Some(if raw < 1_000_000_000_000 {
        raw * 1000
    } else {
        raw
    })
}

pub fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

pub async fn parse_target_with_uuid(
    ctx: &CommandContext<'_>,
    usage_name: &str,
) -> anyhow::Result<Option<(crate::commands::utils::stats_target::StatsTarget, String)>> {
    let Some(target) = parse_stats_target_or_reply(ctx, usage_name) else {
        return Ok(None);
    };
    let Some(uuid) = ctx.state.api.convert_username_to_uuid(&target.search).await else {
        whisper_no_record(ctx, &target.search, "stats");
        return Ok(None);
    };
    Ok(Some((target, uuid)))
}

/// Manually-curated list of accounts (by UUID -- stable across name changes,
/// project convention) to keep out of cross-player stat/quote results, e.g.
/// the bot's own account. Lives at `json/stat_exclusions.json`, a bare JSON
/// array of UUID strings, edited by hand, not hot-reloaded (same read-fresh-
/// per-call pattern as `top.rs`'s `slurcount_list.json`). Resolved to
/// lowercased usernames here since none of the Hub endpoints this filters
/// (top-statistic, unique-users, messages, quotes) return UUIDs in their rows.
pub async fn excluded_usernames(ctx: &CommandContext<'_>) -> HashSet<String> {
    let uuids = match tokio::fs::read_to_string("./json/stat_exclusions.json").await {
        Ok(data) => serde_json::from_str::<Vec<String>>(&data).unwrap_or_default(),
        Err(_) => return HashSet::new(),
    };
    let mut names = HashSet::new();
    for uuid in uuids {
        if let Some(stats) = ctx.state.api.get_stats_by_uuid(&uuid, &ctx.state.mc_server).await
            && let Some(username) = stats.username
        {
            names.insert(username.to_lowercase());
        }
    }
    names
}

pub async fn all_known_usernames(ctx: &CommandContext<'_>) -> Vec<String> {
    all_known_usernames_for_server(ctx, &ctx.state.mc_server).await
}

pub async fn all_known_usernames_for_server(ctx: &CommandContext<'_>, server: &str) -> Vec<String> {
    ctx.state
        .api
        .get_unique_users(server)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|user| user.username)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect()
}

#[derive(Debug, Clone)]
pub struct CachedPlayer {
    pub username: String,
    pub latency: i32,
}

pub fn players_snapshot(ctx: &CommandContext<'_>) -> Vec<CachedPlayer> {
    ctx.state
        .players
        .read()
        .expect("player cache lock poisoned")
        .values()
        .map(|player| CachedPlayer {
            username: player.username.clone(),
            latency: player.latency,
        })
        .collect()
}

pub fn extract_victim_name(
    entry: &crate::structure::endpoints::endpoints::MinecraftPlayerDeathMessage,
) -> Option<String> {
    if !entry.victim.trim().is_empty() {
        return Some(entry.victim.trim().to_owned());
    }
    entry
        .death_message
        .split_whitespace()
        .next()
        .map(|name| {
            name.chars()
                .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
                .collect::<String>()
        })
        .filter(|name| !name.is_empty())
}
