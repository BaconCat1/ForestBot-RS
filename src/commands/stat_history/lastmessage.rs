use super::command;
use super::helpers::message_lookup;
use crate::commands::{CommandContext, CommandFuture};

command!(LAST_MESSAGE_COMMAND, &["lastmessage", "lm"], "Retrieves the last message of a user. Usage: {prefix}lastmessage <username>", last_message);

fn last_message(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    message_lookup(ctx, "DESC")
}
