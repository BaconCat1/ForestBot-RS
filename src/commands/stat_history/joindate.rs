use super::command;
use super::helpers::{format_date_value, parse_target_with_uuid, whisper_no_record};
use crate::commands::{CommandContext, CommandFuture};
use crate::commands::utils::stats_target::{format_server_label, format_server_scope_hint};

command!(JOINDATE_COMMAND, &["joindate", "jd", "firstseen"], "Retrieves the join date of a user. Usage: {prefix}joindate <username> or {prefix}joindate <server|all> <username>", joindate);

fn joindate(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some((target, uuid)) = parse_target_with_uuid(&ctx, "joindate").await? else {
            return Ok(());
        };
        let data = ctx.state.api.get_join_date(&uuid, &target.server).await;
        let Some(data) = data else {
            let hint = format_server_scope_hint(
                target.has_server_arg,
                &target.server,
                &ctx.state.mc_server,
            );
            whisper_no_record(&ctx, &target.search, &format!("join date{hint}"));
            return Ok(());
        };
        let label = format_server_label(&target.server, &ctx.state.mc_server);
        ctx.chat(format!(
            " {}{}, joined on: {}",
            target.search,
            label,
            format_date_value(&data.join_date)
        ));
        Ok(())
    })
}
