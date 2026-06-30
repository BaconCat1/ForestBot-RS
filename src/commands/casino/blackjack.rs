use rand::{Rng, rngs::OsRng};

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::mineflayer::bot::CasinoSession;

use super::chips_str;

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["bj", "blackjack"],
    description: "Blackjack vs dealer. Usage: {prefix}bj <bet> | hit | stand | double | quit",
    whitelisted: false,
    execute,
};

const MIN_BET: i64 = 25;
const MAX_BET: i64 = 5_000;
const RAKE_PCT: f64 = 0.02;

fn draw_card() -> u8 {
    OsRng.gen_range(1u8..=13)
}

fn card_str(c: u8) -> String {
    match c {
        1 => "A".to_string(),
        11 => "J".to_string(),
        12 => "Q".to_string(),
        13 => "K".to_string(),
        n => n.to_string(),
    }
}

fn hand_str(hand: &[u8]) -> String {
    hand.iter().map(|&c| card_str(c)).collect::<Vec<_>>().join(" ")
}

fn card_value(c: u8) -> u32 {
    match c {
        1 => 11,
        11 | 12 | 13 => 10,
        n => n as u32,
    }
}

fn score(hand: &[u8]) -> u32 {
    let mut total: u32 = hand.iter().map(|&c| card_value(c)).sum();
    let aces = hand.iter().filter(|&&c| c == 1).count();
    let mut soft = aces;
    while total > 21 && soft > 0 {
        total -= 10;
        soft -= 1;
    }
    total
}

fn is_blackjack(hand: &[u8]) -> bool {
    hand.len() == 2 && score(hand) == 21
}

fn state_msg(player: &[u8], dealer_up: u8, extra: &str) -> String {
    format!(
        "BJ | You: {} ({}) | Dealer: {} ? | {}",
        hand_str(player),
        score(player),
        card_str(dealer_up),
        extra,
    )
}

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let subcmd = ctx.args.first().copied().unwrap_or("").to_ascii_lowercase();
        match subcmd.as_str() {
            "hit" | "h"    => do_hit(ctx).await,
            "stand" | "s"  => do_stand(ctx).await,
            "double" | "d" => do_double(ctx).await,
            "quit" | "q"   => do_quit(ctx).await,
            _              => do_deal(ctx, &subcmd).await,
        }
    })
}

// ── Deal ─────────────────────────────────────────────────────────────────────

async fn do_deal(ctx: CommandContext<'_>, stake_str: &str) -> anyhow::Result<()> {
    {
        let sessions = ctx.state.casino_sessions.lock().expect("lock");
        if sessions.contains_key(ctx.sender) {
            ctx.whisper("Already in a game. Use !bj hit/stand/double/quit.");
            return Ok(());
        }
    }

    let Ok(bet) = stake_str.parse::<i64>() else {
        ctx.whisper(format!("Usage: !bj <bet> (min {MIN_BET}, max {MAX_BET})"));
        return Ok(());
    };
    if bet < MIN_BET || bet > MAX_BET {
        ctx.whisper(format!("Bet must be {MIN_BET}–{MAX_BET} chips."));
        return Ok(());
    }

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

    let player = vec![draw_card(), draw_card()];
    let dealer = vec![draw_card(), draw_card()];

    let pbj = is_blackjack(&player);
    let dbj = is_blackjack(&dealer);

    if dbj && pbj {
        // Push — return bet
        let new_balance = match ctx.state.api.casino_adjust(ctx.sender, bet).await {
            Ok(b) => b,
            Err(_) => balance + bet,
        };
        ctx.whisper(format!(
            "BJ | You: {} (21) | Dealer: {} {} (21) | Both blackjack — Push | Balance: {}",
            hand_str(&player), card_str(dealer[0]), card_str(dealer[1]), chips_str(new_balance)
        ));
        return Ok(());
    }
    if dbj {
        let rake = ((bet as f64) * RAKE_PCT).max(1.0) as i64;
        ctx.state.api.casino_jackpot_rake(rake).await;
        ctx.whisper(format!(
            "BJ | You: {} ({}) | Dealer: {} {} (21) | Dealer blackjack — Lost {} | Balance: {}",
            hand_str(&player), score(&player),
            card_str(dealer[0]), card_str(dealer[1]),
            chips_str(bet), chips_str(balance)
        ));
        return Ok(());
    }
    if pbj {
        // Natural BJ pays 3:2
        let payout = bet + bet * 3 / 2;
        let new_balance = match ctx.state.api.casino_adjust(ctx.sender, payout).await {
            Ok(b) => b,
            Err(_) => balance + payout,
        };
        ctx.whisper(format!(
            "BJ | You: {} (21) | Dealer: {} ? | BLACKJACK! +{} | Balance: {}",
            hand_str(&player), card_str(dealer[0]),
            chips_str(payout - bet), chips_str(new_balance)
        ));
        return Ok(());
    }

    let ps = score(&player);
    let can_double = balance >= bet; // still have enough after deduction
    let actions = if can_double { "Hit, Stand, or Double" } else { "Hit or Stand" };

    ctx.state.casino_sessions.lock().expect("lock").insert(
        ctx.sender.to_owned(),
        CasinoSession::Blackjack {
            bet,
            player_hand: player.clone(),
            dealer_hand: dealer.clone(),
        },
    );

    ctx.whisper(state_msg(&player, dealer[0], &format!("{actions}? | Balance: {}", chips_str(balance))));
    Ok(())
}

