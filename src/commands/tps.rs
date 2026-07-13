use crate::commands::{CommandContext, CommandDefinition, CommandFuture};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["tps"],
    description: "Show server TPS derived from SetTime packet rate.",
    whitelisted: false,
    execute,
};

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let (oldest_ticks, oldest_ms, newest_ticks, newest_ms, count) = {
            let samples = ctx.state.tps_time_samples.lock().expect("tps_time_samples lock poisoned");
            if samples.len() < 2 {
                ctx.whisper("Not enough data yet — wait a few seconds.");
                return Ok(());
            }
            let &(ot, om) = samples.front().unwrap();
            let &(nt, nm) = samples.back().unwrap();
            (ot, om, nt, nm, samples.len())
        };

        let real_elapsed_s = (newest_ms.saturating_sub(oldest_ms)) as f64 / 1000.0;
        if real_elapsed_s < 1.0 {
            ctx.whisper("Not enough data yet — wait a few seconds.");
            return Ok(());
        }

        let game_ticks = newest_ticks.saturating_sub(oldest_ticks) as f64;
        let tps = (game_ticks / real_elapsed_s).min(20.0);
        let status = if tps >= 19.5 {
            "Good"
        } else if tps >= 15.0 {
            "Moderate"
        } else {
            "Lagging"
        };

        ctx.chat(format!(
            "TPS: {:.1} / 20.0 — {} ({}s window, {} samples)",
            tps,
            status,
            real_elapsed_s as u64,
            count,
        ));
        Ok(())
    })
}
