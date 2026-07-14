use super::helpers::{parse_target_with_uuid, whisper_no_record};
use crate::commands::{CommandContext, CommandFuture};
use crate::commands::utils::stats_target::{format_server_label, format_server_scope_hint};

command!(KD_COMMAND, &["kd", "kills", "deaths"], "Displays the kill/death ratio of a user. Usage: {prefix}kd <username> or {prefix}kd <server|all> <username>", kd);

fn kd(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some((target, uuid)) = parse_target_with_uuid(&ctx, "kd").await? else {
            return Ok(());
        };
        let data = ctx.state.api.get_kd(&uuid, &target.server).await;
        let server_hint =
            format_server_scope_hint(target.has_server_arg, &target.server, &ctx.state.mc_server);
        let Some(data) = data else {
            whisper_no_record(
                &ctx,
                &target.search,
                &format!("kills or deaths{server_hint}"),
            );
            return Ok(());
        };
        let ratio = data.kills as f64 / data.deaths as f64;
        let label = format_server_label(&target.server, &ctx.state.mc_server);
        ctx.chat(format!(
            " {}{}: Kills: {} Deaths: {} KD: {:.2}",
            target.search, label, data.kills, data.deaths, ratio
        ));
        Ok(())
    })
}
