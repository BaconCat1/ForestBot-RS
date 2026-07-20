use rand::{Rng, rngs::OsRng};

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::mineflayer::bot::CasinoSession;

use super::{balance_str, chips_str, format_alimony};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["craps"],
    description: "Craps. Usage: {prefix}craps pass|dontpass <bet> | {prefix}craps roll | quit",
    whitelisted: false,
    execute,
};

const MIN_BET: i64 = 10;
const MAX_BET: i64 = 5_000;


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
                ctx.whisper_success("Usage: !craps pass <bet> | !craps dontpass <bet> | !craps roll | !craps quit");
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
            ctx.whisper_success(format!("Already in a {} game. Use !craps roll or !craps quit.", session_label(s)));
            return Ok(());
        }
    }

    let bet_str = ctx.args.get(1).copied().unwrap_or("");
    let Ok(bet) = bet_str.parse::<i64>() else {
        let line = if pass_line { "pass" } else { "dontpass" };
        ctx.whisper_success(format!("Usage: !craps {line} <bet>"));
        return Ok(());
    };
    if bet < MIN_BET || bet > MAX_BET {
        ctx.whisper_success(format!("Bet must be {MIN_BET}–{MAX_BET} chips."));
        return Ok(());
    }

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

    let (d1, d2) = roll_dice();
    let total = d1 + d2;
    let bet_label = if pass_line { "Pass" } else { "Don't Pass" };

    match come_out_eval(total) {
        ComeOutRoll::Natural => {
            if pass_line {
                let payout = bet * 2;
                match ctx.state.api.casino_win(&player_uuid, payout).await {
                    Ok(result) => {
                        let alimony_note = format_alimony(result.alimony_paid);
                        ctx.whisper_success(format!("Craps [{d1}+{d2}={total}] Natural {total}! {bet_label} wins +{}{alimony_note} | Balance: {}", chips_str(bet), chips_str(result.chips)));
                    }
                    Err(e) => {
                        eprintln!("[Craps] payout failed for {player_uuid}: {e:?}");
                        ctx.whisper_error(format!("Craps [{d1}+{d2}={total}] Natural {total}! {bet_label} wins, but payout failed. Contact an admin."));
                    }
                }
            } else {
                ctx.state.api.casino_jackpot_rake(bet).await;
                ctx.whisper_success(format!("Craps [{d1}+{d2}={total}] Natural {total}! {bet_label} loses {} | Balance: {}", chips_str(bet), chips_str(balance)));
            }
        }
        ComeOutRoll::Craps => {
            if pass_line {
                ctx.state.api.casino_jackpot_rake(bet).await;
                ctx.whisper_success(format!("Craps [{d1}+{d2}={total}] Craps {total}! {bet_label} loses {} | Balance: {}", chips_str(bet), chips_str(balance)));
            } else {
                let payout = bet * 2;
                match ctx.state.api.casino_win(&player_uuid, payout).await {
                    Ok(result) => {
                        let alimony_note = format_alimony(result.alimony_paid);
                        ctx.whisper_success(format!("Craps [{d1}+{d2}={total}] Craps {total}! {bet_label} wins +{}{alimony_note} | Balance: {}", chips_str(bet), chips_str(result.chips)));
                    }
                    Err(e) => {
                        eprintln!("[Craps] payout failed for {player_uuid}: {e:?}");
                        ctx.whisper_error(format!("Craps [{d1}+{d2}={total}] Craps {total}! {bet_label} wins, but payout failed. Contact an admin."));
                    }
                }
            }
        }
        ComeOutRoll::BarTwelve => {
            if pass_line {
                ctx.state.api.casino_jackpot_rake(bet).await;
                ctx.whisper_success(format!("Craps [{d1}+{d2}={total}] Craps 12! {bet_label} loses {} | Balance: {}", chips_str(bet), chips_str(balance)));
            } else {
                let new_balance = ctx.state.api.casino_adjust(&player_uuid, bet).await.unwrap_or(balance + bet);
                ctx.whisper_success(format!("Craps [{d1}+{d2}={total}] Craps 12 — {bet_label} push, returned {} | Balance: {}", chips_str(bet), chips_str(new_balance)));
            }
        }
        ComeOutRoll::Point(point) => {
            if !super::try_start_session(ctx.state, ctx.sender, CasinoSession::Craps { bet, pass_line, point: point as u32 }) {
                let new_balance = ctx.state.api.casino_adjust(&player_uuid, bet).await.unwrap_or(balance + bet);
                ctx.whisper_success(format!(
                    "Craps [{d1}+{d2}={total}] already in another game — this bet refunded ({}) | Balance: {}",
                    chips_str(bet), chips_str(new_balance)
                ));
                return Ok(());
            }
            ctx.whisper_success(format!(
                "Craps [{d1}+{d2}={total}] Point is {point}! {bet_label} {}: roll {point} to win, 7 to lose. Use !craps roll.",
                chips_str(bet),
            ));
        }
    }

    Ok(())
}

