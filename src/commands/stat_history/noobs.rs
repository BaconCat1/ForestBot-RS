use super::helpers::sorted_unique_users;
use crate::commands::{CommandContext, CommandFuture};

command!(
    NOOBS_COMMAND,
    &["noobs", "noob", "newest", "newusers", "newbs", "newb"],
    "Retrieves the 3 newest users. Usage: {prefix}noobs",
    noobs
);

fn noobs(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    sorted_unique_users(ctx, false)
}
