use std::{collections::HashSet, future::Future, pin::Pin};

use azalea::prelude::Client;

pub mod afk;
pub mod ai;
pub mod alias;
pub mod calc;
pub mod daynight;
pub mod greeting;
pub mod askgod;
pub mod crouch;
pub mod discord;
pub mod drop;
pub mod equip;
pub mod fadvs;
pub mod hardware;
pub mod health;
pub mod help;
pub mod link;
pub mod marry;
pub mod report;
pub mod trade;
pub mod joins;
pub mod lastseen;
pub mod msgcount;
pub mod ping;
pub mod playtime;
pub mod quote;
pub mod reload;
pub mod slurcount;
pub mod stat_history;
pub mod utils;
pub mod news;
pub mod translate;
pub mod urbandictionary;
pub mod server_summary;
pub mod trivia;
pub mod weather;
pub mod wiki;
pub mod pearl;
pub mod casino;
pub mod battleship;
pub mod checkers;
pub mod reversi;
pub mod duel;
pub mod market;
pub mod wordle;
pub mod roast;
pub mod url;
pub mod tps;
pub mod poll;

use crate::structure::mineflayer::bot::{AzaleaState, RuntimeConfig};

pub type CommandFuture<'a> = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>>;
pub type CommandExecutor = for<'a> fn(CommandContext<'a>) -> CommandFuture<'a>;

#[derive(Clone, Copy)]
pub struct CommandDefinition {
    pub names: &'static [&'static str],
    pub description: &'static str,
    pub whitelisted: bool,
    pub execute: CommandExecutor,
}

pub struct CommandContext<'a> {
    pub bot: &'a Client,
    pub state: &'a AzaleaState,
    pub runtime: &'a RuntimeConfig,
    pub sender: &'a str,
    pub args: Vec<&'a str>,
    pub reply_as_whisper: bool,
    // Canonical (first-alias) command name, e.g. "lastseen" even if invoked as "!ls".
    // Used to look up json/commands_censorship.json for whisper_success/whisper_error.
    pub command_name: &'a str,
}

// Leading sentinel on a queued message meaning "skip the profanity censor for this
// one" -- stripped in flush_outbound_chat before the message is sent. Reserved for
// bot-generated deterministic numeric output (portfolio/bet reports) where rustrict's
// leetspeak/number normalization produces false-positive severe matches on ticker
// symbols + dollar amounts. Never use this for anything that echoes player-submitted
// text (e.g. !quote) -- that still needs real censoring.
pub const SKIP_CENSOR_MARKER: char = '\u{1}';

