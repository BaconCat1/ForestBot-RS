use super::patterns::death_or_kill;
use crate::commands::{CommandContext, CommandFuture};

command!(FIRST_DEATH_COMMAND, &["firstdeath", "fd"], "Retrieves the first death a user got. Usage: {prefix}firstdeath <username>", first_death);

fn first_death(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    death_or_kill(ctx, true, true)
}
