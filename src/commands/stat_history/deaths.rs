use super::kd::render_kd;
use crate::commands::{CommandContext, CommandFuture};

// Split off `!kd`'s "deaths" alias into its own command (todo.md, server event
// futures scoping) so the death-window betting market can hook in here without
// touching `!kd`/`!kills`'s behavior at all. Rendering is identical to `!kd`
// today, shared via `kd::render_kd` -- this file is where `!deaths <player>
// <bet>` will place a bet once the death-window market (odds endpoint +
// settle-task shape) is built; not wired yet, see
// REFERENCE_MATERIAL/DOCS/casino_event_futures_scoping.md.
command!(DEATHS_COMMAND, &["deaths"], "Displays the kill/death ratio of a user. Usage: {prefix}deaths <username> or {prefix}deaths <server|all> <username>", deaths);

fn deaths(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move { render_kd(&ctx).await.map(|_| ()) })
}
