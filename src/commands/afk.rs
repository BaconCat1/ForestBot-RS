use crate::commands::{CommandContext, CommandDefinition, CommandFuture};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["afk"],
    description: "Set an AFK message. Fires as a whisper when others mention you. Clears when you speak or log out.",
    whitelisted: false,
    bridge_ok: true,
    execute,
};

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let message = ctx.args.join(" ");
        if message.is_empty() {
            ctx.whisper("Usage: !afk <message>");
            return Ok(());
        }
        let key = ctx.sender.to_lowercase();
        ctx.state
            .afk_messages
            .write()
            .expect("afk_messages lock")
            .insert(key, message.clone());
        ctx.whisper(format!("AFK set: {message}"));
        Ok(())
    })
}
