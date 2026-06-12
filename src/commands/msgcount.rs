pub const NAMES: &[&str] = &["msgcount", "messages"];

use crate::commands::{
    CommandContext, CommandDefinition, CommandFuture,
    utils::stats_target::{
        format_server_label, format_server_scope_hint, parse_stats_target_or_reply,
    },
};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: NAMES,
    description: "Retrieves the number of messages a user has sent. Usage: {prefix}msgcount <username> or {prefix}msgcount <server|all> <username>",
    whitelisted: false,
    execute,
};

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(target) = parse_stats_target_or_reply(&ctx, NAMES[0]) else {
            return Ok(());
        };

        let data = ctx
            .state
            .api
            .get_message_count(&target.search, &target.server)
            .await;
        let server_hint =
            format_server_scope_hint(target.has_server_arg, &target.server, &ctx.state.mc_server);

        let Some(data) = data else {
            let text = if target.search.eq_ignore_ascii_case(ctx.sender) {
                format!(
                    "I have not seen any messages from you{server_hint}, or unexpected error occurred."
                )
            } else {
                format!(
                    "I have not seen any messages from {}{}, or unexpected error occurred.",
                    target.search, server_hint
                )
            };
            ctx.whisper(text);
            return Ok(());
        };

        let server_label = format_server_label(&target.server, &ctx.state.mc_server);
        ctx.chat(format!(
            " {}{}: {} messages",
            target.search, server_label, data.message_count
        ));
        Ok(())
    })
}
