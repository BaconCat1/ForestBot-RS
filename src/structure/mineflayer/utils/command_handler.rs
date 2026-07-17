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
        logger::command(format!("Unknown command: {command_name}"));
        return;
    };

    if !command_enabled(&runtime, command, command_name) {
        logger::command(format!("Command disabled: {command_name}"));
        enqueue_command_whisper(
            state,
            &runtime,
            sender,
            format!("Sorry, {sender}, that command is disabled."),
        );
        return;
    }

    if !is_allowed_by_standing(&runtime, sender, state, content) {
        logger::command(format!(
            "Command blocked by blacklist: {command_name} from {sender}"
        ));
        return;
    }

    if command.whitelisted && !is_allowed_whitelisted_command(&runtime, sender, state) {
        logger::command(format!(
            "Command blocked by whitelist: {command_name} from {sender}"
        ));
        return;
    }

    // !poll <N> (vote) shares the "poll" alias with !poll <question?> opt1, opt2 (create),
    // but only creation should be cooldown-gated — voting must stay unlimited. Peek the
    // not-yet-consumed args the same way poll.rs itself distinguishes the two forms.
    let mut poll_vote_peek = parts.clone();
    let is_poll_vote = command.names.contains(&"poll")
        && matches!(
            (poll_vote_peek.next(), poll_vote_peek.next()),
            (Some(arg), None) if arg.parse::<usize>().is_ok()
        );

    let cooldown_remaining = if is_poll_vote {
        None
    } else {
        command_cooldown_remaining(state, &runtime, command, command_name, sender)
    };

    if let Some(remaining) = cooldown_remaining {
        logger::command(format!(
            "Command blocked by cooldown: {command_name} from {sender}, {remaining}s remaining"
        ));
        enqueue_command_whisper(
            state,
            &runtime,
            sender,
            format!("Commands are on cooldown. Try again in {remaining}s."),
        );
        return;
    }

    logger::command(format!("Executing command: {command_name} from {sender}"));

    let canonical_name = command.names.first().copied().unwrap_or(command_name);
    let ctx = CommandContext {
        bot,
        state,
        runtime: &runtime,
        sender,
        args: parts.collect(),
        reply_as_whisper,
        command_name: canonical_name,
    };

    if let Err(error) = (command.execute)(ctx).await {
        logger::command(format!("Command {command_name} failed: {error:#}"));
        enqueue_command_whisper(state, &runtime, sender, "Command failed.");
    }
}

