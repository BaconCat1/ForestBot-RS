use super::helpers::whisper;
use crate::commands::{CommandContext, CommandFuture};

command!(IAM_COMMAND, &["iam"], "Sets your {prefix}whois description. Usage: {prefix}iam <description>", iam);

fn iam(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let description = ctx
            .args
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if description.is_empty() {
            whisper(&ctx, &format!(" View descriptions with {}whois or set one with {}iam", ctx.runtime.prefix, ctx.runtime.prefix));
            return Ok(());
        }
        if !description.is_ascii() {
            whisper(&ctx, " Descriptions must use plain ASCII characters only.");
            return Ok(());
        }
        if description.contains('/') {
            whisper(&ctx, " Descriptions cannot contain '/'.");
            return Ok(());
        }
        if ctx
            .state
            .api
            .post_who_is_description(ctx.sender, &description)
            .await
            .is_some()
        {
            whisper(&ctx, &format!(" your {}whois has been set.", ctx.runtime.prefix));
            crate::commands::utils::flag_content_if_needed(&ctx.state, ctx.sender, "iam", &description);
        } else {
            whisper(
                &ctx,
                " Failed to save your description. Try a shorter/simpler message.",
            );
        }
        Ok(())
    })
}
