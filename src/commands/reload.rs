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
        ctx.chat(format!(
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
        anti_spam_global_cooldown_ms: app_state.config.anti_spam_global_cooldown,
        command_cooldowns: app_state.config.command_cooldowns,
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
        allow_chatbridge_input: app_state.config.allow_chatbridge_input,
        use_live_time_query: app_state.config.use_live_time_query,
        welcome_messages: app_state.config.welcome_messages,
        use_custom_chat_prefix: app_state.config.use_custom_chat_prefix,
        custom_chat_prefix: app_state.config.custom_chat_prefix,
        smart_censoring: app_state.config.smart_censoring,
        censor_threshold: app_state.config.censor_threshold,
        command_censorship: app_state.command_censorship,
        together_api_key: app_state.config.api_keys.together,
        wolfram_app_id: app_state.config.api_keys.wolfram,
        azure_translator_key: app_state.config.api_keys.azure_key,
        azure_translator_region: app_state.config.api_keys.azure_region,
        sharpapi_key: app_state.config.api_keys.sharpapi,
        nasa_api_key: app_state.config.api_keys.nasa,
        airnow_api_key: app_state.config.api_keys.airnow,
        gasbuddy_solver_url: app_state.config.api_keys.gasbuddy_solver_url,
        gasbuddy_csrf_readonly: app_state.config.api_keys.gasbuddy_csrf_readonly,
        google_safe_browsing_key: app_state.config.api_keys.google_safe_browsing,
        queue_probe_command: app_state.config.queue_probe_command,
        queue_retry_delay_ms: app_state.config.queue_retry_delay_ms,
        board_whisper_delay_ms: app_state.config.board_whisper_delay_ms,
    };

    *state.runtime.write().expect("runtime config lock poisoned") = reloaded;
    *state.ai_providers.write().expect("ai_providers lock poisoned") = ai_providers;
    state.ai_model_cache.lock().expect("ai_model_cache lock poisoned").clear();

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
        tokio::spawn(async move {
            let set = crate::structure::mineflayer::url_blocklist::build_blocklist(&sources, &whitelist).await;
            *blocklist_arc.write().expect("url_blocklist write") = Some(set);
        });
    }

    // Rebuild profanity trie so hand-edits to bad_words.json/word_whitelist.json (made
    // outside of !censor/!wordwhitelist) take effect without a restart.
    crate::structure::mineflayer::utils::profanity_filter::rebuild(state).await;

    Ok(())
}
