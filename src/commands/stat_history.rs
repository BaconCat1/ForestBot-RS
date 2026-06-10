use crate::{
    commands::{
        CommandContext, CommandDefinition, CommandFuture,
        utils::stats_target::{
            StatsTargetError, format_server_label, format_server_scope_hint,
            parse_stats_target_args, parse_stats_target_or_reply,
        },
    },
    config::{
        OfflineMessage, load_offline_messages, load_user_list, load_word_list,
        save_offline_messages, save_user_list, save_word_list,
    },
    constants::quote_servers::QUOTE_SERVERS,
    functions::utils::time,
    structure::{endpoints::endpoints::QuoteOptions, logger},
};
use futures_util::stream::{self, StreamExt};
use serde::Deserialize;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicU64, Ordering},
};

const ACTIVE_CACHE_TTL_MS: u64 = 5 * 60 * 1000;
const HISTORICAL_TOP_CACHE_TTL_MS: u64 = 15 * 60 * 1000;
const ACTIVE_TOP_LIMIT: usize = 5;
const ACTIVE_MSG_FETCH: usize = 100;
const ADVANCEMENT_COUNT_FETCH_LIMIT: usize = 1000;
const ACTIVE_CONCURRENCY: usize = 12;
const TOP_LIMIT: usize = 5;
const BACKFILL_CONCURRENCY: usize = 12;
const ONE_DAY_MS: u64 = 24 * 60 * 60 * 1000;
const SHOUT_COOLDOWN_MS: u64 = 60 * 1000;
const MC_WHITELIST_PATH: &str = "./json/mc_whitelist.json";
const MC_BLACKLIST_PATH: &str = "./json/mc_blacklist.json";
const BAD_WORDS_PATH: &str = "./json/bad_words.json";
const WORD_WHITELIST_PATH: &str = "./json/word_whitelist.json";

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

static ACTIVE_CACHE: OnceLock<Mutex<HashMap<String, ActiveCacheEntry>>> = OnceLock::new();
static HISTORICAL_TOP_CACHE: OnceLock<Mutex<HashMap<String, HistoricalTopCacheEntry>>> =
    OnceLock::new();
static LAST_SHOUT_AT: AtomicU64 = AtomicU64::new(0);

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

macro_rules! command {
    ($const_name:ident, $names:expr, $execute:ident) => {
        pub const $const_name: CommandDefinition = CommandDefinition {
            names: $names,
            whitelisted: false,
            execute: $execute,
        };
    };
}

macro_rules! admin_command {
    ($const_name:ident, $names:expr, $execute:ident) => {
        pub const $const_name: CommandDefinition = CommandDefinition {
            names: $names,
            whitelisted: true,
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
command!(ACTIVE_COMMAND, &["active"], active);
command!(ADD_FAQ_COMMAND, &["addfaq"], add_faq);
command!(
    ADVANCEMENT_COUNT_COMMAND,
    &["advancement", "advancementcount"],
    advancement_count
);
admin_command!(BLACKLIST_COMMAND, &["blacklist"], blacklist);
command!(AVERAGE_PING_COMMAND, &["averageping", "ap"], average_ping);
command!(BEST_PING_COMMAND, &["bp", "bestping"], best_ping);
admin_command!(CENSOR_COMMAND, &["censor"], censor);
command!(COORDS_COMMAND, &["coords"], coords);
command!(EDIT_FAQ_COMMAND, &["editfaq"], edit_faq);
command!(EFFICIENCY_COMMAND, &["efficiency", "eff"], efficiency);
admin_command!(EXECUTE_COMMAND, &["execute", "exec", "run"], execute);
command!(FEBZEY_COMMAND, &["febzey"], febzey);
command!(FAQ_COMMAND, &["faq", "getfaq"], faq);
command!(GRUDGE_COMMAND, &["grudge"], grudge);
command!(IAM_COMMAND, &["iam"], iam);
command!(MOUNT_COMMAND, &["mount", "ride", "mush"], mount);
command!(NICKNAME_COMMAND, &["nickname"], nickname);
command!(OLDNAMES_COMMAND, &["oldnames", "dox", "doxx"], oldnames);
command!(
    OWNS_FAQ_COMMAND,
    &["ownsfaq", "ownfaq", "faqowner"],
    owns_faq
);
command!(PROFILE_COMMAND, &["profile"], profile);
command!(
    RANDOM_QUOTE_ALL_COMMAND,
    &["rqa", "randomquoteall"],
    random_quote_all
);
command!(REALNAME_COMMAND, &["realname"], realname);
command!(SET_PRESET_COMMAND, &["setpreset"], set_preset);
command!(SHOUT_COMMAND, &["shout"], shout);
command!(SLEEP_COMMAND, &["sleep"], sleep);
command!(
    SERVERS_COMMAND,
    &["servers", "playerservers", "seenservers"],
    servers
);
command!(SURVIVED_COMMAND, &["survived"], survived);
command!(
    TWERK_COMMAND,
    &["twerk", "bootyshake", "booty", "dance"],
    twerk
);
command!(VICTIMS_COMMAND, &["victims", "murders", "bested"], victims);
command!(VS_COMMAND, &["vs"], vs);
admin_command!(WHITELIST_COMMAND, &["whitelist"], whitelist);
admin_command!(
    WORD_WHITELIST_COMMAND,
    &["wordwhitelist", "wwl"],
    word_whitelist
);
command!(WORST_PING_COMMAND, &["wp", "worstping"], worst_ping);

fn kd(ctx: CommandContext<'_>) -> CommandFuture<'_> {
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
        ctx.chat(format!(
            " {}{}: Kills: {} Deaths: {} KD: {:.2}",
            target.search, label, data.kills, data.deaths, ratio
        ));
        Ok(())
    })
}

fn joindate(ctx: CommandContext<'_>) -> CommandFuture<'_> {
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
        ctx.chat(format!(
            " {}{}, joined on: {}",
            target.search,
            label,
            format_date_value(&data.join_date)
        ));
        Ok(())
    })
}

fn jdpt(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some((target, uuid)) = parse_target_with_uuid(&ctx, "jdpt").await? else {
            return Ok(());
        };
        let (jd, pt) = tokio::join!(
            ctx.state.api.get_join_date(&uuid, &target.server),
            ctx.state.api.get_playtime(&uuid, &target.server)
        );
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
        ctx.chat(format!(
            " {}{}, {}",
            target.search,
            label,
            parts.join(" | ")
        ));
        Ok(())
    })
}

