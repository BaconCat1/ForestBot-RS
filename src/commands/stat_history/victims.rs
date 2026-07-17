use super::helpers::{extract_victim_name, whisper, whisper_no_record};
use crate::commands::{CommandContext, CommandFuture};
use crate::commands::utils::stats_target::{format_server_label, parse_stats_target_or_reply};
use std::collections::HashSet;

command!(VICTIMS_COMMAND, &["victims", "murders", "bested"], "Shows how many unique players a user has killed. Usage: {prefix}victims <username> or {prefix}victims <server|all> <username>", victims);

fn victims(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(target) = parse_stats_target_or_reply(&ctx, "victims") else {
            return Ok(());
        };
        let Some(uuid) = ctx.state.api.convert_username_to_uuid(&target.search).await else {
            whisper_no_record(&ctx, &target.search, "kills");
            return Ok(());
        };
        let Some(kills) = ctx
            .state
            .api
            .get_kills(&uuid, &target.server, 10000, "DESC")
            .await
        else {
            whisper_no_record(&ctx, &target.search, "kills");
            return Ok(());
        };
        let victims = kills
            .iter()
            .filter_map(extract_victim_name)
            .filter(|victim| !victim.eq_ignore_ascii_case(&target.search))
            .map(|victim| victim.to_lowercase())
            .collect::<HashSet<_>>();
        if victims.is_empty() {
            if target.search.eq_ignore_ascii_case(ctx.sender) {
                whisper(
                    &ctx,
                    " I couldn't determine your unique victims, or unexpected error occurred.",
                );
            } else {
                whisper(
                    &ctx,
                    &format!(
                        " I couldn't determine {}'s unique victims, or unexpected error occurred.",
                        target.search
                    ),
                );
            }
            return Ok(());
        }
        let label = format_server_label(&target.server, &ctx.state.mc_server);
        ctx.chat_success(format!(
            " {}{} has killed {} unique player{}.",
            target.search,
            label,
            victims.len(),
            if victims.len() == 1 { "" } else { "s" }
        ));
        Ok(())
    })
}
