use azalea::prelude::Client;

use crate::{
    commands::{self, CommandContext, CommandDefinition},
    structure::{
        logger,
        mineflayer::bot::{AzaleaState, RuntimeConfig},
    },
};

pub async fn handle(bot: &Client, state: &AzaleaState, sender: &str, content: &str) {
    let runtime = state
        .runtime
        .read()
        .expect("runtime config lock poisoned")
        .clone();

    let Some(command_line) = content.trim().strip_prefix(&runtime.prefix) else {
        return;
    };

    let mut parts = command_line.split_whitespace();
    let Some(command_name) = parts.next() else {
        return;
    };

    let Some(command) = commands::find(command_name) else {
        logger::info(format!("Unknown command: {command_name}"));
        return;
    };

    if !command_enabled(&runtime, command) {
        logger::info(format!("Command disabled: {command_name}"));
        return;
    }

    if !is_allowed_by_standing(&runtime, sender, &state, content) {
        logger::info(format!(
            "Command blocked by blacklist: {command_name} from {sender}"
        ));
        return;
    }

    if command.whitelisted && !is_allowed_whitelisted_command(&runtime, sender, &state, command) {
        logger::info(format!(
            "Command blocked by whitelist: {command_name} from {sender}"
        ));
        return;
    }

    logger::info(format!("Executing command: {command_name} from {sender}"));

    let ctx = CommandContext {
        bot,
        state,
        runtime: &runtime,
        sender,
        args: parts.collect(),
    };

    if let Err(error) = (command.execute)(ctx).await {
        logger::warn(format!("Command {command_name} failed: {error:#}"));
        bot.chat(&format!(
            "/{} {sender} Command failed.",
            runtime.whisper_command
        ));
    }
}

fn is_allowed_by_standing(
    runtime: &RuntimeConfig,
    sender: &str,
    state: &AzaleaState,
    content: &str,
) -> bool {
    let uuid = state
        .players
        .read()
        .expect("player cache lock poisoned")
        .get(sender)
        .map(|player| player.uuid.clone());
    let Some(uuid) = uuid else {
        return true;
    };

    !runtime.user_blacklist.contains(&uuid)
        || crate::structure::mineflayer::utils::whisper_parser::is_self_standing_command(
            content,
            &runtime.prefix,
        )
}

fn command_enabled(runtime: &RuntimeConfig, command: &CommandDefinition) -> bool {
    command
        .names
        .iter()
        .any(|name| runtime.command_toggles.get(*name).copied().unwrap_or(true))
}

fn is_allowed_whitelisted_command(
    runtime: &RuntimeConfig,
    sender: &str,
    state: &AzaleaState,
    command: &CommandDefinition,
) -> bool {
    if command
        .names
        .iter()
        .any(|name| runtime.whitelisted_commands.contains(*name))
    {
        return true;
    }

    if !runtime.use_whitelist
        || runtime.user_whitelist.contains(sender)
        || runtime
            .user_whitelist
            .iter()
            .any(|username| username.eq_ignore_ascii_case(sender))
    {
        return true;
    }

    let uuid = state
        .players
        .read()
        .expect("player cache lock poisoned")
        .get(sender)
        .map(|player| player.uuid.clone());
    uuid.is_some_and(|uuid| runtime.user_whitelist.contains(&uuid))
}
