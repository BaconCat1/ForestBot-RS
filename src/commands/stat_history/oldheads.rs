use super::helpers::sorted_unique_users;
use crate::commands::{CommandContext, CommandFuture};

command!(
    OLDHEADS_COMMAND,
    &["oldest", "oldheads", "oldusers", "oldestusers", "oldfags"],
    "Retrieves the 3 oldest users. Usage: {prefix}oldest",
    oldheads
);

fn oldheads(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    sorted_unique_users(ctx, true)
}
