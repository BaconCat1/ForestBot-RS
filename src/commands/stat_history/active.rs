use super::helpers::{epoch_ms_from_string, now_millis, whisper, ONE_DAY_MS};
use crate::commands::{CommandContext, CommandFuture};
use futures_util::stream::{self, StreamExt};
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

const ACTIVE_CACHE_TTL_MS: u64 = 5 * 60 * 1000;
const ACTIVE_TOP_LIMIT: usize = 5;
const ACTIVE_MSG_FETCH: usize = 100;
const ACTIVE_CONCURRENCY: usize = 12;

static ACTIVE_CACHE: OnceLock<Mutex<HashMap<String, ActiveCacheEntry>>> = OnceLock::new();

#[derive(Debug, Clone)]
struct ActiveCacheEntry {
    expires_at: u64,
    data: Vec<ActiveRow>,
}

#[derive(Debug, Clone)]
struct ActiveRow {
    username: String,
    count: usize,
}

command!(ACTIVE_COMMAND, &["active"], "Shows the most active players in the last 24 hours by message count. Usage: {prefix}active", active);

fn active(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let key = ctx.state.mc_server.clone();
        let now = now_millis();
        if let Some(rows) = active_cached(&key, now) {
            send_active_rows(&ctx, &rows);
            return Ok(());
        }

        whisper(&ctx, " Computing active players, this may take a moment...");
        let users = ctx
            .state
            .api
            .get_unique_users(&ctx.state.mc_server)
            .await
            .unwrap_or_default();
        let usernames = users
            .into_iter()
            .map(|user| user.username)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let cutoff = now.saturating_sub(ONE_DAY_MS);
        let server = ctx.state.mc_server.clone();
        let api = ctx.state.api.clone();
        let mut rows = stream::iter(usernames)
            .map(|username| {
                let api = api.clone();
                let server = server.clone();
                async move {
                    let messages = api
                        .get_messages(&username, &server, ACTIVE_MSG_FETCH, "DESC", 0)
                        .await
                        .unwrap_or_default();
                    let count = messages
                        .into_iter()
                        .take_while(|msg| {
                            epoch_ms_from_string(&msg.date).unwrap_or_default() >= cutoff
                        })
                        .count();
                    ActiveRow { username, count }
                }
            })
            .buffer_unordered(ACTIVE_CONCURRENCY)
            .filter(|row| std::future::ready(row.count > 0))
            .collect::<Vec<_>>()
            .await;

        rows.sort_by(|a, b| {
            b.count
                .cmp(&a.count)
                .then_with(|| a.username.cmp(&b.username))
        });
        rows.truncate(ACTIVE_TOP_LIMIT);
        active_cache()
            .lock()
            .expect("active cache lock poisoned")
            .insert(
                key,
                ActiveCacheEntry {
                    expires_at: now.saturating_add(ACTIVE_CACHE_TTL_MS),
                    data: rows.clone(),
                },
            );
        send_active_rows(&ctx, &rows);
        Ok(())
    })
}

fn active_cache() -> &'static Mutex<HashMap<String, ActiveCacheEntry>> {
    ACTIVE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn active_cached(server: &str, now: u64) -> Option<Vec<ActiveRow>> {
    active_cache()
        .lock()
        .expect("active cache lock poisoned")
        .get(server)
        .filter(|entry| now < entry.expires_at)
        .map(|entry| entry.data.clone())
}

fn send_active_rows(ctx: &CommandContext<'_>, rows: &[ActiveRow]) {
    if rows.is_empty() {
        whisper(ctx, " No players found active in the last 24 hours.");
        return;
    }
    let formatted = rows
        .iter()
        .map(|row| format!("{}: {}", row.username, row.count))
        .collect::<Vec<_>>()
        .join(", ");
    ctx.chat(format!(" [ACTIVE 24h]: {formatted}"));
}
