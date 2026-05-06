pub const NAMES: &[&str] = &["quote", "q"];

use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    commands::{CommandContext, CommandDefinition, CommandFuture},
    constants::quote_servers::{QUOTE_SERVERS, is_quote_server},
    functions::utils::time,
};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: NAMES,
    whitelisted: false,
    execute,
};

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let server_arg = ctx.args.first().copied().unwrap_or("").to_lowercase();
        let has_server_arg = ctx.args.len() >= 2;
        let server = if has_server_arg {
            server_arg.as_str()
        } else {
            &ctx.state.mc_server
        };
        let search = if has_server_arg {
            ctx.args.get(1).copied().unwrap_or("")
        } else {
            ctx.args.first().copied().unwrap_or(ctx.sender)
        };

        if !has_server_arg && server_arg == "all" {
            ctx.chat(format!(
                "/{} {}  Usage: !quote all <username>",
                ctx.runtime.whisper_command, ctx.sender
            ));
            return Ok(());
        }

        if has_server_arg && search.trim().is_empty() {
            ctx.chat(format!(
                "/{} {}  Usage: !quote <server> <username>",
                ctx.runtime.whisper_command, ctx.sender
            ));
            return Ok(());
        }

        if has_server_arg && server != "all" && !is_quote_server(server) {
            ctx.chat(format!(
                "/{} {}  Unknown server \"{}\". Use !lq for the list.",
                ctx.runtime.whisper_command, ctx.sender, server
            ));
            return Ok(());
        }

        let mut data = None;
        let mut resolved_server = server.to_owned();

        if server == "all" {
            for candidate in shuffled_quote_servers() {
                let result = ctx.state.api.get_quote(search, candidate, None).await;
                if result
                    .as_ref()
                    .is_some_and(|quote| !quote.message.is_empty())
                {
                    data = result;
                    resolved_server = candidate.to_owned();
                    break;
                }
            }
        } else {
            data = ctx.state.api.get_quote(search, server, None).await;
        }

        let Some(data) = data else {
            let server_hint = if has_server_arg {
                if server == "all" {
                    " on any server".to_owned()
                } else {
                    format!(" on {server}")
                }
            } else {
                String::new()
            };
            let text = if search.eq_ignore_ascii_case(ctx.sender) {
                format!(
                    "I have no quotes recorded for you{server_hint}, or unexpected error occurred."
                )
            } else {
                format!(
                    "I have no quotes recorded for {search}{server_hint}, or unexpected error occurred."
                )
            };
            ctx.chat(format!(
                "/{} {} {}",
                ctx.runtime.whisper_command, ctx.sender, text
            ));
            return Ok(());
        };

        let date = data
            .date
            .as_deref()
            .and_then(epoch_ms_from_string)
            .map(time::time_ago_str)
            .map(|date| format!(" ({date})"))
            .unwrap_or_default();
        let server_label = if resolved_server != ctx.state.mc_server {
            format!(" [{resolved_server}]")
        } else {
            String::new()
        };

        ctx.chat(format!(" {search}{server_label}: {}{date}", data.message));
        Ok(())
    })
}

fn epoch_ms_from_string(value: &str) -> Option<u64> {
    let raw = value.parse::<u64>().ok()?;
    Some(if raw < 1_000_000_000_000 {
        raw * 1000
    } else {
        raw
    })
}

fn shuffled_quote_servers() -> Vec<&'static str> {
    let mut servers = QUOTE_SERVERS.to_vec();
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as usize)
        .unwrap_or_default();
    let len = servers.len();

    for i in 0..len {
        let j = (seed.wrapping_add(i * 31)) % len;
        servers.swap(i, j);
    }

    servers
}
