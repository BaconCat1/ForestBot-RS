use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::commands::utils::stats_target::format_server_label;
use crate::constants::quote_servers;
use futures_util::stream::{self, StreamExt};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["slurcount"],
    description: "Shows how many slurs a player has used. Usage: {prefix}slurcount [server|all] <player>",
    whitelisted: false,
    execute,
};

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let (server, target) = match ctx.args.as_slice() {
            [server, target, ..] => (server.to_lowercase(), target.to_string()),
            [arg] => {
                let lower = arg.to_lowercase();
                if lower == "all" || quote_servers::is_quote_server(&lower) {
                    (lower, ctx.sender.to_owned())
                } else {
                    (ctx.state.mc_server.clone(), arg.to_string())
                }
            }
            [] => (ctx.state.mc_server.clone(), ctx.sender.to_owned()),
        };

        let slurs: Vec<String> = match load_slur_list().await {
            Ok(list) if !list.is_empty() => list,
            Ok(_) => {
                ctx.whisper("No slurs configured (slurcount_list.json is empty).");
                return Ok(());
            }
            Err(_) => {
                ctx.whisper("Failed to load slurcount_list.json.");
                return Ok(());
            }
        };

        let api = &ctx.state.api;
        let total: u64 = stream::iter(slurs.into_iter())
            .map(|slur| {
                let server = server.clone();
                let target = target.clone();
                async move {
                    api.get_word_occurrence(&target, &server, &slur, true)
                        .await
                        .map(|w| w.count)
                        .unwrap_or(0)
                }
            })
            .buffer_unordered(8)
            .fold(0u64, |acc, count| async move { acc + count })
            .await;

        let label = format_server_label(&server, &ctx.state.mc_server);
        if total == 0 {
            ctx.chat(format!("{target}{label} has no recorded slur usage."));
        } else {
            ctx.chat(format!("{target}{label} has used {total} slur(s)."));
        }

        Ok(())
    })
}

async fn load_slur_list() -> anyhow::Result<Vec<String>> {
    let data = tokio::fs::read_to_string("./json/slurcount_list.json").await?;
    Ok(serde_json::from_str(&data)?)
}
