pub mod blackjack;
pub mod connect_four;
pub mod craps;
pub mod hilo;
pub mod poker;
pub mod roulette;
pub mod scratch;
pub mod slots;

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::{CasinoFaucetResult, CasinoLottoPlayerTicket};
use futures_util::future::join_all;

// ── Shared helpers ────────────────────────────────────────────────────────────

pub fn chips_str(n: i64) -> String {
    format!("{} chip{}", n, if n == 1 { "" } else { "s" })
}

pub fn fmt_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

// ── !faucet ───────────────────────────────────────────────────────────────────

pub const FAUCET_COMMAND: CommandDefinition = CommandDefinition {
    names: &["faucet", "daily"],
    description: "Claim your daily chips. Streak bonuses at day 7, 14, 30.",
    whitelisted: false,
    execute: faucet_execute,
};

pub fn faucet_execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        match ctx.state.api.casino_faucet(ctx.sender).await {
            CasinoFaucetResult::Awarded { chips_awarded, streak, chips, lotto_pick, draw_date } => {
                ctx.whisper(format!(
                    "+{} | Day {} streak | Free: lotto {} + jackpot ticket (draw {}) | Balance: {}",
                    chips_str(chips_awarded),
                    streak,
                    lotto_pick.replace('-', " "),
                    draw_date,
                    chips_str(chips),
                ));
            }
            CasinoFaucetResult::OnCooldown { next_secs } => {
                ctx.whisper(format!("Already claimed. Next faucet in {}.", fmt_duration(next_secs)));
            }
            CasinoFaucetResult::Err => {
                ctx.whisper("Faucet unavailable right now.");
            }
        }
        Ok(())
    })
}

// ── !give ─────────────────────────────────────────────────────────────────────

pub const GIVE_COMMAND: CommandDefinition = CommandDefinition {
    names: &["give", "tip"],
    description: "Give chips to another player. Usage: {prefix}give <player> <amount>",
    whitelisted: false,
    execute: give_execute,
};

pub fn give_execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let (Some(target), Some(amount_str)) = (ctx.args.first(), ctx.args.get(1)) else {
            ctx.whisper("Usage: !give <player> <amount>");
            return Ok(());
        };
        let Ok(amount) = amount_str.parse::<i64>() else {
            ctx.whisper("Amount must be a number.");
            return Ok(());
        };
        if amount < 10 {
            ctx.whisper("Minimum transfer is 10 chips.");
            return Ok(());
        }
        if target.eq_ignore_ascii_case(ctx.sender) {
            ctx.whisper("Cannot give chips to yourself.");
            return Ok(());
        }
        match ctx.state.api.casino_transfer(ctx.sender, target, amount).await {
            Ok(()) => ctx.whisper(format!("Sent {} to {}.", chips_str(amount), target)),
            Err(e) => ctx.whisper(format!("Transfer failed: {e}")),
        }
        Ok(())
    })
}

// ── !jackpot ──────────────────────────────────────────────────────────────────

pub const JACKPOT_COMMAND: CommandDefinition = CommandDefinition {
    names: &["jackpot", "jp"],
    description: "View jackpot or buy a ticket. Usage: {prefix}jackpot [buy]",
    whitelisted: false,
    execute: jackpot_execute,
};

pub fn jackpot_execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        match ctx.args.first().copied() {
            Some("buy") => {
                let count: u32 = ctx.args.get(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1)
                    .max(1);
                match ctx.state.api.casino_jackpot_buy_ticket(ctx.sender, count).await {
                    Ok(info) => ctx.whisper(format!(
                        "Bought {} ticket{} | Pot: {} | Your tickets: {}",
                        count,
                        if count == 1 { "" } else { "s" },
                        chips_str(info.pot),
                        info.tickets
                    )),
                    Err(e) => ctx.whisper(format!("Could not buy ticket: {e}")),
                }
            }
            _ => {
                match ctx.state.api.casino_jackpot_get(Some(ctx.sender)).await {
                    Some(info) => ctx.whisper(format!(
                        "Jackpot pot: {} | Your tickets: {} | Draw: {}",
                        chips_str(info.pot),
                        info.tickets,
                        info.next_draw,
                    )),
                    None => ctx.whisper("Jackpot unavailable right now."),
                }
            }
        }
        Ok(())
    })
}

// ── !lotto ────────────────────────────────────────────────────────────────────

pub const LOTTO_COMMAND: CommandDefinition = CommandDefinition {
    names: &["lotto"],
    description: "Buy a lotto ticket. Usage: {prefix}lotto buy <n1> <n2> <n3> <n4> <n5> (5 unique numbers 1-40, costs 50 chips)",
    whitelisted: false,
    execute: lotto_execute,
};

