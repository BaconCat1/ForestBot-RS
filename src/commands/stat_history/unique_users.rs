use super::command;
use crate::commands::{CommandContext, CommandFuture};

command!(
    UNIQUE_USERS_COMMAND,
    &["users", "uniqueusers"],
    "Shows the unique user count. Usage: {prefix}users",
    unique_users
);

fn unique_users(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let users = ctx.state.api.get_unique_users(&ctx.state.mc_server).await;
        let count = users.map(|users| users.len()).unwrap_or_default();
        ctx.chat(format!(
            " I have seen {count} different users on this server! api.forestbot.org/unique-users?server={}",
            ctx.state.mc_server
        ));
        Ok(())
    })
}