impl CommandContext<'_> {
    pub fn chat(&self, message: impl AsRef<str>) {
        enqueue_chat(
            self.state,
            route_chat_message(
                message.as_ref(),
                self.reply_as_whisper,
                &self.runtime.whisper_command,
                self.sender,
            ),
        );
    }

    pub fn whisper(&self, message: impl AsRef<str>) {
        self.chat(format!(
            "/{} {} {}",
            self.runtime.whisper_command,
            self.sender,
            message.as_ref()
        ));
    }

    /// Same as `whisper`, but skips profanity censoring unconditionally. Prefer
    /// `whisper_success`/`whisper_error` below, which decide this per-command via
    /// json/commands_censorship.json instead of hardcoding the bypass at the call site.
    /// Builds the whisper text directly rather than going through `chat`/
    /// `route_chat_message`, since the marker prefix would break that path's `/`-prefix
    /// check.
    pub fn whisper_uncensored(&self, message: impl AsRef<str>) {
        enqueue_chat(
            self.state,
            format!(
                "{SKIP_CENSOR_MARKER}/{} {} {}",
                self.runtime.whisper_command,
                self.sender,
                message.as_ref()
            ),
        );
    }

    /// Same as `chat`, but skips profanity censoring unconditionally. See
    /// `whisper_uncensored`'s note -- prefer `chat_success`/`chat_error`.
    pub fn chat_uncensored(&self, message: impl AsRef<str>) {
        let routed = route_chat_message(
            message.as_ref(),
            self.reply_as_whisper,
            &self.runtime.whisper_command,
            self.sender,
        );
        enqueue_chat(self.state, format!("{SKIP_CENSOR_MARKER}{routed}"));
    }

    // Looks up json/commands_censorship.json for this command's canonical name.
    // Missing command or missing key defaults to censored (true) -- anything not
    // explicitly reviewed fails safe.
    fn censored(&self, success_path: bool) -> bool {
        self.runtime
            .command_censorship
            .get(self.command_name)
            .map(|c| if success_path { c.success } else { c.error })
            .unwrap_or(true)
    }

    /// Looks up json/bet_limits.json for `game`'s min/max bet. Falls back to the
    /// caller-supplied defaults if `game` is missing from the file -- shouldn't
    /// normally happen since bet_limits.json is seeded with every game, but keeps
    /// behavior sane (the game's original hardcoded value) rather than failing open
    /// to an unlimited bet.
    pub fn bet_limit(
        &self,
        game: &str,
        default_min: i64,
        default_max: Option<i64>,
    ) -> crate::config::BetLimit {
        self.runtime
            .bet_limits
            .get(game)
            .copied()
            .unwrap_or(crate::config::BetLimit { min: default_min, max: default_max })
    }

    /// Resolves the sender's real UUID, whispering an error if resolution fails.
    /// `casino_adjust`/`casino_win` are keyed by UUID (`casino_balance.player_uuid`) --
    /// passing a raw username instead silently reads/creates a bogus shadow row keyed
    /// by that literal string, disconnected from the player's real balance. Always
    /// resolve through this (or `resolve_player_uuid` for detached/non-ctx contexts
    /// like spawned timer tasks) before any money-touching API call.
    pub async fn require_player_uuid(&self) -> Option<String> {
        match resolve_player_uuid(self.state, self.sender).await {
            Some(uuid) => Some(uuid),
            None => {
                self.whisper_success("Could not resolve your UUID.");
                None
            }
        }
    }

    /// Sends a multi-line game board (battleship/checkers/chess/connect four/mines/
    /// reversi/wordle) one whisper per line, paced by `board_whisper_delay_ms`.
    /// Unpaced bursts trip RV's anti-spam filter and get the bot kicked -- delay
    /// is skipped after the last line since nothing follows it.
    pub async fn whisper_board(&self, lines: impl IntoIterator<Item = impl AsRef<str>>) {
        let mut lines = lines.into_iter().peekable();
        while let Some(line) = lines.next() {
            self.whisper_success(line);
            if lines.peek().is_some() {
                tokio::time::sleep(std::time::Duration::from_millis(
                    self.runtime.board_whisper_delay_ms,
                ))
                .await;
            }
        }
    }

    /// For a command's normal/computed output (stats, prices, game results). Censored
    /// unless json/commands_censorship.json explicitly marks this command's `success`
    /// path as bypassed.
    pub fn whisper_success(&self, message: impl AsRef<str>) {
        if self.censored(true) {
            self.whisper(message);
        } else {
            self.whisper_uncensored(message);
        }
    }

    /// For messages that echo back raw/unresolved player input (e.g. "no user found
    /// matching X"). Censored unless json/commands_censorship.json explicitly marks
    /// this command's `error` path as bypassed -- almost never should be, since this is
    /// exactly the content class the censor exists for.
    pub fn whisper_error(&self, message: impl AsRef<str>) {
        if self.censored(false) {
            self.whisper(message);
        } else {
            self.whisper_uncensored(message);
        }
    }

    /// `chat` counterpart to `whisper_success`.
    pub fn chat_success(&self, message: impl AsRef<str>) {
        if self.censored(true) {
            self.chat(message);
        } else {
            self.chat_uncensored(message);
        }
    }

    /// `chat` counterpart to `whisper_error`.
    pub fn chat_error(&self, message: impl AsRef<str>) {
        if self.censored(false) {
            self.chat(message);
        } else {
            self.chat_uncensored(message);
        }
    }
}

