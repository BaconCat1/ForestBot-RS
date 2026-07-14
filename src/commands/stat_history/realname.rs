use super::helpers::whisper;
use crate::commands::{CommandContext, CommandFuture};

command!(REALNAME_COMMAND, &["realname"], "Resolves someone's nickname to their real username. Usage: {prefix}realname <username>", realname);

fn realname(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(target) = ctx.args.first() else {
            whisper(&ctx, " Please provide a username to check.");
            return Ok(());
        };
        let players = ctx.state.players.read().expect("player cache lock poisoned").values().cloned().collect::<Vec<_>>();

        let by_display = players.iter().find(|p| {
            p.display_name.as_deref().is_some_and(|d| d.eq_ignore_ascii_case(target))
        });

        if let Some(player) = by_display {
            ctx.chat(format!("{target}'s real username is {}.", player.username));
        } else if players.iter().any(|p| p.username.eq_ignore_ascii_case(target)) {
            ctx.chat(format!("{target} is the real username."));
        } else {
            ctx.chat(format!("No player found matching \"{target}\" online."));
        }
        Ok(())
    })
}
