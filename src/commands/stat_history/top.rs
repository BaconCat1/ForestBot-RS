use super::helpers::{all_known_usernames_for_server, excluded_usernames, whisper, BACKFILL_CONCURRENCY, ONE_DAY_MS};
use crate::commands::{CommandContext, CommandFuture};
use crate::commands::utils::stats_target::format_server_label;
use futures_util::stream::{self, StreamExt};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const TOP_LIMIT: usize = 5;
const HISTORICAL_TOP_CACHE_TTL_MS: u64 = 15 * 60 * 1000;

static HISTORICAL_TOP_CACHE: OnceLock<Mutex<HashMap<String, HistoricalTopCacheEntry>>> =
    OnceLock::new();

#[derive(Debug, Clone)]
struct HistoricalTopCacheEntry {
    expires_at: u64,
    rows: Vec<TopRow>,
}

#[derive(Debug, Clone)]
struct TopRow {
    username: String,
    value: u64,
}

command!(TOP_COMMAND, &["top"], "Shows the top 5 players in a certain statistic. Usage: {prefix}top <kills/deaths/joins/playtime/advancements/messages/slurcount>", top);

fn top(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(stat) = ctx.args.first() else {
            return Ok(());
        };
        let server = match parse_top_server(&ctx) {
            Ok(server) => server,
            Err(message) => {
                whisper(&ctx, &message);
                return Ok(());
            }
        };

        match stat.to_lowercase().as_str() {
            "kills" | "deaths" | "joins" | "playtime" => {
                top_backend_stat(&ctx, stat, &server).await
            }
            "messages" => top_messages(&ctx, &server).await,
            "advancements" => top_advancements(&ctx, &server).await,
            "trades" => top_trades(&ctx, &server).await,
            "rejects" => top_rejects(&ctx).await,
            "slurcount" | "slurs" => top_slurcount(&ctx, &server).await,
            _ => Ok(()),
        }
    })
}

fn parse_top_server(ctx: &CommandContext<'_>) -> Result<String, String> {
    let Some(server) = ctx.args.get(1) else {
        return Ok(ctx.state.mc_server.clone());
    };
    let server = server.to_lowercase();
    if server == "all" {
        return Ok("all".to_owned());
    }
    if !crate::constants::quote_servers::is_quote_server(&server) {
        return Err(format!(
            " Unknown server \"{server}\". Use {}lq for the list.", ctx.runtime.prefix
        ));
    }
    Ok(server)
}

async fn top_backend_stat(
    ctx: &CommandContext<'_>,
    stat: &str,
    server: &str,
) -> anyhow::Result<()> {
    let excluded = excluded_usernames(ctx).await;
    // Hub applies the row limit server-side -- over-fetch by the exclusion
    // count so filtering here can't shrink a real top-5 into a top-4.
    let value = ctx
        .state
        .api
        .get_top_statistic(stat, server, TOP_LIMIT + excluded.len())
        .await;
    let Some(value) = value else {
        whisper(ctx, "Api error");
        return Ok(());
    };
    let rows = value
        .get("top_stat")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let formatted = rows
        .into_iter()
        .filter_map(|row| {
            let username = row.get("username")?.as_str()?;
            if excluded.contains(&username.to_lowercase()) {
                return None;
            }
            let number = row.get(stat)?;
            if stat == "playtime" {
                let days = number.as_u64().unwrap_or_default() / ONE_DAY_MS;
                Some(format!("{username}: {days} Days"))
            } else {
                Some(format!("{username}: {}", value_to_string(number)))
            }
        })
        .take(TOP_LIMIT)
        .collect::<Vec<_>>();
    let title = if stat == "joins" {
        "TOP JOINS/LEAVES".to_owned()
    } else {
        format!("TOP {}", stat.to_uppercase())
    };
    let label = format_server_label(server, &ctx.state.mc_server);
    ctx.chat(format!(" [{title}{label}]: {}", formatted.join(", ")));
    Ok(())
}

