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
        let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
            ctx.whisper_success("Could not resolve your UUID.");
            return Ok(());
        };
        let subcmd = ctx.args.first().copied().unwrap_or("").to_ascii_lowercase();
        match subcmd.as_str() {
            "hit" | "h"    => do_hit(ctx, &player_uuid).await,
            "stand" | "s"  => do_stand(ctx, &player_uuid).await,
            "double" | "d" => do_double(ctx, &player_uuid).await,
            "quit" | "q"   => do_quit(ctx, &player_uuid).await,
            _              => do_deal(ctx, &subcmd, &player_uuid).await,
        }
    })
}

// ── Deal ─────────────────────────────────────────────────────────────────────

async fn do_deal(ctx: CommandContext<'_>, stake_str: &str, player_uuid: &str) -> anyhow::Result<()> {
    {
        let sessions = ctx.state.casino_sessions.lock().expect("lock");
        if sessions.contains_key(ctx.sender) {
            ctx.whisper_success("Already in a game. Use !bj hit/stand/double/quit.");
            return Ok(());
        }
    }

    let Ok(bet) = stake_str.parse::<i64>() else {
        ctx.whisper_success(format!("Usage: !bj <bet> (min {MIN_BET}, max {MAX_BET})"));
        return Ok(());
    };
    if bet < MIN_BET || bet > MAX_BET {
        ctx.whisper_success(format!("Bet must be {MIN_BET}–{MAX_BET} chips."));
        return Ok(());
    }

    let balance = match ctx.state.api.casino_adjust(player_uuid, -bet).await {
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

    let player = vec![draw_card(), draw_card()];
    let dealer = vec![draw_card(), draw_card()];

    let pbj = is_blackjack(&player);
    let dbj = is_blackjack(&dealer);

    if dbj && pbj {
        // Push — return bet
        let new_balance = match ctx.state.api.casino_adjust(player_uuid, bet).await {
            Ok(b) => b,
            Err(_) => balance + bet,
        };
        ctx.whisper_success(format!(
            "BJ | You: {} (21) | Dealer: {} {} (21) | Both blackjack — Push | Balance: {}",
            hand_str(&player), card_str(dealer[0]), card_str(dealer[1]), chips_str(new_balance)
        ));
        return Ok(());
    }
    if dbj {
        ctx.state.api.casino_jackpot_rake(bet).await;
        ctx.whisper_success(format!(
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
        let win = match ctx.state.api.casino_win(player_uuid, payout).await {
            Ok(w) => w,
            Err(_) => crate::structure::endpoints::endpoints::CasinoWinResult {
                chips: balance + payout, alimony_paid: 0, ex_count: 0, net: payout,
            },
        };
        let alimony_note = if win.alimony_paid > 0 { format!(" (-{} alimony)", chips_str(win.alimony_paid)) } else { String::new() };
        ctx.whisper_success(format!(
            "BJ | You: {} (21) | Dealer: {} ? | BLACKJACK! +{}{alimony_note} | Balance: {}",
            hand_str(&player), card_str(dealer[0]),
            chips_str(payout - bet), chips_str(win.chips)
        ));
        return Ok(());
    }

    let _ps = score(&player);
    let can_double = balance >= bet; // still have enough after deduction
    let actions = if can_double { "Hit, Stand, or Double" } else { "Hit or Stand" };

    let started = super::try_start_session(ctx.state, ctx.sender, CasinoSession::Blackjack {
        bet,
        player_hand: player.clone(),
        dealer_hand: dealer.clone(),
    });
    if !started {
        let new_balance = ctx.state.api.casino_adjust(player_uuid, bet).await.unwrap_or(balance + bet);
        ctx.whisper_success(format!("Already in another game — this bet refunded. Balance: {}", chips_str(new_balance)));
        return Ok(());
    }

    ctx.whisper_success(state_msg(&player, dealer[0], &format!("{actions}? | Balance: {}", chips_str(balance))));
    Ok(())
}

// ── Hit ──────────────────────────────────────────────────────────────────────

async fn do_hit(ctx: CommandContext<'_>, player_uuid: &str) -> anyhow::Result<()> {
    let (bet, mut player, dealer) = {
        let sessions = ctx.state.casino_sessions.lock().expect("lock");
        match sessions.get(ctx.sender) {
            Some(CasinoSession::Blackjack { bet, player_hand, dealer_hand }) => {
                (*bet, player_hand.clone(), dealer_hand.clone())
            }
            Some(_) => {
                let label = sessions.get(ctx.sender).map(session_label).unwrap_or("another");
                ctx.whisper_success(format!("In a {label} game, not blackjack."));
                return Ok(());
            }
            None => {
                ctx.whisper_success("No blackjack session. Start with !bj <bet>.");
                return Ok(());
            }
        }
    };

    player.push(draw_card());
    let ps = score(&player);

    if ps > 21 {
        ctx.state.casino_sessions.lock().expect("lock").remove(ctx.sender);
        let balance = ctx.state.api.casino_get_balance(player_uuid).await.map(|b| b.chips).unwrap_or(0);
        ctx.state.api.casino_jackpot_rake(bet).await;
        ctx.whisper_success(format!(
            "BJ | You: {} ({ps}) | Dealer: {} ? | Bust — Lost {} | Balance: {}",
            hand_str(&player), card_str(dealer[0]), chips_str(bet), chips_str(balance)
        ));
        return Ok(());
    }

    if ps == 21 {
        // Auto-stand
        ctx.state.casino_sessions.lock().expect("lock").remove(ctx.sender);
        return resolve_dealer(ctx, bet, player, dealer, player_uuid).await;
    }

    ctx.state.casino_sessions.lock().expect("lock").insert(
        ctx.sender.to_owned(),
        CasinoSession::Blackjack { bet, player_hand: player.clone(), dealer_hand: dealer.clone() },
    );
    ctx.whisper_success(state_msg(&player, dealer[0], &format!("Hit or Stand? ({ps})")));
    Ok(())
}

// ── Stand ────────────────────────────────────────────────────────────────────

async fn do_stand(ctx: CommandContext<'_>, player_uuid: &str) -> anyhow::Result<()> {
    let (bet, player, dealer) = {
        let sessions = ctx.state.casino_sessions.lock().expect("lock");
        match sessions.get(ctx.sender) {
            Some(CasinoSession::Blackjack { bet, player_hand, dealer_hand }) => {
                (*bet, player_hand.clone(), dealer_hand.clone())
            }
            Some(_) => {
                let label = sessions.get(ctx.sender).map(session_label).unwrap_or("another");
                ctx.whisper_success(format!("In a {label} game, not blackjack."));
                return Ok(());
            }
            None => {
                ctx.whisper_success("No blackjack session. Start with !bj <bet>.");
                return Ok(());
            }
        }
    };
    ctx.state.casino_sessions.lock().expect("lock").remove(ctx.sender);
    resolve_dealer(ctx, bet, player, dealer, player_uuid).await
}

// ── Double ───────────────────────────────────────────────────────────────────

async fn do_double(ctx: CommandContext<'_>, player_uuid: &str) -> anyhow::Result<()> {
    let (bet, player, dealer) = {
        let sessions = ctx.state.casino_sessions.lock().expect("lock");
        match sessions.get(ctx.sender) {
            Some(CasinoSession::Blackjack { bet, player_hand, dealer_hand }) => {
                (*bet, player_hand.clone(), dealer_hand.clone())
            }
            Some(_) => {
                let label = sessions.get(ctx.sender).map(session_label).unwrap_or("another");
                ctx.whisper_success(format!("In a {label} game, not blackjack."));
                return Ok(());
            }
            None => {
                ctx.whisper_success("No blackjack session. Start with !bj <bet>.");
                return Ok(());
            }
        }
    };

    if player.len() != 2 {
        ctx.whisper_success("Can only double on first two cards.");
        return Ok(());
    }

    // Deduct additional bet
    let balance = match ctx.state.api.casino_adjust(player_uuid, -bet).await {
        Ok(b) => b,
        Err(CasinoAdjustErr::InsufficientFunds(_)) => {
            ctx.whisper_success("Not enough chips to double.");
            return Ok(());
        }
        Err(CasinoAdjustErr::NetworkErr) => {
            ctx.whisper_success("Casino unavailable right now.");
            return Ok(());
        }
    };

    let doubled_bet = bet * 2;
    let mut new_player = player;
    new_player.push(draw_card());
    let ps = score(&new_player);

    ctx.state.casino_sessions.lock().expect("lock").remove(ctx.sender);

    if ps > 21 {
        ctx.state.api.casino_jackpot_rake(doubled_bet).await;
        ctx.whisper_success(format!(
            "BJ | You: {} ({ps}) | Dealer: {} ? | Bust on double — Lost {} | Balance: {}",
            hand_str(&new_player), card_str(dealer[0]), chips_str(doubled_bet), chips_str(balance)
        ));
        return Ok(());
    }

    resolve_dealer(ctx, doubled_bet, new_player, dealer, player_uuid).await
}

// ── Quit ─────────────────────────────────────────────────────────────────────

async fn do_quit(ctx: CommandContext<'_>, player_uuid: &str) -> anyhow::Result<()> {
    let removed = ctx.state.casino_sessions.lock().expect("lock").remove(ctx.sender);
    match removed {
        Some(CasinoSession::Blackjack { bet, .. }) => {
            ctx.state.api.casino_jackpot_rake(bet).await;
            let balance = ctx.state.api.casino_get_balance(player_uuid).await.map(|b| b.chips).unwrap_or(0);
            ctx.whisper_success(format!("BJ | Quit — forfeited {} | Balance: {}", chips_str(bet), chips_str(balance)));
        }
        Some(_) => ctx.whisper_success("Quit that game with its own quit command."),
        None => ctx.whisper_success("No blackjack session active."),
    }
    Ok(())
}

// ── Dealer resolution ────────────────────────────────────────────────────────

async fn resolve_dealer(ctx: CommandContext<'_>, bet: i64, player: Vec<u8>, mut dealer: Vec<u8>, player_uuid: &str) -> anyhow::Result<()> {
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

    let mut alimony_note = String::new();
    let new_balance = if payout > bet {
        match ctx.state.api.casino_win(player_uuid, payout).await {
            Ok(win) => {
                if win.alimony_paid > 0 {
                    alimony_note = format!(" (-{} alimony)", chips_str(win.alimony_paid));
                }
                win.chips
            }
            Err(_) => 0,
        }
    } else if payout > 0 {
        match ctx.state.api.casino_adjust(player_uuid, payout).await {
            Ok(b) => b,
            Err(_) => 0,
        }
    } else {
        ctx.state.api.casino_jackpot_rake(bet).await;
        ctx.state.api.casino_get_balance(player_uuid).await.map(|b| b.chips).unwrap_or(0)
    };

    ctx.whisper_success(format!(
        "BJ | You: {} ({ps}) | Dealer: {} ({ds}) | {result_msg}{alimony_note} | Balance: {}",
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