pub fn enqueue_chat(state: &AzaleaState, message: impl AsRef<str>) {
    enqueue_chat_raw(&state.runtime, &state.recent_whispers, &state.outbound_chat, message)
}

/// Resolves a username to their real UUID for use with `casino_adjust`/`casino_win`,
/// which are keyed by UUID. Free-function form of `CommandContext::require_player_uuid`
/// for detached contexts (spawned timer tasks, event hooks) that only have
/// `&AzaleaState`, not a `CommandContext` -- e.g. `duel.rs`'s deferred payout tasks,
/// `trivia.rs`'s answer-timer payout loop. Doesn't whisper on failure since there's no
/// command invocation to reply to; callers decide how to handle a `None`.
pub async fn resolve_player_uuid(state: &AzaleaState, username: &str) -> Option<String> {
    state.api.convert_username_to_uuid(username).await
}

/// Real logic behind `enqueue_chat`, taking the 3 fields it actually needs
/// directly instead of the whole `AzaleaState` -- lets settle tasks queue chat
/// through their narrow `SettleDeps` (see `casino::SettleDeps`) without giving
/// them the full state struct just for this.
pub(crate) fn enqueue_chat_raw(
    runtime: &std::sync::Arc<std::sync::RwLock<RuntimeConfig>>,
    recent_whispers: &std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, (String, std::time::Instant)>>>,
    outbound_chat: &std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<String>>>,
    message: impl AsRef<str>,
) {
    let message = message.as_ref().trim_start().to_owned();

    // Record outgoing whispers so the chat handler can recognize the server echoing
    // one back and suppress it, instead of misreading it as the target speaking.
    // Strip the censor-skip marker first, if present, so this detection still works
    // for whisper_uncensored's output.
    let whisper_command = runtime
        .read()
        .expect("runtime config lock poisoned")
        .whisper_command
        .clone();
    let for_whisper_check = message.strip_prefix(SKIP_CENSOR_MARKER).unwrap_or(&message);
    if let Some(rest) = for_whisper_check.strip_prefix(&format!("/{whisper_command} ")) {
        if let Some((target, content)) = rest.split_once(' ') {
            recent_whispers
                .lock()
                .expect("recent_whispers lock poisoned")
                .insert(target.to_lowercase(), (content.to_owned(), std::time::Instant::now()));
        }
    }

    outbound_chat
        .lock()
        .expect("outbound chat queue lock poisoned")
        .push_back(message);
}

fn is_whisper_command(message: &str, whisper_command: &str) -> bool {
    let Some(command) = message.strip_prefix('/') else {
        return false;
    };
    command
        .split_whitespace()
        .next()
        .is_some_and(|name| name.eq_ignore_ascii_case(whisper_command))
}

fn route_chat_message(
    message: &str,
    reply_as_whisper: bool,
    whisper_command: &str,
    sender: &str,
) -> String {
    let message = message.trim_start();
    if reply_as_whisper && !is_whisper_command(message, whisper_command) {
        return format!("/{whisper_command} {sender} {message}");
    }

    message.to_owned()
}

#[cfg(test)]
mod tests {
    use super::route_chat_message;

    #[test]
    fn routes_whisper_invoked_chat_replies_back_to_sender() {
        assert_eq!(
            route_chat_message(" pong", true, "msg", "Alice"),
            "/msg Alice pong"
        );
    }

    #[test]
    fn does_not_double_wrap_explicit_whispers() {
        assert_eq!(
            route_chat_message("/msg Alice already private", true, "msg", "Alice"),
            "/msg Alice already private"
        );
    }

    #[test]
    fn leaves_normal_chat_replies_public() {
        assert_eq!(route_chat_message(" pong", false, "msg", "Alice"), "pong");
    }
}

