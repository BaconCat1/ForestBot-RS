use rand::{Rng, rngs::OsRng};

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;

use super::chips_str;

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["baccarat", "bac"],
    description: "Baccarat. {prefix}baccarat player|banker|tie <chips>. Player 2×, Banker 1.95×, Tie 8×.",
    whitelisted: false,
    execute,
};

const MIN_BET: i64 = 25;

fn draw() -> u8 {
    OsRng.gen_range(1u8..=13)
}

fn bac_value(c: u8) -> u32 {
    match c {
        1 => 1,
        2..=9 => c as u32,
        _ => 0, // 10, J, Q, K
    }
}

fn bac_score(hand: &[u8]) -> u32 {
    hand.iter().map(|&c| bac_value(c)).sum::<u32>() % 10
}

fn card_str(c: u8) -> &'static str {
    match c {
        1 => "A", 2 => "2", 3 => "3", 4 => "4", 5 => "5",
        6 => "6", 7 => "7", 8 => "8", 9 => "9", 10 => "10",
        11 => "J", 12 => "Q", _ => "K",
    }
}

fn hand_str(hand: &[u8]) -> String {
    hand.iter().map(|&c| card_str(c)).collect::<Vec<_>>().join(" ")
}

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let (Some(&side_s), Some(&amt_s)) = (ctx.args.first(), ctx.args.get(1)) else {
            ctx.whisper(format!(
                "Usage: {p}baccarat player|banker|tie <chips>  |  Player 2× | Banker 1.95× | Tie 8×",
                p = ctx.runtime.prefix
            ));
            return Ok(());
        };

        let side = side_s.to_lowercase();
        if !matches!(side.as_str(), "player" | "banker" | "tie" | "p" | "b" | "t") {
            ctx.whisper("Side must be: player, banker, or tie.");
            return Ok(());
        }
        let side = match side.as_str() {
            "p" => "player",
            "b" => "banker",
            "t" => "tie",
            s => s,
        };

        let Ok(stake) = amt_s.parse::<i64>() else {
            ctx.whisper("Chip amount must be a number.");
            return Ok(());
        };
        if stake < MIN_BET {
            ctx.whisper(format!("Minimum bet is {}.", chips_str(MIN_BET)));
            return Ok(());
        }

        let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
            ctx.whisper("Could not resolve your UUID.");
            return Ok(());
        };

        match ctx.state.api.casino_adjust(&player_uuid, -stake).await {
            Ok(_) => {}
            Err(CasinoAdjustErr::InsufficientFunds(have)) => {
                ctx.whisper(format!("Need {} but have {}.", chips_str(stake), chips_str(have)));
                return Ok(());
            }
            Err(CasinoAdjustErr::NetworkErr) => {
                ctx.whisper("Casino unavailable.");
                return Ok(());
            }
        }

        // Deal
        let mut ph = vec![draw(), draw()];
        let mut bh = vec![draw(), draw()];
        let mut pt = bac_score(&ph);
        let mut bt = bac_score(&bh);
        let natural = pt >= 8 || bt >= 8;
        if !natural && pt <= 5 {
            ph.push(draw());
            pt = bac_score(&ph);
        }
        if !natural && bt <= 5 {
            bh.push(draw());
            bt = bac_score(&bh);
        }

        let winner = if pt > bt { "player" } else if bt > pt { "banker" } else { "tie" };

        let (payout, result) = match side {
            "player" => {
                if winner == "player" {
                    let pay = stake * 2;
                    (pay, format!("WIN +{}", chips_str(pay - stake)))
                } else if winner == "tie" {
                    (stake, "TIE — stake returned".to_string())
                } else {
                    let _ = ctx.state.api.casino_jackpot_rake(stake).await;
                    (0, format!("LOSS -{}", chips_str(stake)))
                }
            }
            "banker" => {
                if winner == "banker" {
                    let pay = (stake as f64 * 1.95).floor() as i64;
                    (pay, format!("WIN +{}", chips_str(pay - stake)))
                } else if winner == "tie" {
                    (stake, "TIE — stake returned".to_string())
                } else {
                    let _ = ctx.state.api.casino_jackpot_rake(stake).await;
                    (0, format!("LOSS -{}", chips_str(stake)))
                }
            }
            _ => { // tie
                if winner == "tie" {
                    let pay = stake * 8;
                    (pay, format!("WIN +{}", chips_str(pay - stake)))
                } else {
                    let _ = ctx.state.api.casino_jackpot_rake(stake).await;
                    (0, format!("LOSS -{}", chips_str(stake)))
                }
            }
        };

        if payout > 0 {
            let _ = ctx.state.api.casino_adjust(&player_uuid, payout).await;
        }

        let natural_tag = if natural { " [natural]" } else { "" };
        ctx.whisper(format!(
            "[Baccarat] P: {} ({pt}){natural_tag} B: {} ({bt}) → {} | {result}",
            hand_str(&ph), hand_str(&bh), winner.to_uppercase()
        ));
        Ok(())
    })
}