fn wordcount(ctx: CommandContext<'_>) -> CommandFuture<'_> {
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
        ctx.chat(format!(
            " {search}{label} has said {word} {} times",
            data.count
        ));
        Ok(())
    })
}

fn namefind(ctx: CommandContext<'_>) -> CommandFuture<'_> {
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
            ctx.chat(format!(
                " You could be looking for: {}",
                data.usernames.join(", ")
            ));
        }
        Ok(())
    })
}

fn unique_users(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let users = ctx.state.api.get_unique_users(&ctx.state.mc_server).await;
        let count = users.map(|users| users.len()).unwrap_or_default();
        ctx.chat(format!(
            " I have seen {count} different users on this server! api.forestbot.org/unique-users?server={}",
            ctx.state.mc_server
        ));
        Ok(())
    })
}

fn total_advancements(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let target = match parse_stats_target_args(&ctx.args, ctx.sender, &ctx.state.mc_server) {
            Ok(target) => target,
            Err(StatsTargetError::MissingUsernameForAll) => {
                whisper(&ctx, " Usage: !advs all <username>");
                return Ok(());
            }
            Err(StatsTargetError::UnknownServer(server)) => {
                whisper(
                    &ctx,
                    &format!(" Unknown server \"{server}\". Use !lq for the list."),
                );
                return Ok(());
            }
            Err(StatsTargetError::MissingUsername) => {
                whisper(&ctx, " Usage: !advs <server|all> <username>");
                return Ok(());
            }
        };

        let Some(uuid) = ctx.state.api.convert_username_to_uuid(&target.search).await else {
            let hint = format_server_scope_hint(
                target.has_server_arg,
                &target.server,
                &ctx.state.mc_server,
            );
            if target.search.eq_ignore_ascii_case(ctx.sender) {
                whisper(
                    &ctx,
                    &format!(
                        " I have not seen any advancements from you{hint}, or unexpected error occurred."
                    ),
                );
            } else {
                whisper(
                    &ctx,
                    &format!(
                        " I have not seen any advancements from {}{hint}, or unexpected error occurred.",
                        target.search
                    ),
                );
            }
            return Ok(());
        };
        let count = ctx
            .state
            .api
            .get_total_advancements_count(&uuid, &target.server)
            .await
            .unwrap_or_default();
        if count == 0 {
            let hint = format_server_scope_hint(
                target.has_server_arg,
                &target.server,
                &ctx.state.mc_server,
            );
            if target.search.eq_ignore_ascii_case(ctx.sender) {
                whisper(
                    &ctx,
                    &format!(
                        " I have not seen any advancements from you{hint}, or unexpected error occurred."
                    ),
                );
            } else {
                whisper(
                    &ctx,
                    &format!(
                        " I have not seen any advancements from {}{hint}, or unexpected error occurred.",
                        target.search
                    ),
                );
            }
            return Ok(());
        }

        let label = format_server_label(&target.server, &ctx.state.mc_server);
        ctx.chat(format!(
            " I have seen {count} advancements from {}{}",
            target.search, label
        ));
        Ok(())
    })
}

fn summary(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let search = ctx.args.first().copied().unwrap_or(ctx.sender);
        let Some(uuid) = ctx.state.api.convert_username_to_uuid(search).await else {
            whisper(&ctx, &format!(" Could not find {search}."));
            return Ok(());
        };
        let (kd, pt, mc, adv, jd) = tokio::join!(
            ctx.state.api.get_kd(&uuid, &ctx.state.mc_server),
            ctx.state.api.get_playtime(&uuid, &ctx.state.mc_server),
            ctx.state
                .api
                .get_message_count(search, &ctx.state.mc_server),
            ctx.state
                .api
                .get_total_advancements_count(&uuid, &ctx.state.mc_server),
            ctx.state.api.get_join_date(&uuid, &ctx.state.mc_server)
        );
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
        ctx.chat(format!(
            " [{search}] KD: {kills}/{deaths} ({kdr:.2}) | Playtime: {pt_days}d | Messages: {messages} | Advancements: {adv} | Member for: {age}"
        ));
        Ok(())
    })
}

fn winrate(ctx: CommandContext<'_>) -> CommandFuture<'_> {
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
        ctx.chat(format!(
            " {search}: Win Rate: {winrate:.1}% | Death Rate: {deathrate:.1}% ({}K / {}D)",
            kd.kills, kd.deaths
        ));
        Ok(())
    })
}

fn first_death(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    death_or_kill(ctx, true, true)
}

fn last_death(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    death_or_kill(ctx, true, false)
}

fn first_kill(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    death_or_kill(ctx, false, true)
}

fn last_kill(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    death_or_kill(ctx, false, false)
}

fn death_or_kill(ctx: CommandContext<'_>, death: bool, first: bool) -> CommandFuture<'_> {
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
            time::time_ago_str(row.time as u64)
        ));
        Ok(())
    })
}

