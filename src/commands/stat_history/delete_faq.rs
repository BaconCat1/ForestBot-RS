use super::command;
use super::helpers::whisper;
use crate::commands::{CommandContext, CommandFuture};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

static DELETE_FAQ_PENDING: OnceLock<Mutex<HashMap<String, i64>>> = OnceLock::new();

pub fn clear_delete_faq_pending(username: &str) {
    if let Some(pending) = DELETE_FAQ_PENDING.get() {
        pending
            .lock()
            .expect("delete faq pending lock poisoned")
            .remove(username);
    }
}

command!(
    DELETE_FAQ_COMMAND,
    &["delfaq", "deletefaq"],
    "Deletes an existing FAQ entry. Usage: {prefix}deletefaq <id>",
    delete_faq
);

fn delete_faq(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let pending = DELETE_FAQ_PENDING.get_or_init(|| Mutex::new(HashMap::new()));
        match ctx.args.first().copied() {
            Some("confirm") => {
                let id = {
                    let map = pending.lock().expect("delete faq pending lock poisoned");
                    map.get(ctx.sender).copied()
                };
                let Some(id) = id else {
                    whisper(&ctx, &format!(" No pending deletion. Run {}delfaq <id> first.", ctx.runtime.prefix));
                    return Ok(());
                };
                let Some(data) = ctx.state.api.delete_faq(id, ctx.sender).await else {
                    whisper(&ctx, " An error occurred while deleting the FAQ.");
                    return Ok(());
                };
                pending
                    .lock()
                    .expect("delete faq pending lock poisoned")
                    .remove(ctx.sender);
                if let Some(error) = data.error {
                    whisper(&ctx, &format!(" {error}"));
                } else {
                    whisper(&ctx, &format!(" FAQ #{id} has been deleted."));
                }
            }
            Some(id_raw) => {
                let Ok(id) = id_raw.parse::<i64>() else {
                    let p = &ctx.runtime.prefix;
                    whisper(&ctx, &format!(" Usage: {p}delfaq <id> | {p}delfaq confirm"));
                    return Ok(());
                };
                let Some(faq) = ctx.state.api.get_faq(Some(&id.to_string()), None).await else {
                    whisper(&ctx, " FAQ not found.");
                    return Ok(());
                };
                if faq.username != ctx.sender {
                    whisper(&ctx, " You do not own this FAQ.");
                    return Ok(());
                }
                pending
                    .lock()
                    .expect("delete faq pending lock poisoned")
                    .insert(ctx.sender.to_owned(), id);
                whisper(
                    &ctx,
                    &format!(" Run {}delfaq confirm to delete FAQ #{id}.", ctx.runtime.prefix),
                );
            }
            None => {
                let p = &ctx.runtime.prefix;
                whisper(&ctx, &format!(" Usage: {p}delfaq <id> | {p}delfaq confirm"));
            }
        }
        Ok(())
    })
}
