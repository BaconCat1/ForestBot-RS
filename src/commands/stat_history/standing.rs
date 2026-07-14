use super::command;
use super::helpers::{player_uuid, whisper};
use crate::commands::{CommandContext, CommandFuture};

command!(STANDING_COMMAND, &["standing", "status"], "Shows blacklist/regular/whitelist status. Usage: {prefix}standing <username>(optional)", standing);

fn standing(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let target = ctx.args.first().copied().unwrap_or(ctx.sender);
        if target.starts_with('/') {
            whisper(&ctx, " That's not a valid username.");
            return Ok(());
        }
        let requester_uuid = match player_uuid(&ctx, ctx.sender) {
            Some(uuid) => Some(uuid),
            None => ctx.state.api.convert_username_to_uuid(ctx.sender).await,
        };
        if let Some(uuid) = requester_uuid.as_ref()
            && ctx.runtime.user_blacklist.contains(uuid)
            && !target.eq_ignore_ascii_case(ctx.sender)
        {
            whisper(&ctx, " You can only check your own standing.");
            return Ok(());
        }
        let target_uuid = if target.eq_ignore_ascii_case(ctx.sender) {
            requester_uuid
        } else {
            match player_uuid(&ctx, target) {
                Some(uuid) => Some(uuid),
                None => ctx.state.api.convert_username_to_uuid(target).await,
            }
        };
        let status = match target_uuid {
            Some(uuid) if ctx.runtime.user_blacklist.contains(&uuid) => "blacklisted",
            Some(uuid) if ctx.runtime.user_whitelist.contains(&uuid) => "whitelisted",
            _ => "regular",
        };
        ctx.chat(format!(" {target} is {status}."));
        Ok(())
    })
}
