use azalea::prelude::Client;
use std::time::{Duration, Instant};

use crate::{
    commands::{self, CommandContext, CommandDefinition, enqueue_chat},
    config::CommandCooldownConfig,
    structure::{
        logger,
        mineflayer::bot::{AzaleaState, PlayerCommandCooldown, RuntimeConfig},
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

    if let Some(remaining) =
        command_cooldown_remaining(state, &runtime, command, command_name, sender)
    {
        logger::info(format!(
            "Command blocked by cooldown: {command_name} from {sender}, {remaining}s remaining"
        ));
        enqueue_chat(
            state,
            &format!(
                "/{} {sender} Commands are on cooldown. Try again in {remaining}s.",
                runtime.whisper_command
            ),
        );
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

fn command_cooldown_remaining(
    state: &AzaleaState,
    runtime: &RuntimeConfig,
    command: &CommandDefinition,
    invoked_alias: &str,
    sender: &str,
) -> Option<u64> {
    let now = Instant::now();

    let mut last_command_at = state
        .last_command_at
        .lock()
        .expect("global command cooldown lock poisoned");
    let mut player_command_cooldowns = state
        .player_command_cooldowns
        .lock()
        .expect("player command cooldown lock poisoned");

    if let Some(remaining) =
        remaining_seconds(*last_command_at, now, runtime.anti_spam_global_cooldown_ms)
    {
        return Some(remaining);
    }

    if runtime.anti_spam_global_cooldown_ms > 0 {
        *last_command_at = Some(now);
    }

    let Some(policy) = command_cooldown_config(runtime, command, invoked_alias) else {
        return None;
    };

    if policy.cooldown_ms == 0 {
        return None;
    }

    let command_key = command.names.first().copied().unwrap_or(invoked_alias);
    let cooldown_key = format!("{}\u{0}{}", sender.to_ascii_lowercase(), command_key);
    if let Some(state) = player_command_cooldowns.get_mut(&cooldown_key) {
        let elapsed = now.duration_since(state.last_attempt_at);
        let reset_after = reset_after_duration(state.cooldown_ms, policy.reset_multiplier);
        let should_increase = elapsed < reset_after;

        if elapsed < Duration::from_millis(state.cooldown_ms) {
            if should_increase {
                state.cooldown_ms = increased_cooldown(state.cooldown_ms, policy);
                state.last_attempt_at = now;
            }
            return Some(duration_seconds(Duration::from_millis(state.cooldown_ms)));
        }

        state.cooldown_ms = if should_increase {
            increased_cooldown(state.cooldown_ms, policy)
        } else {
            policy.cooldown_ms
        };
        state.last_attempt_at = now;
        return None;
    }

    player_command_cooldowns.insert(
        cooldown_key,
        PlayerCommandCooldown {
            last_attempt_at: now,
            cooldown_ms: policy.cooldown_ms,
        },
    );
    None
}

fn command_cooldown_config<'a>(
    runtime: &'a RuntimeConfig,
    command: &CommandDefinition,
    invoked_alias: &str,
) -> Option<&'a CommandCooldownConfig> {
    std::iter::once(invoked_alias)
        .chain(command.names.iter().copied())
        .find_map(|alias| runtime.command_cooldowns.get(&alias.to_ascii_lowercase()))
}

fn increased_cooldown(current_ms: u64, policy: &CommandCooldownConfig) -> u64 {
    let increased = current_ms.saturating_add(policy.increment_ms);
    if policy.max_cooldown_ms > 0 {
        increased.min(policy.max_cooldown_ms)
    } else {
        increased
    }
}

fn reset_after_duration(cooldown_ms: u64, reset_multiplier: u64) -> Duration {
    Duration::from_millis(cooldown_ms.saturating_mul(reset_multiplier.max(1)))
}

fn remaining_seconds(last: Option<Instant>, now: Instant, cooldown_ms: u64) -> Option<u64> {
    let elapsed = now.duration_since(last?);
    let cooldown = Duration::from_millis(cooldown_ms);
    if elapsed >= cooldown {
        return None;
    }

    Some(duration_seconds(cooldown - elapsed))
}

fn duration_seconds(duration: Duration) -> u64 {
    duration.as_millis().div_ceil(1_000) as u64
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
