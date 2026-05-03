use crate::{
    commands::{
        CommandContext, CommandDefinition, CommandFuture,
        utils::stats_target::{
            StatsTargetError, format_server_label, format_server_scope_hint,
            parse_stats_target_args,
        },
    },
    config::{OfflineMessage, load_offline_messages, save_offline_messages},
    functions::utils::time,
};

macro_rules! command {
    ($const_name:ident, $names:expr, $execute:ident) => {
        pub const $const_name: CommandDefinition = CommandDefinition {
            names: $names,
            whitelisted: false,
            execute: $execute,
        };
    };
}

command!(KD_COMMAND, &["kd", "kills", "deaths"], kd);
command!(JOINDATE_COMMAND, &["joindate", "jd", "firstseen"], joindate);
command!(
    JDPT_COMMAND,
    &["jdpt", "ptjd", "joindateplaytime", "playtimejoindate"],
    jdpt
);
command!(
    WORDCOUNT_COMMAND,
    &["wordcount", "words", "count"],
    wordcount
);
command!(NAMEFIND_COMMAND, &["search", "lookup", "find"], namefind);
command!(
    UNIQUE_USERS_COMMAND,
    &["users", "uniqueusers"],
    unique_users
);
command!(
    TOTAL_ADVANCEMENTS_COMMAND,
    &["advancements", "totaladvancements", "advs", "adv"],
    total_advancements
);
command!(SUMMARY_COMMAND, &["summary", "sum"], summary);
command!(WINRATE_COMMAND, &["winrate", "wr"], winrate);
command!(FIRST_DEATH_COMMAND, &["firstdeath", "fd"], first_death);
command!(LAST_DEATH_COMMAND, &["lastdeath", "ld"], last_death);
command!(FIRST_KILL_COMMAND, &["firstkill", "fk"], first_kill);
command!(LAST_KILL_COMMAND, &["lastkill", "lk"], last_kill);
command!(
    LAST_ADVANCEMENT_COMMAND,
    &["lastadvancement", "ladv"],
    last_advancement
);
command!(
    FIRST_MESSAGE_COMMAND,
    &["firstmessage", "fm"],
    first_message
);
command!(LAST_MESSAGE_COMMAND, &["lastmessage", "lm"], last_message);
command!(
    OLDHEADS_COMMAND,
    &["oldest", "oldheads", "oldusers", "oldestusers", "oldfags"],
    oldheads
);
command!(
    NOOBS_COMMAND,
    &["noobs", "noob", "newest", "newusers", "newbs", "newb"],
    noobs
);
command!(TOP_COMMAND, &["top"], top);
command!(STANDING_COMMAND, &["standing", "status"], standing);
command!(OFFLINE_MSG_COMMAND, &["offlinemsg"], offline_msg);
command!(WHOIS_COMMAND, &["whois"], whois);
command!(RANDOM_QUOTE_COMMAND, &["rq", "randomquote"], random_quote);
command!(
    LIST_QUOTE_SERVERS_COMMAND,
    &["lq", "listquoteservers"],
    list_quote_servers
);

fn kd<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    Box::pin(async move {
        let Some((target, uuid)) = parse_target_with_uuid(&ctx, "kd").await? else {
            return Ok(());
        };
        let data = ctx.state.api.get_kd(&uuid, &target.server).await;
        let server_hint =
            format_server_scope_hint(target.has_server_arg, &target.server, &ctx.state.mc_server);
        let Some(data) = data else {
            whisper_no_record(
                &ctx,
                &target.search,
                &format!("kills or deaths{server_hint}"),
            );
            return Ok(());
        };
        let ratio = data.kills as f64 / data.deaths as f64;
        let label = format_server_label(&target.server, &ctx.state.mc_server);
        ctx.bot.chat(&format!(
            " {}{}: Kills: {} Deaths: {} KD: {:.2}",
            target.search, label, data.kills, data.deaths, ratio
        ));
        Ok(())
    })
}

fn joindate<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    Box::pin(async move {
        let Some((target, uuid)) = parse_target_with_uuid(&ctx, "joindate").await? else {
            return Ok(());
        };
        let data = ctx.state.api.get_join_date(&uuid, &target.server).await;
        let Some(data) = data else {
            let hint = format_server_scope_hint(
                target.has_server_arg,
                &target.server,
                &ctx.state.mc_server,
            );
            whisper_no_record(&ctx, &target.search, &format!("join date{hint}"));
            return Ok(());
        };
        let label = format_server_label(&target.server, &ctx.state.mc_server);
        ctx.bot.chat(&format!(
            " {}{}, joined on: {}",
            target.search,
            label,
            format_date_value(&data.join_date)
        ));
        Ok(())
    })
}

