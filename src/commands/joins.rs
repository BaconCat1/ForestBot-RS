pub const NAMES: &[&str] = &["joins"];

use crate::commands::{
    CommandContext, CommandDefinition, CommandFuture,
    utils::stats_target::{
        format_server_label, format_server_scope_hint, parse_stats_target_or_reply,
    },
};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: NAMES,
    description: "Shows the number of times a user has joined. Usage: {prefix}joins <username> or {prefix}joins <server|all> <username> or {prefix}joins <player> <chips> to bet they log in soon",
    whitelisted: false,
    execute,
};

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        // Betting form: !joins <player> <bet> -- last arg is a stake, first is the
        // subject being bet on. Checked before the shared stats-target parser so it
        // can't collide with that parser's own <server> <username> shape (which never
        // ends in a bare integer).
        if ctx.args.len() == 2 {
            if let Ok(stake) = ctx.args[1].parse::<i64>() {
                return crate::commands::casino::join_market::place_bet(&ctx, ctx.args[0], stake).await;
            }
        }

        let Some(target) = parse_stats_target_or_reply(&ctx, NAMES[0]) else {
            return Ok(());
        };

        let server_hint =
            format_server_scope_hint(target.has_server_arg, &target.server, &ctx.state.mc_server);

        let Some(uuid) = ctx.state.api.convert_username_to_uuid(&target.search).await else {
            let text = if target.search.eq_ignore_ascii_case(ctx.sender) {
                format!("You have no joins{server_hint}, or unexpected error occurred.")
            } else {
                format!(
                    "{} has no joins{}, or unexpected error occurred.",
                    target.search, server_hint
                )
            };
            ctx.whisper(text);
            return Ok(());
        };

        let data = ctx.state.api.get_join_count(&uuid, &target.server).await;

        let Some(data) = data else {
            let text = if target.search.eq_ignore_ascii_case(ctx.sender) {
                format!("You have no joins{server_hint}, or unexpected error occurred.")
            } else {
                format!(
                    "{} has no joins{}, or unexpected error occurred.",
                    target.search, server_hint
                )
            };
            ctx.whisper(text);
            return Ok(());
        };

        let server_label = format_server_label(&target.server, &ctx.state.mc_server);
        ctx.chat_success(format!(
            " {}{} has joined the server {} times",
            target.search, server_label, data.join_count
        ));

        // Odds/bet hint only for a plain single-player lookup -- a server-scope
        // query (<server>/<all>) isn't about one specific person, and betting on
        // yourself is disallowed anyway (see join_market::place_bet), so showing
        // the hint there would just be a dead end.
        if !target.has_server_arg && !target.search.eq_ignore_ascii_case(ctx.sender) {
            crate::commands::casino::join_market::whisper_odds_hint(&ctx, &uuid, &target.search).await;
        }

        Ok(())
    })
}
