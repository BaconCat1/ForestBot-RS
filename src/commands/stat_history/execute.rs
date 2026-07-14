use super::admin_command;
use super::helpers::whisper;
use crate::commands::{CommandContext, CommandFuture};

admin_command!(EXECUTE_COMMAND, &["execute", "exec", "run"], "Executes a raw server command as the bot. Usage: {prefix}execute </command>", execute);

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let command = ctx.args.join(" ").trim().to_owned();
        if command.is_empty() {
            whisper(&ctx, &format!(" Usage: {}execute </command>", ctx.runtime.prefix));
            return Ok(());
        }
        ctx.chat(&command);
        whisper(&ctx, &format!(" Executed: {command}"));
        Ok(())
    })
}
