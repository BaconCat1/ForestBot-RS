use super::helpers::list_command;
use crate::commands::{CommandContext, CommandFuture};

const MC_BLACKLIST_PATH: &str = "./json/mc_blacklist.json";

admin_command!(BLACKLIST_COMMAND, &["blacklist"], "Adds or removes users from the command blacklist. Usage: {prefix}blacklist add|remove <username>", blacklist);

fn blacklist(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    list_command(ctx, MC_BLACKLIST_PATH, true)
}