fn advancement_count(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let search = ctx.args.join(" ").trim().to_owned();
        if search.is_empty() {
            whisper(&ctx, " Usage: !advancement <advancement>");
            return Ok(());
        }

        whisper(
            &ctx,
            " Counting advancement matches, this may take a moment...",
        );
        let needle = search.to_ascii_lowercase();
        let server = ctx.state.mc_server.clone();
        let api = ctx.state.api.clone();
        let usernames = all_known_usernames(&ctx).await;
        let count = stream::iter(usernames)
            .map(|username| {
                let api = api.clone();
                let server = server.clone();
                let needle = needle.clone();
                async move {
                    let uuid = api.convert_username_to_uuid(&username).await?;
                    let advancements = api
                        .get_advancements(&uuid, &server, ADVANCEMENT_COUNT_FETCH_LIMIT, "DESC")
                        .await
                        .unwrap_or_default();
                    Some(
                        advancements
                            .into_iter()
                            .filter(|row| row.advancement.to_ascii_lowercase().contains(&needle))
                            .count(),
                    )
                }
            })
            .buffer_unordered(BACKFILL_CONCURRENCY)
            .fold(0usize, |total, count| async move {
                total + count.unwrap_or_default()
            })
            .await;

        ctx.chat(format!(
            " Advancement \"{search}\" has been reached {count} time{} on {}.",
            if count == 1 { "" } else { "s" },
            ctx.state.mc_server
        ));
        Ok(())
    })
}

fn last_advancement(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(target) = parse_stats_target_or_reply(&ctx, "lastadvancement") else {
            return Ok(());
        };
        let Some(uuid) = ctx.state.api.convert_username_to_uuid(&target.search).await else {
            whisper(&ctx, &format!(" {} has no advancements.", target.search));
            return Ok(());
        };
        let row = ctx
            .state
            .api
            .get_advancements(&uuid, &target.server, 1, "DESC")
            .await
            .and_then(|mut rows| rows.pop());
        if let Some(row) = row {
            let label = format_server_label(&target.server, &ctx.state.mc_server);
            ctx.chat(format!(
                " {}{}: {} ({})",
                target.search,
                label,
                row.advancement,
                time::time_ago_str(row.time as u64)
            ));
        }
        Ok(())
    })
}

fn first_message(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    message_lookup(ctx, "ASC")
}

fn last_message(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    message_lookup(ctx, "DESC")
}

fn message_lookup<'a>(ctx: CommandContext<'a>, order: &'static str) -> CommandFuture<'a> {
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
                .map(time::time_ago_str)
                .unwrap_or(row.date);
            let label = format_server_label(&target.server, &ctx.state.mc_server);
            ctx.chat(format!(
                " {}{}: {}, {date}",
                target.search, label, row.message
            ));
        } else {
            if target.search.eq_ignore_ascii_case(ctx.sender) {
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
        }
        Ok(())
    })
}

fn oldheads(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    sorted_unique_users(ctx, true)
}

fn noobs(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    sorted_unique_users(ctx, false)
}

fn sorted_unique_users(ctx: CommandContext<'_>, oldest: bool) -> CommandFuture<'_> {
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
        ctx.chat(format!(" The 3 {label} users are: {}", rows.join(", ")));
        Ok(())
    })
}

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
            " Unknown server \"{server}\". Use !lq for the list."
        ));
    }
    Ok(server)
}

async fn top_backend_stat(
    ctx: &CommandContext<'_>,
    stat: &str,
    server: &str,
) -> anyhow::Result<()> {
    let value = ctx
        .state
        .api
        .get_top_statistic(stat, server, TOP_LIMIT)
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
            let number = row.get(stat)?;
            if stat == "playtime" {
                let days = number.as_u64().unwrap_or_default() / ONE_DAY_MS;
                Some(format!("{username}: {days} Days"))
            } else {
                Some(format!("{username}: {}", value_to_string(number)))
            }
        })
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
    let rows = cached_top_rows(server, "messages");
    let rows = match rows {
        Some(rows) => rows,
        None => {
            whisper(ctx, "Running historical backfill for top messages...");
            let rows = get_top_messages_historical(ctx, server).await;
            if !rows.is_empty() {
                cache_top_rows(server, "messages", rows.clone());
            }
            rows
        }
    };

    if rows.is_empty() {
        whisper(ctx, "Could not calculate top messages right now.");
    } else {
        let title = format!(
            "TOP MESSAGES{}",
            format_server_label(server, &ctx.state.mc_server)
        );
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

async fn get_top_messages_historical(ctx: &CommandContext<'_>, server: &str) -> Vec<TopRow> {
    let usernames = all_known_usernames_for_server(ctx, server).await;
    let api = ctx.state.api.clone();
    let server = server.to_owned();
    let mut rows = stream::iter(usernames)
        .map(|username| {
            let api = api.clone();
            let server = server.clone();
            async move {
                api.get_message_count(&username, &server)
                    .await
                    .map(|data| TopRow {
                        username,
                        value: data.message_count,
                    })
            }
        })
        .buffer_unordered(BACKFILL_CONCURRENCY)
        .filter_map(|row| async move { row })
        .collect::<Vec<_>>()
        .await;
    sort_and_truncate_top_rows(&mut rows);
    rows
}

async fn get_top_advancements_historical(ctx: &CommandContext<'_>, server: &str) -> Vec<TopRow> {
    let usernames = all_known_usernames_for_server(ctx, server).await;
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
    let value = ctx.state.api.get_leaderboards(server).await?;
    let mut rows = value
        .get("advancements")?
        .as_array()?
        .iter()
        .filter_map(|row| {
            let username = row.get("player_name")?.as_str()?.to_owned();
            let value = row.get("advancement_count").and_then(number_from_value)?;
            Some(TopRow { username, value })
        })
        .collect::<Vec<_>>();
    rows.truncate(TOP_LIMIT);
    Some(rows)
}

async fn all_known_usernames(ctx: &CommandContext<'_>) -> Vec<String> {
    all_known_usernames_for_server(ctx, &ctx.state.mc_server).await
}

async fn all_known_usernames_for_server(ctx: &CommandContext<'_>, server: &str) -> Vec<String> {
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
    let now = now_millis();
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
                expires_at: now_millis().saturating_add(HISTORICAL_TOP_CACHE_TTL_MS),
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

fn quote_server_chunks(servers: &[&str]) -> Vec<String> {
    const MAX_MESSAGE_LENGTH: usize = 230;
    const CONTINUATION_PREFIX: &str = "More: ";

    let intro = format!("Quotable servers ({}): ", servers.len());
    let mut chunks = Vec::new();
    let mut current = intro;
    let mut has_server_in_chunk = false;

    for server in servers {
        let separator = if has_server_in_chunk { ", " } else { "" };
        let next = format!("{current}{separator}{server}");

        if next.len() > MAX_MESSAGE_LENGTH {
            chunks.push(current);
            current = format!("{CONTINUATION_PREFIX}{server}");
            has_server_in_chunk = true;
            continue;
        }

        current = next;
        has_server_in_chunk = true;
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

fn standing(ctx: CommandContext<'_>) -> CommandFuture<'_> {
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
        ctx.chat(format!(" {target} is {status}."));
        Ok(())
    })
}

fn offline_msg(ctx: CommandContext<'_>) -> CommandFuture<'_> {
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

fn whois(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let target = ctx.args.first().copied().unwrap_or(ctx.sender);
        let data = ctx.state.api.get_who_is(target).await;
        if let Some(data) = data
            && !data.description.is_empty()
        {
            let description = data.description.join(" ");
            let safe_description = description
                .replace(['\r', '\n'], " ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            ctx.chat(format!("User {target} is {safe_description}"));
        } else {
            let message = if target.eq_ignore_ascii_case(ctx.sender) {
                " You have not yet set a description with !iam".to_owned()
            } else {
                format!(" {target} has not yet set a description with !iam")
            };
            whisper(&ctx, &message);
        }
        Ok(())
    })
}

fn random_quote(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let phrase = ctx.args.first().map(|s| (*s).to_owned());
        let data = ctx
            .state
            .api
            .get_quote(
                "none",
                &ctx.state.mc_server,
                Some(QuoteOptions {
                    random: true,
                    phrase,
                }),
            )
            .await;
        if let Some(data) = data {
            let date = data
                .date
                .as_deref()
                .and_then(epoch_ms_from_string)
                .map(time::time_ago_str)
                .map(|date| format!(" ({date})"))
                .unwrap_or_default();
            ctx.chat(format!(
                " Quote from {}: \"{}\"{}",
                data.name, data.message, date
            ));
        } else {
            whisper(&ctx, " unexpected error occurred.");
        }
        Ok(())
    })
}

fn list_quote_servers(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let servers = std::iter::once("all")
            .chain(
                crate::constants::quote_servers::QUOTE_SERVERS
                    .iter()
                    .copied(),
            )
            .collect::<Vec<_>>();
        let chunks = quote_server_chunks(&servers);
        if chunks.len() == 1 {
            ctx.chat(format!(" {}", chunks[0]));
        } else {
            for chunk in chunks {
                whisper(&ctx, &format!(" {chunk}"));
            }
        }
        Ok(())
    })
}

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

