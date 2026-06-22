use std::{future::Future, pin::Pin};

use azalea::prelude::Client;

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
pub mod urbandictionary;
pub mod wiki;

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
}

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
}

pub fn enqueue_chat(state: &AzaleaState, message: impl AsRef<str>) {
    state
        .outbound_chat
        .lock()
        .expect("outbound chat queue lock poisoned")
        .push_back(message.as_ref().trim_start().to_owned());
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
        link::LINK_COMMAND,
        link::UNLINK_COMMAND,
        report::REPORT_COMMAND,
        trade::TRADE_COMMAND,
        trade::TRADES_COMMAND,
        trade::TRADESTATS_COMMAND,
        trade::SCAMMERS_COMMAND,
        wiki::WIKI_COMMAND,
        wiki::MINEWIKI_COMMAND,
        calc::COMMAND,
        greeting::COMMAND,
        daynight::DAY_COMMAND,
        daynight::NIGHT_COMMAND,
        news::COMMAND,
        urbandictionary::COMMAND,
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
