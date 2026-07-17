use super::helpers::whisper;
use crate::commands::{CommandContext, CommandFuture};
use serde::Deserialize;

command!(OLDNAMES_COMMAND, &["oldnames", "dox", "doxx"], "Shows a user's name history. Usage: {prefix}oldnames <username>", oldnames);

fn oldnames(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let target = ctx.args.first().copied().unwrap_or(ctx.sender);
        let url = format!(
            "https://api.crafty.gg/api/v2/players/{}",
            percent_encode_path_segment(target)
        );
        let response = reqwest::get(url).await;
        let Ok(response) = response else {
            ctx.chat(" An error occured while trying to look up the user.");
            return Ok(());
        };
        if response.status().as_u16() == 404 {
            whisper(&ctx, " Could not find the user you were looking for.");
            return Ok(());
        }
        if !response.status().is_success() {
            ctx.chat(" An error occured while trying to look up the user.");
            return Ok(());
        }
        let profile = response.json::<CraftyPlayerResponse>().await.ok();
        let mut names = profile
            .and_then(|p| p.data)
            .map(|d| d.usernames)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|entry| entry.username)
            .filter(|name| name != "1HateN1ggers" && name != "ShriviledP3ck3r")
            .collect::<Vec<_>>();
        names.dedup();
        if names.is_empty() {
            ctx.chat(" No name history was found for that user.");
        } else {
            ctx.chat_success(format!(
                " {target} has used the following names: {}",
                names.join(", ")
            ));
        }
        Ok(())
    })
}

fn percent_encode_path_segment(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

#[derive(Debug, Deserialize)]
struct CraftyPlayerResponse {
    data: Option<CraftyPlayerData>,
}

#[derive(Debug, Deserialize)]
struct CraftyPlayerData {
    #[serde(default)]
    usernames: Vec<CraftyUsername>,
}

#[derive(Debug, Deserialize)]
struct CraftyUsername {
    username: Option<String>,
}
