use rand::{Rng, rngs::OsRng};

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;

use super::chips_str;

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["sicbo", "sic"],
    description: "Sic Bo (3 dice). Usage: {prefix}sicbo small|large|anytriple <stake> | total <4-17> <stake> | single|double|triple <1-6> <stake>",
    whitelisted: false,
    execute,
};

const MIN_BET: i64 = 10;
const MAX_BET: i64 = 50_000;

fn roll_dice() -> [u8; 3] {
    [OsRng.gen_range(1u8..=6), OsRng.gen_range(1u8..=6), OsRng.gen_range(1u8..=6)]
}

enum SicBoBet {
    Small,
    Large,
    Total(u8),
    Single(u8),
    Double(u8),
    Triple(u8),
    AnyTriple,
}

fn parse_bet(args: &[&str]) -> Option<(SicBoBet, i64)> {
    match args {
        [t, stake] if t.eq_ignore_ascii_case("small") => Some((SicBoBet::Small, stake.parse().ok()?)),
        [t, stake] if t.eq_ignore_ascii_case("large") => Some((SicBoBet::Large, stake.parse().ok()?)),
        [t, stake] if t.eq_ignore_ascii_case("anytriple") || t.eq_ignore_ascii_case("any") => Some((SicBoBet::AnyTriple, stake.parse().ok()?)),
        [t, n, stake] if t.eq_ignore_ascii_case("total") => {
            let n: u8 = n.parse().ok()?;
            if (4..=17).contains(&n) { Some((SicBoBet::Total(n), stake.parse().ok()?)) } else { None }
        }
        [t, n, stake] if t.eq_ignore_ascii_case("single") => {
            let n: u8 = n.parse().ok()?;
            if (1..=6).contains(&n) { Some((SicBoBet::Single(n), stake.parse().ok()?)) } else { None }
        }
        [t, n, stake] if t.eq_ignore_ascii_case("double") => {
            let n: u8 = n.parse().ok()?;
            if (1..=6).contains(&n) { Some((SicBoBet::Double(n), stake.parse().ok()?)) } else { None }
        }
        [t, n, stake] if t.eq_ignore_ascii_case("triple") => {
            let n: u8 = n.parse().ok()?;
            if (1..=6).contains(&n) { Some((SicBoBet::Triple(n), stake.parse().ok()?)) } else { None }
        }
        _ => None,
    }
}

// Returns (won, total_return_multiplier) — multiply by stake to get chips returned
fn resolve(bet: &SicBoBet, dice: &[u8; 3]) -> (bool, i64) {
    let total = dice.iter().map(|&d| d as u8).sum::<u8>();
    let is_triple = dice[0] == dice[1] && dice[1] == dice[2];
    match bet {
        SicBoBet::Small     => (total <= 10 && !is_triple, 2),
        SicBoBet::Large     => (total >= 11 && !is_triple, 2),
        SicBoBet::AnyTriple => (is_triple, 31),
        SicBoBet::Total(n) => {
            let mult = match n {
                4 | 17 => 61,
                5 | 16 => 31,
                6 | 15 => 18,
                7 | 14 => 13,
                8 | 13 => 9,
                _      => 7,  // 9-12
            };
            (total == *n, mult)
        }
        SicBoBet::Single(n) => {
            let count = dice.iter().filter(|&&d| d == *n).count() as i64;
            (count > 0, count + 1) // 1 match=2x, 2=3x, 3=4x
        }
        SicBoBet::Double(n) => {
            let count = dice.iter().filter(|&&d| d == *n).count();
            (count >= 2, 11)
        }
        SicBoBet::Triple(n) => (is_triple && dice[0] == *n, 181),
    }
}

fn bet_label(bet: &SicBoBet) -> String {
    match bet {
        SicBoBet::Small     => "Small (4-10)".into(),
        SicBoBet::Large     => "Large (11-17)".into(),
        SicBoBet::AnyTriple => "Any Triple".into(),
        SicBoBet::Total(n)  => format!("Total {n}"),
        SicBoBet::Single(n) => format!("Single {n}"),
        SicBoBet::Double(n) => format!("Double {n}"),
        SicBoBet::Triple(n) => format!("Triple {n}"),
    }
}

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        if ctx.args.is_empty() {
            show_help(&ctx);
            return Ok(());
        }
        let Some((bet, stake)) = parse_bet(&ctx.args) else {
            show_help(&ctx);
            return Ok(());
        };
        if stake < MIN_BET || stake > MAX_BET {
            ctx.whisper_success(format!("Bet must be {MIN_BET}–{MAX_BET} chips."));
            return Ok(());
        }
        let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
            ctx.whisper_success("Could not resolve your UUID.");
            return Ok(());
        };
        let balance = match ctx.state.api.casino_adjust(&player_uuid, -stake).await {
            Ok(b) => b,
            Err(CasinoAdjustErr::InsufficientFunds(have)) => {
                ctx.whisper_success(format!("Not enough chips (have {}, need {}).", chips_str(have), chips_str(stake)));
                return Ok(());
            }
            Err(CasinoAdjustErr::NetworkErr) => {
                ctx.whisper_success("Casino unavailable right now.");
                return Ok(());
            }
        };
        let dice = roll_dice();
        let total: u8 = dice.iter().sum();
        let label = bet_label(&bet);
        let (won, mult) = resolve(&bet, &dice);
        if won {
            let payout = stake * mult;
            let win = ctx.state.api.casino_win(&player_uuid, payout).await.unwrap_or_default();
            let alimony_note = if win.alimony_paid > 0 { format!(" (-{} alimony)", chips_str(win.alimony_paid)) } else { String::new() };
            ctx.whisper_success(format!(
                "Sic Bo [{}-{}-{}={}] {} | WIN +{}{alimony_note} | Balance: {}",
                dice[0], dice[1], dice[2], total,
                label, chips_str(payout - stake), chips_str(win.chips)
            ));
        } else {
            let _ = ctx.state.api.casino_jackpot_rake(stake).await;
            ctx.whisper_success(format!(
                "Sic Bo [{}-{}-{}={}] {} | LOSS -{} | Balance: {}",
                dice[0], dice[1], dice[2], total,
                label, chips_str(stake), chips_str(balance)
            ));
        }
        Ok(())
    })
}

fn show_help(ctx: &CommandContext<'_>) {
    let p = &ctx.runtime.prefix;
    ctx.whisper_success(format!(
        "Sic Bo: {p}sicbo small|large|anytriple <stake> | total <4-17> <stake> | single|double|triple <1-6> <stake> | Payouts: small/large 1:1, total 4/17 60:1, triple 180:1"
    ));
}
