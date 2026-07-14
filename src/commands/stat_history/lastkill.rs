use super::helpers::death_or_kill;
use crate::commands::{CommandContext, CommandFuture};

command!(LAST_KILL_COMMAND, &["lastkill", "lk"], "Retrieves the last kill a user got. Usage: {prefix}lastkill <username>", last_kill);

fn last_kill(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    death_or_kill(ctx, false, false)
}
