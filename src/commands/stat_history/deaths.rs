use super::kd::render_kd;
use crate::commands::{CommandContext, CommandFuture};

// Split off `!kd`'s "deaths" alias into its own command (todo.md, server event
// futures scoping) so the death-window betting market could hook in here
// without touching `!kd`/`!kills`'s behavior at all. Rendering is identical to
// `!kd`, shared via `kd::render_kd`. Betting form + odds hint wired 2026-07-22
// -- see REFERENCE_MATERIAL/DOCS/casino_event_futures_scoping.md.
command!(DEATHS_COMMAND, &["deaths"], "Displays the kill/death ratio of a user. Usage: {prefix}deaths <username> or {prefix}deaths <server|all> <username> or {prefix}deaths <player> <chips> to bet they die soon", deaths);

fn deaths(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {

        let Some((target, uuid)) = render_kd(&ctx).await? else {
            return Ok(());
        };

        Ok(())
    })
}
