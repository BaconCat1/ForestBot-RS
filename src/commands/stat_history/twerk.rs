use super::command;
use super::helpers::{now_millis, BOT_SLEEPING};
use crate::commands::{CommandContext, CommandFuture};
use crate::functions::utils::time;
use std::sync::atomic::Ordering;

command!(
    TWERK_COMMAND,
    &["twerk", "bootyshake", "booty", "dance"],
    "I will twerk for 10 seconds on your command. Usage: {prefix}twerk",
    twerk
);

fn twerk(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        use azalea::protocol::packets::game::s_player_command::{Action, ServerboundPlayerCommand};

        if BOT_SLEEPING.load(Ordering::Relaxed) {
            ctx.bot.write_packet(ServerboundPlayerCommand {
                id: ctx.bot.minecraft_id(),
                action: Action::StopSleeping,
                data: 0,
            });
            BOT_SLEEPING.store(false, Ordering::Relaxed);
            tokio::time::sleep(time::Duration::from_millis(100)).await;
        }

        let bot = ctx.bot.clone();
        tokio::spawn(async move {
            let end = now_millis().saturating_add(10_000);
            let mut state = false;
            while now_millis() < end {
                state = !state;
                bot.set_crouching(state);
                tokio::time::sleep(time::Duration::from_millis(100)).await;
            }
            bot.set_crouching(false);
        });
        Ok(())
    })
}
