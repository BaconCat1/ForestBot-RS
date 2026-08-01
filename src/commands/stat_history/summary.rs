use super::helpers::{epoch_ms_from_string, parse_target_with_uuid};
use crate::commands::utils::stats_target::format_server_label;
use crate::commands::{CommandContext, CommandFuture};

command!(SUMMARY_COMMAND, &["summary", "sum"], "Single-line stats overview for a player. Usage: {prefix}summary <username> or {prefix}summary <server|all> <username>", summary);

fn summary(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some((target, uuid)) = parse_target_with_uuid(&ctx, "summary").await? else {
            return Ok(());
        };
        let search = target.search.as_str();
        let (kd, pt, mc, adv, jd) = tokio::join!(
            ctx.state.api.get_kd(&uuid, &target.server),
            ctx.state.api.get_playtime(&uuid, &target.server),
            ctx.state.api.get_message_count(search, &target.server),
            ctx.state
                .api
                .get_total_advancements_count(&uuid, &target.server),
            ctx.state.api.get_join_date(&uuid, &target.server)
        );
        let kills = kd.as_ref().map(|kd| kd.kills).unwrap_or_default();
        let deaths = kd.as_ref().map(|kd| kd.deaths).unwrap_or_default();
        let kdr = if deaths > 0 {
            kills as f64 / deaths as f64
        } else {
            kills as f64
        };
        let pt_days = pt.map(|pt| pt.playtime / 86_400_000).unwrap_or_default();
        let messages = mc.map(|mc| mc.message_count).unwrap_or_default();
        let adv = adv.unwrap_or_default();
        let age = jd
            .and_then(|jd| epoch_ms_from_string(&jd.join_date))
            .map(member_days)
            .map(|days| format!("{days}d"))
            .unwrap_or_else(|| "?".to_owned());
        let label = format_server_label(&target.server, &ctx.state.mc_server);
        ctx.chat_success(format!(
            " [{search}]{label} KD: {kills}/{deaths} ({kdr:.2}) | Playtime: {pt_days}d | Messages: {messages} | Advancements: {adv} | Member for: {age}"
        ));
        Ok(())
    })
}

fn member_days(join_ms: u64) -> u64 {
    super::helpers::now_millis().saturating_sub(join_ms) / 86_400_000
}
