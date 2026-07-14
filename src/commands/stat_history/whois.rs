use super::command;
use super::helpers::whisper;
use crate::commands::{CommandContext, CommandFuture};

command!(WHOIS_COMMAND, &["whois"], "Shows the description of a user. Usage: {prefix}whois <username>", whois);

fn whois(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let target = ctx.args.first().copied().unwrap_or(ctx.sender);
        let data = ctx.state.api.get_who_is(target).await;
        if let Some(data) = data
            && !data.description.is_empty()
        {
            let description = data.description.join(" ");
            let safe_description = description
                .replace(['\r', '\n'], " ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            ctx.chat(format!("User {target} is {safe_description}"));
        } else {
            let message = if target.eq_ignore_ascii_case(ctx.sender) {
                format!(" You have not yet set a description with {}iam", ctx.runtime.prefix)
            } else {
                format!(" {target} has not yet set a description with {}iam", ctx.runtime.prefix)
            };
            whisper(&ctx, &message);
        }
        Ok(())
    })
}
