pub const NAMES: &[&str] = &["help", "commands"];
pub const RESPONSE: &str = "See all commands: tiny.cc/forcoms";

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: NAMES,
    description: "See all commands, or {prefix}help <command> for details on one command.",
    whitelisted: false,
    bridge_ok: true,
    execute,
};

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        if let Some(query) = ctx.args.first() {
            match crate::commands::find(query) {
                Some(cmd) => {
                    let desc = cmd.description.replace("{prefix}", &ctx.runtime.prefix);
                    let aliases: Vec<String> = cmd
                        .names
                        .iter()
                        .copied()
                        .skip(1)
                        .map(|a| format!("{}{}", ctx.runtime.prefix, a))
                        .collect();
                    if aliases.is_empty() {
                        ctx.whisper(format!("{}{}: {}", ctx.runtime.prefix, cmd.names[0], desc));
                    } else {
                        ctx.whisper(format!(
                            "{}{} (aliases: {}): {}",
                            ctx.runtime.prefix,
                            cmd.names[0],
                            aliases.join(", "),
                            desc
                        ));
                    }
                }
                None => {
                    ctx.whisper(format!(
                        "Unknown command: {}{}. {}",
                        ctx.runtime.prefix, query, RESPONSE
                    ));
                }
            }
            return Ok(());
        }
        ctx.chat(RESPONSE);
        Ok(())
    })
}
