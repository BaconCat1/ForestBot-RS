pub const NAMES: &[&str] = &["joins"];

use crate::commands::{
    CommandContext, CommandDefinition, CommandFuture,
    utils::stats_target::{
        StatsTargetError, format_server_label, format_server_scope_hint, parse_stats_target_args,
    },
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
                ctx.chat(&format!(
                    "/{} {}",
                    ctx.runtime.whisper_command,
                    usage(ctx.sender, error)
                ));
                return Ok(());
            }
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
            ctx.chat(&format!(
                "/{} {} {}",
                ctx.runtime.whisper_command, ctx.sender, text
            ));
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
            ctx.chat(&format!(
                "/{} {} {}",
                ctx.runtime.whisper_command, ctx.sender, text
            ));
            return Ok(());
        };

        let server_label = format_server_label(&target.server, &ctx.state.mc_server);
        ctx.chat(&format!(
            " {}{} has joined the server {} times",
            target.search, server_label, data.join_count
        ));
        Ok(())
    })
}

fn usage(sender: &str, error: StatsTargetError) -> String {
    match error {
        StatsTargetError::MissingUsernameForAll => {
            format!("{sender}  Usage: !joins all <username>")
        }
        StatsTargetError::UnknownServer(server) => {
            format!("{sender}  Unknown server \"{server}\". Use !lq for the list.")
        }
        StatsTargetError::MissingUsername => {
            format!("{sender}  Usage: !joins <server|all> <username>")
        }
    }
}
