use rand::{Rng, rngs::OsRng};

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::mineflayer::bot::CasinoSession;

use super::chips_str;

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["craps"],
    description: "Craps. Usage: {prefix}craps pass|dontpass <bet> | {prefix}craps roll | quit",
    whitelisted: false,
    execute,
};

const MIN_BET: i64 = 10;
const MAX_BET: i64 = 5_000;
const RAKE_PCT: f64 = 0.02;

fn roll_dice() -> (u8, u8) {
    (OsRng.gen_range(1u8..=6), OsRng.gen_range(1u8..=6))
}

// ── Pure game logic ───────────────────────────────────────────────────────────

// Classifies dice total on the come-out roll.
enum ComeOutRoll { Natural, Craps, BarTwelve, Point(u8) }

fn come_out_eval(total: u8) -> ComeOutRoll {
    match total {
        7 | 11 => ComeOutRoll::Natural,
        12     => ComeOutRoll::BarTwelve,
        2 | 3  => ComeOutRoll::Craps,
        _      => ComeOutRoll::Point(total),
    }
}

// Classifies dice total in the point phase.
enum PointRoll { HitPoint, SevenOut, Ongoing }

fn point_phase_eval(total: u8, point: u32) -> PointRoll {
    if total as u32 == point { PointRoll::HitPoint }
    else if total == 7       { PointRoll::SevenOut }
    else                     { PointRoll::Ongoing }
}

// ── Command dispatch ──────────────────────────────────────────────────────────

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let subcmd = ctx.args.first().copied().unwrap_or("").to_ascii_lowercase();
        match subcmd.as_str() {
            "roll" | "r"                       => do_roll(ctx).await,
            "quit" | "q"                       => do_quit(ctx).await,
            "pass" | "p"                       => do_come_out(ctx, true).await,
            "dontpass" | "dp" | "dont" | "no"  => do_come_out(ctx, false).await,
            _ => {
                ctx.whisper("Usage: !craps pass <bet> | !craps dontpass <bet> | !craps roll | !craps quit");
                Ok(())
            }
        }
    })
}

// ── Come-out roll ─────────────────────────────────────────────────────────────

async fn do_come_out(ctx: CommandContext<'_>, pass_line: bool) -> anyhow::Result<()> {
    {
        let sessions = ctx.state.casino_sessions.lock().expect("lock");
        if let Some(s) = sessions.get(ctx.sender) {
            ctx.whisper(format!("Already in a {} game. Use !craps roll or !craps quit.", session_label(s)));
            return Ok(());
        }
    }

    let bet_str = ctx.args.get(1).copied().unwrap_or("");
    let Ok(bet) = bet_str.parse::<i64>() else {
        let line = if pass_line { "pass" } else { "dontpass" };
        ctx.whisper(format!("Usage: !craps {line} <bet>"));
        return Ok(());
    };
    if bet < MIN_BET || bet > MAX_BET {
        ctx.whisper(format!("Bet must be {MIN_BET}–{MAX_BET} chips."));
        return Ok(());
    }

    let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
        ctx.whisper("Could not resolve your UUID.");
        return Ok(());
    };

    let balance = match ctx.state.api.casino_adjust(&player_uuid, -bet).await {
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

    let (d1, d2) = roll_dice();
    let total = d1 + d2;
    let bet_label = if pass_line { "Pass" } else { "Don't Pass" };

    match come_out_eval(total) {
        ComeOutRoll::Natural => {
            if pass_line {
                let payout = bet * 2;
                let new_balance = ctx.state.api.casino_adjust(&player_uuid, payout).await.unwrap_or(balance + payout);
                ctx.whisper(format!("Craps [{d1}+{d2}={total}] Natural {total}! {bet_label} wins +{} | Balance: {}", chips_str(bet), chips_str(new_balance)));
            } else {
                let rake = ((bet as f64) * RAKE_PCT).max(1.0) as i64;
                ctx.state.api.casino_jackpot_rake(rake).await;
                ctx.whisper(format!("Craps [{d1}+{d2}={total}] Natural {total}! {bet_label} loses {} | Balance: {}", chips_str(bet), chips_str(balance)));
            }
        }
        ComeOutRoll::Craps => {
            if pass_line {
                let rake = ((bet as f64) * RAKE_PCT).max(1.0) as i64;
                ctx.state.api.casino_jackpot_rake(rake).await;
                ctx.whisper(format!("Craps [{d1}+{d2}={total}] Craps {total}! {bet_label} loses {} | Balance: {}", chips_str(bet), chips_str(balance)));
            } else {
                let payout = bet * 2;
                let new_balance = ctx.state.api.casino_adjust(&player_uuid, payout).await.unwrap_or(balance + payout);
                ctx.whisper(format!("Craps [{d1}+{d2}={total}] Craps {total}! {bet_label} wins +{} | Balance: {}", chips_str(bet), chips_str(new_balance)));
            }
        }
        ComeOutRoll::BarTwelve => {
            if pass_line {
                let rake = ((bet as f64) * RAKE_PCT).max(1.0) as i64;
                ctx.state.api.casino_jackpot_rake(rake).await;
                ctx.whisper(format!("Craps [{d1}+{d2}={total}] Craps 12! {bet_label} loses {} | Balance: {}", chips_str(bet), chips_str(balance)));
            } else {
                let new_balance = ctx.state.api.casino_adjust(&player_uuid, bet).await.unwrap_or(balance + bet);
                ctx.whisper(format!("Craps [{d1}+{d2}={total}] Craps 12 — {bet_label} push, returned {} | Balance: {}", chips_str(bet), chips_str(new_balance)));
            }
        }
        ComeOutRoll::Point(point) => {
            ctx.state.casino_sessions.lock().expect("lock").insert(
                ctx.sender.to_owned(),
                CasinoSession::Craps { bet, pass_line, point: point as u32 },
            );
            ctx.whisper(format!(
                "Craps [{d1}+{d2}={total}] Point is {point}! {bet_label} {}: roll {point} to win, 7 to lose. Use !craps roll.",
                chips_str(bet),
            ));
        }
    }

    Ok(())
}