fn add_faq(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let faq = ctx.args.join(" ").trim().to_owned();
        if faq.is_empty() {
            whisper(&ctx, " Add a FAQ with !addfaq <text>");
            return Ok(());
        }
        if faq.contains('/') {
            whisper(&ctx, " You can't use '/' in your FAQ.");
            return Ok(());
        }
        let Some(uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
            whisper(&ctx, " An error occurred while adding your FAQ.");
            return Ok(());
        };
        let Some(data) = ctx
            .state
            .api
            .post_new_faq(ctx.sender, &faq, &uuid, &ctx.state.mc_server)
            .await
        else {
            whisper(&ctx, " An error occurred while adding your FAQ.");
            return Ok(());
        };
        if let Some(error) = data.error {
            whisper(&ctx, &format!(" {error}"));
        } else {
            whisper(
                &ctx,
                &format!(" Your FAQ has been added. Your entry ID is {}.", data.id),
            );
        }
        Ok(())
    })
}

fn blacklist(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    list_command(ctx, MC_BLACKLIST_PATH, true)
}

fn whitelist(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    list_command(ctx, MC_WHITELIST_PATH, false)
}

fn average_ping(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let players = players_snapshot(&ctx);
        if players.is_empty() {
            whisper(&ctx, " No players are cached yet.");
            return Ok(());
        }

        let measured = players
            .iter()
            .filter(|player| player.latency > 0)
            .collect::<Vec<_>>();
        let ping_players = if measured.is_empty() {
            players.iter().collect::<Vec<_>>()
        } else {
            measured
        };
        let total = ping_players
            .iter()
            .map(|player| player.latency as i64)
            .sum::<i64>();
        let average = total as f64 / ping_players.len() as f64;
        let best = ping_players
            .iter()
            .min_by_key(|player| player.latency)
            .expect("ping_players is not empty");
        let worst = ping_players
            .iter()
            .max_by_key(|player| player.latency)
            .expect("ping_players is not empty");

        ctx.chat(format!(
            " Average ping: {:.1}ms | Best: {}: {}ms | Worst: {}: {}ms",
            average, best.username, best.latency, worst.username, worst.latency
        ));
        Ok(())
    })
}

fn best_ping(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let players = players_snapshot(&ctx);
        let Some(best) = players
            .iter()
            .filter(|player| player.latency > 0)
            .min_by_key(|player| player.latency)
            .or_else(|| players.first())
        else {
            whisper(&ctx, " No players are cached yet.");
            return Ok(());
        };
        if best.latency == 0 {
            ctx.chat(format!(
                " Best ping: {}: {}ms (Most likely just joined.)",
                best.username, best.latency
            ));
        } else {
            ctx.chat(format!(" Best ping: {}: {}ms", best.username, best.latency));
        }
        Ok(())
    })
}

fn worst_ping(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let players = players_snapshot(&ctx);
        let Some(worst) = players.iter().max_by_key(|player| player.latency) else {
            whisper(&ctx, " No players are cached yet.");
            return Ok(());
        };
        ctx.chat(format!(
            " Worst Ping: {}: {}ms",
            worst.username, worst.latency
        ));
        Ok(())
    })
}

fn censor(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    word_list_command(ctx, BAD_WORDS_PATH, "bad words")
}

fn word_whitelist(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    word_list_command(ctx, WORD_WHITELIST_PATH, "word whitelist")
}

