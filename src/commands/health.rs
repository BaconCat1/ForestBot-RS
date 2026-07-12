use crate::commands::{CommandContext, CommandDefinition, CommandFuture};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["health"],
    description: "Shows bot health, hunger, armor, and active effects. Usage: {prefix}health",
    whitelisted: false,
    execute,
};

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        use azalea::entity::metadata::Health;
        use azalea::entity::{ActiveEffects, inventory::Inventory};
        use azalea::registry::builtin::Attribute;
        use azalea_inventory::components::AttributeModifiers;

        let health = ctx.bot.get_component::<Health>()
            .map(|h| h.0)
            .unwrap_or(0.0);

        let hunger = ctx.bot.hunger();

        let armor: f64 = ctx.bot.get_component::<Inventory>()
            .map(|inv| {
                inv.inventory_menu.as_player().armor.iter()
                    .filter_map(|slot| {
                        slot.get_component::<AttributeModifiers>().map(|mods| {
                            mods.modifiers.iter()
                                .filter(|m| m.kind == Attribute::Armor)
                                .map(|m| m.modifier.amount)
                                .sum::<f64>()
                        })
                    })
                    .sum()
            })
            .unwrap_or(0.0);

        let effects_str: String = ctx.bot.get_component::<ActiveEffects>()
            .filter(|e| !e.0.is_empty())
            .map(|effects| {
                let mut parts: Vec<String> = effects.0.iter().map(|(effect, data)| {
                    let level = data.amplifier + 1;
                    let secs = data.duration / 20;
                    let name = format!("{effect:?}");
                    if level > 1 {
                        format!("{name} {level} ({secs}s)")
                    } else {
                        format!("{name} ({secs}s)")
                    }
                }).collect();
                parts.sort();
                format!(" | Effects: {}", parts.join(", "))
            })
            .unwrap_or_default();

        ctx.chat(format!(
            "Health: {health:.1}/20 | Hunger: {food}/20 (sat: {sat:.1}) | Armor: {armor:.0}{effects_str}",
            food = hunger.food,
            sat = hunger.saturation,
        ));

        Ok(())
    })
}
