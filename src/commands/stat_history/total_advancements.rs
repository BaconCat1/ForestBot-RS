use super::helpers::whisper;
use crate::commands::{CommandContext, CommandFuture};
use crate::commands::utils::stats_target::{
    StatsTargetError, format_server_label, format_server_scope_hint, parse_stats_target_args,
};

command!(
    TOTAL_ADVANCEMENTS_COMMAND,
    &["advancements", "totaladvancements", "advs", "adv"],
    "Retrieves the number of advancements a user has. Usage: {prefix}advs <username> or {prefix}advs <server|all> <username>",
    total_advancements
);

fn total_advancements(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let target = match parse_stats_target_args(&ctx.args, ctx.sender, &ctx.state.mc_server) {
            Ok(target) => target,
            Err(StatsTargetError::MissingUsernameForAll) => {
                whisper(&ctx, &format!(" Usage: {}advs all <username>", ctx.runtime.prefix));
                return Ok(());
            }
            Err(StatsTargetError::UnknownServer(server)) => {
                whisper(
                    &ctx,
                    &format!(" Unknown server \"{server}\". Use {}lq for the list.", ctx.runtime.prefix),
                );
                return Ok(());
            }
            Err(StatsTargetError::MissingUsername) => {
                whisper(&ctx, &format!(" Usage: {}advs <server|all> <username>", ctx.runtime.prefix));
                return Ok(());
            }
        };

        let Some(uuid) = ctx.state.api.convert_username_to_uuid(&target.search).await else {
            let hint = format_server_scope_hint(
                target.has_server_arg,
                &target.server,
                &ctx.state.mc_server,
            );
            if target.search.eq_ignore_ascii_case(ctx.sender) {
                whisper(
                    &ctx,
                    &format!(
                        " I have not seen any advancements from you{hint}, or unexpected error occurred."
                    ),
                );
            } else {
                whisper(
                    &ctx,
                    &format!(
                        " I have not seen any advancements from {}{hint}, or unexpected error occurred.",
                        target.search
                    ),
                );
            }
            return Ok(());
        };
        let count = ctx
            .state
            .api
            .get_total_advancements_count(&uuid, &target.server)
            .await
            .unwrap_or_default();
        if count == 0 {
            let hint = format_server_scope_hint(
                target.has_server_arg,
                &target.server,
                &ctx.state.mc_server,
            );
            if target.search.eq_ignore_ascii_case(ctx.sender) {
                whisper(
                    &ctx,
                    &format!(
                        " I have not seen any advancements from you{hint}, or unexpected error occurred."
                    ),
                );
            } else {
                whisper(
                    &ctx,
                    &format!(
                        " I have not seen any advancements from {}{hint}, or unexpected error occurred.",
                        target.search
                    ),
                );
            }
            return Ok(());
        }

        let label = format_server_label(&target.server, &ctx.state.mc_server);
        ctx.chat(format!(
            " I have seen {count} advancements from {}{}",
            target.search, label
        ));
        Ok(())
    })
}
