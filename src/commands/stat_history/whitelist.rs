use super::helpers::list_command;
use crate::commands::{CommandContext, CommandFuture};

const MC_WHITELIST_PATH: &str = "./json/mc_whitelist.json";

admin_command!(WHITELIST_COMMAND, &["whitelist"], "Adds or removes users from the command whitelist. Usage: {prefix}whitelist add|remove <username>", whitelist);

fn whitelist(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    list_command(ctx, MC_WHITELIST_PATH, false)
}
