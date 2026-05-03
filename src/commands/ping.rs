pub const NAMES: &[&str] = &["ping"];

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: NAMES,
    whitelisted: false,
    execute,
};

pub fn execute<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    Box::pin(async move {
        let target = ctx.args.first().copied().unwrap_or(ctx.sender);
        let latency = {
            let players = ctx
                .state
                .players
                .read()
                .expect("player cache lock poisoned");
            players
                .get(target)
                .or_else(|| {
                    players
                        .iter()
                        .find(|(name, _)| name.eq_ignore_ascii_case(target))
                        .map(|(_, player)| player)
                })
                .map(|player| player.latency)
        };

        ctx.bot.chat(&response(target, latency));
        Ok(())
    })
}

fn response(username: &str, latency: Option<i32>) -> String {
    match latency {
        Some(0) => format!(" {username}: 0ms (Most likely just joined.)"),
        Some(latency) => format!("{username}: {latency}ms"),
        None => format!("{username}: not found in tab list."),
    }
}
