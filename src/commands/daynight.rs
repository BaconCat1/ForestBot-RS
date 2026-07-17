use crate::commands::{CommandContext, CommandDefinition, CommandFuture, enqueue_chat};

pub const DAY_COMMAND: CommandDefinition = CommandDefinition {
    names: &["day"],
    description: "Shows time until next dawn in-game. Usage: {prefix}day",
    whitelisted: false,
    execute: execute_day,
};

pub const NIGHT_COMMAND: CommandDefinition = CommandDefinition {
    names: &["night"],
    description: "Shows time until next nightfall in-game. Usage: {prefix}night",
    whitelisted: false,
    execute: execute_night,
};

const NIGHT_START: u64 = 13188;
const NIGHT_END: u64 = 23460;
const DAY_TICKS: u64 = 24000;

fn execute_day(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let tod = current_tick_of_day(&ctx).await;

        if !is_night(tod) {
            ctx.chat_success("It is currently daytime.".to_owned());
        } else {
            let remaining = NIGHT_END - tod;
            ctx.chat_success(format!("Dawn in {}.", format_ticks(remaining)));
        }
        Ok(())
    })
}

fn execute_night(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let tod = current_tick_of_day(&ctx).await;

        if is_night(tod) {
            ctx.chat_success("It is currently nighttime.".to_owned());
        } else {
            let remaining = (NIGHT_START + DAY_TICKS - tod) % DAY_TICKS;
            ctx.chat_success(format!("Night in {}.", format_ticks(remaining)));
        }
        Ok(())
    })
}

async fn current_tick_of_day(ctx: &CommandContext<'_>) -> u64 {
    let ticks = match query_live_daytime(ctx).await {
        Some(ticks) => ticks,
        // Free-running local estimate (Event::Tick-incremented, SetTime-corrected) --
        // see AzaleaState.day_ticks_accum. Replaces the old static last-packet snapshot.
        None => *ctx
            .state
            .day_ticks_accum
            .lock()
            .expect("day_ticks_accum lock poisoned") as u64,
    };
    ticks % DAY_TICKS
}

/// Sends "/time query day" and awaits the server's command-feedback response, filtering
/// it down to just the digits (the tick count is the only number in the reply either
/// way). Gated behind `use_live_time_query` in config.json -- most servers gate `/time`
/// at operator level, so this stays off unless the operator explicitly opts in. Falls
/// back to the passively-cached SetTime value (via None) if disabled, denied, or timed out.
async fn query_live_daytime(ctx: &CommandContext<'_>) -> Option<u64> {
    if !ctx.runtime.use_live_time_query {
        return None;
    }

    let (tx, rx) = tokio::sync::oneshot::channel();
    *ctx.state
        .pending_time_query
        .lock()
        .expect("pending_time_query lock poisoned") = Some(tx);

    enqueue_chat(ctx.state, "/time query day");

    let result = tokio::time::timeout(std::time::Duration::from_secs(3), rx).await;
    match result {
        Ok(Ok(ticks)) => Some(ticks),
        _ => {
            // Timed out or the sender was dropped without sending -- clear our slot in
            // case it's still ours (a late response arriving after this would otherwise
            // be misread as belonging to some future query).
            *ctx.state
                .pending_time_query
                .lock()
                .expect("pending_time_query lock poisoned") = None;
            None
        }
    }
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
