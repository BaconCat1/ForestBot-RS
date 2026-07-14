use super::command;
use super::helpers::whisper;
use crate::commands::{CommandContext, CommandFuture};
use crate::structure::logger;

command!(ADD_FAQ_COMMAND, &["addfaq"], "Adds a new FAQ entry. Usage: {prefix}addfaq <text>", add_faq);

fn add_faq(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let faq = ctx.args.join(" ").trim().to_owned();
        if faq.is_empty() {
            whisper(&ctx, &format!(" Add a FAQ with {}addfaq <text>", ctx.runtime.prefix));
            return Ok(());
        }
        if !faq.is_ascii() {
            whisper(&ctx, " FAQs must use plain ASCII characters only.");
            return Ok(());
        }
        if faq.contains('/') {
            whisper(&ctx, " You can't use '/' in your FAQ.");
            return Ok(());
        }
        let Some(uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
            whisper(&ctx, " An error occurred while adding your FAQ.");
            return Ok(());
        };
        let Some(data) = ctx
            .state
            .api
            .post_new_faq(ctx.sender, &faq, &uuid, &ctx.state.mc_server)
            .await
        else {
            whisper(&ctx, " An error occurred while adding your FAQ.");
            return Ok(());
        };
        if let Some(error) = data.error {
            whisper(&ctx, &format!(" {error}"));
        } else {
            whisper(
                &ctx,
                &format!(" Your FAQ has been added. Your entry ID is {}.", data.id),
            );
            logger::debug_cat("content_flag", "addfaq: about to call flag_content_if_needed");
            crate::commands::utils::flag_content_if_needed(&ctx.state, ctx.sender, "addfaq", &faq);
            logger::debug_cat("content_flag", "addfaq: flag_content_if_needed returned");
        }
        Ok(())
    })
}