// ── Hit ──────────────────────────────────────────────────────────────────────

async fn do_hit(ctx: CommandContext<'_>) -> anyhow::Result<()> {
    let (bet, mut player, dealer) = {
        let sessions = ctx.state.casino_sessions.lock().expect("lock");
        match sessions.get(ctx.sender) {
            Some(CasinoSession::Blackjack { bet, player_hand, dealer_hand }) => {
                (*bet, player_hand.clone(), dealer_hand.clone())
            }
            Some(_) => {
                let label = sessions.get(ctx.sender).map(session_label).unwrap_or("another");
                ctx.whisper(format!("In a {label} game, not blackjack."));
                return Ok(());
            }
            None => {
                ctx.whisper("No blackjack session. Start with !bj <bet>.");
                return Ok(());
            }
        }
    };

    player.push(draw_card());
    let ps = score(&player);

    if ps > 21 {
        ctx.state.casino_sessions.lock().expect("lock").remove(ctx.sender);
        let balance = ctx.state.api.casino_get_balance(ctx.sender).await.map(|b| b.chips).unwrap_or(0);
        let rake = ((bet as f64) * RAKE_PCT).max(1.0) as i64;
        ctx.state.api.casino_jackpot_rake(rake).await;
        ctx.whisper(format!(
            "BJ | You: {} ({ps}) | Dealer: {} ? | Bust — Lost {} | Balance: {}",
            hand_str(&player), card_str(dealer[0]), chips_str(bet), chips_str(balance)
        ));
        return Ok(());
    }

    if ps == 21 {
        // Auto-stand
        ctx.state.casino_sessions.lock().expect("lock").remove(ctx.sender);
        return resolve_dealer(ctx, bet, player, dealer).await;
    }

    ctx.state.casino_sessions.lock().expect("lock").insert(
        ctx.sender.to_owned(),
        CasinoSession::Blackjack { bet, player_hand: player.clone(), dealer_hand: dealer.clone() },
    );
    ctx.whisper(state_msg(&player, dealer[0], &format!("Hit or Stand? ({ps})")));
    Ok(())
}

// ── Stand ────────────────────────────────────────────────────────────────────

async fn do_stand(ctx: CommandContext<'_>) -> anyhow::Result<()> {
    let (bet, player, dealer) = {
        let sessions = ctx.state.casino_sessions.lock().expect("lock");
        match sessions.get(ctx.sender) {
            Some(CasinoSession::Blackjack { bet, player_hand, dealer_hand }) => {
                (*bet, player_hand.clone(), dealer_hand.clone())
            }
            Some(_) => {
                let label = sessions.get(ctx.sender).map(session_label).unwrap_or("another");
                ctx.whisper(format!("In a {label} game, not blackjack."));
                return Ok(());
            }
            None => {
                ctx.whisper("No blackjack session. Start with !bj <bet>.");
                return Ok(());
            }
        }
    };
    ctx.state.casino_sessions.lock().expect("lock").remove(ctx.sender);
    resolve_dealer(ctx, bet, player, dealer).await
}

// ── Double ───────────────────────────────────────────────────────────────────

