use super::helpers::BOT_SLEEPING;
use crate::commands::{CommandContext, CommandFuture};
use std::sync::atomic::Ordering;

command!(SLEEP_COMMAND, &["sleep"], "Put the bot to sleep. Usage: {prefix}sleep", sleep);

fn sleep(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        use azalea::block::BlockStates;
        use azalea::core::direction::Direction;
        use azalea::core::position::Vec3;
        use azalea::protocol::packets::game::s_interact::InteractionHand;
        use azalea::protocol::packets::game::s_player_command::{
            Action, ServerboundPlayerCommand,
        };
        use azalea::protocol::packets::game::s_use_item_on::{BlockHit, ServerboundUseItemOn};
        use azalea::registry::tags::blocks;

        if BOT_SLEEPING.load(Ordering::Relaxed) {
            ctx.bot.write_packet(ServerboundPlayerCommand {
                id: ctx.bot.minecraft_id(),
                action: Action::StopSleeping,
                data: 0,
            });
            BOT_SLEEPING.store(false, Ordering::Relaxed);
            ctx.whisper(" Good morning!");
            return Ok(());
        }

        let bed_states = BlockStates::from(&blocks::BEDS);
        let bed_pos = ctx.bot.world().read().find_block(ctx.bot.position(), &bed_states);

        let Some(bed_pos) = bed_pos else {
            ctx.whisper(" I couldn't find a bed :(");
            return Ok(());
        };

        ctx.bot.write_packet(ServerboundUseItemOn {
            hand: InteractionHand::MainHand,
            block_hit: BlockHit {
                block_pos: bed_pos,
                direction: Direction::Up,
                location: Vec3 {
                    x: bed_pos.x as f64 + 0.5,
                    y: bed_pos.y as f64 + 0.5,
                    z: bed_pos.z as f64 + 0.5,
                },
                inside: false,
                world_border: false,
            },
            seq: 0,
        });
        BOT_SLEEPING.store(true, Ordering::Relaxed);

        ctx.whisper(" goodnight zzz");
        Ok(())
    })
}
