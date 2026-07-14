use super::command;
use super::helpers::{extract_victim_name, whisper};
use crate::commands::{CommandContext, CommandFuture};

command!(GRUDGE_COMMAND, &["grudge"], "Shows how many times a player has killed a specific victim. Usage: {prefix}grudge <killer> <victim>", grudge);

fn grudge(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let (killer, victim) = match ctx.args.as_slice() {
            [victim] => (ctx.sender, *victim),
            [killer, victim, ..] => (*killer, *victim),
            _ => {
                whisper(&ctx, &format!(" Usage: {}grudge [killer] <victim>", ctx.runtime.prefix));
                return Ok(());
            }
        };
        let Some(uuid) = ctx.state.api.convert_username_to_uuid(killer).await else {
            whisper(
                &ctx,
                &format!(" {killer} has no kills recorded, or unexpected error occurred."),
            );
            return Ok(());
        };
        let Some(kills) = ctx
            .state
            .api
            .get_kills(&uuid, &ctx.state.mc_server, 10000, "DESC")
            .await
        else {
            whisper(
                &ctx,
                &format!(" {killer} has no kills recorded, or unexpected error occurred."),
            );
            return Ok(());
        };
        let count = kills
            .iter()
            .filter_map(extract_victim_name)
            .filter(|name| name.eq_ignore_ascii_case(victim))
            .count();
        if count == 0 {
            ctx.chat(format!(" {killer} has never killed {victim}."));
        } else if count >= 30 {
            ctx.chat(format!(
                " {killer} has killed {victim} {count} times. That's a grudge!"
            ));
        } else {
            ctx.chat(format!(
                " {killer} has killed {victim} {count} time{}.",
                if count == 1 { "" } else { "s" }
            ));
        }
        Ok(())
    })
}
