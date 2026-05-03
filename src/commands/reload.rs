pub const NAMES: &[&str] = &["reload", "reloadconfig"];

use std::collections::HashSet;

use crate::{
    commands::{CommandContext, CommandDefinition, CommandFuture},
    config::AppState,
    structure::mineflayer::bot::RuntimeConfig,
};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: NAMES,
    whitelisted: true,
    execute,
};

pub fn execute<'a>(ctx: CommandContext<'a>) -> CommandFuture<'a> {
    Box::pin(async move {
        reload_runtime(ctx.state).await?;
        ctx.bot.chat(&format!(
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
        use_whitelist: app_state.config.use_mc_whitelist,
        user_whitelist: app_state.mc_whitelist.into_iter().collect::<HashSet<_>>(),
        custom_chat_formats: app_state.config.custom_chat_formats,
        command_toggles: app_state.config.commands,
        whitelisted_commands: app_state
            .config
            .whitelisted_commands
            .into_iter()
            .collect::<HashSet<_>>(),
    };

    *state.runtime.write().expect("runtime config lock poisoned") = reloaded;
    Ok(())
}
