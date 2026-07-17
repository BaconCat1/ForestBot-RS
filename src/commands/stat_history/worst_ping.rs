use super::helpers::players_snapshot;
use crate::commands::{CommandContext, CommandFuture};

command!(WORST_PING_COMMAND, &["wp", "worstping"], "See who has the worst ping. Usage: {prefix}wp", worst_ping);

fn worst_ping(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let players = players_snapshot(&ctx);
        let Some(worst) = players.iter().max_by_key(|player| player.latency) else {
            ctx.whisper_success(" No players are cached yet.");
            return Ok(());
        };
        ctx.chat_success(format!(
            " Worst Ping: {}: {}ms",
            worst.username, worst.latency
        ));
        Ok(())
    })
}
