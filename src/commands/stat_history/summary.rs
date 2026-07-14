use super::command;
use super::helpers::{epoch_ms_from_string, whisper};
use crate::commands::{CommandContext, CommandFuture};

command!(SUMMARY_COMMAND, &["summary", "sum"], "Single-line stats overview for a player. Usage: {prefix}summary <username>", summary);

fn summary(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let search = ctx.args.first().copied().unwrap_or(ctx.sender);
        let Some(uuid) = ctx.state.api.convert_username_to_uuid(search).await else {
            whisper(&ctx, &format!(" Could not find {search}."));
            return Ok(());
        };
        let (kd, pt, mc, adv, jd) = tokio::join!(
            ctx.state.api.get_kd(&uuid, &ctx.state.mc_server),
            ctx.state.api.get_playtime(&uuid, &ctx.state.mc_server),
            ctx.state
                .api
                .get_message_count(search, &ctx.state.mc_server),
            ctx.state
                .api
                .get_total_advancements_count(&uuid, &ctx.state.mc_server),
            ctx.state.api.get_join_date(&uuid, &ctx.state.mc_server)
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
        ctx.chat(format!(
            " [{search}] KD: {kills}/{deaths} ({kdr:.2}) | Playtime: {pt_days}d | Messages: {messages} | Advancements: {adv} | Member for: {age}"
        ));
        Ok(())
    })
}

fn member_days(join_ms: u64) -> u64 {
    super::helpers::now_millis().saturating_sub(join_ms) / 86_400_000
}
