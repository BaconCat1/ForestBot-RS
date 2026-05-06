pub const NAMES: &[&str] = &["lastseen", "seen", "ls"];

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

        let Some(uuid) = ctx.state.api.convert_username_to_uuid(&target.search).await else {
            let message = if target.search.eq_ignore_ascii_case(ctx.sender) {
                format!(
                    "You haven't been seen by me{}, or unexpected error occurred.",
                    format_server_scope_hint(
                        target.has_server_arg,
                        &target.server,
                        &ctx.state.mc_server
                    )
                )
            } else {
                format!(
                    "{} has not been seen by me{}, or unexpected error occurred.",
                    target.search,
                    format_server_scope_hint(
                        target.has_server_arg,
                        &target.server,
                        &ctx.state.mc_server
                    )
                )
            };
            ctx.whisper(message);
            return Ok(());
        };

        let data = ctx.state.api.get_last_seen(&uuid, &target.server).await;
        let server_hint =
            format_server_scope_hint(target.has_server_arg, &target.server, &ctx.state.mc_server);

        let Some(last_seen) = data.map(|data| data.last_seen) else {
            if target.search.eq_ignore_ascii_case(ctx.sender) {
                ctx.whisper(format!(
                    "You haven't been seen by me{server_hint}, or unexpected error occurred."
                ));
            } else {
                ctx.whisper(format!(
                    "{} has not been seen by me{}, or unexpected error occurred.",
                    target.search, server_hint
                ));
            }
            return Ok(());
        };

        let server_label = format_server_label(&target.server, &ctx.state.mc_server);
        let last_seen_string = if let Ok(timestamp) = last_seen.parse::<u64>() {
            format!(
                "{} ({})",
                time::convert_unix_timestamp(timestamp / 1000),
                time::time_ago_str(timestamp)
            )
        } else {
            last_seen
        };

        ctx.chat(format!(
            " I last saw {}{} {}",
            target.search, server_label, last_seen_string
        ));
        Ok(())
    })
}
