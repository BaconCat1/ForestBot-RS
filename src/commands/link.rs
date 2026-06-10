use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::mineflayer::bot::PlayerSnapshot;

pub const UNLINK_COMMAND: CommandDefinition = CommandDefinition {
    names: &["unlink"],
    whitelisted: false,
    execute: execute_unlink,
};

pub fn execute_unlink(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        if !ctx.args.first().map(|s| s.eq_ignore_ascii_case("UNLINK")).unwrap_or(false) {
            ctx.whisper("Type !unlink UNLINK to confirm removing your Discord account link.");
            return Ok(());
        }

        let sender_uuid = {
            let players = ctx.state.players.read().expect("player cache lock poisoned");
            resolve_uuid(ctx.sender, &players)
        };
        let sender_uuid = match sender_uuid {
            Some(u) => u,
            None => match ctx.state.api.convert_username_to_uuid(ctx.sender).await {
                Some(u) => u,
                None => {
                    ctx.whisper("Could not resolve your UUID.");
                    return Ok(());
                }
            },
        };

        if ctx.state.api.tradebot_unlink(&sender_uuid).await {
            ctx.whisper("Your Discord account has been unlinked.");
        } else {
            ctx.whisper("No linked Discord account found.");
        }

        Ok(())
    })
}

pub const LINK_COMMAND: CommandDefinition = CommandDefinition {
    names: &["link"],
    whitelisted: false,
    execute: execute_link,
};

pub fn execute_link(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let sender_uuid = {
            let players = ctx.state.players.read().expect("player cache lock poisoned");
            resolve_uuid(ctx.sender, &players)
        };
        let sender_uuid = match sender_uuid {
            Some(u) => u,
            None => match ctx.state.api.convert_username_to_uuid(ctx.sender).await {
                Some(u) => u,
                None => {
                    ctx.whisper("Could not resolve your UUID.");
                    return Ok(());
                }
            },
        };

        let code = gen_link_code();

        if ctx.state.api.tradebot_request_link_code(&sender_uuid, &code).await {
            ctx.whisper(format!(
                "Link code: {code}  (5 min). In Discord: /link {code}"
            ));
        } else {
            ctx.whisper("Could not generate link code. Try again later.");
        }

        Ok(())
    })
}

fn gen_link_code() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("{:06X}", nanos % 0x100_0000)
}

fn resolve_uuid(username: &str, players: &HashMap<String, PlayerSnapshot>) -> Option<String> {
    players.get(username).map(|p| p.uuid.clone())
}
