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
        welcome_messages: app_state.config.welcome_messages,
        use_custom_chat_prefix: app_state.config.use_custom_chat_prefix,
        custom_chat_prefix: app_state.config.custom_chat_prefix,
        smart_censoring: app_state.config.smart_censoring,
        together_api_key: app_state.config.api_keys.together,
        wolfram_app_id: app_state.config.api_keys.wolfram,
        azure_translator_key: app_state.config.api_keys.azure_key,
        azure_translator_region: app_state.config.api_keys.azure_region,
        sharpapi_key: app_state.config.api_keys.sharpapi,
        nasa_api_key: app_state.config.api_keys.nasa,
        airnow_api_key: app_state.config.api_keys.airnow,
        gasbuddy_solver_url: app_state.config.api_keys.gasbuddy_solver_url,
        gasbuddy_csrf_readonly: app_state.config.api_keys.gasbuddy_csrf_readonly,
    };

    *state.runtime.write().expect("runtime config lock poisoned") = reloaded;
    Ok(())
}
