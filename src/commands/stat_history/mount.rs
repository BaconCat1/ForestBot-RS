use super::command;
use crate::commands::{CommandContext, CommandFuture};

command!(MOUNT_COMMAND, &["mount", "ride", "mush"], "Mount the nearest rideable entity. Usage: {prefix}mount <entity>(optional)", mount);

fn mount(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        use azalea::ecs::query::Without;
        use azalea::entity::{EntityKindComponent, LocalEntity};
        use azalea::registry::builtin::EntityKind;

        fn is_mountable(kind: EntityKind) -> bool {
            matches!(
                kind,
                EntityKind::Horse
                    | EntityKind::Donkey
                    | EntityKind::Mule
                    | EntityKind::Camel
                    | EntityKind::Llama
                    | EntityKind::TraderLlama
                    | EntityKind::Pig
                    | EntityKind::Strider
                    | EntityKind::SkeletonHorse
                    | EntityKind::ZombieHorse
                    | EntityKind::OakBoat
                    | EntityKind::SpruceBoat
                    | EntityKind::BirchBoat
                    | EntityKind::JungleBoat
                    | EntityKind::AcaciaBoat
                    | EntityKind::DarkOakBoat
                    | EntityKind::MangroveBoat
                    | EntityKind::CherryBoat
                    | EntityKind::PaleOakBoat
                    | EntityKind::OakChestBoat
                    | EntityKind::SpruceChestBoat
                    | EntityKind::BirchChestBoat
                    | EntityKind::JungleChestBoat
                    | EntityKind::AcaciaChestBoat
                    | EntityKind::DarkOakChestBoat
                    | EntityKind::MangroveChestBoat
                    | EntityKind::CherryChestBoat
                    | EntityKind::PaleOakChestBoat
                    | EntityKind::BambooRaft
                    | EntityKind::BambooChestRaft
                    | EntityKind::Minecart
            )
        }

        fn kind_matches_target(kind: EntityKind, target: &str) -> bool {
            match target {
                "horse" => kind == EntityKind::Horse,
                "donkey" => kind == EntityKind::Donkey,
                "mule" => kind == EntityKind::Mule,
                "camel" => kind == EntityKind::Camel,
                "llama" => kind == EntityKind::Llama,
                "trader_llama" => kind == EntityKind::TraderLlama,
                "pig" => kind == EntityKind::Pig,
                "strider" => kind == EntityKind::Strider,
                "skeleton_horse" => kind == EntityKind::SkeletonHorse,
                "zombie_horse" => kind == EntityKind::ZombieHorse,
                "boat" => matches!(
                    kind,
                    EntityKind::OakBoat
                        | EntityKind::SpruceBoat
                        | EntityKind::BirchBoat
                        | EntityKind::JungleBoat
                        | EntityKind::AcaciaBoat
                        | EntityKind::DarkOakBoat
                        | EntityKind::MangroveBoat
                        | EntityKind::CherryBoat
                        | EntityKind::PaleOakBoat
                        | EntityKind::OakChestBoat
                        | EntityKind::SpruceChestBoat
                        | EntityKind::BirchChestBoat
                        | EntityKind::JungleChestBoat
                        | EntityKind::AcaciaChestBoat
                        | EntityKind::DarkOakChestBoat
                        | EntityKind::MangroveChestBoat
                        | EntityKind::CherryChestBoat
                        | EntityKind::PaleOakChestBoat
                        | EntityKind::BambooRaft
                        | EntityKind::BambooChestRaft
                ),
                "chest_boat" => matches!(
                    kind,
                    EntityKind::OakChestBoat
                        | EntityKind::SpruceChestBoat
                        | EntityKind::BirchChestBoat
                        | EntityKind::JungleChestBoat
                        | EntityKind::AcaciaChestBoat
                        | EntityKind::DarkOakChestBoat
                        | EntityKind::MangroveChestBoat
                        | EntityKind::CherryChestBoat
                        | EntityKind::PaleOakChestBoat
                        | EntityKind::BambooChestRaft
                ),
                "raft" => matches!(kind, EntityKind::BambooRaft | EntityKind::BambooChestRaft),
                "chest_raft" => kind == EntityKind::BambooChestRaft,
                "minecart" => kind == EntityKind::Minecart,
                "oak_boat" => matches!(kind, EntityKind::OakBoat | EntityKind::OakChestBoat),
                "oak_chest_boat" => kind == EntityKind::OakChestBoat,
                "spruce_boat" => matches!(kind, EntityKind::SpruceBoat | EntityKind::SpruceChestBoat),
                "spruce_chest_boat" => kind == EntityKind::SpruceChestBoat,
                "birch_boat" => matches!(kind, EntityKind::BirchBoat | EntityKind::BirchChestBoat),
                "birch_chest_boat" => kind == EntityKind::BirchChestBoat,
                "jungle_boat" => matches!(kind, EntityKind::JungleBoat | EntityKind::JungleChestBoat),
                "jungle_chest_boat" => kind == EntityKind::JungleChestBoat,
                "acacia_boat" => matches!(kind, EntityKind::AcaciaBoat | EntityKind::AcaciaChestBoat),
                "acacia_chest_boat" => kind == EntityKind::AcaciaChestBoat,
                "dark_oak_boat" => matches!(kind, EntityKind::DarkOakBoat | EntityKind::DarkOakChestBoat),
                "dark_oak_chest_boat" => kind == EntityKind::DarkOakChestBoat,
                "mangrove_boat" => matches!(kind, EntityKind::MangroveBoat | EntityKind::MangroveChestBoat),
                "mangrove_chest_boat" => kind == EntityKind::MangroveChestBoat,
                "cherry_boat" => matches!(kind, EntityKind::CherryBoat | EntityKind::CherryChestBoat),
                "cherry_chest_boat" => kind == EntityKind::CherryChestBoat,
                "pale_oak_boat" => matches!(kind, EntityKind::PaleOakBoat | EntityKind::PaleOakChestBoat),
                "pale_oak_chest_boat" => kind == EntityKind::PaleOakChestBoat,
                "bamboo_raft" => matches!(kind, EntityKind::BambooRaft | EntityKind::BambooChestRaft),
                "bamboo_chest_raft" => kind == EntityKind::BambooChestRaft,
                _ => false,
            }
        }

        fn kind_display_name(kind: EntityKind) -> &'static str {
            match kind {
                EntityKind::Horse => "horse",
                EntityKind::Donkey => "donkey",
                EntityKind::Mule => "mule",
                EntityKind::Camel => "camel",
                EntityKind::Llama => "llama",
                EntityKind::TraderLlama => "trader llama",
                EntityKind::Pig => "pig",
                EntityKind::Strider => "strider",
                EntityKind::SkeletonHorse => "skeleton horse",
                EntityKind::ZombieHorse => "zombie horse",
                EntityKind::OakBoat => "oak boat",
                EntityKind::SpruceBoat => "spruce boat",
                EntityKind::BirchBoat => "birch boat",
                EntityKind::JungleBoat => "jungle boat",
                EntityKind::AcaciaBoat => "acacia boat",
                EntityKind::DarkOakBoat => "dark oak boat",
                EntityKind::MangroveBoat => "mangrove boat",
                EntityKind::CherryBoat => "cherry boat",
                EntityKind::PaleOakBoat => "pale oak boat",
                EntityKind::OakChestBoat => "oak chest boat",
                EntityKind::SpruceChestBoat => "spruce chest boat",
                EntityKind::BirchChestBoat => "birch chest boat",
                EntityKind::JungleChestBoat => "jungle chest boat",
                EntityKind::AcaciaChestBoat => "acacia chest boat",
                EntityKind::DarkOakChestBoat => "dark oak chest boat",
                EntityKind::MangroveChestBoat => "mangrove chest boat",
                EntityKind::CherryChestBoat => "cherry chest boat",
                EntityKind::PaleOakChestBoat => "pale oak chest boat",
                EntityKind::BambooRaft => "bamboo raft",
                EntityKind::BambooChestRaft => "bamboo chest raft",
                EntityKind::Minecart => "minecart",
                _ => "entity",
            }
        }

        let target_name: Option<String> = ctx
            .args
            .first()
            .map(|s| s.replace("minecraft:", "").to_lowercase());

        let candidates = ctx.bot.nearest_entities_by::<&EntityKindComponent, Without<LocalEntity>>(
            |kind: &EntityKindComponent| {
                is_mountable(**kind)
                    && target_name
                        .as_deref()
                        .map_or(true, |t| kind_matches_target(**kind, t))
            },
        );

        if candidates.is_empty() {
            if let Some(ref t) = target_name {
                ctx.whisper(format!(" I could not find a {t} nearby."));
            } else {
                ctx.whisper(" I could not find any mountable nearby.");
            }
            return Ok(());
        }

        let entity_ref = &candidates[0];
        let kind_name = kind_display_name(entity_ref.kind());
        ctx.whisper(format!(
            " {}",
            if let Some(ref t) = target_name {
                format!("Searching for nearest {t} to mount...")
            } else {
                "Searching for nearest mountable...".to_owned()
            }
        ));
        entity_ref.interact();
        ctx.whisper(format!(" Attempting to mount {kind_name}!"));
        Ok(())
    })
}
