use super::helpers::whisper;
use crate::commands::{CommandContext, CommandFuture};
use crate::commands::utils::stats_target::{format_server_label, parse_stats_target_or_reply};
use crate::functions::utils::time;

command!(
    LAST_ADVANCEMENT_COMMAND,
    &["lastadvancement", "ladv"],
    "Retrieves the most recent advancement of a user. Usage: {prefix}lastadvancement <username> or {prefix}lastadvancement <server|all> <username>",
    last_advancement
);

fn last_advancement(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(target) = parse_stats_target_or_reply(&ctx, "lastadvancement") else {
            return Ok(());
        };
        let Some(uuid) = ctx.state.api.convert_username_to_uuid(&target.search).await else {
            whisper(&ctx, &format!(" {} has no advancements.", target.search));
            return Ok(());
        };
        let row = ctx
            .state
            .api
            .get_advancements(&uuid, &target.server, 1, "DESC")
            .await
            .and_then(|mut rows| rows.pop());
        if let Some(row) = row {
            let label = format_server_label(&target.server, &ctx.state.mc_server);
            ctx.chat_success(format!(
                " {}{}: {} ({})",
                target.search,
                label,
                row.advancement,
                time::time_ago_str(row.time as u64)
            ));
        }
        Ok(())
    })
}