fn jdpt<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    Box::pin(async move {
        let Some((target, uuid)) = parse_target_with_uuid(&ctx, "jdpt").await? else {
            return Ok(());
        };
        let jd = ctx.state.api.get_join_date(&uuid, &target.server).await;
        let pt = ctx.state.api.get_playtime(&uuid, &target.server).await;
        if jd.is_none() && pt.is_none() {
            let hint = format_server_scope_hint(
                target.has_server_arg,
                &target.server,
                &ctx.state.mc_server,
            );
            whisper_no_record(
                &ctx,
                &target.search,
                &format!("join date or playtime recorded{hint}"),
            );
            return Ok(());
        }
        let mut parts = Vec::new();
        if let Some(jd) = jd {
            parts.push(format!("joined on: {}", format_date_value(&jd.join_date)));
        }
        if let Some(pt) = pt {
            parts.push(format!("total playtime: {}", time::dhms(pt.playtime)));
        }
        let label = format_server_label(&target.server, &ctx.state.mc_server);
        ctx.bot.chat(&format!(
            " {}{}, {}",
            target.search,
            label,
            parts.join(" | ")
        ));
        Ok(())
    })
}

fn wordcount<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    Box::pin(async move {
        let (server, search, word, has_server_arg) = match ctx.args.as_slice() {
            [server, search, word, ..] => ((*server).to_lowercase(), *search, *word, true),
            [search, word] => (ctx.state.mc_server.clone(), *search, *word, false),
            _ => {
                whisper(
                    &ctx,
                    " Usage: !wordcount <username> <word> or !wordcount <server|all> <username> <word>",
                );
                return Ok(());
            }
        };
        let data = ctx
            .state
            .api
            .get_word_occurrence(search, &server, word)
            .await;
        let Some(data) = data else {
            let hint = if has_server_arg {
                if server == "all" {
                    " on all servers".to_owned()
                } else {
                    format!(" on {server}")
                }
            } else {
                String::new()
            };
            whisper(&ctx, &format!(" {search} has not said {word}{hint}"));
            return Ok(());
        };
        let label = format_server_label(&server, &ctx.state.mc_server);
        ctx.bot.chat(&format!(
            " {search}{label} has said {word} {} times",
            data.count
        ));
        Ok(())
    })
}

fn namefind<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    Box::pin(async move {
        let Some(search) = ctx.args.first() else {
            whisper(&ctx, " Usage: !find <username>");
            return Ok(());
        };
        let data = ctx
            .state
            .api
            .get_name_finder(search, &ctx.state.mc_server)
            .await;
        if let Some(data) = data
            && !data.usernames.is_empty()
        {
            ctx.bot.chat(&format!(
                " You could be looking for: {}",
                data.usernames.join(", ")
            ));
        }
        Ok(())
    })
}

fn unique_users<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    Box::pin(async move {
        let users = ctx.state.api.get_unique_users(&ctx.state.mc_server).await;
        let count = users.map(|users| users.len()).unwrap_or_default();
        ctx.bot.chat(&format!(
            " I have seen {count} different users on this server! api.forestbot.org/unique-users?server={}",
            ctx.state.mc_server
        ));
        Ok(())
    })
}

fn total_advancements<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    Box::pin(async move {
        let search = ctx.args.first().copied().unwrap_or(ctx.sender);
        let Some(uuid) = ctx.state.api.convert_username_to_uuid(search).await else {
            whisper(
                &ctx,
                &format!(" {search} has no advancements, or unexpected error occurred."),
            );
            return Ok(());
        };
        let count = ctx
            .state
            .api
            .get_total_advancements_count(&uuid, &ctx.state.mc_server)
            .await
            .unwrap_or_default();
        ctx.bot
            .chat(&format!(" {search} has {count} advancements."));
        Ok(())
    })
}

