use crate::commands::{CommandContext, CommandDefinition, CommandFuture};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["greeting"],
    description: "Set a personal welcome message. Usage: {prefix}greeting <message> | preview | clear",
    whitelisted: false,
    bridge_ok: true,
    execute: execute,
};

const MAX_LEN: usize = 200;

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let api = ctx.state.api.clone();
        let username = ctx.sender;

        match ctx.args.as_slice() {
            [] => {
                // Show format explanation + current greeting
                ctx.whisper(format!(
                    "Greetings fire when you join (12h cooldown). Format: \"<your message>, {username}!\" \
                     Set with: {}greeting <message> | preview: {}greeting preview | remove: {}greeting clear",
                    ctx.runtime.prefix, ctx.runtime.prefix, ctx.runtime.prefix
                ));
                if let Some((Some(g), _)) = api.tradebot_get_greeting(username).await {
                    ctx.whisper(format!("Your current greeting: \"{g}\""));
                } else {
                    ctx.whisper("You have no greeting set.".to_owned());
                }
            }

            ["preview"] => {
                match api.tradebot_get_greeting(username).await {
                    Some((Some(g), _)) => {
                        ctx.whisper(format!("{g}, {username}!"));
                    }
                    _ => {
                        ctx.whisper("You have no greeting set.".to_owned());
                    }
                }
            }

            ["clear"] => {
                if api.tradebot_set_greeting(username, None).await {
                    ctx.whisper("Greeting cleared.".to_owned());
                } else {
                    ctx.whisper("Failed to clear greeting.".to_owned());
                }
            }

            args => {
                let message = args.join(" ");
                if !message.is_ascii() {
                    ctx.whisper("Greetings must use plain ASCII characters only.".to_owned());
                    return Ok(());
                }
                if message.chars().count() > MAX_LEN {
                    ctx.whisper(format!("Greeting too long ({} chars max).", MAX_LEN));
                    return Ok(());
                }
                if api.tradebot_set_greeting(username, Some(&message)).await {
                    ctx.whisper(format!(
                        "Greeting set. Preview: \"{message}, {username}!\""
                    ));
                    crate::commands::utils::flag_content_if_needed(ctx.state, username, "greeting", &message);
                } else {
                    ctx.whisper("Failed to set greeting.".to_owned());
                }
            }
        }

        Ok(())
    })
}
