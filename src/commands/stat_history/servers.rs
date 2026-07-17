use super::helpers::{whisper, BACKFILL_CONCURRENCY};
use crate::commands::{CommandContext, CommandFuture};
use crate::constants::quote_servers::QUOTE_SERVERS;
use futures_util::stream::{self, StreamExt};

command!(
    SERVERS_COMMAND,
    &["servers", "playerservers", "seenservers"],
    "Shows which servers a user has been seen on. Usage: {prefix}servers <username>",
    servers
);

fn servers(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let search = ctx.args.first().copied().unwrap_or(ctx.sender);
        let Some(uuid) = ctx.state.api.convert_username_to_uuid(search).await else {
            whisper(&ctx, &format!(" Could not find {search}."));
            return Ok(());
        };

        let mut candidates = std::iter::once(ctx.state.mc_server.clone())
            .chain(QUOTE_SERVERS.iter().map(|server| (*server).to_owned()))
            .collect::<Vec<_>>();
        candidates.sort_unstable();
        candidates.dedup();

        let api = ctx.state.api.clone();
        let mut servers = stream::iter(candidates)
            .map(|server| {
                let api = api.clone();
                let uuid = uuid.clone();
                async move {
                    let stats = api.get_stats_by_uuid(&uuid, &server).await?;
                    has_player_data(&stats).then_some(server)
                }
            })
            .buffer_unordered(BACKFILL_CONCURRENCY)
            .filter_map(|server| async move { server })
            .collect::<Vec<_>>()
            .await;

        servers.sort_unstable();
        if servers.is_empty() {
            whisper(&ctx, &format!(" I have no server data for {search}."));
            return Ok(());
        }

        const CHAT_LIMIT: usize = 250;
        let header = format!(
            " I have data for {search} on {} server{}: ",
            servers.len(),
            if servers.len() == 1 { "" } else { "s" },
        );
        let mut current = header;
        let mut first = true;
        for (i, server) in servers.iter().enumerate() {
            let part = if i + 1 < servers.len() {
                format!("{server}, ")
            } else {
                server.clone()
            };
            if !first && current.len() + part.len() > CHAT_LIMIT {
                ctx.chat_success(&current);
                current = format!(" (cont.): {part}");
            } else {
                current.push_str(&part);
            }
            first = false;
        }
        if !current.is_empty() {
            ctx.chat_success(&current);
        }
        Ok(())
    })
}

fn has_player_data(stats: &crate::structure::endpoints::endpoints::AllPlayerStats) -> bool {
    stats
        .username
        .as_deref()
        .is_some_and(|name| !name.is_empty())
        || stats.uuid.as_deref().is_some_and(|uuid| !uuid.is_empty())
        || stats
            .join_date
            .as_deref()
            .is_some_and(|date| !date.is_empty())
        || stats
            .last_seen
            .as_deref()
            .is_some_and(|date| !date.is_empty())
        || stats.playtime.unwrap_or_default() > 0
        || stats.joins.unwrap_or_default() > 0
        || stats.leaves.unwrap_or_default() > 0
        || stats.kills.unwrap_or_default() > 0
        || stats.deaths.unwrap_or_default() > 0
}