fn summary<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    Box::pin(async move {
        let search = ctx.args.first().copied().unwrap_or(ctx.sender);
        let Some(uuid) = ctx.state.api.convert_username_to_uuid(search).await else {
            whisper(&ctx, &format!(" Could not find {search}."));
            return Ok(());
        };
        let kd = ctx.state.api.get_kd(&uuid, &ctx.state.mc_server).await;
        let pt = ctx
            .state
            .api
            .get_playtime(&uuid, &ctx.state.mc_server)
            .await;
        let mc = ctx
            .state
            .api
            .get_message_count(search, &ctx.state.mc_server)
            .await;
        let adv = ctx
            .state
            .api
            .get_total_advancements_count(&uuid, &ctx.state.mc_server)
            .await;
        let jd = ctx
            .state
            .api
            .get_join_date(&uuid, &ctx.state.mc_server)
            .await;
        let kills = kd.as_ref().map(|kd| kd.kills).unwrap_or_default();
        let deaths = kd.as_ref().map(|kd| kd.deaths).unwrap_or_default();
        let kdr = if deaths > 0 {
            kills as f64 / deaths as f64
        } else {
            kills as f64
        };
        let pt_days = pt.map(|pt| pt.playtime / 86_400_000).unwrap_or_default();
        let messages = mc.map(|mc| mc.message_count).unwrap_or_default();
        let adv = adv.unwrap_or_default();
        let age = jd
            .and_then(|jd| epoch_ms_from_string(&jd.join_date))
            .map(member_days)
            .map(|days| format!("{days}d"))
            .unwrap_or_else(|| "?".to_owned());
        ctx.bot.chat(&format!(
            " [{search}] KD: {kills}/{deaths} ({kdr:.2}) | Playtime: {pt_days}d | Messages: {messages} | Advancements: {adv} | Member for: {age}"
        ));
        Ok(())
    })
}

fn winrate<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    Box::pin(async move {
        let search = ctx.args.first().copied().unwrap_or(ctx.sender);
        let Some(uuid) = ctx.state.api.convert_username_to_uuid(search).await else {
            whisper(
                &ctx,
                &format!(
                    " {search} has no kills or deaths recorded, or unexpected error occurred."
                ),
            );
            return Ok(());
        };
        let Some(kd) = ctx.state.api.get_kd(&uuid, &ctx.state.mc_server).await else {
            whisper(
                &ctx,
                &format!(
                    " {search} has no kills or deaths recorded, or unexpected error occurred."
                ),
            );
            return Ok(());
        };
        let total = kd.kills + kd.deaths;
        if total == 0 {
            whisper(&ctx, &format!(" {search} has no kills or deaths recorded."));
            return Ok(());
        }
        let winrate = (kd.kills as f64 / total as f64) * 100.0;
        let deathrate = (kd.deaths as f64 / total as f64) * 100.0;
        ctx.bot.chat(&format!(
            " {search}: Win Rate: {winrate:.1}% | Death Rate: {deathrate:.1}% ({}K / {}D)",
            kd.kills, kd.deaths
        ));
        Ok(())
    })
}

fn first_death<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    death_or_kill(ctx, true, true)
}

fn last_death<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    death_or_kill(ctx, true, false)
}

fn first_kill<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    death_or_kill(ctx, false, true)
}

fn last_kill<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    death_or_kill(ctx, false, false)
}

fn death_or_kill<'a>(ctx: CommandContext<'a>, death: bool, first: bool) -> CommandFuture<'a> {
    Box::pin(async move {
        let search = ctx.args.first().copied().unwrap_or(ctx.sender);
        let Some(uuid) = ctx.state.api.convert_username_to_uuid(search).await else {
            whisper(
                &ctx,
                &format!(" {search} has no deaths, or unexpected error occurred."),
            );
            return Ok(());
        };
        let order = if first { "ASC" } else { "DESC" };
        let rows = if death {
            ctx.state
                .api
                .get_deaths(&uuid, &ctx.state.mc_server, 1, order, "all")
                .await
        } else {
            ctx.state
                .api
                .get_kills(&uuid, &ctx.state.mc_server, 1, order)
                .await
        };
        let Some(row) = rows.and_then(|mut rows| rows.pop()) else {
            whisper(
                &ctx,
                &format!(" {search} has no deaths, or unexpected error occurred."),
            );
            return Ok(());
        };
        ctx.bot.chat(&format!(
            " {}, {}",
            row.death_message,
            time::time_ago_str(row.time as u64)
        ));
        Ok(())
    })
}

fn last_advancement<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    Box::pin(async move {
        let search = ctx.args.first().copied().unwrap_or(ctx.sender);
        let Some(uuid) = ctx.state.api.convert_username_to_uuid(search).await else {
            whisper(&ctx, &format!(" {search} has no advancements."));
            return Ok(());
        };
        let row = ctx
            .state
            .api
            .get_advancements(&uuid, &ctx.state.mc_server, 1, "DESC")
            .await
            .and_then(|mut rows| rows.pop());
        if let Some(row) = row {
            ctx.bot.chat(&format!(
                " {}: {} ({})",
                search,
                row.advancement,
                time::time_ago_str(row.time as u64)
            ));
        }
        Ok(())
    })
}

fn first_message<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    message_lookup(ctx, "ASC")
}

