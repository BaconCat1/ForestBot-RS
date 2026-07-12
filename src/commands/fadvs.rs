pub const NAMES: &[&str] = &["fadvs", "fadv"];

use std::collections::HashSet;

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: NAMES,
    description: "View Forest Advancements. Usage: {prefix}fadvs | {prefix}fadvs <category> | {prefix}fadvs <player>",
    whitelisted: false,
    execute,
};

struct Category {
    key: &'static str,
    aliases: &'static [&'static str],
    label: &'static str,
    tiers: &'static [(&'static str, &'static str)],
}

static CATEGORIES: &[Category] = &[
    Category {
        key: "kills",
        aliases: &[],
        label: "Kills",
        tiers: &[
            ("kills_1",    "First Blood"),
            ("kills_10",   "Street Thug"),
            ("kills_50",   "Mafia Boss"),
            ("kills_100",  "Decimation"),
            ("kills_500",  "Serial Killer"),
            ("kills_1000", "Empire Crusher"),
            ("kills_5000", "Genocide Expert"),
        ],
    },
    Category {
        key: "deaths",
        aliases: &[],
        label: "Deaths",
        tiers: &[
            ("deaths_1",    "Critical Misstep"),
            ("deaths_10",   "Tripping Hazard"),
            ("deaths_50",   "All Thumbs"),
            ("deaths_500",  "Practically Undead"),
            ("deaths_5000", "Underworld Connoisseur"),
        ],
    },
    Category {
        key: "playtime",
        aliases: &[],
        label: "Playtime",
        tiers: &[
            ("playtime_1h",    "Getting Your Bearings"),
            ("playtime_10h",   "Decathlon"),
            ("playtime_24h",   "Full Day"),
            ("playtime_100h",  "Beginner No Lifer"),
            ("playtime_500h",  "Mid Level No Lifer"),
            ("playtime_1000h", "Final Boss No Lifer"),
        ],
    },
    Category {
        key: "messages",
        aliases: &[],
        label: "Messages",
        tiers: &[
            ("messages_100",   "Baby Yapper"),
            ("messages_1000",  "Conversationalist"),
            ("messages_10000", "Certified Yapper"),
            ("messages_50000", "The Chattiest Cathy"),
        ],
    },
    Category {
        key: "joins",
        aliases: &[],
        label: "Joins",
        tiers: &[
            ("joins_10",   "Gym Membership"),
            ("joins_100",  "Frequent Flyer"),
            ("joins_500",  "Quit Button Extraordinaire"),
            ("joins_1000", "Alt+F4 Guru"),
        ],
    },
    Category {
        key: "trades",
        aliases: &[],
        label: "Trades",
        tiers: &[
            ("trades_1",   "Capitalism 101"),
            ("trades_5",   "Certified Trader I"),
            ("trades_10",  "Certified Trader II"),
            ("trades_25",  "Certified Trader IV"),
            ("trades_50",  "Certified Trader V"),
            ("trades_100", "Master Trader VII"),
            ("trades_500", "Sigma Trader XIII"),
        ],
    },
    Category {
        key: "kd",
        aliases: &[],
        label: "K/D",
        tiers: &[
            ("kd_2",  "Combat Lieutenant"),
            ("kd_5",  "Combat Commander"),
            ("kd_10", "Combat Veteran"),
        ],
    },
    Category {
        key: "killmethods",
        aliases: &["kill methods", "kill method"],
        label: "Kill Methods",
        tiers: &[
            ("kill_method_melee",     "Smacking Expert"),
            ("kill_method_ranged",    "Elven Menace"),
            ("kill_method_trident",   "Neptune's Might"),
            ("kill_method_explosion", "Bevy Of Dynamite"),
            ("kill_method_fire",      "Flamin' Hot"),
            ("kill_method_magic",     "Wizardly Ways"),
            ("kill_method_spear",     "Phalanxer"),
            ("kill_method_all",       "God Of War"),
            ("kill_method_forest",    "Bot Nuker"),
        ],
    },
    Category {
        key: "deathmethods",
        aliases: &["death methods", "death method"],
        label: "Death Methods",
        tiers: &[
            ("death_method_player",       "1V1"),
            ("death_method_mob",          "Blocks Fight Back"),
            ("death_method_fall",         "Gravity Tester"),
            ("death_method_drown",        "Gill-less"),
            ("death_method_fire",         "Roasted & Toasted"),
            ("death_method_lava",         "Krakatoa"),
            ("death_method_explosion",    "Trigger Happy"),
            ("death_method_suffocation",  "Lack Of Oxygen"),
            ("death_method_starve",       "Child In Africa"),
            ("death_method_magic",        "Arcane Victim"),
            ("death_method_void",         "Black Hole Diver"),
            ("death_method_elytra",       "Turbulence"),
            ("death_method_cactus",       "Mean & Green"),
            ("death_method_lightning",    "Wrath of God"),
            ("death_method_freeze",       "Forgot Snowshoes"),
            ("death_method_sweet_berry",  "Sweet Release"),
            ("death_method_sting",        "Arthropod Attack"),
            ("death_method_stalagmite",   "Pointy Rock"),
            ("death_method_dragon_breath","Scent Of The End"),
            ("death_method_warden",       "MegaMegaphone"),
            ("death_method_spear",        "Impalee"),
            ("death_method_all",          "God Of Death"),
        ],
    },
];