async fn do_double(ctx: CommandContext<'_>) -> anyhow::Result<()> {
    let (bet, player, dealer) = {
        let sessions = ctx.state.casino_sessions.lock().expect("lock");
        match sessions.get(ctx.sender) {
            Some(CasinoSession::Blackjack { bet, player_hand, dealer_hand }) => {
                (*bet, player_hand.clone(), dealer_hand.clone())
            }
            Some(_) => {
                let label = sessions.get(ctx.sender).map(session_label).unwrap_or("another");
                ctx.whisper(format!("In a {label} game, not blackjack."));
                return Ok(());
            }
            None => {
                ctx.whisper("No blackjack session. Start with !bj <bet>.");
                return Ok(());
            }
        }
    };

    if player.len() != 2 {
        ctx.whisper("Can only double on first two cards.");
        return Ok(());
    }

    // Deduct additional bet
    let balance = match ctx.state.api.casino_adjust(ctx.sender, -bet).await {
        Ok(b) => b,
        Err(CasinoAdjustErr::InsufficientFunds(_)) => {
            ctx.whisper("Not enough chips to double.");
            return Ok(());
        }
        Err(CasinoAdjustErr::NetworkErr) => {
            ctx.whisper("Casino unavailable right now.");
            return Ok(());
        }
    };

    let doubled_bet = bet * 2;
    let mut new_player = player;
    new_player.push(draw_card());
    let ps = score(&new_player);

    ctx.state.casino_sessions.lock().expect("lock").remove(ctx.sender);

    if ps > 21 {
        let rake = ((doubled_bet as f64) * RAKE_PCT).max(1.0) as i64;
        ctx.state.api.casino_jackpot_rake(rake).await;
        ctx.whisper(format!(
            "BJ | You: {} ({ps}) | Dealer: {} ? | Bust on double — Lost {} | Balance: {}",
            hand_str(&new_player), card_str(dealer[0]), chips_str(doubled_bet), chips_str(balance)
        ));
        return Ok(());
    }

    resolve_dealer(ctx, doubled_bet, new_player, dealer).await
}

// ── Quit ─────────────────────────────────────────────────────────────────────

async fn do_quit(ctx: CommandContext<'_>) -> anyhow::Result<()> {
    let removed = ctx.state.casino_sessions.lock().expect("lock").remove(ctx.sender);
    match removed {
        Some(CasinoSession::Blackjack { bet, .. }) => {
            let rake = ((bet as f64) * RAKE_PCT).max(1.0) as i64;
            ctx.state.api.casino_jackpot_rake(rake).await;
            let balance = ctx.state.api.casino_get_balance(ctx.sender).await.map(|b| b.chips).unwrap_or(0);
            ctx.whisper(format!("BJ | Quit — forfeited {} | Balance: {}", chips_str(bet), chips_str(balance)));
        }
        Some(_) => ctx.whisper("Quit that game with its own quit command."),
        None => ctx.whisper("No blackjack session active."),
    }
    Ok(())
}

// ── Dealer resolution ────────────────────────────────────────────────────────

async fn resolve_dealer(ctx: CommandContext<'_>, bet: i64, player: Vec<u8>, mut dealer: Vec<u8>) -> anyhow::Result<()> {
    // Dealer hits until >= 17
    while score(&dealer) < 17 {
        dealer.push(draw_card());
    }

    let ps = score(&player);
    let ds = score(&dealer);
    let dealer_display = hand_str(&dealer);

    let (result_msg, payout) = if ps > 21 {
        (format!("Bust — Lost {}", chips_str(bet)), 0i64)
    } else if ds > 21 {
        (format!("Dealer busts! +{}", chips_str(bet)), bet * 2)
    } else if ps > ds {
        (format!("Win! +{}", chips_str(bet)), bet * 2)
    } else if ps == ds {
        (format!("Push — returned {}", chips_str(bet)), bet)
    } else {
        (format!("Dealer wins — Lost {}", chips_str(bet)), 0)
    };

    let new_balance = if payout > 0 {
        match ctx.state.api.casino_adjust(ctx.sender, payout).await {
            Ok(b) => b,
            Err(_) => 0,
        }
    } else {
        let rake = ((bet as f64) * RAKE_PCT).max(1.0) as i64;
        ctx.state.api.casino_jackpot_rake(rake).await;
        ctx.state.api.casino_get_balance(ctx.sender).await.map(|b| b.chips).unwrap_or(0)
    };

    ctx.whisper(format!(
        "BJ | You: {} ({ps}) | Dealer: {} ({ds}) | {result_msg} | Balance: {}",
        hand_str(&player), dealer_display, chips_str(new_balance)
    ));
    Ok(())
}

fn session_label(s: &CasinoSession) -> &'static str {
    match s {
        CasinoSession::Craps { .. }       => "craps",
        CasinoSession::Hilo { .. }        => "hilo",
        CasinoSession::Blackjack { .. }   => "blackjack",
        CasinoSession::Poker { .. }       => "poker",
        CasinoSession::ConnectFour { .. } => "Connect Four (!c4 <1-7>)",
        CasinoSession::Chess { .. }       => "chess (!chess <from> <to>)",
    }
}