fn last_message<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    message_lookup(ctx, "DESC")
}

fn message_lookup<'a>(ctx: CommandContext<'a>, order: &'static str) -> CommandFuture<'a> {
    Box::pin(async move {
        let search = ctx.args.first().copied().unwrap_or(ctx.sender);
        let row = ctx
            .state
            .api
            .get_messages(search, &ctx.state.mc_server, 1, order, 0)
            .await
            .and_then(|mut rows| rows.pop());
        if let Some(row) = row {
            ctx.bot
                .chat(&format!(" {}: {} ({})", row.name, row.message, row.date));
        } else {
            whisper(&ctx, &format!(" I have no messages recorded for {search}."));
        }
        Ok(())
    })
}

fn oldheads<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    sorted_unique_users(ctx, true)
}

fn noobs<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    sorted_unique_users(ctx, false)
}

fn sorted_unique_users<'a>(ctx: CommandContext<'a>, oldest: bool) -> CommandFuture<'a> {
    Box::pin(async move {
        let mut users = ctx
            .state
            .api
            .get_unique_users(&ctx.state.mc_server)
            .await
            .unwrap_or_default();
        users.sort_by_key(|user| user.joindate.parse::<u64>().unwrap_or_default());
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
                    time::time_ago_str(user.joindate.parse().unwrap_or_default())
                )
            })
            .collect::<Vec<_>>();
        ctx.bot
            .chat(&format!(" The 3 {label} users are: {}", rows.join(", ")));
        Ok(())
    })
}

fn top<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    Box::pin(async move {
        let Some(stat) = ctx.args.first() else {
            whisper(
                &ctx,
                " Usage: !top <kills/deaths/joins/playtime/advancements/messages>",
            );
            return Ok(());
        };
        let value = ctx
            .state
            .api
            .get_top_statistic(stat, &ctx.state.mc_server, 5)
            .await;
        let Some(value) = value else {
            whisper(&ctx, " Could not fetch top statistic right now.");
            return Ok(());
        };
        let rows = value
            .get("top_stat")
            .or_else(|| value.get(*stat))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let formatted = rows
            .into_iter()
            .filter_map(|row| {
                let username = row.get("username")?.as_str()?;
                let number = row
                    .get(*stat)
                    .or_else(|| row.get("count"))
                    .or_else(|| row.get("advancement_count"))?;
                Some(format!("{username}: {}", value_to_string(number)))
            })
            .collect::<Vec<_>>();
        ctx.bot.chat(&format!(
            " [TOP {}]: {}",
            stat.to_uppercase(),
            formatted.join(", ")
        ));
        Ok(())
    })
}

fn standing<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    Box::pin(async move {
        let target = ctx.args.first().copied().unwrap_or(ctx.sender);
        let requester_uuid = match player_uuid(&ctx, ctx.sender) {
            Some(uuid) => Some(uuid),
            None => ctx.state.api.convert_username_to_uuid(ctx.sender).await,
        };
        if let Some(uuid) = requester_uuid.as_ref()
            && ctx.runtime.user_blacklist.contains(uuid)
            && !target.eq_ignore_ascii_case(ctx.sender)
        {
            whisper(&ctx, " You can only check your own standing.");
            return Ok(());
        }
        let target_uuid = if target.eq_ignore_ascii_case(ctx.sender) {
            requester_uuid
        } else {
            match player_uuid(&ctx, target) {
                Some(uuid) => Some(uuid),
                None => ctx.state.api.convert_username_to_uuid(target).await,
            }
        };
        let status = match target_uuid {
            Some(uuid) if ctx.runtime.user_blacklist.contains(&uuid) => "blacklisted",
            Some(uuid) if ctx.runtime.user_whitelist.contains(&uuid) => "whitelisted",
            _ => "regular",
        };
        ctx.bot.chat(&format!(" {target} is {status}."));
        Ok(())
    })
}

