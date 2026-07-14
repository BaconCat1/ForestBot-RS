use super::helpers::death_or_kill;
use crate::commands::{CommandContext, CommandFuture};

command!(FIRST_KILL_COMMAND, &["firstkill", "fk"], "Retrieves the first kill a user got. Usage: {prefix}firstkill <username>", first_kill);

fn first_kill(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    death_or_kill(ctx, false, true)
}
