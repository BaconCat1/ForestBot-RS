use super::helpers::{epoch_ms_from_string, whisper};
use crate::commands::{CommandContext, CommandFuture};
use crate::functions::utils::time;
use crate::structure::endpoints::endpoints::QuoteOptions;

command!(RANDOM_QUOTE_COMMAND, &["rq", "randomquote"], "Retrieves a random quote. Usage: {prefix}rq <phrase>(optional)", random_quote);

fn random_quote(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let phrase = ctx.args.first().map(|s| (*s).to_owned());
        let data = ctx
            .state
            .api
            .get_quote(
                "none",
                &ctx.state.mc_server,
                Some(QuoteOptions {
                    random: true,
                    phrase,
                }),
            )
            .await;
        if let Some(data) = data {
            let date = data
                .date
                .as_deref()
                .and_then(epoch_ms_from_string)
                .map(time::time_ago_str)
                .map(|date| format!(" ({date})"))
                .unwrap_or_default();
            ctx.chat(format!(
                " Quote from {}: \"{}\"{}",
                data.name, data.message, date
            ));
        } else {
            whisper(&ctx, " unexpected error occurred.");
        }
        Ok(())
    })
}
