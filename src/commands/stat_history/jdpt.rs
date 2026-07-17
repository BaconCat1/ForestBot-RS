use super::helpers::{format_date_value, parse_target_with_uuid, whisper_no_record};
use crate::commands::{CommandContext, CommandFuture};
use crate::commands::utils::stats_target::{format_server_label, format_server_scope_hint};
use crate::functions::utils::time;

command!(
    JDPT_COMMAND,
    &["jdpt", "ptjd", "joindateplaytime", "playtimejoindate"],
    "Retrieves join date and total playtime. Usage: {prefix}jdpt <username> or {prefix}jdpt <server|all> <username>",
    jdpt
);

fn jdpt(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some((target, uuid)) = parse_target_with_uuid(&ctx, "jdpt").await? else {
            return Ok(());
        };
        let (jd, pt) = tokio::join!(
            ctx.state.api.get_join_date(&uuid, &target.server),
            ctx.state.api.get_playtime(&uuid, &target.server)
        );
        if jd.is_none() && pt.is_none() {
            let hint = format_server_scope_hint(
                target.has_server_arg,
                &target.server,
                &ctx.state.mc_server,
            );
            whisper_no_record(
                &ctx,
                &target.search,
                &format!("join date or playtime recorded{hint}"),
            );
            return Ok(());
        }
        let mut parts = Vec::new();
        if let Some(jd) = jd {
            parts.push(format!("joined on: {}", format_date_value(&jd.join_date)));
        }
        if let Some(pt) = pt {
            parts.push(format!("total playtime: {}", time::dhms(pt.playtime)));
        }
        let label = format_server_label(&target.server, &ctx.state.mc_server);
        ctx.chat_success(format!(
            " {}{}, {}",
            target.search,
            label,
            parts.join(" | ")
        ));
        Ok(())
    })
}
