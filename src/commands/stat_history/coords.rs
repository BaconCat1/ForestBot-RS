use super::command;
use super::helpers::whisper;
use crate::commands::{CommandContext, CommandFuture};

command!(COORDS_COMMAND, &["coords"], "Shows the bot's current coordinates. Usage: {prefix}coords", coords);

fn coords(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        if ctx.runtime.use_whitelist && !sender_whitelisted(&ctx) {
            return Ok(());
        }
        let pos = ctx.bot.position();
        whisper(
            &ctx,
            &format!(
                " I am currently at: X: {} Y: {} Z: {}",
                pos.x.floor() as i64,
                pos.y.floor() as i64,
                pos.z.floor() as i64
            ),
        );
        Ok(())
    })
}

fn sender_whitelisted(ctx: &CommandContext<'_>) -> bool {
    if ctx
        .runtime
        .user_whitelist
        .iter()
        .any(|entry| entry.eq_ignore_ascii_case(ctx.sender))
    {
        return true;
    }
    super::helpers::player_uuid(ctx, ctx.sender).is_some_and(|uuid| ctx.runtime.user_whitelist.contains(&uuid))
}