pub fn registry() -> &'static [CommandDefinition] {
    &[
        ping::COMMAND,
        help::COMMAND,
        alias::COMMAND,
        crouch::COMMAND,
        hardware::COMMAND,
        health::COMMAND,
        slurcount::COMMAND,
        discord::COMMAND,
        reload::COMMAND,
        lastseen::COMMAND,
        msgcount::COMMAND,
        playtime::COMMAND,
        joins::COMMAND,
        quote::COMMAND,
        stat_history::KD_COMMAND,
        stat_history::JOINDATE_COMMAND,
        stat_history::JDPT_COMMAND,
        stat_history::WORDCOUNT_COMMAND,
        stat_history::NAMEFIND_COMMAND,
        stat_history::UNIQUE_USERS_COMMAND,
        stat_history::TOTAL_ADVANCEMENTS_COMMAND,
        stat_history::SUMMARY_COMMAND,
        stat_history::WINRATE_COMMAND,
        stat_history::FIRST_DEATH_COMMAND,
        stat_history::LAST_DEATH_COMMAND,
        stat_history::FIRST_KILL_COMMAND,
        stat_history::LAST_KILL_COMMAND,
        stat_history::LAST_ADVANCEMENT_COMMAND,
        stat_history::FIRST_MESSAGE_COMMAND,
        stat_history::LAST_MESSAGE_COMMAND,
        stat_history::OLDHEADS_COMMAND,
        stat_history::NOOBS_COMMAND,
        stat_history::TOP_COMMAND,
        stat_history::STANDING_COMMAND,
        stat_history::OFFLINE_MSG_COMMAND,
        stat_history::REMIND_COMMAND,
        stat_history::WHOIS_COMMAND,
        stat_history::RANDOM_QUOTE_COMMAND,
        stat_history::LIST_QUOTE_SERVERS_COMMAND,
        stat_history::ACTIVE_COMMAND,
        stat_history::ADD_FAQ_COMMAND,
        stat_history::DELETE_FAQ_COMMAND,
        stat_history::ADVANCEMENT_COUNT_COMMAND,
        stat_history::BLACKLIST_COMMAND,
        stat_history::AVERAGE_PING_COMMAND,
        stat_history::BEST_PING_COMMAND,
        stat_history::CENSOR_COMMAND,
        stat_history::COORDS_COMMAND,
        drop::COMMAND,
        equip::COMMAND,
        fadvs::COMMAND,
        equip::UNEQUIP_COMMAND,
        stat_history::EDIT_FAQ_COMMAND,
        stat_history::EFFICIENCY_COMMAND,
        stat_history::EXECUTE_COMMAND,
        stat_history::FEBZEY_COMMAND,
        stat_history::FAQ_COMMAND,
        stat_history::GRUDGE_COMMAND,
        stat_history::IAM_COMMAND,
        stat_history::MOUNT_COMMAND,
        stat_history::NICKNAME_COMMAND,
        stat_history::OLDNAMES_COMMAND,
        stat_history::OWNS_FAQ_COMMAND,
        stat_history::PROFILE_COMMAND,
        stat_history::RANDOM_QUOTE_ALL_COMMAND,
        stat_history::REALNAME_COMMAND,
        stat_history::SET_PRESET_COMMAND,
        stat_history::SHOUT_COMMAND,
        stat_history::SLEEP_COMMAND,
        stat_history::SERVERS_COMMAND,
        stat_history::SURVIVED_COMMAND,
        stat_history::TWERK_COMMAND,
        stat_history::VICTIMS_COMMAND,
        stat_history::VS_COMMAND,
        stat_history::WHITELIST_COMMAND,
        stat_history::WORD_WHITELIST_COMMAND,
        stat_history::WORST_PING_COMMAND,
        askgod::COMMAND,
        askgod::LISTGODS_COMMAND,
        askgod::SEARCHGOD_COMMAND,
        askgod::GODVERSE_COMMAND,
        askgod::GODSTATS_COMMAND,
        askgod::GODFIGHT_COMMAND,
        link::LINK_COMMAND,
        link::UNLINK_COMMAND,
        report::REPORT_COMMAND,
        trade::TRADE_COMMAND,
        trade::TRADES_COMMAND,
        trade::TRADESTATS_COMMAND,
        trade::SCAMMERS_COMMAND,
        marry::MARRY_COMMAND,
        marry::DIVORCE_COMMAND,
        marry::SPOUSE_COMMAND,
        server_summary::SERVER_SUMMARY_COMMAND,
        server_summary::COMPARE_COMMAND,
        wiki::WIKI_COMMAND,
        wiki::MINEWIKI_COMMAND,
        calc::COMMAND,
        greeting::COMMAND,
        daynight::DAY_COMMAND,
        daynight::NIGHT_COMMAND,
        news::COMMAND,
        translate::COMMAND,
        weather::COMMAND,
        urbandictionary::COMMAND,
        trivia::TRIVIA_COMMAND,
        trivia::ANSWER_COMMAND,
        pearl::COMMAND,
        casino::ADDCHIPS_COMMAND,
        casino::FAUCET_COMMAND,
        casino::GIVE_COMMAND,
        casino::WALLET_COMMAND,
        casino::JACKPOT_COMMAND,
        casino::LOTTO_COMMAND,
        casino::DRAW_COMMAND,
        casino::roulette::COMMAND,
        casino::scratch::COMMAND,
        casino::craps::COMMAND,
        casino::sic_bo::COMMAND,
        casino::baccarat::COMMAND,
        casino::faa_airport::COMMAND,
        casino::kalshi::COMMAND,
        casino::nasa_space_weather::COMMAND,
        casino::noaa_flooding::COMMAND,
        casino::train::COMMAND,
        casino::seismic::QUAKE_COMMAND,
        casino::seismic::VOLCANO_COMMAND,
        casino::blackjack::COMMAND,
        casino::poker::COMMAND,
        casino::chess::COMMAND,
        casino::connect_four::COMMAND,
        casino::hilo::COMMAND,
        casino::slots::COMMAND,
        casino::sports::COMMAND,
        market::COMMAND,
        market::PORTFOLIO_COMMAND,
        duel::COMMAND,
        wordle::COMMAND,
        roast::COMMAND,
        battleship::COMMAND,
        casino::mines::COMMAND,
        casino::aqi::COMMAND,
        casino::launch::COMMAND,
        casino::gas::COMMAND,
        casino::bets::COMMAND,
        checkers::COMMAND,
        reversi::COMMAND,
        url::COMMAND,
        tps::COMMAND,
        afk::COMMAND,
        poll::COMMAND,
        ai::COMMAND,
    ]
}