// ── Point-phase roll ──────────────────────────────────────────────────────────

async fn do_roll(ctx: CommandContext<'_>) -> anyhow::Result<()> {
    let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
        ctx.whisper("Could not resolve your UUID.");
        return Ok(());
    };

    let (bet, pass_line, point) = {
        let sessions = ctx.state.casino_sessions.lock().expect("lock");
        match sessions.get(ctx.sender) {
            Some(CasinoSession::Craps { bet, pass_line, point }) => (*bet, *pass_line, *point),
            Some(s) => {
                ctx.whisper(format!("In a {} game, not craps.", session_label(s)));
                return Ok(());
            }
            None => {
                ctx.whisper("No craps session. Start with !craps pass/dontpass <bet>.");
                return Ok(());
            }
        }
    };

    let (d1, d2) = roll_dice();
    let total = d1 + d2;
    let bet_label = if pass_line { "Pass" } else { "Don't Pass" };

    match point_phase_eval(total, point) {
        PointRoll::HitPoint => {
            ctx.state.casino_sessions.lock().expect("lock").remove(ctx.sender);
            if pass_line {
                let payout = bet * 2;
                let new_balance = ctx.state.api.casino_adjust(&player_uuid, payout).await.unwrap_or(0);
                ctx.whisper(format!("Craps [{d1}+{d2}={total}] Hit the point {point}! {bet_label} wins +{} | Balance: {}", chips_str(bet), chips_str(new_balance)));
            } else {
                let rake = ((bet as f64) * RAKE_PCT).max(1.0) as i64;
                ctx.state.api.casino_jackpot_rake(rake).await;
                let balance = ctx.state.api.casino_get_balance(&player_uuid).await.map(|b| b.chips).unwrap_or(0);
                ctx.whisper(format!("Craps [{d1}+{d2}={total}] Hit the point {point}! {bet_label} loses {} | Balance: {}", chips_str(bet), chips_str(balance)));
            }
        }
        PointRoll::SevenOut => {
            ctx.state.casino_sessions.lock().expect("lock").remove(ctx.sender);
            if pass_line {
                let rake = ((bet as f64) * RAKE_PCT).max(1.0) as i64;
                ctx.state.api.casino_jackpot_rake(rake).await;
                let balance = ctx.state.api.casino_get_balance(&player_uuid).await.map(|b| b.chips).unwrap_or(0);
                ctx.whisper(format!("Craps [{d1}+{d2}={total}] Seven out! {bet_label} loses {} | Balance: {}", chips_str(bet), chips_str(balance)));
            } else {
                let payout = bet * 2;
                let new_balance = ctx.state.api.casino_adjust(&player_uuid, payout).await.unwrap_or(0);
                ctx.whisper(format!("Craps [{d1}+{d2}={total}] Seven out! {bet_label} wins +{} | Balance: {}", chips_str(bet), chips_str(new_balance)));
            }
        }
        PointRoll::Ongoing => {
            ctx.whisper(format!(
                "Craps [{d1}+{d2}={total}] Rolled {total} (need {point} or 7). Keep rolling with !craps roll.",
            ));
        }
    }

    Ok(())
}

// ── Quit ──────────────────────────────────────────────────────────────────────

async fn do_quit(ctx: CommandContext<'_>) -> anyhow::Result<()> {
    let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
        ctx.whisper("Could not resolve your UUID.");
        return Ok(());
    };
    let removed = ctx.state.casino_sessions.lock().expect("lock").remove(ctx.sender);
    match removed {
        Some(CasinoSession::Craps { bet, .. }) => {
            let rake = ((bet as f64) * RAKE_PCT).max(1.0) as i64;
            ctx.state.api.casino_jackpot_rake(rake).await;
            let balance = ctx.state.api.casino_get_balance(&player_uuid).await.map(|b| b.chips).unwrap_or(0);
            ctx.whisper(format!("Craps | Quit — forfeited {} | Balance: {}", chips_str(bet), chips_str(balance)));
        }
        Some(_) => ctx.whisper("Quit that game with its own quit command."),
        None    => ctx.whisper("No craps session active."),
    }
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
