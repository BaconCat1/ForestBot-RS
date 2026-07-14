use super::helpers::{all_known_usernames, whisper, BACKFILL_CONCURRENCY};
use crate::commands::{CommandContext, CommandFuture};
use futures_util::stream::{self, StreamExt};

const ADVANCEMENT_COUNT_FETCH_LIMIT: usize = 1000;

command!(
    ADVANCEMENT_COUNT_COMMAND,
    &["advancement", "advancementcount"],
    "Shows the number of advancements a user has made. Usage: {prefix}advancement <username>",
    advancement_count
);

fn advancement_count(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let search = ctx.args.join(" ").trim().to_owned();
        if search.is_empty() {
            whisper(&ctx, &format!(" Usage: {}advancement <advancement>", ctx.runtime.prefix));
            return Ok(());
        }

        whisper(
            &ctx,
            " Counting advancement matches, this may take a moment...",
        );
        let needle = search.to_ascii_lowercase();
        let server = ctx.state.mc_server.clone();
        let api = ctx.state.api.clone();
        let usernames = all_known_usernames(&ctx).await;
        let count = stream::iter(usernames)
            .map(|username| {
                let api = api.clone();
                let server = server.clone();
                let needle = needle.clone();
                async move {
                    let uuid = api.convert_username_to_uuid(&username).await?;
                    let advancements = api
                        .get_advancements(&uuid, &server, ADVANCEMENT_COUNT_FETCH_LIMIT, "DESC")
                        .await
                        .unwrap_or_default();
                    Some(
                        advancements
                            .into_iter()
                            .filter(|row| row.advancement.to_ascii_lowercase().contains(&needle))
                            .count(),
                    )
                }
            })
            .buffer_unordered(BACKFILL_CONCURRENCY)
            .fold(0usize, |total, count| async move {
                total + count.unwrap_or_default()
            })
            .await;

        ctx.chat(format!(
            " Advancement \"{search}\" has been reached {count} time{} on {}.",
            if count == 1 { "" } else { "s" },
            ctx.state.mc_server
        ));
        Ok(())
    })
}