async fn top_messages(ctx: &CommandContext<'_>, server: &str) -> anyhow::Result<()> {
    let excluded = excluded_usernames(ctx).await;
    let Some(value) = ctx.state.api.get_top_messages(server, TOP_LIMIT + excluded.len()).await else {
        whisper(ctx, "Could not calculate top messages right now.");
        return Ok(());
    };
    let mut rows: Vec<_> = value
        .get("top_messages")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|row| {
                    let username = row.get("name")?.as_str()?.to_owned();
                    if excluded.contains(&username.to_lowercase()) {
                        return None;
                    }
                    let value = row.get("count").and_then(number_from_value)?;
                    Some(TopRow { username, value })
                })
                .collect()
        })
        .unwrap_or_default();
    rows.truncate(TOP_LIMIT);
    if rows.is_empty() {
        whisper(ctx, "Could not calculate top messages right now.");
    } else {
        let title = format!("TOP MESSAGES{}", format_server_label(server, &ctx.state.mc_server));
        send_top_rows(ctx, &title, &rows);
    }
    Ok(())
}

async fn top_slurcount(ctx: &CommandContext<'_>, server: &str) -> anyhow::Result<()> {
    let slurs = match tokio::fs::read_to_string("./json/slurcount_list.json").await {
        Ok(data) => serde_json::from_str::<Vec<String>>(&data).unwrap_or_default(),
        Err(_) => {
            whisper(ctx, "slurcount_list.json not found or unreadable.");
            return Ok(());
        }
    };
    if slurs.is_empty() {
        whisper(ctx, "No slurs configured (slurcount_list.json is empty).");
        return Ok(());
    }
    let excluded = excluded_usernames(ctx).await;
    let Some(value) = ctx.state.api.get_top_slurcount(server, &slurs, TOP_LIMIT + excluded.len()).await else {
        whisper(ctx, "No slur data found.");
        return Ok(());
    };
    let mut rows: Vec<_> = value
        .get("top_slurcount")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|row| {
                    let username = row.get("name")?.as_str()?.to_owned();
                    if excluded.contains(&username.to_lowercase()) {
                        return None;
                    }
                    let value = row.get("count").and_then(number_from_value)?;
                    Some(TopRow { username, value })
                })
                .collect()
        })
        .unwrap_or_default();
    rows.truncate(TOP_LIMIT);
    if rows.is_empty() {
        whisper(ctx, "No slur data found.");
    } else {
        let title = format!("TOP SLURCOUNT{}", format_server_label(server, &ctx.state.mc_server));
        send_top_rows(ctx, &title, &rows);
    }
    Ok(())
}

async fn top_advancements(ctx: &CommandContext<'_>, server: &str) -> anyhow::Result<()> {
    if let Some(rows) = get_top_advancements_from_leaderboards(ctx, server).await
        && !rows.is_empty()
    {
        let title = format!(
            "TOP ADVANCEMENTS{}",
            format_server_label(server, &ctx.state.mc_server)
        );
        send_top_rows(ctx, &title, &rows);
        return Ok(());
    }

    let rows = cached_top_rows(server, "advancements");
    let rows = match rows {
        Some(rows) => rows,
        None => {
            whisper(ctx, "Running historical backfill for top advancements...");
            let rows = get_top_advancements_historical(ctx, server).await;
            if !rows.is_empty() {
                cache_top_rows(server, "advancements", rows.clone());
            }
            rows
        }
    };

    if rows.is_empty() {
        whisper(ctx, "Could not calculate top advancements right now.");
    } else {
        let title = format!(
            "TOP ADVANCEMENTS{}",
            format_server_label(server, &ctx.state.mc_server)
        );
        send_top_rows(ctx, &title, &rows);
    }
    Ok(())
}

async fn top_trades(ctx: &CommandContext<'_>, _server: &str) -> anyhow::Result<()> {
    let excluded = excluded_usernames(ctx).await;
    let Some(value) = ctx.state.api.get_trade_leaderboard().await else {
        whisper(ctx, "Api error");
        return Ok(());
    };
    let rows: Vec<_> = value
        .get("trades")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|row| {
                    let username = row.get("player_name")?.as_str()?.to_owned();
                    if excluded.contains(&username.to_lowercase()) {
                        return None;
                    }
                    let value = row.get("trade_count").and_then(number_from_value)?;
                    Some(TopRow { username, value })
                })
                .collect()
        })
        .unwrap_or_default();
    if rows.is_empty() {
        whisper(ctx, "No trade data yet.");
    } else {
        send_top_rows(ctx, "TOP TRADES", &rows);
    }
    Ok(())
}

