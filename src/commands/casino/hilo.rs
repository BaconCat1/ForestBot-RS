use rand::{Rng, rngs::OsRng};

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::mineflayer::bot::CasinoSession;

use super::chips_str;

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["hilo", "hi-lo"],
    description: "HiLo card game. !hilo <bet> to start, then !hilo hi/lo/skip/cash",
    whitelisted: false,
    execute,
};

const MIN_BET: i64 = 10;
const MAX_BET: i64 = 5_000;
const HOUSE_EDGE: f64 = 0.99; // 1% house edge per prediction

fn rank_name(r: u8) -> &'static str {
    match r {
        1 => "A", 2 => "2", 3 => "3", 4 => "4", 5 => "5",
        6 => "6", 7 => "7", 8 => "8", 9 => "9", 10 => "10",
        11 => "J", 12 => "Q", 13 => "K", _ => "?",
    }
}

fn build_deck() -> Vec<u8> {
    let mut deck = Vec::with_capacity(52);
    for rank in 1u8..=13 {
        for _ in 0..4 { deck.push(rank); }
    }
    let mut rng = OsRng;
    for i in (1..deck.len()).rev() {
        let j = rng.gen_range(0..=i);
        deck.swap(i, j);
    }
    deck
}

fn prob_hi(current: u8, deck: &[u8]) -> f64 {
    deck.iter().filter(|&&c| c >= current).count() as f64 / deck.len() as f64
}

fn prob_lo(current: u8, deck: &[u8]) -> f64 {
    deck.iter().filter(|&&c| c <= current).count() as f64 / deck.len() as f64
}

fn show_state(ctx: &CommandContext, card: u8, _deck: &[u8], stake: i64, multiplier: f64, can_cashout: bool) {
    let cashout_val = (stake as f64 * multiplier) as i64;
    let cash_str = if can_cashout { "/cash" } else { "" };
    ctx.whisper(format!(
        "HiLo | {} | x{:.2}={} | hi/lo/skip{}",
        rank_name(card), multiplier, chips_str(cashout_val), cash_str
    ));
}

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let arg = ctx.args.first().copied().unwrap_or("");

        // ── Start new round ──────────────────────────────────────────────────
        if let Ok(bet) = arg.parse::<i64>() {
            {
                let sessions = ctx.state.casino_sessions.lock().expect("casino sessions lock");
                if let Some(s) = sessions.get(ctx.sender) {
                    ctx.whisper(format!("Already in a {} game. Use !hilo hi/lo/skip/cash.", session_label(s)));
                    return Ok(());
                }
            }
            if bet < MIN_BET || bet > MAX_BET {
                ctx.whisper(format!("Bet must be {}-{}.", chips_str(MIN_BET), chips_str(MAX_BET)));
                return Ok(());
            }
            match ctx.state.api.casino_adjust(ctx.sender, -bet).await {
                Ok(_) => {}
                Err(CasinoAdjustErr::InsufficientFunds(have)) => {
                    ctx.whisper(format!("Need {} but have {}.", chips_str(bet), chips_str(have)));
                    return Ok(());
                }
                Err(CasinoAdjustErr::NetworkErr) => {
                    ctx.whisper("Casino unavailable.");
                    return Ok(());
                }
            }
            let mut deck = build_deck();
            let current_card = deck.pop().unwrap(); // 51 remain
            {
                let mut sessions = ctx.state.casino_sessions.lock().expect("casino sessions lock");
                sessions.insert(ctx.sender.to_owned(), CasinoSession::Hilo {
                    stake: bet,
                    deck: deck.clone(),
                    current_card,
                    multiplier: 1.0,
                    guesses: 0,
                });
            }
            show_state(&ctx, current_card, &deck, bet, 1.0, false);
            return Ok(());
        }

        // ── In-round actions ─────────────────────────────────────────────────
        let action = arg.to_ascii_lowercase();

        let (stake, mut deck, current_card, multiplier, guesses) = {
            let sessions = ctx.state.casino_sessions.lock().expect("casino sessions lock");
            match sessions.get(ctx.sender) {
                Some(CasinoSession::Hilo { stake, deck, current_card, multiplier, guesses }) => {
                    (*stake, deck.clone(), *current_card, *multiplier, *guesses)
                }
                Some(s) => {
                    ctx.whisper(format!("In a {} game, not HiLo.", session_label(s)));
                    return Ok(());
                }
                None => {
                    ctx.whisper("No active HiLo game. Start with !hilo <bet>.");
                    return Ok(());
                }
            }
        };

        match action.as_str() {
            "hi" | "h" | "higher" => {
                predict(&ctx, true, stake, &mut deck, current_card, multiplier, guesses).await?;
            }
            "lo" | "l" | "lower" => {
                predict(&ctx, false, stake, &mut deck, current_card, multiplier, guesses).await?;
            }
            "skip" | "s" => {
                if deck.is_empty() {
                    ctx.whisper("No cards left to skip to.");
                    return Ok(());
                }
                let new_card = deck.pop().unwrap();
                if deck.is_empty() && guesses == 0 {
                    // Exhausted on skip, no guesses yet — refund
                    {
                        let mut sessions = ctx.state.casino_sessions.lock().expect("casino sessions lock");
                        sessions.remove(ctx.sender);
                    }
                    let bal = ctx.state.api.casino_adjust(ctx.sender, stake).await.unwrap_or(stake);
                    ctx.whisper(format!("Deck exhausted — bet refunded. | Balance: {}", chips_str(bal)));
                    return Ok(());
                }
                if deck.is_empty() && guesses > 0 {
                    // Exhausted on skip after guesses — auto-cashout
                    do_cashout(&ctx, stake, multiplier, guesses).await?;
                    return Ok(());
                }
                {
                    let mut sessions = ctx.state.casino_sessions.lock().expect("casino sessions lock");
                    sessions.insert(ctx.sender.to_owned(), CasinoSession::Hilo {
                        stake, deck: deck.clone(), current_card: new_card, multiplier, guesses,
                    });
                }
                ctx.whisper(format!("Skipped {} → {}", rank_name(current_card), rank_name(new_card)));
                show_state(&ctx, new_card, &deck, stake, multiplier, guesses > 0);
            }
            "cash" | "cashout" | "c" => {
                if guesses == 0 {
                    ctx.whisper("Make at least one correct guess before cashing out.");
                    return Ok(());
                }
                do_cashout(&ctx, stake, multiplier, guesses).await?;
            }
            _ => {
                if guesses > 0 {
                    ctx.whisper("Usage: !hilo hi / lo / skip / cash");
                } else {
                    ctx.whisper("Usage: !hilo hi / lo / skip");
                }
            }
        }

        Ok(())
    })
}

