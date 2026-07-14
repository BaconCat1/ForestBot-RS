use super::helpers::whisper;
use crate::commands::{CommandContext, CommandFuture};
use crate::structure::endpoints::endpoints::OwnedFaqEntry;

command!(
    OWNS_FAQ_COMMAND,
    &["ownsfaq", "ownfaq", "faqowner"],
    "Says the owner of a FAQ, or lists all FAQs for a user. Usage: {prefix}ownsfaq <id> or {prefix}ownsfaq <username>",
    owns_faq
);

fn owns_faq(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(arg) = ctx.args.first() else {
            let Some(faqs) = ctx.state.api.get_owned_faq_ids(ctx.sender).await else {
                whisper(&ctx, " You have no FAQs.");
                return Ok(());
            };
            for chunk in owned_faq_chunks(ctx.sender, &faqs) {
                whisper(&ctx, &format!(" {chunk}"));
            }
            return Ok(());
        };
        if arg.parse::<i64>().is_ok() {
            let Some(data) = ctx
                .state
                .api
                .get_faq(Some(arg), Some(&ctx.state.mc_server))
                .await
            else {
                whisper(&ctx, &format!(" Could not find FAQ #{arg}."));
                return Ok(());
            };
            ctx.chat(format!(" FAQ #{} owner: {}", data.id, data.username));
        } else {
            let Some(faqs) = ctx.state.api.get_owned_faq_ids(arg).await else {
                whisper(&ctx, &format!(" No FAQs found for {arg}."));
                return Ok(());
            };
            for chunk in owned_faq_chunks(arg, &faqs) {
                whisper(&ctx, &format!(" {chunk}"));
            }
        }
        Ok(())
    })
}

fn owned_faq_chunks(username: &str, faqs: &[OwnedFaqEntry]) -> Vec<String> {
    const MAX_MESSAGE_LENGTH: usize = 230;
    const CONTINUATION_PREFIX: &str = "More: ";

    let intro = format!("{}'s FAQs ({}): ", username, faqs.len());
    let mut chunks = Vec::new();
    let mut current = intro;
    let mut has_entry = false;

    for entry in faqs {
        let label = format!("#{}", entry.id);
        let separator = if has_entry { ", " } else { "" };
        let next = format!("{current}{separator}{label}");

        if next.len() > MAX_MESSAGE_LENGTH {
            chunks.push(current);
            current = format!("{CONTINUATION_PREFIX}{label}");
            has_entry = true;
            continue;
        }

        current = next;
        has_entry = true;
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}
