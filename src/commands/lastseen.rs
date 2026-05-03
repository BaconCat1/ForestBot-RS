pub const NAMES: &[&str] = &["lastseen", "seen", "ls"];

use crate::{
    commands::{
        CommandContext, CommandDefinition, CommandFuture,
        utils::stats_target::{
            StatsTargetError, format_server_label, format_server_scope_hint,
            parse_stats_target_args,
        },
    },
    functions::utils::time,
};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: NAMES,
    whitelisted: false,
    execute,
};

pub fn execute<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    Box::pin(async move {
        let target = match parse_stats_target_args(&ctx.args, ctx.sender, &ctx.state.mc_server) {
            Ok(target) => target,
            Err(error) => {
                ctx.bot.chat(&format!(
                    "/{} {}",
                    ctx.runtime.whisper_command,
                    usage(ctx.sender, error)
                ));
                return Ok(());
            }
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
            ctx.bot.chat(&format!(
                "/{} {} {}",
                ctx.runtime.whisper_command, ctx.sender, message
            ));
            return Ok(());
        };

        let data = ctx.state.api.get_last_seen(&uuid, &target.server).await;
        let server_hint =
            format_server_scope_hint(target.has_server_arg, &target.server, &ctx.state.mc_server);

        let Some(last_seen) = data.map(|data| data.last_seen) else {
            if target.search.eq_ignore_ascii_case(ctx.sender) {
                ctx.bot.chat(&format!(
                    "/{} {} You haven't been seen by me{}, or unexpected error occurred.",
                    ctx.runtime.whisper_command, ctx.sender, server_hint
                ));
            } else {
                ctx.bot.chat(&format!(
                    "/{} {} {} has not been seen by me{}, or unexpected error occurred.",
                    ctx.runtime.whisper_command, ctx.sender, target.search, server_hint
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

        ctx.bot.chat(&format!(
            " I last saw {}{} {}",
            target.search, server_label, last_seen_string
        ));
        Ok(())
    })
}

fn usage(sender: &str, error: StatsTargetError) -> String {
    match error {
        StatsTargetError::MissingUsernameForAll => {
            format!("{sender}  Usage: !lastseen all <username>")
        }
        StatsTargetError::UnknownServer(server) => {
            format!("{sender}  Unknown server \"{server}\". Use !lq for the list.")
        }
        StatsTargetError::MissingUsername => {
            format!("{sender}  Usage: !lastseen <server|all> <username>")
        }
    }
}
