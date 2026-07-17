use super::helpers::{epoch_ms_from_string, player_uuid};
use crate::commands::{CommandContext, CommandFuture};
use crate::functions::utils::time;
use crate::structure::logger;

command!(FEBZEY_COMMAND, &["febzey"], "Bully Febzey for being AWOL and not maintaining his bot! Usage: {prefix}febzey", febzey);

fn febzey(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let pos = ctx.bot.position();
        logger::world(format!("[COORDS] x={:.1} y={:.1} z={:.1}", pos.x, pos.y, pos.z));
        let target = "Febzey_";
        let Some(uuid) = ctx.state.api.convert_username_to_uuid(target).await else {
            ctx.chat_success(format!(" I couldn't even find {target}. Truly absent."));
            return Ok(());
        };
        let last_seen = ctx
            .state
            .api
            .get_last_seen(&uuid, &ctx.state.mc_server)
            .await;
        let online = player_uuid(&ctx, target).is_some();
        match last_seen.and_then(|row| epoch_ms_from_string(&row.last_seen)) {
            Some(ts) if online => ctx.chat_success(format!(
                " {target} is online after being gone for {}. Someone check on the bot maintainer.",
                time::time_ago_str(ts)
            )),
            Some(ts) => ctx.chat_success(format!(
                " Last seen {target}: {} ({}). Still not maintaining his bot.",
                time::convert_unix_timestamp(ts / 1000),
                time::time_ago_str(ts)
            )),
            None => ctx.chat_success(format!(
                " No last seen data for {target}. The disappearance is complete."
            )),
        }
        Ok(())
    })
}
