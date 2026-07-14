use super::command;
use super::helpers::{now_millis, player_uuid, whisper};
use crate::commands::{CommandContext, CommandFuture};
use crate::config::{OfflineMessage, load_offline_messages, save_offline_messages};

command!(OFFLINE_MSG_COMMAND, &["offlinemsg"], "Store a message to be delivered when the player next comes online. Usage: {prefix}offlinemsg <username> <message>", offline_msg);

fn offline_msg(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(recipient) = ctx.args.first().copied() else {
            whisper(&ctx, &format!(" Usage: {}offlinemsg <username> <message>", ctx.runtime.prefix));
            return Ok(());
        };
        let message = ctx
            .args
            .iter()
            .skip(1)
            .copied()
            .collect::<Vec<_>>()
            .join(" ");
        if recipient.eq_ignore_ascii_case(ctx.sender) {
            whisper(&ctx, " You can't send a message to yourself, sorry.");
            return Ok(());
        }
        if message.len() > 250 {
            whisper(
                &ctx,
                " Message is too long, must be less than 250 characters.",
            );
            return Ok(());
        }
        if player_uuid(&ctx, recipient).is_some() {
            whisper(
                &ctx,
                &format!(" User {recipient} is online, please send them a message directly."),
            );
            return Ok(());
        }
        if ctx
            .state
            .api
            .convert_username_to_uuid(recipient)
            .await
            .is_none()
        {
            whisper(&ctx, &format!(" User {recipient} is not in the database."));
            return Ok(());
        }
        let mut messages = load_offline_messages().await.unwrap_or_default();
        let pending_count = messages
            .iter()
            .filter(|msg| msg.recipient.eq_ignore_ascii_case(recipient))
            .count();
        if pending_count >= 5 {
            whisper(
                &ctx,
                &format!(" User {recipient} has too many offline messages pending..."),
            );
            return Ok(());
        }
        messages.push(OfflineMessage {
            sender: ctx.sender.to_owned(),
            recipient: recipient.to_owned(),
            message,
            timestamp: now_millis(),
            deliver_at: None,
        });
        save_offline_messages(&messages).await?;
        whisper(
            &ctx,
            &format!(
                " Your message has been saved and will be delivered to {recipient} when they are next online."
            ),
        );
        Ok(())
    })
}
