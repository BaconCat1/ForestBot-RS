use super::command;
use super::helpers::{epoch_ms_from_string, now_millis, whisper};
use crate::commands::{CommandContext, CommandFuture};
use crate::functions::utils::time;
use crate::structure::endpoints::endpoints::QuoteOptions;

command!(
    RANDOM_QUOTE_ALL_COMMAND,
    &["rqa", "randomquoteall"],
    "Retrieves a random quote from all servers. Usage: {prefix}rqa",
    random_quote_all
);

fn random_quote_all(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let phrase = ctx.args.first().map(|s| (*s).to_owned());
        let servers = crate::constants::quote_servers::QUOTE_SERVERS;
        let server = servers[(now_millis() as usize) % servers.len()];
        let data = ctx
            .state
            .api
            .get_quote(
                "none",
                server,
                Some(QuoteOptions {
                    random: true,
                    phrase: phrase.clone(),
                }),
            )
            .await;
        let Some(data) = data else {
            let phrase_label = phrase
                .as_deref()
                .map(|phrase| format!(" for \"{phrase}\""))
                .unwrap_or_default();
            whisper(
                &ctx,
                &format!(" No quotes found{phrase_label} on {server}."),
            );
            return Ok(());
        };
        let date = data
            .date
            .as_deref()
            .and_then(epoch_ms_from_string)
            .map(time::time_ago_str)
            .map(|date| format!(" ({date})"))
            .unwrap_or_default();
        ctx.chat(format!(
            " Quote from {} [{}]: \"{}\"{}",
            data.name, server, data.message, date
        ));
        Ok(())
    })
}
