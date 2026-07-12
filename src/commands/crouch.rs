pub const NAMES: &[&str] = &["crouch"];

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: NAMES,
    description: "Crouches down. Usage: {prefix}crouch or {prefix}crouch hold",
    whitelisted: false,
    execute,
};

static HOLD_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        use azalea::protocol::packets::game::s_player_command::{Action, ServerboundPlayerCommand};

        if crate::commands::stat_history::BOT_SLEEPING.load(std::sync::atomic::Ordering::Relaxed) {
            ctx.bot.write_packet(ServerboundPlayerCommand {
                id: ctx.bot.minecraft_id(),
                action: Action::StopSleeping,
                data: 0,
            });
            crate::commands::stat_history::BOT_SLEEPING.store(false, std::sync::atomic::Ordering::Relaxed);
            return Ok(());
        }

        let is_hold = ctx.args.first().is_some_and(|a| a.eq_ignore_ascii_case("hold"));

        if HOLD_ACTIVE.load(Ordering::Relaxed) {
            HOLD_ACTIVE.store(false, Ordering::Relaxed);
            ctx.bot.set_crouching(false);
            return Ok(());
        }

        if is_hold {
            HOLD_ACTIVE.store(true, Ordering::Relaxed);
            ctx.bot.set_crouching(true);
            ctx.whisper(format!(
                "Crouching for up to 10 minutes. Run {}crouch to release.",
                ctx.runtime.prefix
            ));
            let bot = ctx.bot.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(600)).await;
                if HOLD_ACTIVE.swap(false, Ordering::Relaxed) {
                    bot.set_crouching(false);
                }
            });
        } else {
            ctx.bot.set_crouching(true);
            tokio::time::sleep(Duration::from_millis(50)).await;
            ctx.bot.set_crouching(false);
        }

        Ok(())
    })
}
