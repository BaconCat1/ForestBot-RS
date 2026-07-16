use super::helpers::{epoch_ms_from_string, now_millis, whisper};
use crate::commands::{CommandContext, CommandFuture};

command!(EFFICIENCY_COMMAND, &["efficiency", "eff"], "Shows rate-based efficiency stats. Usage: {prefix}efficiency <username> <kills|deaths|messages>", efficiency);

fn efficiency(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let (search, stat) = match ctx.args.as_slice() {
            [stat] => (ctx.sender, stat.to_lowercase()),
            [search, stat, ..] => (*search, stat.to_lowercase()),
            _ => {
                whisper(
                    &ctx,
                    &format!(" Valid stats: kills, deaths, messages. Usage: {}efficiency [username] <stat>", ctx.runtime.prefix),
                );
                return Ok(());
            }
        };
        if !matches!(stat.as_str(), "kills" | "deaths" | "messages") {
            whisper(
                &ctx,
                &format!(" Valid stats: kills, deaths, messages. Usage: {}efficiency [username] <stat>", ctx.runtime.prefix),
            );
            return Ok(());
        }
        let Some(uuid) = ctx.state.api.convert_username_to_uuid(search).await else {
            whisper(
                &ctx,
                &format!(" Couldn't get stats for {search}, or unexpected error occurred."),
            );
            return Ok(());
        };
        if stat == "kills" || stat == "deaths" {
            let (kd, pt) = tokio::join!(
                ctx.state.api.get_kd(&uuid, &ctx.state.mc_server),
                ctx.state.api.get_playtime(&uuid, &ctx.state.mc_server)
            );
            let (Some(kd), Some(pt)) = (kd, pt) else {
                whisper(
                    &ctx,
                    &format!(" Couldn't get stats for {search}, or unexpected error occurred."),
                );
                return Ok(());
            };
            let hours = pt.playtime as f64 / 3_600_000_f64;
            if hours == 0.0 {
                whisper(&ctx, &format!(" {search} has no playtime recorded."));
                return Ok(());
            }
            let count = if stat == "kills" { kd.kills } else { kd.deaths };
            ctx.chat(format!(
                " {search}: {count} {stat} over {hours:.1} hours = {:.3} {stat}/hr",
                count as f64 / hours
            ));
        } else {
            let (mc, jd) = tokio::join!(
                ctx.state
                    .api
                    .get_message_count(search, &ctx.state.mc_server),
                ctx.state.api.get_join_date(&uuid, &ctx.state.mc_server)
            );
            let (Some(mc), Some(jd)) = (mc, jd) else {
                whisper(
                    &ctx,
                    &format!(" Couldn't get stats for {search}, or unexpected error occurred."),
                );
                return Ok(());
            };
            let Some(join_ms) = epoch_ms_from_string(&jd.join_date) else {
                whisper(
                    &ctx,
                    &format!(" Couldn't determine join date for {search}."),
                );
                return Ok(());
            };
            let days = now_millis().saturating_sub(join_ms) as f64 / 86_400_000_f64;
            if days <= 0.0 {
                whisper(
                    &ctx,
                    &format!(" Couldn't calculate message rate for {search}."),
                );
                return Ok(());
            }
            ctx.chat(format!(
                " {search}: {} messages over {} days = {:.2} messages/day",
                mc.message_count,
                days.floor() as u64,
                mc.message_count as f64 / days
            ));
        }
        Ok(())
    })
}
