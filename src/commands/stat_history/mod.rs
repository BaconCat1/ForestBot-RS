//! Split from a single 3161-line stat_history.rs into one file per command,
//! matching the rest of src/commands/'s per-command-per-file convention.
//! Every command constant and the two externally-referenced items
//! (BOT_SLEEPING, clear_delete_faq_pending) are re-exported here unchanged, so
//! no caller outside this module needed to change.

mod helpers;
mod patterns;

macro_rules! command {
    ($const_name:ident, $names:expr, $description:expr, $execute:ident) => {
        pub const $const_name: crate::commands::CommandDefinition = crate::commands::CommandDefinition {
            names: $names,
            description: $description,
            whitelisted: false,
            execute: $execute,
        };
    };
}

macro_rules! admin_command {
    ($const_name:ident, $names:expr, $description:expr, $execute:ident) => {
        pub const $const_name: crate::commands::CommandDefinition = crate::commands::CommandDefinition {
            names: $names,
            description: $description,
            whitelisted: true,
            execute: $execute,
        };
    };
}

mod kd;
mod joindate;
mod jdpt;
mod wordcount;
mod namefind;
mod unique_users;
mod total_advancements;
mod summary;
mod winrate;
mod firstdeath;
mod lastdeath;
mod firstkill;
mod lastkill;
mod last_advancement;
mod firstmessage;
mod lastmessage;
mod oldheads;
mod noobs;
mod top;
mod standing;
mod offline_msg;
mod remind_me;
mod whois;
mod random_quote;
mod list_quote_servers;
mod active;
mod add_faq;
mod delete_faq;
mod advancement_count;
mod blacklist;
mod average_ping;
mod best_ping;
mod censor;
mod coords;
mod edit_faq;
mod efficiency;
mod execute;
mod febzey;
mod faq;
mod grudge;
mod iam;
mod mount;
mod nickname;
mod oldnames;
mod owns_faq;
mod profile;
mod random_quote_all;
mod realname;
mod set_preset;
mod shout;
mod sleep;
mod servers;
mod survived;
mod twerk;
mod victims;
mod vs;
mod whitelist;
mod word_whitelist;
mod worst_ping;

pub use active::ACTIVE_COMMAND;
pub use add_faq::ADD_FAQ_COMMAND;
pub use advancement_count::ADVANCEMENT_COUNT_COMMAND;
pub use average_ping::AVERAGE_PING_COMMAND;
pub use best_ping::BEST_PING_COMMAND;
pub use blacklist::BLACKLIST_COMMAND;
pub use censor::CENSOR_COMMAND;
pub use coords::COORDS_COMMAND;
pub use delete_faq::{DELETE_FAQ_COMMAND, clear_delete_faq_pending};
pub use edit_faq::EDIT_FAQ_COMMAND;
pub use efficiency::EFFICIENCY_COMMAND;
pub use execute::EXECUTE_COMMAND;
pub use faq::FAQ_COMMAND;
pub use febzey::FEBZEY_COMMAND;
pub use firstdeath::FIRST_DEATH_COMMAND;
pub use firstkill::FIRST_KILL_COMMAND;
pub use firstmessage::FIRST_MESSAGE_COMMAND;
pub use grudge::GRUDGE_COMMAND;
pub use iam::IAM_COMMAND;
pub use jdpt::JDPT_COMMAND;
pub use joindate::JOINDATE_COMMAND;
pub use kd::KD_COMMAND;
pub use last_advancement::LAST_ADVANCEMENT_COMMAND;
pub use lastdeath::LAST_DEATH_COMMAND;
pub use lastkill::LAST_KILL_COMMAND;
pub use lastmessage::LAST_MESSAGE_COMMAND;
pub use list_quote_servers::LIST_QUOTE_SERVERS_COMMAND;
pub use mount::MOUNT_COMMAND;
pub use namefind::NAMEFIND_COMMAND;
pub use nickname::NICKNAME_COMMAND;
pub use noobs::NOOBS_COMMAND;
pub use offline_msg::OFFLINE_MSG_COMMAND;
pub use oldheads::OLDHEADS_COMMAND;
pub use oldnames::OLDNAMES_COMMAND;
pub use owns_faq::OWNS_FAQ_COMMAND;
pub use profile::PROFILE_COMMAND;
pub use random_quote::RANDOM_QUOTE_COMMAND;
pub use random_quote_all::RANDOM_QUOTE_ALL_COMMAND;
pub use realname::REALNAME_COMMAND;
pub use remind_me::REMIND_COMMAND;
pub use servers::SERVERS_COMMAND;
pub use set_preset::SET_PRESET_COMMAND;
pub use shout::SHOUT_COMMAND;
pub use helpers::BOT_SLEEPING;
pub use sleep::SLEEP_COMMAND;
pub use standing::STANDING_COMMAND;
pub use summary::SUMMARY_COMMAND;
pub use survived::SURVIVED_COMMAND;
pub use top::TOP_COMMAND;
pub use total_advancements::TOTAL_ADVANCEMENTS_COMMAND;
pub use twerk::TWERK_COMMAND;
pub use unique_users::UNIQUE_USERS_COMMAND;
pub use victims::VICTIMS_COMMAND;
pub use vs::VS_COMMAND;
pub use whitelist::WHITELIST_COMMAND;
pub use whois::WHOIS_COMMAND;
pub use winrate::WINRATE_COMMAND;
pub use word_whitelist::WORD_WHITELIST_COMMAND;
pub use wordcount::WORDCOUNT_COMMAND;
pub use worst_ping::WORST_PING_COMMAND;