pub fn lotto_execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        match ctx.args.first().copied() {
            Some("quick") | Some("q") => {
                let count: u32 = ctx.args.get(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1)
                    .max(1);
                if count == 1 {
                    let nums_str = lotto_quick_pick();
                    match ctx.state.api.casino_lotto_buy_ticket(ctx.sender, &nums_str).await {
                        Ok(info) => ctx.whisper(format!(
                            "Quick pick: {} | Draw: {} | Pot: {} | Balance: {}",
                            info.numbers.replace('-', " "),
                            info.draw_date,
                            chips_str(info.pot),
                            chips_str(info.chips)
                        )),
                        Err(e) => ctx.whisper(format!("Could not buy ticket: {e}")),
                    }
                } else {
                    match ctx.state.api.casino_lotto_buy_quick(ctx.sender, count).await {
                        Ok(info) => ctx.whisper(format!(
                            "Bought {} tickets | Draw: {} | Pot: {} | Balance: {}",
                            info.tickets.len(),
                            info.draw_date,
                            chips_str(info.pot),
                            chips_str(info.chips)
                        )),
                        Err(e) => ctx.whisper(format!("Could not buy tickets: {e}")),
                    }
                }
            }
            Some("pot") => {
                match ctx.state.api.casino_lotto_get_pot().await {
                    Some(info) => {
                        let draw = info.draw_date.as_deref().unwrap_or("TBD");
                        ctx.whisper(format!("Lotto pot: {} | Draw: {}", chips_str(info.pot), draw));
                    }
                    None => ctx.whisper("Could not fetch lotto pot."),
                }
            }
            Some("results") | Some("last") => {
                match ctx.state.api.casino_lotto_last_draw().await {
                    Some(draw) => ctx.whisper(format!(
                        "Last draw ({}): {}",
                        draw.draw_date,
                        draw.numbers.replace('-', " ")
                    )),
                    None => ctx.whisper("No draws yet."),
                }
            }
            Some("check") | Some("tickets") | Some("my") => {
                let tickets = ctx.state.api.casino_lotto_get_tickets(ctx.sender).await;
                if tickets.is_empty() {
                    ctx.whisper("No lotto tickets for the next draw.");
                } else {
                    ctx.whisper(format_lotto_tickets(&tickets));
                }
            }
            Some("buy") | Some("b") => {
                let num_args = &ctx.args[1..];
                if num_args.len() != 5 {
                    ctx.whisper("Pick exactly 5 numbers. Example: !lotto buy 3 12 17 28 35");
                    return Ok(());
                }

                let mut picks: Vec<u8> = match num_args.iter().map(|s| s.parse::<u8>()).collect() {
                    Ok(v) => v,
                    Err(_) => {
                        ctx.whisper("Invalid number. Each must be an integer between 1-40.");
                        return Ok(());
                    }
                };

                if picks.iter().any(|&n| n < 1 || n > 40) {
                    ctx.whisper("Numbers must be between 1 and 40.");
                    return Ok(());
                }

                picks.sort();
                if picks.windows(2).any(|w| w[0] == w[1]) {
                    ctx.whisper("Numbers must be unique.");
                    return Ok(());
                }

                let nums_str = picks.iter().map(|n| n.to_string()).collect::<Vec<_>>().join("-");
                match ctx.state.api.casino_lotto_buy_ticket(ctx.sender, &nums_str).await {
                    Ok(info) => ctx.whisper(format!(
                        "Ticket: {} | Draw: {} | Pot: {} | Balance: {}",
                        info.numbers.replace('-', " "),
                        info.draw_date,
                        chips_str(info.pot),
                        chips_str(info.chips)
                    )),
                    Err(e) => ctx.whisper(format!("Could not buy ticket: {e}")),
                }
            }
            _ => {
                ctx.whisper("Usage: !lotto buy <n1..n5> | !lotto quick | !lotto pot | !lotto check | !lotto results | 5 unique nums 1-40 | costs 50 chips | draw every Saturday.");
            }
        }
        Ok(())
    })
}

fn lotto_quick_pick() -> String {
    use rand::seq::SliceRandom;
    let mut pool: Vec<u8> = (1..=40).collect();
    pool.shuffle(&mut rand::thread_rng());
    let mut picks = pool[..5].to_vec();
    picks.sort();
    picks.iter().map(|n| n.to_string()).collect::<Vec<_>>().join("-")
}

fn format_lotto_tickets(tickets: &[CasinoLottoPlayerTicket]) -> String {
    let count = tickets.len();
    let draw_date = &tickets[0].draw_date;
    let shown: Vec<String> = tickets.iter().take(5).map(|t| t.numbers.replace('-', " ")).collect();
    let mut msg = format!("{} ticket{} (draw {}): {}", count, if count == 1 { "" } else { "s" }, draw_date, shown.join(" | "));
    if count > 5 {
        msg.push_str(&format!(" | ...and {} more", count - 5));
    }
    msg
}

// ── !wallet ───────────────────────────────────────────────────────────────────

