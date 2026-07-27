use super::helpers::whisper;
use crate::commands::{CommandContext, CommandFuture};

command!(
    ADVANCEMENT_COUNT_COMMAND,
    &["advancement", "advancementcount"],
    "Shows the number of advancements a user has made. Usage: {prefix}advancement <username>",
    advancement_count
);

fn advancement_count(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let search = ctx.args.join(" ").trim().to_owned();
        if search.is_empty() {
            whisper(&ctx, &format!(" Usage: {}advancement <advancement>", ctx.runtime.prefix));
            return Ok(());
        }

        let server = ctx.state.mc_server.clone();
        let Some(count) = ctx.state.api.get_advancement_name_count(&search, &server).await else {
            whisper(&ctx, "Could not count advancement matches right now.");
            return Ok(());
        };

        ctx.chat_success(format!(
            " Advancement \"{search}\" has been reached {count} time{} on {}.",
            if count == 1 { "" } else { "s" },
            ctx.state.mc_server
        ));
        Ok(())
    })
}
