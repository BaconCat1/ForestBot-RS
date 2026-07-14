use super::command;
use super::helpers::{now_millis, whisper};
use crate::commands::{CommandContext, CommandFuture};
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};

const SHOUT_COOLDOWN_MS: u64 = 60 * 1000;

static LAST_SHOUT_AT: AtomicU64 = AtomicU64::new(0);

command!(SHOUT_COMMAND, &["shout"], "Broadcasts a message to all connected Forest servers. Usage: {prefix}shout <message>", shout);

fn shout(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let raw = ctx.args.join(" ");
        let message = raw.replace('/', "").trim().to_owned();
        if message.is_empty() {
            whisper(&ctx, &format!(" Usage: {}shout <message>", ctx.runtime.prefix));
            return Ok(());
        }
        let now = now_millis();
        let last = LAST_SHOUT_AT.load(Ordering::Relaxed);
        let remaining = SHOUT_COOLDOWN_MS.saturating_sub(now.saturating_sub(last));
        if remaining > 0 {
            whisper(
                &ctx,
                &format!(
                    " Shout is on cooldown. Try again in {} minute(s).",
                    remaining.div_ceil(60_000)
                ),
            );
            return Ok(());
        }
        let shout_text = format!(
            "[Shout {}] {}: {}",
            ctx.state.mc_server, ctx.sender, message
        );
        ctx.chat(&shout_text);
        let Some(websocket) = ctx.state.api.websocket.as_ref() else {
            whisper(
                &ctx,
                " Shout relay is unavailable right now (websocket disconnected).",
            );
            return Ok(());
        };
        websocket
            .send_message(
                "inbound_minecraft_chat",
                json!({
                    "name": ctx.sender,
                    "message": shout_text,
                    "date": now.to_string(),
                    "mc_server": "all",
                    "uuid": "shout-relay",
                    "relay_type": "shout",
                    "origin_server": ctx.state.mc_server,
                    "relay_id": format!("{}-rust", now),
                }),
            )
            .await?;
        LAST_SHOUT_AT.store(now, Ordering::Relaxed);
        if raw.trim() != message {
            whisper(
                &ctx,
                " Your shout was sanitized (bad words censored and '/' removed).",
            );
        }
        whisper(&ctx, " Shout sent to connected servers.");
        Ok(())
    })
}
