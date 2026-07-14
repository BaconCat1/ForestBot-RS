use super::helpers::word_list_command;
use crate::commands::{CommandContext, CommandFuture};

const WORD_WHITELIST_PATH: &str = "./json/word_whitelist.json";

admin_command!(
    WORD_WHITELIST_COMMAND,
    &["wordwhitelist", "wwl"],
    "Manage always-allowed words. Usage: {prefix}wordwhitelist add <word> | {prefix}wordwhitelist remove <word>",
    word_whitelist
);

fn word_whitelist(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    word_list_command(ctx, WORD_WHITELIST_PATH, "word whitelist")
}
