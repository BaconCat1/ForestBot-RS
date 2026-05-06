pub const NAMES: &[&str] = &["playtime", "pt"];

use crate::{
    commands::{
        CommandContext, CommandDefinition, CommandFuture,
        utils::stats_target::{
            format_server_label, format_server_scope_hint, parse_stats_target_or_reply,
        },
    },
    functions::utils::time,
};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: NAMES,
    whitelisted: false,
    execute,
};

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(target) = parse_stats_target_or_reply(&ctx, NAMES[0]) else {
            return Ok(());
        };

        let server_hint =
            format_server_scope_hint(target.has_server_arg, &target.server, &ctx.state.mc_server);

        let Some(uuid) = ctx.state.api.convert_username_to_uuid(&target.search).await else {
            let text = if target.search.eq_ignore_ascii_case(ctx.sender) {
                format!(
                    "I have no playtime recorded for you{server_hint}, or unexpected error occurred."
                )
            } else {
                format!(
                    "I have no playtime recorded for {}{}, or unexpected error occurred.",
                    target.search, server_hint
                )
            };
            ctx.whisper(text);
            return Ok(());
        };

        let data = ctx.state.api.get_playtime(&uuid, &target.server).await;

        let Some(data) = data else {
            let text = if target.search.eq_ignore_ascii_case(ctx.sender) {
                format!(
                    "I have no playtime recorded for you{server_hint}, or unexpected error occurred."
                )
            } else {
                format!(
                    "I have no playtime recorded for {}{}, or unexpected error occurred.",
                    target.search, server_hint
                )
            };
            ctx.whisper(text);
            return Ok(());
        };

        let playtime = time::dhms(data.playtime);
        let server_label = format_server_label(&target.server, &ctx.state.mc_server);
        ctx.chat(format!(
            " {}{}'s total playtime is {}",
            target.search, server_label, playtime
        ));
        Ok(())
    })
}
