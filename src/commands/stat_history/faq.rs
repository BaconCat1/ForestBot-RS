use super::command;
use super::helpers::whisper;
use crate::commands::{CommandContext, CommandFuture};

command!(FAQ_COMMAND, &["faq", "getfaq"], "Retrieves a FAQ entry by ID. Usage: {prefix}faq <id>(optional)", faq);

fn faq(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let id = ctx.args.first().copied();
        let Some(data) = ctx.state.api.get_faq(id, Some(&ctx.state.mc_server)).await else {
            whisper(
                &ctx,
                " There was an error getting your FAQ, it may not exist.",
            );
            return Ok(());
        };
        ctx.chat(format!(" #{}/{}: {}", data.id, data.total, data.faq));
        Ok(())
    })
}
