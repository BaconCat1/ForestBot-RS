use rand::{Rng, rngs::OsRng};
use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use super::{chips_str, deduct_stake};

const MIN_BET: i64 = 10;
const MAX_BET: i64 = 5_000;
struct Symbol {
    label: &'static str,
    triple_mult: f64,
}

// Weights (strip counts / 21): $ 6, ♣ 5, ♠ 4, ♥ 3, ♦ 2, 7 1 -- every symbol has a
// distinct rarity (needed so a real payout curve, below, assigns each a distinct
// multiplier instead of two rarest symbols tying).
//
// Multipliers are NOT invented -- derived from real slot machine data. This game pays
// on 5 lines at once (3 rows + 2 diagonals) from the same 3 reel stops; no public PAR
// sheet exists for a real 3-reel/5-line machine (checked -- genuine gap in public
// data), so the per-symbol rarity-to-payout RELATIONSHIP was fitted (log-log power
// regression) from Lucky Larry's Lobstermania's real, FOIA-obtained 96.2%-RTP reel/pay
// table (Harrigan & Dixon, 2009, "PAR Sheets, probabilities, and slot machine play,"
// Journal of Gambling Issues 23 -- see REFERENCE_MATERIAL/DOCS/Harrigan-Dixon-2009-
// PAR-Sheets-Probabilities-Slot-Machine-Play.pdf, Table 3), then applied to our own
// weights above and rescaled to this project's mandatory 3% house rake (97% RTP,
// verified via brute-force over all 21^3 reel-stop combinations -- see
// REFERENCE_MATERIAL/DOCS for the fit). Real reel-strip data still doesn't exist for
// this exact 5-line shape; the pricing CURVE does.
// All symbols 5px in MC 26.x: $ (ascii), ♣♠♥♦ (nonlatin_european bitmap), 7 (ascii).
const SYMBOLS: &[Symbol] = &[
    Symbol { label: "$",         triple_mult:   2.028 }, // 0 $ Dollar
    Symbol { label: "\u{2663}", triple_mult:   3.114 }, // 1 ♣ Clubs
    Symbol { label: "\u{2660}", triple_mult:   5.265 }, // 2 ♠ Spades
    Symbol { label: "\u{2665}", triple_mult:  10.361 }, // 3 ♥ Hearts
    Symbol { label: "\u{2666}", triple_mult:  26.905 }, // 4 ♦ Diamonds
    Symbol { label: "7",        triple_mult: 137.498 }, // 5   Seven
];

// 21-position strip. Counts: $=6, ♣=5, ♠=4, ♥=3, ♦=2, 7=1.
const STRIP: &[usize] = &[0, 1, 2, 0, 1, 0, 3, 2, 1, 0, 4, 1, 2, 0, 1, 3, 2, 0, 4, 5, 3];

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
        let limit = ctx.bet_limit("slots", MIN_BET, Some(MAX_BET));
        let max = limit.max.unwrap_or(MAX_BET);
        let Some(bet_str) = ctx.args.first() else {
            ctx.whisper_success(format!("Usage: !slots <bet> ({}-{})", chips_str(limit.min), chips_str(max)));
            return Ok(());
        };
        let Ok(bet) = bet_str.parse::<i64>() else {
            ctx.whisper_success("Bet must be a number.");
            return Ok(());
        };
        if bet < limit.min || bet > max {
            ctx.whisper_success(format!("Bet must be {}-{}.", chips_str(limit.min), chips_str(max)));
            return Ok(());
        }

        let Some(player_uuid) = ctx.require_player_uuid().await else { return Ok(()); };

        let Some(balance) = deduct_stake(&ctx, &player_uuid, bet).await else { return Ok(()); };

        let mut rng = OsRng;
        let pos    = [spin_reel(&mut rng), spin_reel(&mut rng), spin_reel(&mut rng)];
        let above  = [sym_at(pos[0] + STRIP.len() - 1), sym_at(pos[1] + STRIP.len() - 1), sym_at(pos[2] + STRIP.len() - 1)];
        let center = [sym_at(pos[0]),                    sym_at(pos[1]),                    sym_at(pos[2])];
        let below  = [sym_at(pos[0] + 1),                sym_at(pos[1] + 1),                sym_at(pos[2] + 1)];

        let l = |i: usize| SYMBOLS[i].label;
        ctx.whisper_success(format!("{} | {} | {}", l(above[0]),  l(above[1]),  l(above[2])));
        ctx.whisper_success(format!("{} | {} | {}", l(center[0]), l(center[1]), l(center[2])));
        ctx.whisper_success(format!("{} | {} | {}", l(below[0]),  l(below[1]),  l(below[2])));

        tokio::time::sleep(std::time::Duration::from_millis(ctx.runtime.casino.slots_animation_delay_ms)).await;

        let (total_win, line_names) = evaluate_paylines(above, center, below, bet);

        if total_win == 0 {
            ctx.state.api.casino_jackpot_rake(bet).await;
            ctx.whisper_success(format!("-{} | Balance: {}", chips_str(bet), chips_str(balance)));
        } else {
            match ctx.state.api.casino_win(&player_uuid, total_win).await {
                Ok(result) => {
                    let alimony_note = if result.alimony_paid > 0 {
                        format!(" (-{} alimony to {} ex)", chips_str(result.alimony_paid), result.ex_count)
                    } else {
                        String::new()
                    };
                    ctx.whisper_success(format!(
                        "{} match! +{}{alimony_note} | Balance: {}",
                        line_names.join(" + "), chips_str(result.net), chips_str(result.chips)
                    ));
                }
                Err(e) => {
                    eprintln!("[Slots] payout failed for {player_uuid}: {e:?}");
                    ctx.whisper_error(format!(
                        "{} match! but payout failed. Contact an admin.",
                        line_names.join(" + ")
                    ));
                }
            }
        }

        Ok(())
    })
}