fn coords(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        if ctx.runtime.use_whitelist && !sender_whitelisted(&ctx) {
            return Ok(());
        }
        let pos = ctx.bot.position();
        whisper(
            &ctx,
            &format!(
                " I am currently at: X: {} Y: {} Z: {}",
                pos.x.floor() as i64,
                pos.y.floor() as i64,
                pos.z.floor() as i64
            ),
        );
        Ok(())
    })
}

fn edit_faq(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(id_raw) = ctx.args.first() else {
            whisper(
                &ctx,
                " Please provide a valid FAQ ID. Usage: !editfaq <id> <new text>",
            );
            return Ok(());
        };
        let Ok(id) = id_raw.parse::<i64>() else {
            whisper(
                &ctx,
                " Please provide a valid FAQ ID. Usage: !editfaq <id> <new text>",
            );
            return Ok(());
        };
        let faq = ctx
            .args
            .iter()
            .skip(1)
            .copied()
            .collect::<Vec<_>>()
            .join(" ");
        if faq.starts_with('/') {
            whisper(&ctx, " FAQ text cannot start with '/'.");
            return Ok(());
        }
        if faq.len() < 5 {
            whisper(&ctx, " FAQ text must be at least 5 characters long.");
            return Ok(());
        }
        let Some(uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
            whisper(&ctx, " An error occurred while editing your FAQ.");
            return Ok(());
        };
        let Some(data) = ctx
            .state
            .api
            .edit_faq(id, ctx.sender, &faq, &uuid, &ctx.state.mc_server)
            .await
        else {
            whisper(&ctx, " An error occurred while editing your FAQ.");
            return Ok(());
        };
        if let Some(error) = data.error {
            whisper(&ctx, &format!(" {error}"));
        } else {
            whisper(
                &ctx,
                &format!(" Your FAQ has been successfully updated. ID: {id}."),
            );
        }
        Ok(())
    })
}

fn efficiency(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let (search, stat) = match ctx.args.as_slice() {
            [stat] => (ctx.sender, stat.to_lowercase()),
            [search, stat, ..] => (*search, stat.to_lowercase()),
            _ => {
                whisper(
                    &ctx,
                    " Valid stats: kills, deaths, messages. Usage: !efficiency [username] <stat>",
                );
                return Ok(());
            }
        };
        if !matches!(stat.as_str(), "kills" | "deaths" | "messages") {
            whisper(
                &ctx,
                " Valid stats: kills, deaths, messages. Usage: !efficiency [username] <stat>",
            );
            return Ok(());
        }
        let Some(uuid) = ctx.state.api.convert_username_to_uuid(search).await else {
            whisper(
                &ctx,
                &format!(" Couldn't get stats for {search}, or unexpected error occurred."),
            );
            return Ok(());
        };
        if stat == "kills" || stat == "deaths" {
            let (kd, pt) = tokio::join!(
                ctx.state.api.get_kd(&uuid, &ctx.state.mc_server),
                ctx.state.api.get_playtime(&uuid, &ctx.state.mc_server)
            );
            let (Some(kd), Some(pt)) = (kd, pt) else {
                whisper(
                    &ctx,
                    &format!(" Couldn't get stats for {search}, or unexpected error occurred."),
                );
                return Ok(());
            };
            let hours = pt.playtime as f64 / 3_600_000_f64;
            if hours == 0.0 {
                whisper(&ctx, &format!(" {search} has no playtime recorded."));
                return Ok(());
            }
            let count = if stat == "kills" { kd.kills } else { kd.deaths };
            ctx.chat(format!(
                " {search}: {count} {stat} over {hours:.1} hours = {:.3} {stat}/hr",
                count as f64 / hours
            ));
        } else {
            let (mc, jd) = tokio::join!(
                ctx.state
                    .api
                    .get_message_count(search, &ctx.state.mc_server),
                ctx.state.api.get_join_date(&uuid, &ctx.state.mc_server)
            );
            let (Some(mc), Some(jd)) = (mc, jd) else {
                whisper(
                    &ctx,
                    &format!(" Couldn't get stats for {search}, or unexpected error occurred."),
                );
                return Ok(());
            };
            let Some(join_ms) = epoch_ms_from_string(&jd.join_date) else {
                whisper(
                    &ctx,
                    &format!(" Couldn't determine join date for {search}."),
                );
                return Ok(());
            };
            let days = now_millis().saturating_sub(join_ms) as f64 / 86_400_000_f64;
            if days <= 0.0 {
                whisper(
                    &ctx,
                    &format!(" Couldn't calculate message rate for {search}."),
                );
                return Ok(());
            }
            ctx.chat(format!(
                " {search}: {} messages over {} days = {:.2} messages/day",
                mc.message_count,
                days.floor() as u64,
                mc.message_count as f64 / days
            ));
        }
        Ok(())
    })
}

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let command = ctx.args.join(" ").trim().to_owned();
        if command.is_empty() {
            whisper(&ctx, " Usage: !execute </command>");
            return Ok(());
        }
        ctx.chat(&command);
        whisper(&ctx, &format!(" Executed: {command}"));
        Ok(())
    })
}

fn febzey(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let pos = ctx.bot.position();
        logger::info(format!("[COORDS] x={:.1} y={:.1} z={:.1}", pos.x, pos.y, pos.z));
        let target = "Febzey_";
        let Some(uuid) = ctx.state.api.convert_username_to_uuid(target).await else {
            ctx.chat(format!(" I couldn't even find {target}. Truly absent."));
            return Ok(());
        };
        let last_seen = ctx
            .state
            .api
            .get_last_seen(&uuid, &ctx.state.mc_server)
            .await;
        let online = player_uuid(&ctx, target).is_some();
        match last_seen.and_then(|row| epoch_ms_from_string(&row.last_seen)) {
            Some(ts) if online => ctx.chat(format!(
                " {target} is online after being gone for {}. Someone check on the bot maintainer.",
                time::time_ago_str(ts)
            )),
            Some(ts) => ctx.chat(format!(
                " Last seen {target}: {} ({}). Still not maintaining his bot.",
                time::convert_unix_timestamp(ts / 1000),
                time::time_ago_str(ts)
            )),
            None => ctx.chat(format!(
                " No last seen data for {target}. The disappearance is complete."
            )),
        }
        Ok(())
    })
}

