use rand::{Rng, rngs::OsRng};

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::{CasinoAdjustErr, CasinoScratchResult};

use super::{chips_str, fmt_duration};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["scratch"],
    description: "Scratch a ticket. Usage: {prefix}scratch [copper|gold|diamond]",
    whitelisted: false,
    execute,
};

const RAKE_PCT: f64 = 0.02;
const FREE_SCRATCH_COOLDOWN_SECS: u64 = 600;

// Prize rank → symbol, rarest first (mirrors slots rarity: ♦ rarest, 🍒 most common).
const PRIZE_SYMBOLS: &[&str] = &[
    "\u{2666}", // ♦ index 0 = highest/rarest prize
    "\u{2665}", // ♥ index 1
    "\u{2660}", // ♠ index 2
    "\u{2663}", // ♣ index 3
    "\u{1f352}", // 🍒 index 4 = lowest/most common prize
];

#[derive(Clone, Copy)]
struct Tier {
    name: &'static str,
    cost: i64,
    prizes: &'static [(u32, i64)], // (cumulative_weight_out_of_10000, prize_chips)
}

// Prize tables: cumulative weights out of 10,000.
// Ordered highest → lowest value (index 0 = rarest/biggest = ♦).
const COPPER_PRIZES: &[(u32, i64)] = &[
    (1,    500),
    (11,   100),
    (111,   50),
    (611,   25),
    (1611,  10),
];

const GOLD_PRIZES: &[(u32, i64)] = &[
    (2,    2000),
    (22,    500),
    (122,   200),
    (622,   100),
    (1622,   75),
];

const DIAMOND_PRIZES: &[(u32, i64)] = &[
    (1,    7500),
    (11,   2000),
    (111,   500),
    (611,   200),
    (1611,  100),
];

const COPPER:  Tier = Tier { name: "Copper",  cost:  25, prizes: COPPER_PRIZES  };
const GOLD:    Tier = Tier { name: "Gold",    cost:  75, prizes: GOLD_PRIZES    };
const DIAMOND: Tier = Tier { name: "Diamond", cost: 200, prizes: DIAMOND_PRIZES };

// Returns (prize_chips, prize_table_index). Index 0 = rarest.
fn scratch_ticket(prizes: &[(u32, i64)]) -> (i64, usize) {
    let roll: u32 = OsRng.gen_range(0..10_000);
    for (i, &(weight, prize)) in prizes.iter().enumerate() {
        if roll < weight {
            return (prize, i);
        }
    }
    (0, 0)
}

// Build 5 display cells as symbol indices into PRIZE_SYMBOLS.
// Win:  3 cells = winner index, 2 cells = 2 distinct other indices, shuffled.
// Loss: each of the 5 prize indices exactly once, shuffled (no 3-match possible).
fn build_cells(tier: &Tier, win_idx: Option<usize>, rng: &mut OsRng) -> [usize; 5] {
    let n = tier.prizes.len(); // always 5
    let mut cells = [0usize; 5];

    if let Some(wi) = win_idx {
        let mut others: Vec<usize> = (0..n).filter(|&i| i != wi).collect();
        fisher_yates(&mut others, rng);
        cells[0] = wi; cells[1] = wi; cells[2] = wi;
        cells[3] = others[0]; cells[4] = others[1];
        fisher_yates(&mut cells, rng);
    } else {
        let mut indices: Vec<usize> = (0..n).collect();
        fisher_yates(&mut indices, rng);
        cells.copy_from_slice(&indices);
    }

    cells
}

fn fisher_yates<T>(v: &mut [T], rng: &mut OsRng) {
    for i in (1..v.len()).rev() {
        let j = rng.gen_range(0..=i);
        v.swap(i, j);
    }
}

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let raw_arg = ctx.args.first().copied();
        let is_free = raw_arg.is_none();
        let tier_arg = raw_arg.unwrap_or("copper");

        let tier = match tier_arg.to_ascii_lowercase().as_str() {
            "copper" | "c" => &COPPER,
            "gold"   | "g" => &GOLD,
            "diamond"| "d" => &DIAMOND,
            _ => {
                ctx.whisper("Usage: !scratch (free copper) | !scratch copper/gold/diamond");
                return Ok(());
            }
        };

        if is_free {
            {
                let mut cooldowns = ctx.state.free_scratch_cooldowns.lock().unwrap();
                if let Some(&last) = cooldowns.get(ctx.sender) {
                    let elapsed = last.elapsed().as_secs();
                    if elapsed < FREE_SCRATCH_COOLDOWN_SECS {
                        let remaining = FREE_SCRATCH_COOLDOWN_SECS - elapsed;
                        ctx.whisper(format!(
                            "Free scratch on cooldown. Next in {}. Buy with !scratch copper/gold/diamond.",
                            fmt_duration(remaining)
                        ));
                        return Ok(());
                    }
                }
                cooldowns.insert(ctx.sender.to_owned(), std::time::Instant::now());
            }
            if let CasinoScratchResult::Err = ctx.state.api.casino_free_scratch(ctx.sender).await {
                ctx.whisper("Scratch service unavailable.");
                return Ok(());
            }
        } else {
            match ctx.state.api.casino_adjust(ctx.sender, -tier.cost).await {
                Ok(_) => {}
                Err(CasinoAdjustErr::InsufficientFunds(have)) => {
                    ctx.whisper(format!(
                        "{} ticket costs {} — you have {}.",
                        tier.name, chips_str(tier.cost), chips_str(have)
                    ));
                    return Ok(());
                }
                Err(CasinoAdjustErr::NetworkErr) => {
                    ctx.whisper("Casino unavailable.");
                    return Ok(());
                }
            }
        }

        let (prize, prize_idx) = scratch_ticket(tier.prizes);
        let win_idx = if prize > 0 { Some(prize_idx) } else { None };
        let mut rng = OsRng;
        let cells = build_cells(tier, win_idx, &mut rng);

        let tier_label = if is_free { "FREE COPPER".to_owned() } else { tier.name.to_uppercase() };
        let cell_str = cells.iter().map(|&i| PRIZE_SYMBOLS[i]).collect::<Vec<_>>().join(" ");

        ctx.whisper(format!("=== {tier_label} SCRATCHER ==="));
        ctx.whisper(cell_str);

        tokio::time::sleep(std::time::Duration::from_millis(600)).await;

        if prize > 0 {
            let sym = PRIZE_SYMBOLS[prize_idx];
            let new_balance = ctx.state.api.casino_adjust(ctx.sender, prize).await.unwrap_or(0);
            ctx.whisper(format!(
                "3x {sym} — WIN! +{} | Balance: {}",
                chips_str(prize), chips_str(new_balance)
            ));
        } else {
            let rake_base = if is_free { 25 } else { tier.cost };
            let rake = ((rake_base as f64) * RAKE_PCT).max(1.0) as i64;
            ctx.state.api.casino_jackpot_rake(rake).await;
            let balance = ctx.state.api.casino_get_balance(ctx.sender).await.map(|b| b.chips).unwrap_or(0);
            ctx.whisper(format!("No match. | Balance: {}", chips_str(balance)));
        }

        Ok(())
    })
}
