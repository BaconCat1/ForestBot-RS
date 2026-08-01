pub const NAMES: &[&str] = &["ping"];

use crate::commands::{url::is_private_ip, utils::flag_content_if_needed, CommandContext, CommandDefinition, CommandFuture};
use crate::structure::mineflayer::{
    url_blocklist::is_blocked,
    utils::profanity_filter::{censor_message, censor_threshold_from_config},
};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: NAMES,
    description: "Check your ping or another user's, or ping a Minecraft server. Usage: {prefix}ping <username> or {prefix}ping server|s <host[:port]>",
    whitelisted: false,
    execute,
};

const SERVER_PING_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        if matches!(ctx.args.first().copied(), Some("server") | Some("s")) {
            let Some(host) = ctx.args.get(1).copied() else {
                ctx.whisper(format!("Usage: {}ping server <host[:port]>", ctx.runtime.prefix));
                return Ok(());
            };
            return ping_mc_server(&ctx, host).await;
        }

        let target = ctx.args.first().copied().unwrap_or(ctx.sender);
        let latency = {
            let players = ctx
                .state
                .players
                .read()
                .expect("player cache lock poisoned");
            players
                .get(target)
                .or_else(|| {
                    players
                        .iter()
                        .find(|(name, _)| name.eq_ignore_ascii_case(target))
                        .map(|(_, player)| player)
                })
                .map(|player| player.latency)
        };

        ctx.chat_success(response(target, latency));
        Ok(())
    })
}

fn response(username: &str, latency: Option<i32>) -> String {
    match latency {
        Some(0) => format!(" {username}: 0ms (Most likely just joined.)"),
        Some(latency) => format!("{username}: {latency}ms"),
        None => format!("{username}: not found in tab list."),
    }
}

// Server List Ping (status handshake) for an arbitrary Minecraft server, reusing !url's
// domain blocklist + SSRF private-IP guard since this opens our own outbound TCP connection
// to a user-supplied host, same trust boundary as !url's HTTP fetch.
async fn ping_mc_server(ctx: &CommandContext<'_>, host: &str) -> anyhow::Result<()> {
    let blocklist = {
        let guard = ctx.state.url_blocklist.read().expect("url_blocklist read");
        match guard.as_ref() {
            None => {
                ctx.whisper("Blocklist still loading, try again shortly.");
                return Ok(());
            }
            Some(bl) => bl.clone(),
        }
    };

    let domain = host.split(':').next().unwrap_or(host);
    if is_blocked(domain, &blocklist) {
        crate::structure::logger::warn(format!(
            "ping: blocked by local blocklist: {domain} (requested by {})",
            ctx.sender
        ));
        ctx.whisper("That server is blocked.");
        return Ok(());
    }

    let lookup_target = if host.contains(':') {
        host.to_owned()
    } else {
        format!("{host}:25565")
    };
    match tokio::net::lookup_host(&lookup_target).await {
        Ok(mut addrs) => {
            if let Some(addr) = addrs.next() {
                if is_private_ip(addr.ip()) {
                    crate::structure::logger::warn(format!(
                        "ping: SSRF block on {domain} (requested by {})",
                        ctx.sender
                    ));
                    ctx.whisper("That server is blocked.");
                    return Ok(());
                }
            }
        }
        Err(_) => {
            ctx.whisper("Could not resolve that address.");
            return Ok(());
        }
    }

    let response = match tokio::time::timeout(SERVER_PING_TIMEOUT, azalea::ping::ping_server(host.to_owned())).await
    {
        Ok(Ok(response)) => response,
        Ok(Err(e)) => {
            crate::structure::logger::debug_cat("ping", format!("ping_server failed for {host}: {e}"));
            ctx.whisper("Could not reach that server.");
            return Ok(());
        }
        Err(_) => {
            ctx.whisper("Server ping timed out.");
            return Ok(());
        }
    };

    // MC MOTDs are often 2 lines; chat is single-line, so collapse to spaces.
    let motd = response
        .description
        .to_string()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let trie = *ctx.state.profanity_trie.read().expect("profanity_trie read");
    let threshold = censor_threshold_from_config(&ctx.runtime.censor_threshold);
    let censored_motd = match trie {
        Some(trie) => censor_message(trie, &motd, threshold, ctx.runtime.log_censorship_hits),
        None => motd.clone(),
    };
    if censored_motd != motd {
        flag_content_if_needed(ctx.state, ctx.sender, "ping", &format!("{host}\n{motd}"));
    }

    ctx.chat_success(format!(
        "[{host}] {censored_motd} | {}/{} players | {}",
        response.players.online, response.players.max, response.version.name
    ));
    Ok(())
}