fn enqueue_command_whisper(
    state: &AzaleaState,
    runtime: &RuntimeConfig,
    sender: &str,
    message: impl AsRef<str>,
) {
    enqueue_chat(
        state,
        format!(
            "/{} {} {}",
            runtime.whisper_command,
            sender,
            message.as_ref()
        ),
    );
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

    let Some(policy) = command_cooldown_config(runtime, command, invoked_alias) else {
        if runtime.anti_spam_global_cooldown_ms > 0 {
            *last_command_at = Some(now);
        }
        return None;
    };

    if policy.cooldown_ms == 0 {
        if runtime.anti_spam_global_cooldown_ms > 0 {
            *last_command_at = Some(now);
        }
        return None;
    }

    let command_key = command.names.first().copied().unwrap_or(invoked_alias);
    let cooldown_key = format!("{}\u{0}{}", sender.to_ascii_lowercase(), command_key);
    if let Some(state) = player_command_cooldowns.get_mut(&cooldown_key) {
        let elapsed = now.duration_since(state.last_success_at);
        let reset_after = reset_after_duration(state.cooldown_ms, policy.reset_multiplier);
        let should_increase = elapsed < reset_after;

        if elapsed < Duration::from_millis(state.cooldown_ms) {
            if policy.increment_ms > 0 {
                state.cooldown_ms = increased_cooldown(state.cooldown_ms, policy);
            }
            return Some(duration_seconds(
                Duration::from_millis(state.cooldown_ms) - elapsed,
            ));
        }

        state.cooldown_ms = if should_increase {
            increased_cooldown(state.cooldown_ms, policy)
        } else {
            policy.cooldown_ms
        };
        state.last_success_at = now;
        if runtime.anti_spam_global_cooldown_ms > 0 {
            *last_command_at = Some(now);
        }
        return None;
    }

    player_command_cooldowns.insert(
        cooldown_key,
        PlayerCommandCooldown {
            last_success_at: now,
            cooldown_ms: policy.cooldown_ms,
        },
    );
    if runtime.anti_spam_global_cooldown_ms > 0 {
        *last_command_at = Some(now);
    }
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
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(sender))
        .map(|(_, player)| player.uuid.clone());
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
) -> bool {
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
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(sender))
        .map(|(_, player)| player.uuid.clone());
    uuid.is_some_and(|uuid| runtime.user_whitelist.contains(&uuid))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structure::mineflayer::bot::PlayerSnapshot;
    use uuid::Uuid;

    fn make_state_with_player(profile_name: &str, uuid: &str) -> AzaleaState {
        let state = AzaleaState::default();
        state.players.write().unwrap().insert(
            profile_name.to_owned(),
            PlayerSnapshot {
                username: profile_name.to_owned(),
                uuid: uuid.to_owned(),
                entity_uuid: Uuid::nil(),
                latency: 0,
                display_name: None,
            },
        );
        state
    }

    fn make_runtime(whitelist: &[&str], blacklist: &[&str], use_whitelist: bool) -> RuntimeConfig {
        let base = AzaleaState::default().runtime.read().unwrap().clone();
        RuntimeConfig {
            use_whitelist,
            user_whitelist: whitelist.iter().map(|s| s.to_string()).collect(),
            user_blacklist: blacklist.iter().map(|s| s.to_string()).collect(),
            ..base
        }
    }

    const UUID: &str = "550e8400-e29b-41d4-a716-446655440000";

    #[test]
    fn blacklist_blocks_exact_case() {
        let state = make_state_with_player("Player1", UUID);
        let runtime = make_runtime(&[], &[UUID], false);
        assert!(!is_allowed_by_standing(&runtime, "Player1", &state, "!cmd"));
    }

    #[test]
    fn blacklist_blocks_case_insensitive_sender() {
        let state = make_state_with_player("Player1", UUID);
        let runtime = make_runtime(&[], &[UUID], false);
        assert!(!is_allowed_by_standing(&runtime, "player1", &state, "!cmd"));
    }

    #[test]
    fn blacklist_allows_unknown_sender() {
        let state = AzaleaState::default();
        let runtime = make_runtime(&[], &[UUID], false);
        assert!(is_allowed_by_standing(&runtime, "ghost", &state, "!cmd"));
    }

    #[test]
    fn blacklist_allows_non_blacklisted_player() {
        let state = make_state_with_player("Player1", "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
        let runtime = make_runtime(&[], &[UUID], false);
        assert!(is_allowed_by_standing(&runtime, "Player1", &state, "!cmd"));
    }

    #[test]
    fn whitelist_allows_exact_case() {
        let state = make_state_with_player("Admin", UUID);
        let runtime = make_runtime(&[UUID], &[], true);
        assert!(is_allowed_whitelisted_command(&runtime, "Admin", &state));
    }

    #[test]
    fn whitelist_allows_case_insensitive_sender() {
        let state = make_state_with_player("Admin", UUID);
        let runtime = make_runtime(&[UUID], &[], true);
        assert!(is_allowed_whitelisted_command(&runtime, "admin", &state));
    }

    #[test]
    fn whitelist_blocks_non_whitelisted_player() {
        let state = make_state_with_player("Stranger", "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
        let runtime = make_runtime(&[UUID], &[], true);
        assert!(!is_allowed_whitelisted_command(&runtime, "Stranger", &state));
    }

    #[test]
    fn whitelist_blocks_unknown_sender() {
        let state = AzaleaState::default();
        let runtime = make_runtime(&[UUID], &[], true);
        assert!(!is_allowed_whitelisted_command(&runtime, "ghost", &state));
    }

    #[test]
    fn whitelist_disabled_allows_everyone() {
        let state = AzaleaState::default();
        let runtime = make_runtime(&[], &[], false);
        assert!(is_allowed_whitelisted_command(&runtime, "anyone", &state));
    }
}
