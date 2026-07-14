use super::helpers::death_or_kill;
use crate::commands::{CommandContext, CommandFuture};

command!(LAST_DEATH_COMMAND, &["lastdeath", "ld"], "Retrieves the last death of a user. Usage: {prefix}lastdeath <username>", last_death);

fn last_death(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    death_or_kill(ctx, true, false)
}