fn faq(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let id = ctx.args.first().copied();
        let Some(data) = ctx.state.api.get_faq(id, Some(&ctx.state.mc_server)).await else {
            whisper(
                &ctx,
                " There was an error getting your FAQ, it may not exist.",
            );
            return Ok(());
        };
        ctx.chat(format!(" #{}/{}: {}", data.id, data.total, data.faq));
        Ok(())
    })
}

fn grudge(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let (killer, victim) = match ctx.args.as_slice() {
            [victim] => (ctx.sender, *victim),
            [killer, victim, ..] => (*killer, *victim),
            _ => {
                whisper(&ctx, " Usage: !grudge [killer] <victim>");
                return Ok(());
            }
        };
        let Some(uuid) = ctx.state.api.convert_username_to_uuid(killer).await else {
            whisper(
                &ctx,
                &format!(" {killer} has no kills recorded, or unexpected error occurred."),
            );
            return Ok(());
        };
        let Some(kills) = ctx
            .state
            .api
            .get_kills(&uuid, &ctx.state.mc_server, 10000, "DESC")
            .await
        else {
            whisper(
                &ctx,
                &format!(" {killer} has no kills recorded, or unexpected error occurred."),
            );
            return Ok(());
        };
        let count = kills
            .iter()
            .filter_map(extract_victim_name)
            .filter(|name| name.eq_ignore_ascii_case(victim))
            .count();
        if count == 0 {
            ctx.chat(format!(" {killer} has never killed {victim}."));
        } else if count >= 30 {
            ctx.chat(format!(
                " {killer} has killed {victim} {count} times. That's a grudge!"
            ));
        } else {
            ctx.chat(format!(
                " {killer} has killed {victim} {count} time{}.",
                if count == 1 { "" } else { "s" }
            ));
        }
        Ok(())
    })
}

fn iam(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let description = ctx
            .args
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if description.is_empty() {
            whisper(&ctx, " View descriptions with !whois or set one with !iam");
            return Ok(());
        }
        if description.contains('/') {
            whisper(&ctx, " Descriptions cannot contain '/'.");
            return Ok(());
        }
        if ctx
            .state
            .api
            .post_who_is_description(ctx.sender, &description)
            .await
            .is_some()
        {
            whisper(&ctx, " your !whois has been set.");
        } else {
            whisper(
                &ctx,
                " Failed to save your description. Try a shorter/simpler message.",
            );
        }
        Ok(())
    })
}

fn mount(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        whisper(
            &ctx,
            " Mount is registered, but Azalea entity mounting parity is not wired yet.",
        );
        Ok(())
    })
}

fn nickname(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let nickname = ctx.args.join(" ").trim().to_owned();
        if nickname.is_empty() {
            whisper(&ctx, " Usage: !nickname <nickname>");
            return Ok(());
        }
        ctx.chat(format!(" /nick {nickname}"));
        Ok(())
    })
}

fn oldnames(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let target = ctx.args.first().copied().unwrap_or(ctx.sender);
        let url = format!(
            "https://api.ashcon.app/mojang/v2/user/{}",
            percent_encode_path_segment(target)
        );
        let response = reqwest::get(url).await;
        let Ok(response) = response else {
            ctx.chat(" An error occured while trying to look up the user.");
            return Ok(());
        };
        if response.status().as_u16() == 404 {
            whisper(
                &ctx,
                " Could not find the user you were looking for on the Ashcon API.",
            );
            return Ok(());
        }
        if !response.status().is_success() {
            ctx.chat(" An error occured while trying to look up the user.");
            return Ok(());
        }
        let profile = response.json::<AshconProfile>().await.ok();
        let mut names = profile
            .map(|profile| profile.username_history)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|entry| entry.username)
            .filter(|name| name != "1HateN1ggers" && name != "ShriviledP3ck3r")
            .collect::<Vec<_>>();
        names.dedup();
        if names.is_empty() {
            ctx.chat(" No name history was found for that user.");
        } else {
            ctx.chat(format!(
                " {target} has used the following names: {}",
                names.join(", ")
            ));
        }
        Ok(())
    })
}

fn owns_faq(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(id) = ctx.args.first() else {
            whisper(&ctx, " Usage: !ownsfaq <id>");
            return Ok(());
        };
        if id.parse::<i64>().is_err() {
            whisper(&ctx, " Usage: !ownsfaq <id>");
            return Ok(());
        }
        let Some(data) = ctx
            .state
            .api
            .get_faq(Some(id), Some(&ctx.state.mc_server))
            .await
        else {
            whisper(&ctx, &format!(" Could not find FAQ #{id}."));
            return Ok(());
        };
        ctx.chat(format!(" FAQ #{} owner: {}", data.id, data.username));
        Ok(())
    })
}

fn profile(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let target = ctx.args.first().copied().unwrap_or(ctx.sender);
        ctx.chat(format!(" https://forestbot.org/u/{target}"));
        Ok(())
    })
}

fn random_quote_all(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let phrase = ctx.args.first().map(|s| (*s).to_owned());
        let servers = crate::constants::quote_servers::QUOTE_SERVERS;
        let server = servers[(now_millis() as usize) % servers.len()];
        let data = ctx
            .state
            .api
            .get_quote(
                "none",
                server,
                Some(QuoteOptions {
                    random: true,
                    phrase: phrase.clone(),
                }),
            )
            .await;
        let Some(data) = data else {
            let phrase_label = phrase
                .as_deref()
                .map(|phrase| format!(" for \"{phrase}\""))
                .unwrap_or_default();
            whisper(
                &ctx,
                &format!(" No quotes found{phrase_label} on {server}."),
            );
            return Ok(());
        };
        let date = data
            .date
            .as_deref()
            .and_then(epoch_ms_from_string)
            .map(time::time_ago_str)
            .map(|date| format!(" ({date})"))
            .unwrap_or_default();
        ctx.chat(format!(
            " Quote from {} [{}]: \"{}\"{}",
            data.name, server, data.message, date
        ));
        Ok(())
    })
}

