use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use serde_json::json;

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["pearl", "p"],
    description: "Activate your stasis pearl. Usage: {prefix}pearl <slot>",
    whitelisted: false,
    bridge_ok: true,
    execute,
};

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(slot_str) = ctx.args.first() else {
            ctx.whisper("Usage: !pearl <slot number>");
            return Ok(());
        };

        let Ok(slot) = slot_str.parse::<u8>() else {
            ctx.whisper(format!("Invalid slot: {slot_str}"));
            return Ok(());
        };

        let Some(ws) = ctx.state.api.websocket.as_ref() else {
            ctx.whisper("Pearl service unavailable (no Hub connection).");
            return Ok(());
        };

        let uuid = {
            let Ok(players) = ctx.state.players.read() else {
                ctx.whisper("Internal error: player cache unavailable.");
                return Ok(());
            };
            players.get(ctx.sender).map(|p| p.uuid.clone())
        };
        let uuid = match uuid {
            Some(u) => u,
            None => match ctx.state.api.convert_username_to_uuid(ctx.sender).await {
                Some(u) => u,
                None => {
                    ctx.whisper("Could not resolve your UUID. Try again in a moment.");
                    return Ok(());
                }
            },
        };

        if let Err(e) = ws.send_message("pearl_request", json!({ "slot": slot, "requester": ctx.sender, "requester_uuid": uuid })).await {
            ctx.whisper(format!("Failed to send pearl request: {e}"));
        }

        Ok(())
    })
}
