use super::helpers::whisper;
use crate::commands::{CommandContext, CommandFuture};

command!(EDIT_FAQ_COMMAND, &["editfaq"], "Edits an existing FAQ entry. Usage: {prefix}editfaq <id> <new text>", edit_faq);

fn edit_faq(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(id_raw) = ctx.args.first() else {
            whisper(
                &ctx,
                &format!(" Please provide a valid FAQ ID. Usage: {}editfaq <id> <new text>", ctx.runtime.prefix),
            );
            return Ok(());
        };
        let Ok(id) = id_raw.parse::<i64>() else {
            whisper(
                &ctx,
                &format!(" Please provide a valid FAQ ID. Usage: {}editfaq <id> <new text>", ctx.runtime.prefix),
            );
            return Ok(());
        };
        let faq = ctx
            .args
            .iter()
            .skip(1)
            .copied()
            .collect::<Vec<_>>()
            .join(" ");
        if !faq.is_ascii() {
            whisper(&ctx, " FAQs must use plain ASCII characters only.");
            return Ok(());
        }
        if faq.starts_with('/') {
            whisper(&ctx, " FAQ text cannot start with '/'.");
            return Ok(());
        }
        if faq.len() < 5 {
            whisper(&ctx, " FAQ text must be at least 5 characters long.");
            return Ok(());
        }
        let Some(uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
            whisper(&ctx, " An error occurred while editing your FAQ.");
            return Ok(());
        };
        let Some(data) = ctx
            .state
            .api
            .edit_faq(id, ctx.sender, &faq, &uuid, &ctx.state.mc_server)
            .await
        else {
            whisper(&ctx, " An error occurred while editing your FAQ.");
            return Ok(());
        };
        if let Some(error) = data.error {
            whisper(&ctx, &format!(" {error}"));
        } else {
            whisper(
                &ctx,
                &format!(" Your FAQ has been successfully updated. ID: {id}."),
            );
        }
        Ok(())
    })
}
