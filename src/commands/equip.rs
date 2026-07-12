use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use azalea_inventory::{ItemStack, operations::{PickupClick, QuickMoveClick}};
use azalea_inventory::components::{Equippable, EquipmentSlot};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["equip"],
    description: "Equip any armor pieces found in my inventory. Usage: {prefix}equip",
    whitelisted: false,
    bridge_ok: true,
    execute,
};

pub const UNEQUIP_COMMAND: CommandDefinition = CommandDefinition {
    names: &["unequip"],
    description: "Remove all equipped armor back to inventory. Usage: {prefix}unequip",
    whitelisted: false,
    bridge_ok: true,
    execute: unequip,
};

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(inv) = ctx.bot.open_inventory() else {
            ctx.whisper(" Already in a container — close it first.");
            return Ok(());
        };

        let Some(menu) = inv.menu() else {
            return Ok(());
        };

        let slots = menu.slots();

        // Armor slot indices in player inventory: 5=head 6=chest 7=legs 8=feet
        let mut claimed: std::collections::HashSet<usize> = std::collections::HashSet::new();
        for armor_slot in 5..=8usize {
            if slots.get(armor_slot).is_some_and(|s| matches!(s, ItemStack::Present(_))) {
                claimed.insert(armor_slot);
            }
        }

        let mut equipped = 0u32;
        for source_slot in 9..=44usize {
            let Some(slot) = slots.get(source_slot) else { continue };
            if !matches!(slot, ItemStack::Present(_)) { continue }

            let Some(equippable) = slot.get_component::<Equippable>() else { continue };
            let armor_slot = match equippable.slot {
                EquipmentSlot::Head => 5usize,
                EquipmentSlot::Chest => 6,
                EquipmentSlot::Legs => 7,
                EquipmentSlot::Feet => 8,
                _ => continue,
            };

            if claimed.contains(&armor_slot) { continue }

            inv.click(PickupClick::Left { slot: Some(source_slot as u16) });
            ctx.bot.wait_ticks(2).await;
            inv.click(PickupClick::Left { slot: Some(armor_slot as u16) });
            ctx.bot.wait_ticks(2).await;

            claimed.insert(armor_slot);
            equipped += 1;
        }

        if equipped == 0 {
            ctx.whisper(" No unequipped armor found in inventory.");
        } else {
            ctx.whisper(format!(" Equipped {equipped} piece(s). Use !health to check armor value."));
        }

        Ok(())
    })
}

pub fn unequip(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(inv) = ctx.bot.open_inventory() else {
            ctx.whisper(" Already in a container — close it first.");
            return Ok(());
        };

        let Some(menu) = inv.menu() else {
            return Ok(());
        };

        let slots = menu.slots();
        let mut removed = 0u32;

        // Armor slots 5=head 6=chest 7=legs 8=feet; shift-click from armor → inventory
        for armor_slot in 5..=8usize {
            if slots.get(armor_slot).is_some_and(|s| matches!(s, ItemStack::Present(_))) {
                inv.click(QuickMoveClick::Left { slot: armor_slot as u16 });
                ctx.bot.wait_ticks(2).await;
                removed += 1;
            }
        }

        if removed == 0 {
            ctx.whisper(" No armor equipped.");
        } else {
            ctx.whisper(format!(" Removed {removed} piece(s) to inventory."));
        }

        Ok(())
    })
}
