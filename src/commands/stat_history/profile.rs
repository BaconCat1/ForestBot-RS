use super::command;
use crate::commands::{CommandContext, CommandFuture};

command!(PROFILE_COMMAND, &["profile"], "Shares a link to your ForestBot Profile. Usage: {prefix}profile <user>", profile);

fn profile(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let target = ctx.args.first().copied().unwrap_or(ctx.sender);
        ctx.chat(format!(" https://forestbot.org/u/{target}"));
        Ok(())
    })
}
