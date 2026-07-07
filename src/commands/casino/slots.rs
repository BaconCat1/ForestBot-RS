use rand::{Rng, rngs::OsRng};
use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use super::chips_str;

const MIN_BET: i64 = 10;
const MAX_BET: i64 = 5_000;
const RAKE_PCT: f64 = 0.02;

struct Symbol {
    label: &'static str,
    triple_mult: f64,
}

// Weights (strip counts / 20): $ 30%, ♣ 25%, ♠ 20%, ♥ 15%, ♦ 5%, 7 5%
// Triple-match RTP ≈ 68% across all symbols.
// Adapted from slot-machine-gen strip model (MIT, Marc S. Brooks 2020-2025).
// All symbols 5px in MC 26.x: $ (ascii), ♣♠♥♦ (nonlatin_european bitmap), 7 (ascii).
const SYMBOLS: &[Symbol] = &[
    Symbol { label: "$",         triple_mult:   3.0 }, // 0 $ Dollar
    Symbol { label: "\u{2663}", triple_mult:  10.0 }, // 1 ♣ Clubs
    Symbol { label: "\u{2660}", triple_mult:  20.0 }, // 2 ♠ Spades
    Symbol { label: "\u{2665}", triple_mult:  50.0 }, // 3 ♥ Hearts
    Symbol { label: "\u{2666}", triple_mult: 200.0 }, // 4 ♦ Diamonds
    Symbol { label: "7",        triple_mult: 777.0 }, // 5   Seven
];

// 20-position strip. Counts: $=6, ♣=5, ♠=4, ♥=3, ♦=1, 7=1.
//        🍒 ♣  ♠  🍒 ♣  🍒 ♥  ♠  ♣  🍒 ♥  ♣  ♠  🍒 ♣  ♥  ♠  🍒 ♦  7
const STRIP: &[usize] = &[0, 1, 2, 0, 1, 0, 3, 2, 1, 0, 3, 1, 2, 0, 1, 3, 2, 0, 4, 5];

fn spin_reel(rng: &mut OsRng) -> usize { rng.gen_range(0..STRIP.len()) }
fn sym_at(pos: usize) -> usize { STRIP[pos % STRIP.len()] }

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["slots", "slot"],
    description: "Spin the slots. !slots <bet>",
    whitelisted: false,
    execute,
};

// ── Pure game logic ───────────────────────────────────────────────────────────

// Returns (total_win, matched_line_names). total_win=0 means no win.
fn evaluate_paylines(above: [usize; 3], center: [usize; 3], below: [usize; 3], bet: i64) -> (i64, Vec<&'static str>) {
    let paylines: [([usize; 3], &'static str); 5] = [
        (above,                          "Top row"),
        (center,                         "Center row"),
        (below,                          "Bottom row"),
        ([above[0], center[1], below[2]], "Diagonal"),
        ([below[0], center[1], above[2]], "Diagonal"),
    ];
    let wins: Vec<(i64, &'static str)> = paylines.iter()
        .filter(|(line, _)| line[0] == line[1] && line[1] == line[2])
        .map(|(line, name)| ((bet as f64 * SYMBOLS[line[0]].triple_mult) as i64, *name))
        .collect();
    let total: i64 = wins.iter().map(|(w, _)| w).sum();
    let names: Vec<&'static str> = wins.into_iter().map(|(_, n)| n).collect();
    (total, names)
}

// ── Imperative shell ──────────────────────────────────────────────────────────

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(bet_str) = ctx.args.first() else {
            ctx.whisper(format!("Usage: !slots <bet> ({}-{})", chips_str(MIN_BET), chips_str(MAX_BET)));
            return Ok(());
        };
        let Ok(bet) = bet_str.parse::<i64>() else {
            ctx.whisper("Bet must be a number.");
            return Ok(());
        };
        if bet < MIN_BET || bet > MAX_BET {
            ctx.whisper(format!("Bet must be {}-{}.", chips_str(MIN_BET), chips_str(MAX_BET)));
            return Ok(());
        }

        let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
            ctx.whisper("Could not resolve your UUID.");
            return Ok(());
        };

        let balance = match ctx.state.api.casino_adjust(&player_uuid, -bet).await {
            Ok(b) => b,
            Err(CasinoAdjustErr::InsufficientFunds(have)) => {
                ctx.whisper(format!("Need {} but have {}.", chips_str(bet), chips_str(have)));
                return Ok(());
            }
            Err(CasinoAdjustErr::NetworkErr) => {
                ctx.whisper("Casino unavailable.");
                return Ok(());
            }
        };

        let mut rng = OsRng;
        let pos    = [spin_reel(&mut rng), spin_reel(&mut rng), spin_reel(&mut rng)];
        let above  = [sym_at(pos[0] + STRIP.len() - 1), sym_at(pos[1] + STRIP.len() - 1), sym_at(pos[2] + STRIP.len() - 1)];
        let center = [sym_at(pos[0]),                    sym_at(pos[1]),                    sym_at(pos[2])];
        let below  = [sym_at(pos[0] + 1),                sym_at(pos[1] + 1),                sym_at(pos[2] + 1)];

        let l = |i: usize| SYMBOLS[i].label;
        ctx.whisper(format!("{} | {} | {}", l(above[0]),  l(above[1]),  l(above[2])));
        ctx.whisper(format!("{} | {} | {}", l(center[0]), l(center[1]), l(center[2])));
        ctx.whisper(format!("{} | {} | {}", l(below[0]),  l(below[1]),  l(below[2])));

        tokio::time::sleep(std::time::Duration::from_millis(800)).await;

        let (total_win, line_names) = evaluate_paylines(above, center, below, bet);

        if total_win == 0 {
            let rake = ((bet as f64) * RAKE_PCT).max(1.0) as i64;
            ctx.state.api.casino_jackpot_rake(rake).await;
            ctx.whisper(format!("-{} | Balance: {}", chips_str(bet), chips_str(balance)));
        } else {
            let bal = ctx.state.api.casino_adjust(&player_uuid, total_win).await.unwrap_or(balance + total_win);
            ctx.whisper(format!("{} match! +{} | Balance: {}", line_names.join(" + "), chips_str(total_win), chips_str(bal)));
        }

        Ok(())
    })
}
