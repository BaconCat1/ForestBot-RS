use rand::{Rng, rngs::OsRng};

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;

use super::{chips_str, format_alimony};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["roulette", "rl"],
    description: "European roulette. Usage: {prefix}roulette <type> <selection> <bet>",
    whitelisted: false,
    execute,
};

const MIN_BET: i64 = 10;
const MAX_BET: i64 = 5_000;
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

// ── Pure game logic ───────────────────────────────────────────────────────────

// Returns (wins, total_return_multiplier, label) or an error string to display.
fn eval_spin(bet_type: &str, selection: &str, spin: u8) -> Result<(bool, i64, &'static str), &'static str> {
    match bet_type {
        "color" => match selection {
            "red"   => Ok((spin != 0 && is_red(spin),   2, "Color Red")),
            "black" => Ok((is_black(spin),               2, "Color Black")),
            "green" => Ok((spin == 0,                   36, "Color Green")),
            _ => Err(USAGE),
        },
        "parity" => match selection {
            "odd"  => Ok((spin != 0 && spin % 2 == 1, 2, "Odd")),
            "even" => Ok((spin != 0 && spin % 2 == 0, 2, "Even")),
            _ => Err(USAGE),
        },
        "half" => match selection {
            "low"  | "1-18"  => Ok((spin >= 1 && spin <= 18, 2, "Low 1-18")),
            "high" | "19-36" => Ok((spin >= 19,              2, "High 19-36")),
            _ => Err(USAGE),
        },
        "column" => {
            let col: u8 = match selection {
                "1" | "1st" => 1, "2" | "2nd" => 2, "3" | "3rd" => 3, _ => 0,
            };
            if col == 0 { return Err(USAGE); }
            Ok((spin != 0 && column_of(spin) == col, 3, "Column"))
        },
        "dozen" => {
            let doz: u8 = match selection {
                "1" | "1st" | "1-12"  => 1,
                "2" | "2nd" | "13-24" => 2,
                "3" | "3rd" | "25-36" => 3,
                _ => 0,
            };
            if doz == 0 { return Err(USAGE); }
            Ok((spin != 0 && dozen_of(spin) == doz, 3, "Dozen"))
        },
        "number" => {
            match selection.parse::<u8>() {
                Ok(n) if n <= 36 => Ok((spin == n, 36, "Number")),
                _ => Err("Number must be 0–36."),
            }
        },
        _ => Err(USAGE),
    }
}

// ── Imperative shell ──────────────────────────────────────────────────────────

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        if ctx.args.len() < 3 {
            ctx.whisper_success(USAGE);
            return Ok(());
        }

        let bet_str = ctx.args[ctx.args.len() - 1];
        let Ok(bet) = bet_str.parse::<i64>() else {
            ctx.whisper_success("Bet must be a number.");
            return Ok(());
        };
        if bet < MIN_BET || bet > MAX_BET {
            ctx.whisper_success(format!("Bet must be {MIN_BET}–{MAX_BET} chips."));
            return Ok(());
        }

        let bet_type = ctx.args[0].to_ascii_lowercase();
        let selection = ctx.args[1..ctx.args.len() - 1].join(" ").to_ascii_lowercase();

        let spin: u8 = OsRng.gen_range(0..=36);
        let color_str = if spin == 0 { "GREEN" } else if is_red(spin) { "RED" } else { "BLACK" };

        let (wins, multiplier, label) = match eval_spin(&bet_type, &selection, spin) {
            Ok(outcome) => outcome,
            Err(msg) => { ctx.whisper_success(msg); return Ok(()); }
        };

        let Some(player_uuid) = ctx.require_player_uuid().await else { return Ok(()); };

        let balance = match ctx.state.api.casino_adjust(&player_uuid, -bet).await {
            Ok(b) => b,
            Err(CasinoAdjustErr::InsufficientFunds(have)) => {
                ctx.whisper_success(format!("Not enough chips (have {}, need {}).", chips_str(have), chips_str(bet)));
                return Ok(());
            }
            Err(CasinoAdjustErr::NetworkErr) => {
                ctx.whisper_success("Casino unavailable right now.");
                return Ok(());
            }
        };

        if wins {
            let total_return = bet * multiplier;
            match ctx.state.api.casino_win(&player_uuid, total_return).await {
                Ok(result) => {
                    let alimony_note = format_alimony(result.alimony_paid);
                    ctx.whisper_success(format!(
                        "Roulette: {} {color_str} | {label} — Win! +{}{alimony_note} | Balance: {}",
                        spin, chips_str(result.net - bet), chips_str(result.chips),
                    ));
                }
                Err(e) => {
                    eprintln!("[Roulette] payout failed for {player_uuid}: {e:?}");
                    ctx.whisper_error(format!(
                        "Roulette: {} {color_str} | {label} — Win! but payout failed. Contact an admin.",
                        spin,
                    ));
                }
            }
        } else {
            ctx.state.api.casino_jackpot_rake(bet).await;
            ctx.whisper_success(format!(
                "Roulette: {} {color_str} | {label} — Lost {} | Balance: {}",
                spin, chips_str(bet), chips_str(balance),
            ));
        }

        Ok(())
    })
}
