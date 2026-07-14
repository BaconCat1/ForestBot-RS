use crate::commands::{
    CommandContext, CommandFuture,
    utils::stats_target::{format_server_label, parse_stats_target_or_reply},
};
use crate::config::{load_user_list, load_word_list, save_user_list, save_word_list};
use std::collections::HashSet;
use std::sync::atomic::AtomicBool;

pub static BOT_SLEEPING: AtomicBool = AtomicBool::new(false);

pub const BACKFILL_CONCURRENCY: usize = 12;

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

/// Shared by firstdeath/lastdeath/firstkill/lastkill -- same lookup, different
/// order/table depending on (death, first).
pub fn death_or_kill(ctx: CommandContext<'_>, death: bool, first: bool) -> CommandFuture<'_> {
    Box::pin(async move {
        let command_name = match (death, first) {
            (true, true) => "firstdeath",
            (true, false) => "lastdeath",
            (false, true) => "firstkill",
            (false, false) => "lastkill",
        };
        let Some(target) = parse_stats_target_or_reply(&ctx, command_name) else {
            return Ok(());
        };
        let Some(uuid) = ctx.state.api.convert_username_to_uuid(&target.search).await else {
            whisper_no_record(&ctx, &target.search, if death { "deaths" } else { "kills" });
            return Ok(());
        };
        let order = if first { "ASC" } else { "DESC" };
        let rows = if death {
            ctx.state
                .api
                .get_deaths(&uuid, &target.server, 1, order, "all")
                .await
        } else {
            ctx.state
                .api
                .get_kills(&uuid, &target.server, 1, order)
                .await
        };
        let Some(row) = rows.and_then(|mut rows| rows.pop()) else {
            whisper_no_record(&ctx, &target.search, if death { "deaths" } else { "kills" });
            return Ok(());
        };
        let label = format_server_label(&target.server, &ctx.state.mc_server);
        ctx.chat(format!(
            " {}{}: {}, {}",
            target.search,
            label,
            row.death_message,
            crate::functions::utils::time::time_ago_str(row.time as u64)
        ));
        Ok(())
    })
}

/// Shared by firstmessage/lastmessage -- same lookup, different sort order.
pub fn message_lookup<'a>(ctx: CommandContext<'a>, order: &'static str) -> CommandFuture<'a> {
    Box::pin(async move {
        let command_name = if order == "ASC" {
            "firstmessage"
        } else {
            "lastmessage"
        };
        let Some(target) = parse_stats_target_or_reply(&ctx, command_name) else {
            return Ok(());
        };
        let row = ctx
            .state
            .api
            .get_messages(&target.search, &target.server, 1, order, 0)
            .await
            .and_then(|mut rows| rows.pop());
        if let Some(row) = row {
            let date = epoch_ms_from_string(&row.date)
                .map(crate::functions::utils::time::time_ago_str)
                .unwrap_or(row.date);
            let label = format_server_label(&target.server, &ctx.state.mc_server);
            ctx.chat(format!(
                " {}{}: {}, {date}",
                target.search, label, row.message
            ));
        } else if target.search.eq_ignore_ascii_case(ctx.sender) {
            whisper(&ctx, " You have no messages, or unexpected error occurred.");
        } else {
            whisper(
                &ctx,
                &format!(
                    " {} has no messages, or unexpected error occurred.",
                    target.search
                ),
            );
        }
        Ok(())
    })
}

/// Shared by oldheads/noobs -- same online-users-sorted-by-joindate logic, reversed.
pub fn sorted_unique_users(ctx: CommandContext<'_>, oldest: bool) -> CommandFuture<'_> {
    Box::pin(async move {
        let online: HashSet<String> = {
            let players = ctx.state.players.read().expect("player cache lock poisoned");
            players.keys().map(|k| k.to_lowercase()).collect()
        };
        let mut users = ctx
            .state
            .api
            .get_unique_users(&ctx.state.mc_server)
            .await
            .unwrap_or_default();
        users.retain(|user| online.contains(&user.username.to_lowercase()));
        if users.is_empty() {
            ctx.whisper("No online players found in database.");
            return Ok(());
        }
        users.sort_by_key(|user| user.joindate.as_deref().unwrap_or("").parse::<u64>().unwrap_or_default());
        if !oldest {
            users.reverse();
        }
        let label = if oldest { "oldest" } else { "newest" };
        let rows = users
            .into_iter()
            .take(3)
            .map(|user| {
                format!(
                    "{} ({})",
                    user.username,
                    crate::functions::utils::time::time_ago_str(user.joindate.as_deref().unwrap_or("").parse().unwrap_or_default())
                )
            })
            .collect::<Vec<_>>();
        ctx.chat(format!(" The 3 {label} online players are: {}", rows.join(", ")));
        Ok(())
    })
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

/// Shared by blacklist/whitelist -- same add/remove/list flow against different files
/// and different runtime set.
pub fn list_command<'a>(
    ctx: CommandContext<'a>,
    path: &'static str,
    blacklist: bool,
) -> CommandFuture<'a> {
    Box::pin(async move {
        let list_name = if blacklist { "blacklist" } else { "whitelist" };
        let action = ctx.args.first().copied().unwrap_or_default().to_lowercase();
        if action != "add" && action != "remove" && action != "list" {
            whisper(
                &ctx,
                &format!(" Invalid action. Use !{list_name} add|remove"),
            );
            return Ok(());
        }
        let mut list = load_user_list(path).await.unwrap_or_default();
        if action == "list" {
            whisper(&ctx, &format!(" {list_name}: {}", list.join(", ")));
            return Ok(());
        }
        let Some(target) = ctx.args.get(1).copied() else {
            whisper(
                &ctx,
                &format!(" Please specify a user to {action} from the {list_name}."),
            );
            return Ok(());
        };
        let uuid = match player_uuid(&ctx, target) {
            Some(uuid) => Some(uuid),
            None => ctx.state.api.convert_username_to_uuid(target).await,
        };
        let Some(uuid) = uuid else {
            whisper(&ctx, &format!(" Could not resolve UUID for {target}."));
            return Ok(());
        };
        if action == "add" {
            if !list.iter().any(|entry| entry == &uuid) {
                list.push(uuid.clone());
            }
        } else {
            list.retain(|entry| entry != &uuid);
        }
        save_user_list(path, &list).await?;
        {
            let mut runtime = ctx
                .state
                .runtime
                .write()
                .expect("runtime config lock poisoned");
            if blacklist {
                runtime.user_blacklist = list.iter().cloned().collect();
            } else {
                runtime.user_whitelist = list.iter().cloned().collect();
            }
        }
        let verb = if action == "add" { "Added" } else { "Removed" };
        whisper(
            &ctx,
            &format!(
                " {verb} {target} {} the {list_name}.",
                if action == "add" { "to" } else { "from" }
            ),
        );
        Ok(())
    })
}

/// Shared by censor/wordwhitelist -- same add/remove flow against different word lists.
pub fn word_list_command<'a>(
    ctx: CommandContext<'a>,
    path: &'static str,
    list_name: &'static str,
) -> CommandFuture<'a> {
    Box::pin(async move {
        let action = ctx.args.first().copied().unwrap_or_default().to_lowercase();
        let word = ctx
            .args
            .iter()
            .skip(1)
            .copied()
            .collect::<Vec<_>>()
            .join(" ");
        let word = word.trim();
        if word.is_empty() || !matches!(action.as_str(), "add" | "remove" | "delete" | "rm") {
            let command = if list_name == "bad words" {
                "censor"
            } else {
                "wordwhitelist"
            };
            whisper(
                &ctx,
                &format!(" Usage: !{command} add <word> | !{command} remove <word>"),
            );
            return Ok(());
        }
        let mut words = load_word_list(path).await.unwrap_or_default();
        let exists = words.iter().any(|entry| entry.eq_ignore_ascii_case(word));
        if action == "add" {
            if !exists {
                words.push(word.to_owned());
                save_word_list(path, &words).await?;
                whisper(&ctx, &format!(" Added \"{word}\" to {list_name}."));
            } else {
                whisper(
                    &ctx,
                    &format!(" \"{word}\" is already in {list_name} or invalid."),
                );
            }
        } else if exists {
            words.retain(|entry| !entry.eq_ignore_ascii_case(word));
            save_word_list(path, &words).await?;
            whisper(&ctx, &format!(" Removed \"{word}\" from {list_name}."));
        } else {
            whisper(&ctx, &format!(" \"{word}\" was not found in {list_name}."));
        }
        Ok(())
    })
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
