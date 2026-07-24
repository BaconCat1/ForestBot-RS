use super::helpers::{epoch_ms_from_string, excluded_usernames, whisper};
use crate::commands::{CommandContext, CommandFuture};
use crate::functions::utils::time;
use crate::structure::endpoints::endpoints::QuoteOptions;

/// Hub picks one random row server-side per call, so unlike the batch stat
/// commands there's no result set to filter -- retry a few times instead if
/// an excluded account's quote comes back. Small cap since a tiny exclusion
/// list against a real quote corpus should essentially never exhaust this.
const EXCLUDED_RETRY_ATTEMPTS: u32 = 8;

command!(RANDOM_QUOTE_COMMAND, &["rq", "randomquote"], "Retrieves a random quote. Usage: {prefix}rq <phrase>(optional)", random_quote);

fn random_quote(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let phrase = ctx.args.first().map(|s| (*s).to_owned());
        let excluded = excluded_usernames(&ctx).await;
        let mut data = None;
        for _ in 0..EXCLUDED_RETRY_ATTEMPTS {
            let attempt = ctx
                .state
                .api
                .get_quote(
                    "none",
                    &ctx.state.mc_server,
                    Some(QuoteOptions {
                        random: true,
                        phrase: phrase.clone(),
                    }),
                )
                .await;
            match &attempt {
                Some(quote) if excluded.contains(&quote.name.to_lowercase()) => continue,
                _ => {
                    data = attempt;
                    break;
                }
            }
        }
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