pub fn find(command_name: &str) -> Option<&'static CommandDefinition> {
    registry().iter().find(|command| {
        command
            .names
            .iter()
            .any(|name| name.eq_ignore_ascii_case(command_name))
    })
}

/// Loads the flat list of command names that are unsafe to run via the Discord
/// chat bridge (sender identity can't be trusted as a real MC player there).
/// Everything not listed defaults to bridge-safe. See json/bridge_unsafe_commands.json.
pub async fn load_bridge_unsafe_commands(path: &str) -> HashSet<String> {
    let json = match tokio::fs::read_to_string(path).await {
        Ok(s) => s,
        Err(e) => {
            crate::structure::logger::warn(format!(
                "Could not load bridge command overrides from {path}: {e}"
            ));
            return HashSet::new();
        }
    };
    match serde_json::from_str::<Vec<String>>(&json) {
        Ok(names) => names.into_iter().map(|n| n.to_lowercase()).collect(),
        Err(e) => {
            crate::structure::logger::warn(format!("Bad bridge command overrides JSON: {e}"));
            HashSet::new()
        }
    }
}

/// Builds the full {name, bridge_ok}[] list (every alias of every command) to push to Hub.
pub fn build_bridge_command_list(unsafe_names: &HashSet<String>) -> Vec<(String, bool)> {
    let mut list = Vec::new();
    for command in registry() {
        let is_unsafe = command
            .names
            .iter()
            .any(|name| unsafe_names.contains(&name.to_lowercase()));
        for name in command.names {
            list.push((name.to_string(), !is_unsafe));
        }
    }
    list
}
