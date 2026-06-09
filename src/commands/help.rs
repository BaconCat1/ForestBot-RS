pub const NAMES: &[&str] = &["help", "commands"];
pub const RESPONSE: &str = "See all commands: tiny.cc/forcoms";

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: NAMES,
    whitelisted: false,
    execute,
};

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        ctx.chat(RESPONSE);
        Ok(())
    })
}