async fn predict(
    ctx: &CommandContext<'_>,
    hi: bool,
    stake: i64,
    deck: &mut Vec<u8>,
    current_card: u8,
    multiplier: f64,
    guesses: u32,
) -> anyhow::Result<()> {
    let p = if hi { prob_hi(current_card, deck) } else { prob_lo(current_card, deck) };
    let step_mult = HOUSE_EDGE / p;
    let next_card = deck.pop().unwrap();
    let correct = if hi { next_card >= current_card } else { next_card <= current_card };

    if correct {
        let new_mult = multiplier * step_mult;
        let new_guesses = guesses + 1;
        if deck.is_empty() {
            {
                let mut sessions = ctx.state.casino_sessions.lock().expect("casino sessions lock");
                sessions.remove(ctx.sender);
            }
            let cashout = (stake as f64 * new_mult) as i64;
            let bal = ctx.state.api.casino_adjust(ctx.sender, cashout).await.unwrap_or(0);
            ctx.whisper(format!(
                "Correct! {} → {} | Deck exhausted — auto-cashout: x{:.2}={} | Balance: {}",
                rank_name(current_card), rank_name(next_card), new_mult, chips_str(cashout), chips_str(bal)
            ));
        } else {
            {
                let mut sessions = ctx.state.casino_sessions.lock().expect("casino sessions lock");
                sessions.insert(ctx.sender.to_owned(), CasinoSession::Hilo {
                    stake, deck: deck.clone(), current_card: next_card,
                    multiplier: new_mult, guesses: new_guesses,
                });
            }
            ctx.whisper(format!("Correct! {} → {}", rank_name(current_card), rank_name(next_card)));
            show_state(ctx, next_card, deck, stake, new_mult, true);
        }
    } else {
        {
            let mut sessions = ctx.state.casino_sessions.lock().expect("casino sessions lock");
            sessions.remove(ctx.sender);
        }
        let bal = ctx.state.api.casino_get_balance(ctx.sender).await.map(|b| b.chips).unwrap_or(0);
        ctx.whisper(format!(
            "Wrong! {} came up. Lost {}. | Balance: {}",
            rank_name(next_card), chips_str(stake), chips_str(bal)
        ));
    }
    Ok(())
}

async fn do_cashout(ctx: &CommandContext<'_>, stake: i64, multiplier: f64, guesses: u32) -> anyhow::Result<()> {
    let cashout = (stake as f64 * multiplier) as i64;
    let profit = cashout - stake;
    {
        let mut sessions = ctx.state.casino_sessions.lock().expect("casino sessions lock");
        sessions.remove(ctx.sender);
    }
    let bal = ctx.state.api.casino_adjust(ctx.sender, cashout).await.unwrap_or(0);
    ctx.whisper(format!(
        "Cashed out! x{:.2} × {} = {} (+{}) after {} guess{} | Balance: {}",
        multiplier, chips_str(stake), chips_str(cashout), chips_str(profit),
        guesses, if guesses == 1 { "" } else { "es" }, chips_str(bal)
    ));
    Ok(())
}

fn session_label(s: &CasinoSession) -> &'static str {
    match s {
        CasinoSession::Craps { .. }       => "craps (!craps roll)",
        CasinoSession::Hilo { .. }        => "hilo (!hilo hi/lo/skip/cash)",
        CasinoSession::Blackjack { .. }   => "blackjack (!bj hit/stand/double)",
        CasinoSession::Poker { .. }       => "poker (!poker call/fold/raise/check)",
        CasinoSession::ConnectFour { .. } => "Connect Four (!c4 <1-7>)",
    }
}
