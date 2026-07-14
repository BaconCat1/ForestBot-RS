use super::command;
use super::helpers::whisper;
use crate::commands::{CommandContext, CommandFuture};

command!(WINRATE_COMMAND, &["winrate", "wr"], "Shows a player's kill win rate: kills/(kills+deaths)%. Usage: {prefix}winrate <username>", winrate);

fn winrate(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let search = ctx.args.first().copied().unwrap_or(ctx.sender);
        let Some(uuid) = ctx.state.api.convert_username_to_uuid(search).await else {
            whisper(
                &ctx,
                &format!(
                    " {search} has no kills or deaths recorded, or unexpected error occurred."
                ),
            );
            return Ok(());
        };
        let Some(kd) = ctx.state.api.get_kd(&uuid, &ctx.state.mc_server).await else {
            whisper(
                &ctx,
                &format!(
                    " {search} has no kills or deaths recorded, or unexpected error occurred."
                ),
            );
            return Ok(());
        };
        let total = kd.kills + kd.deaths;
        if total == 0 {
            whisper(&ctx, &format!(" {search} has no kills or deaths recorded."));
            return Ok(());
        }
        let winrate = (kd.kills as f64 / total as f64) * 100.0;
        let deathrate = (kd.deaths as f64 / total as f64) * 100.0;
        ctx.chat(format!(
            " {search}: Win Rate: {winrate:.1}% | Death Rate: {deathrate:.1}% ({}K / {}D)",
            kd.kills, kd.deaths
        ));
        Ok(())
    })
}
