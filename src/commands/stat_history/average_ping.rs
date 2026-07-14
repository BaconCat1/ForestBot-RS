use super::helpers::{players_snapshot, whisper};
use crate::commands::{CommandContext, CommandFuture};

command!(AVERAGE_PING_COMMAND, &["averageping", "ap"], "Shows the average ping for the server. Usage: {prefix}averageping <username>", average_ping);

fn average_ping(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let players = players_snapshot(&ctx);
        if players.is_empty() {
            whisper(&ctx, " No players are cached yet.");
            return Ok(());
        }

        let measured = players
            .iter()
            .filter(|player| player.latency > 0)
            .collect::<Vec<_>>();
        let ping_players = if measured.is_empty() {
            players.iter().collect::<Vec<_>>()
        } else {
            measured
        };
        let total = ping_players
            .iter()
            .map(|player| player.latency as i64)
            .sum::<i64>();
        let average = total as f64 / ping_players.len() as f64;
        let best = ping_players
            .iter()
            .min_by_key(|player| player.latency)
            .expect("ping_players is not empty");
        let worst = ping_players
            .iter()
            .max_by_key(|player| player.latency)
            .expect("ping_players is not empty");

        ctx.chat(format!(
            " Average ping: {:.1}ms | Best: {}: {}ms | Worst: {}: {}ms",
            average, best.username, best.latency, worst.username, worst.latency
        ));
        Ok(())
    })
}
