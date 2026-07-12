pub const NAMES: &[&str] = &["drop"];

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use azalea::{BlockPos, Client};
use azalea::core::direction::Direction;
use azalea_inventory::{operations::ThrowClick, ItemStack};
use azalea::protocol::packets::game::s_player_action::{
    Action,
    ServerboundPlayerAction,
};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: NAMES,
    description: "I will drop items in my hand, or all of my items. Usage: {prefix}drop <all>(optional)",
    whitelisted: false,
    execute,
};

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        if ctx.args.first().map_or(false, |&arg| arg.eq_ignore_ascii_case("all")) {
            //DUMP IT ALL
            dump_inventory(ctx.bot).await?;

        } else {
            // Drop held item
            drop_held_stack(ctx.bot);
        }
        Ok(())
    })
}

fn drop_held_stack(bot: &Client) {
    bot.write_packet(ServerboundPlayerAction {
        action: Action::DropAllItems,
        pos: BlockPos::ZERO,
        direction: Direction::Down,
        seq: 0,
    });
}

pub async fn dump_inventory(bot: &Client) -> anyhow::Result<()> {
    let Some(inv) = bot.open_inventory() else {
        bot.chat("Close the open container first.");
        return Ok(());
    };

    let Some(menu) = inv.menu() else {
        return Ok(());
    };

    for (slot_index, slot) in menu.slots().iter().enumerate().skip(1) {
        // GET IT ALL OUT
        if matches!(slot, ItemStack::Present(_)) {
            inv.click(ThrowClick::All {
                slot: slot_index as u16,
            });
            bot.wait_ticks(1).await;
        }
    }

    Ok(())
}