fn realname(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(target) = ctx.args.first() else {
            whisper(&ctx, " Please provide a username to check.");
            return Ok(());
        };
        let players = players_snapshot(&ctx);
        if players
            .iter()
            .any(|player| player.username.eq_ignore_ascii_case(target))
        {
            ctx.chat(format!("{target} is the real username."));
        } else {
            ctx.chat(format!("No player found matching \"{target}\" online."));
        }
        Ok(())
    })
}

fn set_preset(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(preset) = ctx.args.first() else {
            return Ok(());
        };
        ctx.chat(format!("/nc preset {preset}"));
        ctx.chat(format!(" Set the preset {preset} successfully!"));
        Ok(())
    })
}

fn shout(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let raw = ctx.args.join(" ");
        let message = raw.replace('/', "").trim().to_owned();
        if message.is_empty() {
            whisper(&ctx, " Usage: !shout <message>");
            return Ok(());
        }
        let now = now_millis();
        let last = LAST_SHOUT_AT.load(Ordering::Relaxed);
        let remaining = SHOUT_COOLDOWN_MS.saturating_sub(now.saturating_sub(last));
        if remaining > 0 {
            whisper(
                &ctx,
                &format!(
                    " Shout is on cooldown. Try again in {} minute(s).",
                    remaining.div_ceil(60_000)
                ),
            );
            return Ok(());
        }
        let shout_text = format!(
            "[Shout {}] {}: {}",
            ctx.state.mc_server, ctx.sender, message
        );
        ctx.chat(&shout_text);
        let Some(websocket) = ctx.state.api.websocket.as_ref() else {
            whisper(
                &ctx,
                " Shout relay is unavailable right now (websocket disconnected).",
            );
            return Ok(());
        };
        websocket
            .send_message(
                "inbound_minecraft_chat",
                json!({
                    "name": ctx.sender,
                    "message": shout_text,
                    "date": now.to_string(),
                    "mc_server": "all",
                    "uuid": "shout-relay",
                    "relay_type": "shout",
                    "origin_server": ctx.state.mc_server,
                    "relay_id": format!("{}-rust", now),
                }),
            )
            .await?;
        LAST_SHOUT_AT.store(now, Ordering::Relaxed);
        if raw.trim() != message {
            whisper(
                &ctx,
                " Your shout was sanitized (bad words censored and '/' removed).",
            );
        }
        whisper(&ctx, " Shout sent to connected servers.");
        Ok(())
    })
}

fn sleep(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        ctx.chat(" I couldn't find a bed :(");
        Ok(())
    })
}

fn servers(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let search = ctx.args.first().copied().unwrap_or(ctx.sender);
        let Some(uuid) = ctx.state.api.convert_username_to_uuid(search).await else {
            whisper(&ctx, &format!(" Could not find {search}."));
            return Ok(());
        };

        let mut candidates = std::iter::once(ctx.state.mc_server.clone())
            .chain(QUOTE_SERVERS.iter().map(|server| (*server).to_owned()))
            .collect::<Vec<_>>();
        candidates.sort_unstable();
        candidates.dedup();

        let api = ctx.state.api.clone();
        let mut servers = stream::iter(candidates)
            .map(|server| {
                let api = api.clone();
                let uuid = uuid.clone();
                async move {
                    let stats = api.get_stats_by_uuid(&uuid, &server).await?;
                    has_player_data(&stats).then_some(server)
                }
            })
            .buffer_unordered(BACKFILL_CONCURRENCY)
            .filter_map(|server| async move { server })
            .collect::<Vec<_>>()
            .await;

        servers.sort_unstable();
        if servers.is_empty() {
            whisper(&ctx, &format!(" I have no server data for {search}."));
            return Ok(());
        }

        ctx.chat(format!(
            " I have data for {search} on {} server{}: {}",
            servers.len(),
            if servers.len() == 1 { "" } else { "s" },
            servers.join(", ")
        ));
        Ok(())
    })
}

fn has_player_data(stats: &crate::structure::endpoints::endpoints::AllPlayerStats) -> bool {
    stats
        .username
        .as_deref()
        .is_some_and(|name| !name.is_empty())
        || stats.uuid.as_deref().is_some_and(|uuid| !uuid.is_empty())
        || stats
            .join_date
            .as_deref()
            .is_some_and(|date| !date.is_empty())
        || stats
            .last_seen
            .as_deref()
            .is_some_and(|date| !date.is_empty())
        || stats.playtime.unwrap_or_default() > 0
        || stats.joins.unwrap_or_default() > 0
        || stats.leaves.unwrap_or_default() > 0
        || stats.kills.unwrap_or_default() > 0
        || stats.deaths.unwrap_or_default() > 0
}

fn survived(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let search = ctx.args.first().copied().unwrap_or(ctx.sender);
        let Some(uuid) = ctx.state.api.convert_username_to_uuid(search).await else {
            whisper_no_record(&ctx, search, "deaths");
            return Ok(());
        };
        let death = ctx
            .state
            .api
            .get_deaths(&uuid, &ctx.state.mc_server, 1, "DESC", "all")
            .await
            .and_then(|mut rows| rows.pop());
        let Some(death) = death else {
            whisper_no_record(&ctx, search, "deaths");
            return Ok(());
        };
        let Some(death_ms) = epoch_ms_from_string(&death.time.to_string()) else {
            whisper(
                &ctx,
                &format!(" Unable to determine last death time for {search}."),
            );
            return Ok(());
        };
        let survived = time::dhms(now_millis().saturating_sub(death_ms))
            .trim_end_matches('.')
            .to_owned();
        ctx.chat(format!(
            " {search} has survived for {survived} since their last death."
        ));
        Ok(())
    })
}

