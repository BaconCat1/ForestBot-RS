pub const NAMES: &[&str] = &["quote", "q"];

use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    commands::{CommandContext, CommandDefinition, CommandFuture},
    constants::quote_servers::{QUOTE_SERVERS, is_quote_server},
    functions::utils::time,
    structure::endpoints::endpoints::{MinecraftChatMessage, QuoteOptions},
};

const KEYWORD_MESSAGE_FETCH_LIMIT: usize = 100;

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: NAMES,
    description: "Retrieves a random quote from a user. Usage: {prefix}quote <username> or {prefix}quote <server|all> <username>",
    whitelisted: false,
    execute,
};

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let query = QuoteQuery::from_args(&ctx.args, ctx.sender, &ctx.state.mc_server);
        let server = query.server.as_str();
        let search = query.search.as_str();
        let phrase = query.phrase.as_deref();

        if matches!(query.mode, QuoteQueryMode::MissingAllTarget) {
            ctx.chat(format!(
                "/{} {}  Usage: {}quote all <username>",
                ctx.runtime.whisper_command, ctx.sender, ctx.runtime.prefix
            ));
            return Ok(());
        }

        if query.mode == QuoteQueryMode::Server && search.trim().is_empty() {
            ctx.chat(format!(
                "/{} {}  Usage: {}quote <server> <username>",
                ctx.runtime.whisper_command, ctx.sender, ctx.runtime.prefix
            ));
            return Ok(());
        }

        let mut data = None;
        let mut resolved_server = server.to_owned();

        if query.mode == QuoteQueryMode::ServerRandom {
            data = ctx
                .state
                .api
                .get_quote(
                    "none",
                    server,
                    Some(QuoteOptions {
                        random: true,
                        phrase: None,
                    }),
                )
                .await
                .map(QuoteResult::from);
        } else if let Some(phrase) = phrase {
            if server == "all" {
                for candidate in shuffled_quote_servers() {
                    let result = matching_message(ctx.state, search, candidate, phrase).await;
                    if result.is_some() {
                        data = result;
                        resolved_server = candidate.to_owned();
                        break;
                    }
                }
            } else {
                data = matching_message(ctx.state, search, server, phrase).await;
            }
        } else if server == "all" {
            for candidate in shuffled_quote_servers() {
                let result = ctx.state.api.get_quote(search, candidate, None).await;
                if result
                    .as_ref()
                    .is_some_and(|quote| !quote.message.is_empty())
                {
                    data = result.map(QuoteResult::from);
                    resolved_server = candidate.to_owned();
                    break;
                }
            }
        } else {
            data = ctx
                .state
                .api
                .get_quote(search, server, None)
                .await
                .map(QuoteResult::from);
        }

        let Some(data) = data else {
            let server_hint = if query.mode == QuoteQueryMode::Server {
                if server == "all" {
                    " on any server".to_owned()
                } else {
                    format!(" on {server}")
                }
            } else {
                String::new()
            };
            let phrase_hint = phrase
                .map(|phrase| format!(" matching \"{phrase}\""))
                .unwrap_or_default();
            let text = if search.eq_ignore_ascii_case(ctx.sender) {
                format!(
                    "I have no quotes recorded for you{server_hint}{phrase_hint}, or unexpected error occurred."
                )
            } else {
                format!(
                    "I have no quotes recorded for {search}{server_hint}{phrase_hint}, or unexpected error occurred."
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
        let display_name = if query.mode == QuoteQueryMode::ServerRandom {
            data.name.as_deref().unwrap_or(search)
        } else {
            search
        };
        let server_label = if resolved_server != ctx.state.mc_server {
            format!(" [{resolved_server}]")
        } else {
            String::new()
        };

        ctx.chat(format!(
            " {display_name}{server_label}: {}{date}",
            data.message
        ));
        Ok(())
    })
}

#[derive(Debug, PartialEq, Eq)]
enum QuoteQueryMode {
    Default,
    Server,
    ServerRandom,
    MissingAllTarget,
}

#[derive(Debug, PartialEq, Eq)]
struct QuoteQuery {
    mode: QuoteQueryMode,
    server: String,
    search: String,
    phrase: Option<String>,
}

