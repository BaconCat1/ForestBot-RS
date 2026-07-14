use super::admin_command;
use super::helpers::word_list_command;
use crate::commands::{CommandContext, CommandFuture};

const BAD_WORDS_PATH: &str = "./json/bad_words.json";

admin_command!(CENSOR_COMMAND, &["censor"], "Manage bad-words list. Usage: {prefix}censor add <word> | {prefix}censor remove <word>", censor);

fn censor(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    word_list_command(ctx, BAD_WORDS_PATH, "bad words")
}