fn offline_msg<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    Box::pin(async move {
        let Some(recipient) = ctx.args.first().copied() else {
            whisper(&ctx, " Usage: !offlinemsg <username> <message>");
            return Ok(());
        };
        let message = ctx
            .args
            .iter()
            .skip(1)
            .copied()
            .collect::<Vec<_>>()
            .join(" ");
        if recipient.eq_ignore_ascii_case(ctx.sender) {
            whisper(&ctx, " You can't send a message to yourself, sorry.");
            return Ok(());
        }
        if message.len() > 250 {
            whisper(
                &ctx,
                " Message is too long, must be less than 250 characters.",
            );
            return Ok(());
        }
        if player_uuid(&ctx, recipient).is_some() {
            whisper(
                &ctx,
                &format!(" User {recipient} is online, please send them a message directly."),
            );
            return Ok(());
        }
        if ctx
            .state
            .api
            .convert_username_to_uuid(recipient)
            .await
            .is_none()
        {
            whisper(&ctx, &format!(" User {recipient} is not in the database."));
            return Ok(());
        }
        let mut messages = load_offline_messages().await.unwrap_or_default();
        let pending_count = messages
            .iter()
            .filter(|msg| msg.recipient.eq_ignore_ascii_case(recipient))
            .count();
        if pending_count >= 5 {
            whisper(
                &ctx,
                &format!(" User {recipient} has too many offline messages pending..."),
            );
            return Ok(());
        }
        messages.push(OfflineMessage {
            sender: ctx.sender.to_owned(),
            recipient: recipient.to_owned(),
            message,
            timestamp: now_millis(),
        });
        save_offline_messages(&messages).await?;
        whisper(
            &ctx,
            &format!(
                " Your message has been saved and will be delivered to {recipient} when they are next online."
            ),
        );
        Ok(())
    })
}

fn whois<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    Box::pin(async move {
        let target = ctx.args.first().copied().unwrap_or(ctx.sender);
        let data = ctx.state.api.get_who_is(target).await;
        if let Some(data) = data
            && !data.description.is_empty()
        {
            ctx.bot
                .chat(&format!(" {target}: {}", data.description.join(" ")));
        } else {
            ctx.bot.chat(&format!(" {target} has no description."));
        }
        Ok(())
    })
}

fn random_quote<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    Box::pin(async move {
        let phrase = ctx.args.first().map(|s| (*s).to_owned());
        let data = ctx
            .state
            .api
            .get_quote(
                "",
                &ctx.state.mc_server,
                Some(crate::structure::endpoints::endpoints::QuoteOptions {
                    random: true,
                    phrase,
                }),
            )
            .await;
        if let Some(data) = data {
            ctx.bot
                .chat(&format!(" Quote from {}: \"{}\"", data.name, data.message));
        }
        Ok(())
    })
}

fn list_quote_servers<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    Box::pin(async move {
        let servers = crate::constants::quote_servers::QUOTE_SERVERS.join(", ");
        ctx.bot.chat(&format!(
            " Quotable servers ({}): all, {}",
            crate::constants::quote_servers::QUOTE_SERVERS.len() + 1,
            servers
        ));
        Ok(())
    })
}

async fn parse_target_with_uuid<'a>(
    ctx: &CommandContext<'a>,
    usage_name: &str,
) -> anyhow::Result<Option<(crate::commands::utils::stats_target::StatsTarget, String)>> {
    let target = match parse_stats_target_args(&ctx.args, ctx.sender, &ctx.state.mc_server) {
        Ok(target) => target,
        Err(error) => {
            let msg = match error {
                StatsTargetError::MissingUsernameForAll => {
                    format!(" Usage: !{usage_name} all <username>")
                }
                StatsTargetError::MissingUsername => {
                    format!(" Usage: !{usage_name} <server|all> <username>")
                }
                StatsTargetError::UnknownServer(server) => {
                    format!(" Unknown server \"{server}\". Use !lq for the list.")
                }
            };
            whisper(ctx, &msg);
            return Ok(None);
        }
    };
    let Some(uuid) = ctx.state.api.convert_username_to_uuid(&target.search).await else {
        whisper_no_record(ctx, &target.search, "stats");
        return Ok(None);
    };
    Ok(Some((target, uuid)))
}

fn whisper(ctx: &CommandContext<'_>, message: &str) {
    ctx.bot.chat(&format!(
        "/{} {} {}",
        ctx.runtime.whisper_command, ctx.sender, message
    ));
}

fn whisper_no_record(ctx: &CommandContext<'_>, search: &str, thing: &str) {
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

fn player_uuid(ctx: &CommandContext<'_>, username: &str) -> Option<String> {
    ctx.state
        .players
        .read()
        .expect("player cache lock poisoned")
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(username))
        .map(|(_, player)| player.uuid.clone())
}

fn format_date_value(value: &str) -> String {
    if let Some(ms) = epoch_ms_from_string(value) {
        time::convert_unix_timestamp(ms / 1000)
    } else {
        value.to_owned()
    }
}

fn epoch_ms_from_string(value: &str) -> Option<u64> {
    let raw = value.parse::<u64>().ok()?;
    Some(if raw < 1_000_000_000_000 {
        raw * 1000
    } else {
        raw
    })
}

fn member_days(join_ms: u64) -> u64 {
    now_millis().saturating_sub(join_ms) / 86_400_000
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
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
