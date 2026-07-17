use super::helpers::players_snapshot;
use crate::commands::{CommandContext, CommandFuture};

command!(BEST_PING_COMMAND, &["bp", "bestping"], "See who has the best ping. Usage: {prefix}bp", best_ping);

fn best_ping(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let players = players_snapshot(&ctx);
        let Some(best) = players
            .iter()
            .filter(|player| player.latency > 0)
            .min_by_key(|player| player.latency)
            .or_else(|| players.first())
        else {
            ctx.whisper_success(" No players are cached yet.");
            return Ok(());
        };
        if best.latency == 0 {
            ctx.chat_success(format!(
                " Best ping: {}: {}ms (Most likely just joined.)",
                best.username, best.latency
            ));
        } else {
            ctx.chat_success(format!(" Best ping: {}: {}ms", best.username, best.latency));
        }
        Ok(())
    })
}