async fn top_rejects(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let excluded = excluded_usernames(ctx).await;
    let Some(value) = ctx.state.api.get_trade_leaderboard().await else {
        whisper(ctx, "Api error");
        return Ok(());
    };
    let rows: Vec<_> = value
        .get("rejects")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|row| {
                    let username = row.get("player_name")?.as_str()?.to_owned();
                    if excluded.contains(&username.to_lowercase()) {
                        return None;
                    }
                    let value = row.get("reject_count").and_then(number_from_value)?;
                    Some(TopRow { username, value })
                })
                .collect()
        })
        .unwrap_or_default();
    if rows.is_empty() {
        whisper(ctx, "No reject data yet.");
    } else {
        send_top_rows(ctx, "TOP REJECTS", &rows);
    }
    Ok(())
}

async fn get_top_advancements_historical(ctx: &CommandContext<'_>, server: &str) -> Vec<TopRow> {
    let excluded = excluded_usernames(ctx).await;
    let usernames = all_known_usernames_for_server(ctx, server)
        .await
        .into_iter()
        .filter(|username| !excluded.contains(&username.to_lowercase()))
        .collect::<Vec<_>>();
    let api = ctx.state.api.clone();
    let server = server.to_owned();
    let mut rows = stream::iter(usernames)
        .map(|username| {
            let api = api.clone();
            let server = server.clone();
            async move {
                let uuid = api.convert_username_to_uuid(&username).await?;
                let value = api.get_total_advancements_count(&uuid, &server).await?;
                Some(TopRow { username, value })
            }
        })
        .buffer_unordered(BACKFILL_CONCURRENCY)
        .filter_map(|row| async move { row })
        .collect::<Vec<_>>()
        .await;
    sort_and_truncate_top_rows(&mut rows);
    rows
}

async fn get_top_advancements_from_leaderboards(
    ctx: &CommandContext<'_>,
    server: &str,
) -> Option<Vec<TopRow>> {
    let excluded = excluded_usernames(ctx).await;
    let value = ctx.state.api.get_leaderboards(server).await?;
    let mut rows = value
        .get("advancements")?
        .as_array()?
        .iter()
        .filter_map(|row| {
            let username = row.get("player_name")?.as_str()?.to_owned();
            if excluded.contains(&username.to_lowercase()) {
                return None;
            }
            let value = row.get("advancement_count").and_then(number_from_value)?;
            Some(TopRow { username, value })
        })
        .collect::<Vec<_>>();
    rows.truncate(TOP_LIMIT);
    Some(rows)
}

fn sort_and_truncate_top_rows(rows: &mut Vec<TopRow>) {
    rows.sort_by(|a, b| {
        b.value
            .cmp(&a.value)
            .then_with(|| a.username.cmp(&b.username))
    });
    rows.truncate(TOP_LIMIT);
}

fn send_top_rows(ctx: &CommandContext<'_>, title: &str, rows: &[TopRow]) {
    let formatted = rows
        .iter()
        .map(|row| format!("{}: {}", row.username, row.value))
        .collect::<Vec<_>>()
        .join(", ");
    ctx.chat(format!(" [{title}]: {formatted}"));
}

fn cached_top_rows(server: &str, metric: &str) -> Option<Vec<TopRow>> {
    let key = format!("{server}:{metric}");
    let mut cache = historical_top_cache()
        .lock()
        .expect("historical top cache lock poisoned");
    let now = super::helpers::now_millis();
    match cache.get(&key) {
        Some(entry) if entry.expires_at > now => Some(entry.rows.clone()),
        Some(_) => {
            cache.remove(&key);
            None
        }
        None => None,
    }
}

fn cache_top_rows(server: &str, metric: &str, rows: Vec<TopRow>) {
    historical_top_cache()
        .lock()
        .expect("historical top cache lock poisoned")
        .insert(
            format!("{server}:{metric}"),
            HistoricalTopCacheEntry {
                expires_at: super::helpers::now_millis().saturating_add(HISTORICAL_TOP_CACHE_TTL_MS),
                rows,
            },
        );
}

fn historical_top_cache() -> &'static Mutex<HashMap<String, HistoricalTopCacheEntry>> {
    HISTORICAL_TOP_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn number_from_value(value: &serde_json::Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|value| value.try_into().ok()))
        .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
}

fn value_to_string(value: &serde_json::Value) -> String {
    value
        .as_u64()
        .map(|v| v.to_string())
        .or_else(|| value.as_i64().map(|v| v.to_string()))
        .or_else(|| value.as_f64().map(|v| v.to_string()))
        .or_else(|| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| value.to_string())
}
