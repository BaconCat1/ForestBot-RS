use super::helpers::whisper;
use crate::commands::{CommandContext, CommandFuture};

command!(NICKNAME_COMMAND, &["nickname"], "Set the bots nickname in the server. Usage: {prefix}nickname <nickname>", nickname);

fn nickname(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let nickname = ctx.args.join(" ").trim().to_owned();
        if nickname.is_empty() {
            whisper(&ctx, &format!(" Usage: {}nickname <nickname>", ctx.runtime.prefix));
            return Ok(());
        }
        ctx.chat(format!(" /nick {nickname}"));
        Ok(())
    })
}