pub const WALLET_COMMAND: CommandDefinition = CommandDefinition {
    names: &["wallet", "inv", "inventory", "chips", "balance", "bal"],
    description: "Show chips, streak, and ticket counts. Usage: {prefix}wallet [player]",
    whitelisted: false,
    execute: wallet_execute,
};

pub fn wallet_execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let target = ctx.args.first().copied().unwrap_or(ctx.sender);
        let (bal, jackpot, lotto_tickets, portfolio_summary) = tokio::join!(
            ctx.state.api.casino_get_balance(target),
            ctx.state.api.casino_jackpot_get(Some(target)),
            ctx.state.api.casino_lotto_get_tickets(target),
            portfolio_value_summary(ctx.state, target),
        );
        match bal {
            Some(b) => {
                let jp_info = jackpot;
                let jp_tickets = jp_info.as_ref().map(|j| j.tickets).unwrap_or(0);
                let jp_draw = jp_info.as_ref().map(|j| j.next_draw.as_str()).unwrap_or("").to_owned();
                let lotto_count = lotto_tickets.len();
                let lotto_draw_info = if lotto_count > 0 {
                    format!(" (draw {})", lotto_tickets[0].draw_date)
                } else {
                    String::new()
                };
                let jp_draw_info = if !jp_draw.is_empty() {
                    format!(" (draw {})", jp_draw)
                } else {
                    String::new()
                };
                let portfolio_part = match portfolio_summary {
                    Some((value, count)) => format!(" | Portfolio: {} ({} pos)", chips_str(value), count),
                    None => String::new(),
                };
                ctx.whisper(format!(
                    "{}: {}{} | Streak: {}d | Lotto: {} ticket{}{} | Jackpot: {} ticket{}{}",
                    target,
                    chips_str(b.chips),
                    portfolio_part,
                    b.streak,
                    lotto_count, if lotto_count == 1 { "" } else { "s" }, lotto_draw_info,
                    jp_tickets, if jp_tickets == 1 { "" } else { "s" }, jp_draw_info,
                ));
            }
            None => ctx.whisper(format!("Could not fetch wallet for {target}.")),
        }
        Ok(())
    })
}

async fn portfolio_value_summary(
    state: &crate::structure::mineflayer::bot::AzaleaState,
    player: &str,
) -> Option<(i64, usize)> {
    let positions = {
        let map = state.portfolio_positions.lock().ok()?;
        let v = map.get(player)?;
        if v.is_empty() { return None; }
        v.clone()
    };
    let count = positions.len();
    let quote_futures: Vec<_> = positions.iter()
        .map(|p| state.market_service.quote(&p.symbol))
        .collect();
    let quotes = join_all(quote_futures).await;
    let total: i64 = positions.iter().zip(quotes.iter()).map(|(pos, result)| {
        match result {
            Ok(q) => (pos.stake as f64 * q.price / pos.entry_price).ceil() as i64,
            Err(_) => pos.stake,
        }
    }).sum();
    Some((total, count))
}

// ── !addchips (admin) ─────────────────────────────────────────────────────────

pub const ADDCHIPS_COMMAND: CommandDefinition = CommandDefinition {
    names: &["addchips"],
    description: "Admin: give chips to a player. Usage: {prefix}addchips <player> <amount>",
    whitelisted: true,
    execute: addchips_execute,
};

pub fn addchips_execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let (Some(&target), Some(&amount_str)) = (ctx.args.first(), ctx.args.get(1)) else {
            ctx.whisper("Usage: !addchips <player> <amount>");
            return Ok(());
        };
        let Ok(amount) = amount_str.parse::<i64>() else {
            ctx.whisper("Amount must be a number.");
            return Ok(());
        };
        match ctx.state.api.casino_adjust(target, amount).await {
            Ok(bal) => ctx.whisper(format!("Gave {} to {}. Balance: {}.", chips_str(amount), target, chips_str(bal))),
            Err(_) => ctx.whisper("Failed."),
        }
        Ok(())
    })
}

// ── !draw (admin) ─────────────────────────────────────────────────────────────

pub const DRAW_COMMAND: CommandDefinition = CommandDefinition {
    names: &["draw"],
    description: "Force a casino draw (admin). Usage: {prefix}draw lotto|jackpot",
    whitelisted: true,
    execute: draw_execute,
};

pub fn draw_execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        match ctx.args.first().copied() {
            Some("lotto") => {
                if ctx.state.api.casino_fire_lotto_draw().await {
                    ctx.whisper("Lotto draw triggered.");
                } else {
                    ctx.whisper("Draw request failed.");
                }
            }
            Some("jackpot") => {
                if ctx.state.api.casino_fire_jackpot_draw().await {
                    ctx.whisper("Jackpot draw triggered.");
                } else {
                    ctx.whisper("Draw request failed.");
                }
            }
            _ => ctx.whisper("Usage: !draw lotto|jackpot"),
        }
        Ok(())
    })
}
