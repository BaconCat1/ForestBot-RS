pub const NAMES: &[&str] = &["discord"];
pub const RESPONSE: &str = "You can join the ForestBot discord here: https://discord.gg/2P8enrdY6t";

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: NAMES,
    description: "Shares the Discord server invite link. Usage: {prefix}discord",
    whitelisted: false,
    bridge_ok: true,
    execute,
};

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        ctx.chat(RESPONSE);
        Ok(())
    })
}
