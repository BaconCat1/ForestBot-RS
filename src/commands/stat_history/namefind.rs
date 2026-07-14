use super::command;
use super::helpers::whisper;
use crate::commands::{CommandContext, CommandFuture};

command!(NAMEFIND_COMMAND, &["search", "lookup", "find"], "Retrieves likely usernames related to your search. Usage: {prefix}find <username>", namefind);

fn namefind(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(search) = ctx.args.first() else {
            whisper(&ctx, &format!(" Usage: {}find <username>", ctx.runtime.prefix));
            return Ok(());
        };
        let data = ctx
            .state
            .api
            .get_name_finder(search, &ctx.state.mc_server)
            .await;
        if let Some(data) = data
            && !data.usernames.is_empty()
        {
            ctx.chat(format!(
                " You could be looking for: {}",
                data.usernames.join(", ")
            ));
        }
        Ok(())
    })
}
