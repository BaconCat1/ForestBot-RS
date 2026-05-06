use azalea::prelude::Client;

use crate::{
    commands::{self, CommandContext, CommandDefinition, enqueue_chat},
    structure::{
        logger,
        mineflayer::bot::{AzaleaState, RuntimeConfig},
    },
};

pub async fn handle(bot: &Client, state: &AzaleaState, sender: &str, content: &str) {
    handle_with_reply_mode(bot, state, sender, content, false).await;
}

pub async fn handle_as_whisper(bot: &Client, state: &AzaleaState, sender: &str, content: &str) {
    handle_with_reply_mode(bot, state, sender, content, true).await;
}

async fn handle_with_reply_mode(
    bot: &Client,
    state: &AzaleaState,
    sender: &str,
    content: &str,
    reply_as_whisper: bool,
) {
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

    if !runtime.use_commands {
        return;
    }

    let Some(command) = commands::find(command_name) else {
        logger::info(format!("Unknown command: {command_name}"));
        return;
    };

    if !command_enabled(&runtime, command, command_name) {
        logger::info(format!("Command disabled: {command_name}"));
        enqueue_chat(
            state,
            &format!(
                "/{} {sender} Sorry, {sender}, that command is disabled.",
                runtime.whisper_command
            ),
        );
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
        reply_as_whisper,
    };

    if let Err(error) = (command.execute)(ctx).await {
        logger::warn(format!("Command {command_name} failed: {error:#}"));
        enqueue_chat(
            state,
            &format!("/{} {sender} Command failed.", runtime.whisper_command),
        );
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

fn command_enabled(
    runtime: &RuntimeConfig,
    command: &CommandDefinition,
    invoked_alias: &str,
) -> bool {
    let mut saw_true = false;
    let mut saw_any_explicit = false;

    for alias in std::iter::once(invoked_alias).chain(command.names.iter().copied()) {
        let Some(value) = read_command_toggle(runtime, alias) else {
            continue;
        };
        saw_any_explicit = true;
        if !value {
            return false;
        }
        saw_true = true;
    }

    !saw_any_explicit || saw_true
}

fn read_command_toggle(runtime: &RuntimeConfig, alias: &str) -> Option<bool> {
    let normalized = alias
        .trim()
        .trim_start_matches(&runtime.prefix)
        .to_lowercase();
    if normalized.is_empty() {
        return None;
    }

    runtime.command_toggles.iter().find_map(|(key, value)| {
        (normalize_command_key(&runtime.prefix, key) == normalized).then_some(*value)
    })
}

fn normalize_command_key(prefix: &str, key: &str) -> String {
    key.trim().trim_start_matches(prefix).to_lowercase()
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