const MSG_MAX: usize = 230;

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let joined = ctx.args.join(" ");
        let category = if joined.is_empty() {
            None
        } else {
            CATEGORIES.iter().find(|c| {
                c.key.eq_ignore_ascii_case(&joined)
                    || c.aliases.iter().any(|a| a.eq_ignore_ascii_case(&joined))
            })
        };

        let username = if category.is_some() {
            ctx.sender
        } else {
            ctx.args.first().copied().unwrap_or(ctx.sender)
        };

        let Some(uuid) = ctx.state.api.convert_username_to_uuid(username).await else {
            ctx.whisper(format!("No data found for {}.", username));
            return Ok(());
        };

        let Some(earned_ids) = ctx.state.api.get_user_fadv_ids(&uuid, &ctx.state.mc_server).await else {
            ctx.whisper("Could not fetch advancement data. Try again later.".to_owned());
            return Ok(());
        };

        let earned: HashSet<String> = earned_ids.into_iter().collect();

        if let Some(cat) = category {
            let earned_names: Vec<&str> = cat.tiers.iter()
                .filter(|(id, _)| earned.contains(*id))
                .map(|(_, name)| *name)
                .collect();

            let total = cat.tiers.len();
            let earned_count = earned_names.len();
            let remaining = total - earned_count;

            let prefix = format!(
                "{}'s {} ({}/{}): ",
                username, cat.label, earned_count, total
            );

            if earned_names.is_empty() {
                ctx.whisper(format!("{}None earned yet.", prefix));
                return Ok(());
            }

            whisper_chunked(&ctx, &prefix, &earned_names, remaining);
        } else {
            let total_all: usize = CATEGORIES.iter().map(|c| c.tiers.len()).sum();
            let total_earned: usize = CATEGORIES.iter()
                .flat_map(|c| c.tiers.iter())
                .filter(|(id, _)| earned.contains(*id))
                .count();

            let cat_parts: Vec<String> = CATEGORIES.iter()
                .map(|c| {
                    let cat_earned = c.tiers.iter()
                        .filter(|(id, _)| earned.contains(*id))
                        .count();
                    format!("{} {}/{}", c.label, cat_earned, c.tiers.len())
                })
                .collect();

            ctx.whisper(format!(
                "{}'s Forest Advancements ({}/{}): {}",
                username, total_earned, total_all,
                cat_parts.join(", ")
            ));
        }

        Ok(())
    })
}

fn whisper_chunked(ctx: &CommandContext, prefix: &str, names: &[&str], remaining: usize) {
    let mut chunks: Vec<String> = Vec::new();
    let mut current = prefix.to_owned();
    let mut first = true;

    for name in names {
        let sep = if first { "" } else { ", " };
        let candidate = format!("{current}{sep}{name}");
        if candidate.len() > MSG_MAX && !first {
            chunks.push(current);
            current = name.to_string();
        } else {
            current = candidate;
            first = false;
        }
    }

    if remaining > 0 {
        let suffix = format!(" [{remaining} remaining]");
        if current.len() + suffix.len() <= MSG_MAX {
            current.push_str(&suffix);
        } else {
            chunks.push(current);
            current = format!("(continued) [{remaining} remaining]");
        }
    }

    chunks.push(current);

    for chunk in chunks {
        ctx.whisper(chunk);
    }
}