impl QuoteQuery {
    fn from_args(args: &[&str], sender: &str, default_server: &str) -> Self {
        let first = args.first().copied().unwrap_or("");
        let server_arg = first.to_lowercase();

        if server_arg == "all" && args.len() < 2 {
            return Self {
                mode: QuoteQueryMode::MissingAllTarget,
                server: default_server.to_owned(),
                search: sender.to_owned(),
                phrase: None,
            };
        }

        if args.len() >= 2 && (server_arg == "all" || is_quote_server(&server_arg)) {
            return Self {
                mode: QuoteQueryMode::Server,
                server: server_arg,
                search: args[1].to_owned(),
                phrase: phrase_from_args(&args[2..]),
            };
        }

        if args.len() == 1 && is_quote_server(&server_arg) {
            return Self {
                mode: QuoteQueryMode::ServerRandom,
                server: server_arg,
                search: "none".to_owned(),
                phrase: None,
            };
        }

        Self {
            mode: QuoteQueryMode::Default,
            server: default_server.to_owned(),
            search: first_or_sender(args, sender).to_owned(),
            phrase: phrase_from_args(args.get(1..).unwrap_or_default()),
        }
    }
}

fn first_or_sender<'a>(args: &'a [&str], sender: &'a str) -> &'a str {
    args.first().copied().unwrap_or(sender)
}

fn phrase_from_args(args: &[&str]) -> Option<String> {
    let phrase = args.join(" ");
    (!phrase.trim().is_empty()).then_some(phrase)
}

struct QuoteResult {
    name: Option<String>,
    message: String,
    date: Option<String>,
}

impl From<crate::structure::endpoints::endpoints::Quote> for QuoteResult {
    fn from(value: crate::structure::endpoints::endpoints::Quote) -> Self {
        Self {
            name: Some(value.name),
            message: value.message,
            date: value.date,
        }
    }
}

impl From<MinecraftChatMessage> for QuoteResult {
    fn from(value: MinecraftChatMessage) -> Self {
        Self {
            name: Some(value.name),
            message: value.message,
            date: Some(value.date),
        }
    }
}

async fn matching_message(
    state: &crate::structure::mineflayer::bot::AzaleaState,
    search: &str,
    server: &str,
    phrase: &str,
) -> Option<QuoteResult> {
    let phrase = phrase.to_ascii_lowercase();
    let mut matches = state
        .api
        .get_messages(search, server, KEYWORD_MESSAGE_FETCH_LIMIT, "DESC", 0)
        .await?
        .into_iter()
        .filter(|message| message.message.to_ascii_lowercase().contains(&phrase))
        .collect::<Vec<_>>();

    if matches.is_empty() {
        return None;
    }

    let index = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as usize)
        .unwrap_or_default())
        % matches.len();
    Some(matches.swap_remove(index).into())
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

#[cfg(test)]
mod tests {
    use super::{QuoteQuery, QuoteQueryMode};

    #[test]
    fn parses_username_keyword_query_without_server() {
        assert_eq!(
            QuoteQuery::from_args(&["JollyCurve_", "diamond"], "Sender", "vanilla"),
            QuoteQuery {
                mode: QuoteQueryMode::Default,
                server: "vanilla".to_owned(),
                search: "JollyCurve_".to_owned(),
                phrase: Some("diamond".to_owned()),
            }
        );
    }

    #[test]
    fn parses_multi_word_keyword_query() {
        assert_eq!(
            QuoteQuery::from_args(&["JollyCurve_", "diamond", "ore"], "Sender", "vanilla")
                .phrase
                .as_deref(),
            Some("diamond ore")
        );
    }

    #[test]
    fn preserves_server_username_query() {
        assert_eq!(
            QuoteQuery::from_args(
                &["refinedvanilla", "JollyCurve_", "diamond"],
                "Sender",
                "fallback"
            ),
            QuoteQuery {
                mode: QuoteQueryMode::Server,
                server: "refinedvanilla".to_owned(),
                search: "JollyCurve_".to_owned(),
                phrase: Some("diamond".to_owned()),
            }
        );
    }

    #[test]
    fn parses_single_server_as_random_quote_from_that_server() {
        assert_eq!(
            QuoteQuery::from_args(&["refinedvanilla"], "Sender", "fallback"),
            QuoteQuery {
                mode: QuoteQueryMode::ServerRandom,
                server: "refinedvanilla".to_owned(),
                search: "none".to_owned(),
                phrase: None,
            }
        );
    }

    #[test]
    fn preserves_all_server_usage_validation() {
        assert_eq!(
            QuoteQuery::from_args(&["all"], "Sender", "vanilla").mode,
            QuoteQueryMode::MissingAllTarget
        );
    }
}
