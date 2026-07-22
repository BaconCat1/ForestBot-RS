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
        // Betting form: !deaths <player> <bet> -- same shape/rationale as
        // !joins's betting form, checked first so it can't collide with the
        // shared stats-target parser's <server> <username> shape.
        if ctx.args.len() == 2 {
            if let Ok(stake) = ctx.args[1].parse::<i64>() {
                return crate::commands::casino::death_market::place_bet(&ctx, ctx.args[0], stake).await;
            }
        }

        let Some((target, uuid)) = render_kd(&ctx).await? else {
            return Ok(());
        };

        // Odds/bet hint only for a plain single-player lookup, same rationale
        // as join-window's hint on !joins.
        if !target.has_server_arg && !target.search.eq_ignore_ascii_case(ctx.sender) {
            crate::commands::casino::death_market::whisper_odds_hint(&ctx, &uuid, &target.search).await;
        }

        Ok(())
    })
}
