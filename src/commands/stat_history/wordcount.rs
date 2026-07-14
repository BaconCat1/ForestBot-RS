use super::command;
use super::helpers::whisper;
use crate::commands::{CommandContext, CommandFuture};
use crate::commands::utils::stats_target::format_server_label;

command!(
    WORDCOUNT_COMMAND,
    &["wordcount", "words", "count"],
    "Shows the number of times a user has said a word. Usage: {prefix}wordcount <username> <word> or {prefix}wordcount <server|all> <username> <word>",
    wordcount
);

fn wordcount(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let (server, search, word, has_server_arg) = match ctx.args.as_slice() {
            [server, search, word, ..] => ((*server).to_lowercase(), *search, *word, true),
            [search, word] => (ctx.state.mc_server.clone(), *search, *word, false),
            _ => {
                let p = &ctx.runtime.prefix;
                whisper(
                    &ctx,
                    &format!(" Usage: {p}wordcount <username> <word> or {p}wordcount <server|all> <username> <word>"),
                );
                return Ok(());
            }
        };
        let data = ctx
            .state
            .api
            .get_word_occurrence(search, &server, word, false)
            .await;
        let Some(data) = data else {
            let hint = if has_server_arg {
                if server == "all" {
                    " on all servers".to_owned()
                } else {
                    format!(" on {server}")
                }
            } else {
                String::new()
            };
            whisper(&ctx, &format!(" {search} has not said {word}{hint}"));
            return Ok(());
        };
        let label = format_server_label(&server, &ctx.state.mc_server);
        ctx.chat(format!(
            " {search}{label} has said {word} {} times",
            data.count
        ));
        Ok(())
    })
}
