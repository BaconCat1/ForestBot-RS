use crate::commands::{CommandContext, CommandDefinition, CommandFuture};

pub const DAY_COMMAND: CommandDefinition = CommandDefinition {
    names: &["day"],
    description: "Shows time until next dawn in-game. Usage: {prefix}day",
    whitelisted: false,
    bridge_ok: true,
    execute: execute_day,
};

pub const NIGHT_COMMAND: CommandDefinition = CommandDefinition {
    names: &["night"],
    description: "Shows time until next nightfall in-game. Usage: {prefix}night",
    whitelisted: false,
    bridge_ok: true,
    execute: execute_night,
};

const NIGHT_START: u64 = 13188;
const NIGHT_END: u64 = 23460;
const DAY_TICKS: u64 = 24000;

fn execute_day(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let ticks = *ctx.state.world_time_ticks.read().expect("world_time_ticks lock poisoned");
        let tod = ticks % DAY_TICKS;

        if !is_night(tod) {
            ctx.chat("It is currently daytime.".to_owned());
        } else {
            let remaining = NIGHT_END - tod;
            ctx.chat(format!("Dawn in {}.", format_ticks(remaining)));
        }
        Ok(())
    })
}

fn execute_night(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let ticks = *ctx.state.world_time_ticks.read().expect("world_time_ticks lock poisoned");
        let tod = ticks % DAY_TICKS;

        if is_night(tod) {
            ctx.chat("It is currently nighttime.".to_owned());
        } else {
            let remaining = (NIGHT_START + DAY_TICKS - tod) % DAY_TICKS;
            ctx.chat(format!("Night in {}.", format_ticks(remaining)));
        }
        Ok(())
    })
}

fn is_night(tod: u64) -> bool {
    tod >= NIGHT_START && tod < NIGHT_END
}

fn format_ticks(ticks: u64) -> String {
    let secs = ticks / 20;
    let mins = secs / 60;
    let secs = secs % 60;
    if mins > 0 {
        format!("{mins}m {secs}s")
    } else {
        format!("{secs}s")
    }
}
