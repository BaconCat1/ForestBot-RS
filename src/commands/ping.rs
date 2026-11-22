pub const NAMES: &[&str] = &["ping"];

use crate::commands::{url::is_private_ip, CommandContext, CommandDefinition, CommandFuture};
use crate::structure::mineflayer::url_blocklist::is_blocked;

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

    // The host is user input; the MOTD and version name are remote-controlled.
    // Keep public status replies useful without relaying any of those strings.
    ctx.chat_success(server_status_message(
        response.players.online,
        response.players.max,
    ));
    Ok(())
}

fn server_status_message(online: i32, max: i32) -> String {
    format!("Server is online: {online}/{max} players")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_status_is_only_a_fixed_label_and_player_counts() {
        assert_eq!(
            server_status_message(30, 60),
            "Server is online: 30/60 players"
        );
        assert_eq!(server_status_message(0, 0), "Server is online: 0/0 players");
    }

    #[test]
    fn ordinary_player_ping_keeps_its_existing_response() {
        assert_eq!(response("Steve", Some(42)), "Steve: 42ms");
        assert_eq!(response("Steve", None), "Steve: not found in tab list.");
    }
}
