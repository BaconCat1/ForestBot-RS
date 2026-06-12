pub const NAMES: &[&str] = &["alias"];

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: NAMES,
    description: "Shows any aliases for a command. Usage: {prefix}alias <command>",
    whitelisted: false,
    execute,
};

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(name) = ctx.args.first().copied() else {
            ctx.whisper(" Usage: !alias <command>");
            return Ok(());
        };
        match crate::commands::find(name) {
            None => ctx.whisper(format!(" Unknown command: {name}")),
            Some(def) => ctx.chat(format!(" {name} aliases: {}", def.names.join(", "))),
        }
        Ok(())
    })
}
