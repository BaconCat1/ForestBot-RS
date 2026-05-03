pub const NAMES: &[&str] = &["discord"];
pub const RESPONSE: &str = "You can join the ForestBot discord here: https://discord.gg/2P8enrdY6t";

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: NAMES,
    whitelisted: false,
    execute,
};

pub fn execute<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    Box::pin(async move {
        ctx.bot.chat(RESPONSE);
        Ok(())
    })
}
