use crate::{commands::CommandContext, constants::quote_servers};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatsTarget {
    pub server: String,
    pub search: String,
    pub has_server_arg: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatsTargetError {
    MissingUsernameForAll,
    MissingUsername,
    UnknownServer(String),
}

pub fn parse_stats_target_args(
    args: &[&str],
    user: &str,
    default_server: &str,
) -> Result<StatsTarget, StatsTargetError> {
    if args.is_empty() {
        return Ok(StatsTarget {
            server: default_server.to_owned(),
            search: user.to_owned(),
            has_server_arg: false,
        });
    }

    if args.len() == 1 {
        let single_arg = args[0].trim();
        if single_arg.eq_ignore_ascii_case("all") {
            return Err(StatsTargetError::MissingUsernameForAll);
        }

        return Ok(StatsTarget {
            server: default_server.to_owned(),
            search: single_arg.to_owned(),
            has_server_arg: false,
        });
    }

    let server = args[0].trim().to_lowercase();
    let search = args[1].trim();

    if search.is_empty() {
        return Err(StatsTargetError::MissingUsername);
    }

    if server != "all" && !quote_servers::is_quote_server(&server) {
        return Err(StatsTargetError::UnknownServer(server));
    }

    Ok(StatsTarget {
        server,
        search: search.to_owned(),
        has_server_arg: true,
    })
}

pub fn parse_stats_target_or_reply(
    ctx: &CommandContext<'_>,
    command_name: &str,
) -> Option<StatsTarget> {
    match parse_stats_target_args(&ctx.args, ctx.sender, &ctx.state.mc_server) {
        Ok(target) => Some(target),
        Err(error) => {
            ctx.whisper(stats_target_usage(&ctx.runtime.prefix, command_name, error));
            None
        }
    }
}

pub fn stats_target_usage(prefix: &str, command_name: &str, error: StatsTargetError) -> String {
    match error {
        StatsTargetError::MissingUsernameForAll => {
            format!(" Usage: {prefix}{command_name} all <username>")
        }
        StatsTargetError::UnknownServer(server) => {
            format!(" Unknown server \"{server}\". Use {prefix}lq for the list.")
        }
        StatsTargetError::MissingUsername => {
            format!(" Usage: {prefix}{command_name} <server|all> <username>")
        }
    }
}

pub fn format_server_label(resolved_server: &str, default_server: &str) -> String {
    if resolved_server != default_server {
        format!(" [{resolved_server}]")
    } else {
        String::new()
    }
}

pub fn format_server_scope_hint(
    has_server_arg: bool,
    resolved_server: &str,
    default_server: &str,
) -> String {
    if !has_server_arg {
        String::new()
    } else if resolved_server == "all" {
        " on all servers".to_owned()
    } else if resolved_server == default_server {
        String::new()
    } else {
        format!(" on {resolved_server}")
    }
}
