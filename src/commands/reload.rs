pub const NAMES: &[&str] = &["reload", "reloadconfig"];

use std::collections::HashSet;

use crate::{
    commands::{CommandContext, CommandDefinition, CommandFuture},
    config::AppState,
    structure::mineflayer::bot::RuntimeConfig,
};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: NAMES,
    description: "Reloads config and whitelist/blacklist files. Usage: {prefix}reload",
    whitelisted: true,
    execute,
};

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        reload_runtime(ctx.state).await?;
        ctx.chat_success(format!(
            "/{} {} {}",
            ctx.runtime.whisper_command,
            ctx.sender,
            response()
        ));
        Ok(())
    })
}

pub fn response() -> &'static str {
    " Config reloaded."
}

async fn reload_runtime(
    state: &crate::structure::mineflayer::bot::AzaleaState,
) -> anyhow::Result<()> {
    let app_state = AppState::load().await?;

    // Rebuild AI providers from the fresh api_keys so !reload actually picks up key changes.
    // Must happen before RuntimeConfig below, which moves individual fields out of api_keys.
    let ai_providers = crate::commands::ai::load_ai_providers(
        "json/ai_providers.json",
        &app_state.config.api_keys,
    )
    .await;

    let reloaded = RuntimeConfig {
        prefix: app_state.config.prefix,
        whisper_command: app_state.config.whisper_command,
        use_commands: app_state.config.use_commands,
        use_whitelist: app_state.config.use_mc_whitelist,
        user_whitelist: app_state.mc_whitelist.into_iter().collect::<HashSet<_>>(),
        user_blacklist: app_state.mc_blacklist.into_iter().collect::<HashSet<_>>(),
        custom_chat_formats: if app_state.config.use_custom_chat_format_parser {
            app_state.config.custom_chat_formats
        } else {
            Vec::new()
        },
        command_toggles: app_state.config.commands,
        disabled_events: app_state
            .config
            .disabled_events
            .into_iter()
            .collect::<HashSet<_>>(),
        discord_bridge: app_state.config.discord_bridge.clone(),
        use_live_time_query: app_state.config.use_live_time_query,
        day_night_game_time_fallback: app_state.config.day_night_game_time_fallback,
        welcome_messages: app_state.config.welcome_messages,
        use_custom_chat_prefix: app_state.config.use_custom_chat_prefix,
        custom_chat_prefix: app_state.config.custom_chat_prefix,
        censorship: app_state.config.censorship.clone(),
        command_censorship: app_state.command_censorship,
        bet_limits: app_state.bet_limits,
        api_keys: app_state.config.api_keys.clone(),
        anti_spam: app_state.config.anti_spam.clone(),
        connection: app_state.config.connection.clone(),
        misc_timing: app_state.config.misc_timing.clone(),
        detection: app_state.config.detection.clone(),
        player_economy: app_state.config.player_economy.clone(),
        translate: app_state.config.translate.clone(),
        casino: app_state.config.casino.clone(),
    };

    *state.runtime.write().expect("runtime config lock poisoned") = reloaded;
    *state.ai_providers.write().expect("ai_providers lock poisoned") = ai_providers;
    state.ai_model_cache.lock().expect("ai_model_cache lock poisoned").clear();

    // Pick up a casino_deck_count config edit without a full restart -- forces
    // both table shoes to reshuffle at the new size on their next deal.
    crate::commands::casino::shoe::set_deck_count(&state.blackjack_shoe, app_state.config.casino_deck_count);
    crate::commands::casino::shoe::set_deck_count(&state.baccarat_shoe, app_state.config.casino_deck_count);

    // Unlike a plain OnceLock, this re-reads debug.json and overwrites the live
    // categories -- so flipping a category off actually takes effect on !reload.
    crate::structure::logger::load_debug_categories();

    // Reload bridge command classification so edits to json/bridge_unsafe_commands.json
    // take effect without a restart: refresh the local dispatch-time copy synchronously
    // (so the very next bridged command sees it), then re-push to Hub in the background.
    {
        let unsafe_names = crate::commands::load_bridge_unsafe_commands(
            "json/bridge_unsafe_commands.json",
        )
        .await;
        *state
            .bridge_unsafe_commands
            .write()
            .expect("bridge_unsafe_commands lock poisoned") = unsafe_names.clone();

        let push_state = state.clone();
        tokio::spawn(async move {
            let list = crate::commands::build_bridge_command_list(&unsafe_names);
            push_state.api.push_bridge_commands(&list).await;
        });
    }

    // Rebuild URL blocklist in background
    {
        let blocklist_arc = state.url_blocklist.clone();
        let sources = app_state.config.url_blocklist_sources;
        let whitelist = app_state.config.url_whitelist_file;
        let blocklist_timeout_ms = app_state.config.url_blocklist_timeout_ms;
        tokio::spawn(async move {
            let set = crate::structure::mineflayer::url_blocklist::build_blocklist(&sources, &whitelist, blocklist_timeout_ms).await;
            *blocklist_arc.write().expect("url_blocklist write") = Some(set);
        });
    }

    // Rebuild profanity trie so hand-edits to bad_words.json/word_whitelist.json (made
    // outside of !censor/!wordwhitelist) take effect without a restart.
    crate::structure::mineflayer::utils::profanity_filter::rebuild(state).await;

    Ok(())
}