fn twerk(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let bot = ctx.bot.clone();
        tokio::spawn(async move {
            let end = now_millis().saturating_add(10_000);
            let mut state = false;
            while now_millis() < end {
                state = !state;
                bot.set_crouching(state);
                tokio::time::sleep(time::Duration::from_millis(100)).await;
            }
            bot.set_crouching(false);
        });
        Ok(())
    })
}

fn victims(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(target) = parse_stats_target_or_reply(&ctx, "victims") else {
            return Ok(());
        };
        let Some(uuid) = ctx.state.api.convert_username_to_uuid(&target.search).await else {
            whisper_no_record(&ctx, &target.search, "kills");
            return Ok(());
        };
        let Some(kills) = ctx
            .state
            .api
            .get_kills(&uuid, &target.server, 10000, "DESC")
            .await
        else {
            whisper_no_record(&ctx, &target.search, "kills");
            return Ok(());
        };
        let victims = kills
            .iter()
            .filter_map(extract_victim_name)
            .filter(|victim| !victim.eq_ignore_ascii_case(&target.search))
            .map(|victim| victim.to_lowercase())
            .collect::<HashSet<_>>();
        if victims.is_empty() {
            if target.search.eq_ignore_ascii_case(ctx.sender) {
                whisper(
                    &ctx,
                    " I couldn't determine your unique victims, or unexpected error occurred.",
                );
            } else {
                whisper(
                    &ctx,
                    &format!(
                        " I couldn't determine {}'s unique victims, or unexpected error occurred.",
                        target.search
                    ),
                );
            }
            return Ok(());
        }
        let label = format_server_label(&target.server, &ctx.state.mc_server);
        ctx.chat(format!(
            " {}{} has killed {} unique player{}.",
            target.search,
            label,
            victims.len(),
            if victims.len() == 1 { "" } else { "s" }
        ));
        Ok(())
    })
}

fn vs(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let [name1, name2] = match ctx.args.as_slice() {
            [name1, name2] => [*name1, *name2],
            _ => {
                whisper(&ctx, " Usage: !vs <player1> <player2>");
                return Ok(());
            }
        };
        let (uuid1, uuid2) = tokio::join!(
            ctx.state.api.convert_username_to_uuid(name1),
            ctx.state.api.convert_username_to_uuid(name2)
        );
        let (Some(uuid1), Some(uuid2)) = (uuid1, uuid2) else {
            whisper(&ctx, " Could not resolve one or both usernames.");
            return Ok(());
        };
        let (kd1, kd2, pt1, pt2, mc1, mc2) = tokio::join!(
            ctx.state.api.get_kd(&uuid1, &ctx.state.mc_server),
            ctx.state.api.get_kd(&uuid2, &ctx.state.mc_server),
            ctx.state.api.get_playtime(&uuid1, &ctx.state.mc_server),
            ctx.state.api.get_playtime(&uuid2, &ctx.state.mc_server),
            ctx.state.api.get_message_count(name1, &ctx.state.mc_server),
            ctx.state.api.get_message_count(name2, &ctx.state.mc_server)
        );
        let (kills1, deaths1) = kd1.map(|kd| (kd.kills, kd.deaths)).unwrap_or_default();
        let (kills2, deaths2) = kd2.map(|kd| (kd.kills, kd.deaths)).unwrap_or_default();
        let kdr1 = if deaths1 > 0 {
            kills1 as f64 / deaths1 as f64
        } else {
            kills1 as f64
        };
        let kdr2 = if deaths2 > 0 {
            kills2 as f64 / deaths2 as f64
        } else {
            kills2 as f64
        };
        let pt_days1 = pt1.map(|pt| pt.playtime / 86_400_000).unwrap_or_default();
        let pt_days2 = pt2.map(|pt| pt.playtime / 86_400_000).unwrap_or_default();
        let msgs1 = mc1.map(|mc| mc.message_count).unwrap_or_default();
        let msgs2 = mc2.map(|mc| mc.message_count).unwrap_or_default();
        ctx.chat(format!(
            " [VS] {name1} vs {name2} | K: {kills1} {} {kills2} | D: {deaths1} {} {deaths2} | KD: {kdr1:.2} {} {kdr2:.2} | PT: {pt_days1}d {} {pt_days2}d | Msgs: {msgs1} {} {msgs2}",
            compare_u64(kills1, kills2),
            compare_u64(deaths2, deaths1),
            compare_f64(kdr1, kdr2),
            compare_u64(pt_days1, pt_days2),
            compare_u64(msgs1, msgs2),
        ));
        Ok(())
    })
}

async fn parse_target_with_uuid(
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

fn whisper(ctx: &CommandContext<'_>, message: &str) {
    ctx.whisper(message);
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

#[derive(Debug, Clone)]
struct CachedPlayer {
    username: String,
    latency: i32,
}

fn players_snapshot(ctx: &CommandContext<'_>) -> Vec<CachedPlayer> {
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

fn sender_whitelisted(ctx: &CommandContext<'_>) -> bool {
    if ctx
        .runtime
        .user_whitelist
        .iter()
        .any(|entry| entry.eq_ignore_ascii_case(ctx.sender))
    {
        return true;
    }
    player_uuid(ctx, ctx.sender).is_some_and(|uuid| ctx.runtime.user_whitelist.contains(&uuid))
}

fn list_command<'a>(
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

fn word_list_command<'a>(
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

fn extract_victim_name(
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

fn compare_u64(left: u64, right: u64) -> &'static str {
    match left.cmp(&right) {
        std::cmp::Ordering::Greater => ">",
        std::cmp::Ordering::Less => "<",
        std::cmp::Ordering::Equal => "=",
    }
}

fn compare_f64(left: f64, right: f64) -> &'static str {
    if left > right {
        ">"
    } else if left < right {
        "<"
    } else {
        "="
    }
}

fn percent_encode_path_segment(value: &str) -> String {
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

#[derive(Debug, Deserialize)]
struct AshconProfile {
    #[serde(default)]
    username_history: Vec<AshconUsernameHistory>,
}

#[derive(Debug, Deserialize)]
struct AshconUsernameHistory {
    username: Option<String>,
}
