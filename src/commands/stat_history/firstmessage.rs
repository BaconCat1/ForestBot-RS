use super::patterns::message_lookup;
use crate::commands::{CommandContext, CommandFuture};

command!(
    FIRST_MESSAGE_COMMAND,
    &["firstmessage", "fm"],
    "Retrieves the first message of a user. Usage: {prefix}firstmessage <username>",
    first_message
);

fn first_message(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    message_lookup(ctx, "ASC")
}
