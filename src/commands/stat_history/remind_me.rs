use super::command;
use super::helpers::{now_millis, whisper};
use crate::commands::{CommandContext, CommandFuture};
use crate::config::{OfflineMessage, load_offline_messages, save_offline_messages};

command!(REMIND_COMMAND, &["remindme", "remind"], "Set a self-reminder. Usage: {prefix}remindme [1s2m3h4d] <message> | {prefix}remindme stop", remind_me);

fn remind_me(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        if ctx.args.is_empty() {
            whisper(
                &ctx,
                &format!(
                    " Usage: {0}remindme [1s2m3h4d] <message>  or  {0}remindme stop",
                    ctx.runtime.prefix
                ),
            );
            return Ok(());
        }

        if ctx.args[0].eq_ignore_ascii_case("stop") {
            let mut messages = load_offline_messages().await.unwrap_or_default();
            let before = messages.len();
            messages.retain(|m| {
                !(m.sender.eq_ignore_ascii_case(ctx.sender)
                    && m.recipient.eq_ignore_ascii_case(ctx.sender))
            });
            let cancelled = before - messages.len();
            save_offline_messages(&messages).await?;
            whisper(
                &ctx,
                &format!(" Cancelled {cancelled} reminder(s)."),
            );
            return Ok(());
        }

        let (deliver_at, message) = if let Some(ms) = parse_duration_ms(ctx.args[0]) {
            let text = ctx.args[1..].join(" ");
            if text.is_empty() {
                whisper(&ctx, " No message specified after duration.");
                return Ok(());
            }
            (Some(now_millis() + ms), text)
        } else {
            (None, ctx.args.join(" "))
        };

        if message.len() > 250 {
            whisper(&ctx, " Message is too long, must be less than 250 characters.");
            return Ok(());
        }

        let mut messages = load_offline_messages().await.unwrap_or_default();
        let pending = messages
            .iter()
            .filter(|m| {
                m.sender.eq_ignore_ascii_case(ctx.sender)
                    && m.recipient.eq_ignore_ascii_case(ctx.sender)
            })
            .count();
        if pending >= 5 {
            whisper(&ctx, " You have too many pending reminders (max 5). Use remindme stop to clear them.");
            return Ok(());
        }

        messages.push(OfflineMessage {
            sender: ctx.sender.to_owned(),
            recipient: ctx.sender.to_owned(),
            message,
            timestamp: now_millis(),
            deliver_at,
        });
        save_offline_messages(&messages).await?;

        let timing = match deliver_at {
            Some(at) => {
                let secs = (at - now_millis()) / 1000;
                format!("in ~{}s", secs)
            }
            None => "on your next login".to_owned(),
        };
        whisper(
            &ctx,
            &format!(" Reminder set ({timing}). Use {0}remindme stop to cancel.", ctx.runtime.prefix),
        );
        Ok(())
    })
}

fn parse_duration_ms(s: &str) -> Option<u64> {
    let mut total: u64 = 0;
    let mut rest = s;
    let mut found = false;
    while !rest.is_empty() {
        let digit_end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
        if digit_end == 0 {
            return None;
        }
        let n: u64 = rest[..digit_end].parse().ok()?;
        rest = &rest[digit_end..];
        let unit = rest.chars().next()?;
        rest = &rest[unit.len_utf8()..];
        let ms = match unit {
            's' => 1_000u64,
            'm' => 60_000,
            'h' => 3_600_000,
            'd' => 86_400_000,
            _ => return None,
        };
        total += n * ms;
        found = true;
    }
    found.then_some(total)
}
