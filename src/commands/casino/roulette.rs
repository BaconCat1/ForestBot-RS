use rand::{Rng, rngs::OsRng};

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;

use super::chips_str;

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["roulette", "rl"],
    description: "European roulette. Usage: {prefix}roulette <type> <selection> <bet>",
    whitelisted: false,
    execute,
};

const MIN_BET: i64 = 10;
const MAX_BET: i64 = 5_000;
const RAKE_PCT: f64 = 0.02;

const RED: &[u8] = &[1,3,5,7,9,12,14,16,18,19,21,23,25,27,30,32,34,36];

fn is_red(n: u8) -> bool { RED.contains(&n) }
fn is_black(n: u8) -> bool { n != 0 && !is_red(n) }
fn column_of(n: u8) -> u8 { ((n - 1) % 3) + 1 }
fn dozen_of(n: u8) -> u8 {
    if n <= 12 { 1 } else if n <= 24 { 2 } else { 3 }
}

const USAGE: &str = "Usage: !roulette <type> <selection> <bet>  |  \
    types: color red/black/green | parity odd/even | \
    half low/high | column 1/2/3 | dozen 1/2/3 | number 0-36";

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        if ctx.args.len() < 3 {
            ctx.whisper(USAGE);
            return Ok(());
        }

        let bet_str = ctx.args[ctx.args.len() - 1];
        let Ok(bet) = bet_str.parse::<i64>() else {
            ctx.whisper("Bet must be a number.");
            return Ok(());
        };
        if bet < MIN_BET || bet > MAX_BET {
            ctx.whisper(format!("Bet must be {MIN_BET}–{MAX_BET} chips."));
            return Ok(());
        }

        let bet_type = ctx.args[0].to_ascii_lowercase();
        let selection = ctx.args[1..ctx.args.len() - 1].join(" ").to_ascii_lowercase();

        let spin: u8 = OsRng.gen_range(0..=36);
        let color_str = if spin == 0 {
            "GREEN"
        } else if is_red(spin) {
            "RED"
        } else {
            "BLACK"
        };

        // (wins, total_return_multiplier, label)
        let parsed = match bet_type.as_str() {
            "color" => match selection.as_str() {
                "red"   => Some((spin != 0 && is_red(spin),   2i64, "Color Red")),
                "black" => Some((is_black(spin),               2,    "Color Black")),
                "green" => Some((spin == 0,                    36,   "Color Green")),
                _ => None,
            },
            "parity" => match selection.as_str() {
                "odd"  => Some((spin != 0 && spin % 2 == 1, 2, "Odd")),
                "even" => Some((spin != 0 && spin % 2 == 0, 2, "Even")),
                _ => None,
            },
            "half" => match selection.as_str() {
                "low"  | "1-18"  => Some((spin >= 1 && spin <= 18, 2, "Low 1-18")),
                "high" | "19-36" => Some((spin >= 19,              2, "High 19-36")),
                _ => None,
            },
            "column" => {
                let col: u8 = match selection.as_str() {
                    "1" | "1st" => 1, "2" | "2nd" => 2, "3" | "3rd" => 3, _ => 0,
                };
                if col == 0 { None } else { Some((spin != 0 && column_of(spin) == col, 3, "Column")) }
            },
            "dozen" => {
                let doz: u8 = match selection.as_str() {
                    "1" | "1st" | "1-12"  => 1,
                    "2" | "2nd" | "13-24" => 2,
                    "3" | "3rd" | "25-36" => 3,
                    _ => 0,
                };
                if doz == 0 { None } else { Some((spin != 0 && dozen_of(spin) == doz, 3, "Dozen")) }
            },
            "number" => {
                let Ok(n) = selection.parse::<u8>() else {
                    ctx.whisper("Number must be 0–36.");
                    return Ok(());
                };
                if n > 36 {
                    ctx.whisper("Number must be 0–36.");
                    return Ok(());
                }
                Some((spin == n, 36, "Number"))
            },
            _ => None,
        };

        let Some((wins, multiplier, label)) = parsed else {
            ctx.whisper(USAGE);
            return Ok(());
        };

        let balance = match ctx.state.api.casino_adjust(ctx.sender, -bet).await {
            Ok(b) => b,
            Err(CasinoAdjustErr::InsufficientFunds(have)) => {
                ctx.whisper(format!("Not enough chips (have {}, need {}).", chips_str(have), chips_str(bet)));
                return Ok(());
            }
            Err(CasinoAdjustErr::NetworkErr) => {
                ctx.whisper("Casino unavailable right now.");
                return Ok(());
            }
        };

        if wins {
            let total_return = bet * multiplier;
            let new_balance = match ctx.state.api.casino_adjust(ctx.sender, total_return).await {
                Ok(b) => b,
                Err(_) => balance + total_return,
            };
            ctx.whisper(format!(
                "Roulette: {} {color_str} | {label} — Win! +{} | Balance: {}",
                spin,
                chips_str(total_return - bet),
                chips_str(new_balance),
            ));
        } else {
            let rake = ((bet as f64) * RAKE_PCT).max(1.0) as i64;
            ctx.state.api.casino_jackpot_rake(rake).await;
            ctx.whisper(format!(
                "Roulette: {} {color_str} | {label} — Lost {} | Balance: {}",
                spin,
                chips_str(bet),
                chips_str(balance),
            ));
        }

        Ok(())
    })
}
