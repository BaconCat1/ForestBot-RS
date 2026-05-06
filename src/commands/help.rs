pub const NAMES: &[&str] = &["help", "commands"];
pub const RESPONSE: &str = "Commands: !ping, !help, !discord, !reload, !lastseen, !msgcount, !playtime, !joins, !quote. More commands are still being ported. See here for all commands: https://pastebin.com/VP3t7mJ9";

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: NAMES,
    whitelisted: false,
    execute,
};

pub fn execute<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    Box::pin(async move {
        ctx.chat(RESPONSE);
        Ok(())
    })
}