// ── Point-phase roll ──────────────────────────────────────────────────────────

async fn do_roll(ctx: CommandContext<'_>) -> anyhow::Result<()> {
    let Some(player_uuid) = ctx.require_player_uuid().await else { return Ok(()); };

    let (bet, pass_line, point) = {
        let sessions = ctx.state.casino_sessions.lock().expect("lock");
        match sessions.get(ctx.sender) {
            Some(CasinoSession::Craps { bet, pass_line, point }) => (*bet, *pass_line, *point),
            Some(s) => {
                ctx.whisper_success(format!("In a {} game, not craps.", session_label(s)));
                return Ok(());
            }
            None => {
                ctx.whisper_success("No craps session. Start with !craps pass/dontpass <bet>.");
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
                match ctx.state.api.casino_win(&player_uuid, payout).await {
                    Ok(result) => {
                        let alimony_note = format_alimony(result.alimony_paid);
                        ctx.whisper_success(format!("Craps [{d1}+{d2}={total}] Hit the point {point}! {bet_label} wins +{}{alimony_note} | Balance: {}", chips_str(bet), chips_str(result.chips)));
                    }
                    Err(e) => {
                        eprintln!("[Craps] payout failed for {player_uuid}: {e:?}");
                        ctx.whisper_error(format!("Craps [{d1}+{d2}={total}] Hit the point {point}! {bet_label} wins, but payout failed. Contact an admin."));
                    }
                }
            } else {
                ctx.state.api.casino_jackpot_rake(bet).await;
                let balance = ctx.state.api.casino_get_balance(&player_uuid).await.map(|b| b.chips);
                ctx.whisper_success(format!("Craps [{d1}+{d2}={total}] Hit the point {point}! {bet_label} loses {} | Balance: {}", chips_str(bet), balance_str(balance)));
            }
        }
        PointRoll::SevenOut => {
            ctx.state.casino_sessions.lock().expect("lock").remove(ctx.sender);
            if pass_line {
                ctx.state.api.casino_jackpot_rake(bet).await;
                let balance = ctx.state.api.casino_get_balance(&player_uuid).await.map(|b| b.chips);
                ctx.whisper_success(format!("Craps [{d1}+{d2}={total}] Seven out! {bet_label} loses {} | Balance: {}", chips_str(bet), balance_str(balance)));
            } else {
                let payout = bet * 2;
                match ctx.state.api.casino_win(&player_uuid, payout).await {
                    Ok(result) => {
                        let alimony_note = format_alimony(result.alimony_paid);
                        ctx.whisper_success(format!("Craps [{d1}+{d2}={total}] Seven out! {bet_label} wins +{}{alimony_note} | Balance: {}", chips_str(bet), chips_str(result.chips)));
                    }
                    Err(e) => {
                        eprintln!("[Craps] payout failed for {player_uuid}: {e:?}");
                        ctx.whisper_error(format!("Craps [{d1}+{d2}={total}] Seven out! {bet_label} wins, but payout failed. Contact an admin."));
                    }
                }
            }
        }
        PointRoll::Ongoing => {
            ctx.whisper_success(format!(
                "Craps [{d1}+{d2}={total}] Rolled {total} (need {point} or 7). Keep rolling with !craps roll.",
            ));
        }
    }

    Ok(())
}

// ── Quit ──────────────────────────────────────────────────────────────────────

async fn do_quit(ctx: CommandContext<'_>) -> anyhow::Result<()> {
    let Some(player_uuid) = ctx.require_player_uuid().await else { return Ok(()); };
    let removed = ctx.state.casino_sessions.lock().expect("lock").remove(ctx.sender);
    match removed {
        Some(CasinoSession::Craps { bet, .. }) => {
            ctx.state.api.casino_jackpot_rake(bet).await;
            let balance = ctx.state.api.casino_get_balance(&player_uuid).await.map(|b| b.chips);
            ctx.whisper_success(format!("Craps | Quit — forfeited {} | Balance: {}", chips_str(bet), balance_str(balance)));
        }
        Some(_) => ctx.whisper_success("Quit that game with its own quit command."),
        None    => ctx.whisper_success("No craps session active."),
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
