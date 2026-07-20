pub mod baccarat;
pub mod sic_bo;
pub mod blackjack;
pub mod faa_airport;
pub mod kalshi;
pub mod nasa_space_weather;
pub mod noaa_flooding;
pub mod gtfs_rt;
pub mod seismic;
pub mod train;
pub mod sports;
pub mod chess;
pub mod connect_four;
pub mod craps;
pub mod hilo;
pub mod poker;
pub mod roulette;
pub mod scratch;
pub mod slots;
pub mod mines;
pub mod aqi;
pub mod launch;
pub mod gas;
pub mod bets;

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::{CasinoFaucetResult, CasinoLottoPlayerTicket};
use crate::structure::market::types::now_unix;
use crate::structure::mineflayer::bot::AzaleaState;
use futures_util::future::join_all;

// ── Shared helpers ────────────────────────────────────────────────────────────

pub const MIN_BET: i64 = 25;
pub const HOUSE_EDGE: f64 = 0.03;

pub fn chips_str(n: i64) -> String {
    format!("{} chip{}", n, if n == 1 { "" } else { "s" })
}

/// Formats the "(-X alimony)" suffix appended to win messages when a forced-divorce
/// ex garnished part of the payout, or an empty string when none did.
pub fn format_alimony(alimony_paid: i64) -> String {
    if alimony_paid > 0 {
        format!(" (-{} alimony)", chips_str(alimony_paid))
    } else {
        String::new()
    }
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

pub fn to_price(p: f64) -> f64 {
    (p / (1.0 - HOUSE_EDGE)).clamp(0.05, 0.98)
}

pub fn fmt_odds(price: f64) -> String {
    format!("{:.2}×", 1.0 / price)
}

pub fn fmt_time(unix: u64) -> String {
    let now = now_unix();
    if unix <= now { return "now".into(); }
    let d = unix - now;
    if d < 3600       { format!("{}m", d / 60) }
    else if d < 86400 { format!("{}h", d / 3600) }
    else              { format!("{}d {}h", d / 86400, (d % 86400) / 3600) }
}

pub fn fmt_close(close_time: u64) -> String {
    let now = now_unix();
    if close_time <= now { return "settling".into(); }
    let secs = close_time - now;
    if secs < 3600       { format!("{}m", secs / 60) }
    else if secs < 86400 { format!("{}h", secs / 3600) }
    else                 { format!("{}d", secs / 86400) }
}

pub fn calc_payout(stake: i64, price: f64) -> i64 {
    (stake as f64 / price).floor() as i64
}

/// Atomically inserts a new session for `player`, but only if one doesn't already exist.
/// Every stateful game (craps/hilo/blackjack/chess/poker/connect_four) previously did an
/// early "already in a game?" check, then released the lock, deducted chips, and only
/// inserted the session afterward -- leaving a real race window where two concurrent
/// commands from the same player could both pass the early check before either inserted,
/// each deducting its own stake but only one session surviving in the map (the other
/// stake silently orphaned, no session left to resolve or refund it through). This closes
/// that window at the one place it actually matters: the atomic transition from "no
/// session" to "session exists". Returns `true` if inserted, `false` if a session already
/// existed (caller must refund whatever was staked for this attempt instead of proceeding).
pub fn try_start_session(
    state: &crate::structure::mineflayer::bot::AzaleaState,
    player: &str,
    session: crate::structure::mineflayer::bot::CasinoSession,
) -> bool {
    let mut sessions = state.casino_sessions.lock().expect("casino_sessions lock poisoned");
    if sessions.contains_key(player) {
        return false;
    }
    sessions.insert(player.to_owned(), session);
    true
}

/// Shared shape for the 11 event-bet types backed by Hub's consolidated
/// `/casino/bet/{type}` routes. `to_insert_json`/`from_json` carry the same
/// field-name mapping each type's insert/list method already hand-built --
/// relocated behind this trait so `ApiClient` needs only 3 generic methods
/// instead of one insert/list/delete trio per type.
pub trait CasinoBet: Sized {
    const TYPE: &'static str;
    fn to_insert_json(&self) -> serde_json::Value;
    fn from_json(item: &serde_json::Value) -> Option<Self>;
}

pub async fn sleep_until(t: u64) {
    let now = now_unix();
    if t > now {
        tokio::time::sleep(std::time::Duration::from_secs(t - now)).await;
    }
}

// ── API rate-limit helper ─────────────────────────────────────────────────────

#[derive(Debug)]
pub enum FetchErr { RateLimit, Error }

pub async fn check_resp(resp: reqwest::Response) -> Result<reqwest::Response, FetchErr> {
    match resp.status().as_u16() {
        429 => Err(FetchErr::RateLimit),
        200..=299 => Ok(resp),
        _ => Err(FetchErr::Error),
    }
}

pub async fn deliver(state: &AzaleaState, whisper_cmd: &str, player: &str, msg: String) {
    SettleDeps::from(state).deliver(whisper_cmd, player, msg).await
}

/// Minimal deps for a settle task -- every one of the ~13 async settle tasks
/// (aqi/gas/kalshi/launch/faa_airport/nasa_space_weather/noaa_flooding/quake/
/// volcano/train/sports/weather/market) turns out to touch exactly this same
/// 5-field core (confirmed via a direct field-usage audit, not guessed) and
/// nothing else from `AzaleaState`'s 69 fields except its own bet-type cache
/// map, which stays a separate explicit parameter since it's the one thing
/// that's genuinely different per task. Cheap to clone (all `Arc`), same as
/// cloning `AzaleaState` itself was before -- this is a coupling/readability
/// fix, not a performance one: a settle task's signature now says exactly
/// what it can touch, instead of "anything in the whole bot."
#[derive(Clone)]
pub struct SettleDeps {
    pub api: std::sync::Arc<crate::structure::endpoints::endpoints::ApiClient>,
    pub players: std::sync::Arc<std::sync::RwLock<std::collections::HashMap<String, crate::structure::mineflayer::bot::PlayerSnapshot>>>,
    pub runtime: std::sync::Arc<std::sync::RwLock<crate::structure::mineflayer::bot::RuntimeConfig>>,
    pub outbound_chat: std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<String>>>,
    pub recent_whispers: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, (String, std::time::Instant)>>>,
}

impl From<&AzaleaState> for SettleDeps {
    fn from(state: &AzaleaState) -> Self {
        SettleDeps {
            api: state.api.clone(),
            players: state.players.clone(),
            runtime: state.runtime.clone(),
            outbound_chat: state.outbound_chat.clone(),
            recent_whispers: state.recent_whispers.clone(),
        }
    }
}

impl SettleDeps {
    pub fn enqueue_chat(&self, message: impl AsRef<str>) {
        crate::commands::enqueue_chat_raw(&self.runtime, &self.recent_whispers, &self.outbound_chat, message)
    }

    pub async fn deliver(&self, whisper_cmd: &str, player: &str, msg: String) {
        let online = self.players.read().ok()
            .and_then(|pl| pl.values().find(|s| s.uuid == player).map(|s| s.username.clone()));
        if let Some(username) = online {
            self.enqueue_chat(format!("/{whisper_cmd} {username} {msg}"));
        } else {
            self.api.casino_add_notification(player, &msg).await;
        }
    }
}

// ── !faucet ───────────────────────────────────────────────────────────────────

pub const FAUCET_COMMAND: CommandDefinition = CommandDefinition {
    names: &["faucet", "daily"],
    description: "Claim your daily chips. Streak bonuses at day 7, 14, 30. Usage: {prefix}faucet",
    whitelisted: false,
    execute: faucet_execute,
};

pub fn faucet_execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(player_uuid) = ctx.require_player_uuid().await else { return Ok(()); };
        match ctx.state.api.casino_faucet(&player_uuid).await {
            CasinoFaucetResult::Awarded { chips_awarded, streak, chips, lotto_pick, draw_date } => {
                ctx.whisper_success(format!(
                    "+{} | Day {} streak | Free: lotto {} + jackpot ticket (draw {}) | Balance: {}",
                    chips_str(chips_awarded),
                    streak,
                    lotto_pick.replace('-', " "),
                    draw_date,
                    chips_str(chips),
                ));
            }
            CasinoFaucetResult::OnCooldown { next_secs } => {
                ctx.whisper_success(format!("Already claimed. Next faucet in {}.", fmt_duration(next_secs)));
            }
            CasinoFaucetResult::Err => {
                ctx.whisper_success("Faucet unavailable right now.");
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
            ctx.whisper_success("Usage: !give <player> <amount>");
            return Ok(());
        };
        let Ok(amount) = amount_str.parse::<i64>() else {
            ctx.whisper_success("Amount must be a number.");
            return Ok(());
        };
        if amount < 10 {
            ctx.whisper_success("Minimum transfer is 10 chips.");
            return Ok(());
        }
        if target.eq_ignore_ascii_case(ctx.sender) {
            ctx.whisper_success("Cannot give chips to yourself.");
            return Ok(());
        }
        let Some(from_uuid) = ctx.require_player_uuid().await else { return Ok(()); };
        let Some(to_uuid) = ctx.state.api.convert_username_to_uuid(target).await else {
            ctx.whisper_error(format!("Could not find UUID for {}.", target));
            return Ok(());
        };
        match ctx.state.api.casino_transfer(&from_uuid, &to_uuid, amount).await {
            Ok(()) => ctx.whisper_success(format!("Sent {} to {}.", chips_str(amount), target)),
            Err(e) => ctx.whisper_success(format!("Transfer failed: {e}")),
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
        let Some(player_uuid) = ctx.require_player_uuid().await else { return Ok(()); };
        match ctx.args.first().copied() {
            Some("buy") => {
                let count: u32 = ctx.args.get(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1)
                    .max(1);
                match ctx.state.api.casino_jackpot_buy_ticket(&player_uuid, count).await {
                    Ok(info) => ctx.whisper_success(format!(
                        "Bought {} ticket{} | Pot: {} | Your tickets: {}",
                        count,
                        if count == 1 { "" } else { "s" },
                        chips_str(info.pot),
                        info.tickets
                    )),
                    Err(e) => ctx.whisper_success(format!("Could not buy ticket: {e}")),
                }
            }
            _ => {
                match ctx.state.api.casino_jackpot_get(Some(&player_uuid)).await {
                    Some(info) => ctx.whisper_success(format!(
                        "Jackpot pot: {} | Your tickets: {} | Draw: {}",
                        chips_str(info.pot),
                        info.tickets,
                        info.next_draw,
                    )),
                    None => ctx.whisper_success("Jackpot unavailable right now."),
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
        let Some(player_uuid) = ctx.require_player_uuid().await else { return Ok(()); };
        match ctx.args.first().copied() {
            Some("quick") | Some("q") => {
                let count: u32 = ctx.args.get(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1)
                    .max(1);
                if count == 1 {
                    let nums_str = lotto_quick_pick();
                    match ctx.state.api.casino_lotto_buy_ticket(&player_uuid, &nums_str).await {
                        Ok(info) => ctx.whisper_success(format!(
                            "Quick pick: {} | Draw: {} | Pot: {} | Balance: {}",
                            info.numbers.replace('-', " "),
                            info.draw_date,
                            chips_str(info.pot),
                            chips_str(info.chips)
                        )),
                        Err(e) => ctx.whisper_success(format!("Could not buy ticket: {e}")),
                    }
                } else {
                    match ctx.state.api.casino_lotto_buy_quick(&player_uuid, count).await {
                        Ok(info) => ctx.whisper_success(format!(
                            "Bought {} tickets | Draw: {} | Pot: {} | Balance: {}",
                            info.tickets.len(),
                            info.draw_date,
                            chips_str(info.pot),
                            chips_str(info.chips)
                        )),
                        Err(e) => ctx.whisper_success(format!("Could not buy tickets: {e}")),
                    }
                }
            }
            Some("pot") => {
                match ctx.state.api.casino_lotto_get_pot().await {
                    Some(info) => {
                        let draw = info.draw_date.as_deref().unwrap_or("TBD");
                        ctx.whisper_success(format!("Lotto pot: {} | Draw: {}", chips_str(info.pot), draw));
                    }
                    None => ctx.whisper_success("Could not fetch lotto pot."),
                }
            }
            Some("results") | Some("last") => {
                match ctx.state.api.casino_lotto_last_draw().await {
                    Some(draw) => ctx.whisper_success(format!(
                        "Last draw ({}): {}",
                        draw.draw_date,
                        draw.numbers.replace('-', " ")
                    )),
                    None => ctx.whisper_success("No draws yet."),
                }
            }
            Some("check") | Some("tickets") | Some("my") => {
                let tickets = ctx.state.api.casino_lotto_get_tickets(&player_uuid).await;
                if tickets.is_empty() {
                    ctx.whisper_success("No lotto tickets for the next draw.");
                } else {
                    ctx.whisper_success(format_lotto_tickets(&tickets));
                }
            }
            Some("buy") | Some("b") => {
                let num_args = &ctx.args[1..];
                if num_args.len() != 5 {
                    ctx.whisper_success("Pick exactly 5 numbers. Example: !lotto buy 3 12 17 28 35");
                    return Ok(());
                }

                let mut picks: Vec<u8> = match num_args.iter().map(|s| s.parse::<u8>()).collect() {
                    Ok(v) => v,
                    Err(_) => {
                        ctx.whisper_success("Invalid number. Each must be an integer between 1-40.");
                        return Ok(());
                    }
                };

                if picks.iter().any(|&n| n < 1 || n > 40) {
                    ctx.whisper_success("Numbers must be between 1 and 40.");
                    return Ok(());
                }

                picks.sort();
                if picks.windows(2).any(|w| w[0] == w[1]) {
                    ctx.whisper_success("Numbers must be unique.");
                    return Ok(());
                }

                let nums_str = picks.iter().map(|n| n.to_string()).collect::<Vec<_>>().join("-");
                match ctx.state.api.casino_lotto_buy_ticket(&player_uuid, &nums_str).await {
                    Ok(info) => ctx.whisper_success(format!(
                        "Ticket: {} | Draw: {} | Pot: {} | Balance: {}",
                        info.numbers.replace('-', " "),
                        info.draw_date,
                        chips_str(info.pot),
                        chips_str(info.chips)
                    )),
                    Err(e) => ctx.whisper_success(format!("Could not buy ticket: {e}")),
                }
            }
            _ => {
                ctx.whisper_success("Usage: !lotto buy <n1..n5> | !lotto quick | !lotto pot | !lotto check | !lotto results | 5 unique nums 1-40 | costs 50 chips | draw every Saturday.");
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
        let target_name = ctx.args.first().copied().unwrap_or(ctx.sender);
        let Some(target_uuid) = ctx.state.api.convert_username_to_uuid(target_name).await else {
            ctx.whisper_error(format!("Could not resolve UUID for {}.", target_name));
            return Ok(());
        };
        let (bal, jackpot, lotto_tickets, portfolio_summary) = tokio::join!(
            ctx.state.api.casino_get_balance(&target_uuid),
            ctx.state.api.casino_jackpot_get(Some(target_uuid.as_str())),
            ctx.state.api.casino_lotto_get_tickets(&target_uuid),
            portfolio_value_summary(ctx.state, &target_uuid),
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
                ctx.whisper_success(format!(
                    "{}: {}{} | Streak: {}d | Lotto: {} ticket{}{} | Jackpot: {} ticket{}{}",
                    target_name,
                    chips_str(b.chips),
                    portfolio_part,
                    b.streak,
                    lotto_count, if lotto_count == 1 { "" } else { "s" }, lotto_draw_info,
                    jp_tickets, if jp_tickets == 1 { "" } else { "s" }, jp_draw_info,
                ));
            }
            None => ctx.whisper_success(format!("Could not fetch wallet for {target_name}.")),
        }
        Ok(())
    })
}

pub fn count_event_bets(state: &AzaleaState, player_uuid: &str) -> usize {
    let mut total = 0;
    if let Ok(m) = state.sports_bets.lock()        { total += m.get(player_uuid).map(|v| v.len()).unwrap_or(0); }
    if let Ok(m) = state.kalshi_bets.lock()        { total += m.get(player_uuid).map(|v| v.len()).unwrap_or(0); }
    if let Ok(m) = state.noaa_flooding_bets.lock() { total += m.get(player_uuid).map(|v| v.len()).unwrap_or(0); }
    if let Ok(m) = state.quake_bets.lock()         { total += m.get(player_uuid).map(|v| v.len()).unwrap_or(0); }
    if let Ok(m) = state.volcano_bets.lock()       { total += m.get(player_uuid).map(|v| v.len()).unwrap_or(0); }
    if let Ok(m) = state.aqi_bets.lock()           { total += m.get(player_uuid).map(|v| v.len()).unwrap_or(0); }
    if let Ok(m) = state.launch_bets.lock()        { total += m.get(player_uuid).map(|v| v.len()).unwrap_or(0); }
    if let Ok(m) = state.gas_bets.lock()           { total += m.get(player_uuid).map(|v| v.len()).unwrap_or(0); }
    total
}

async fn portfolio_value_summary(
    state: &AzaleaState,
    player_uuid: &str,
) -> Option<(i64, usize)> {
    let positions = {
        let map = state.portfolio_positions.lock().ok()?;
        let v = map.get(player_uuid)?;
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
            ctx.whisper_success("Usage: !addchips <player> <amount>");
            return Ok(());
        };
        let Ok(amount) = amount_str.parse::<i64>() else {
            ctx.whisper_success("Amount must be a number.");
            return Ok(());
        };
        let Some(target_uuid) = ctx.state.api.convert_username_to_uuid(target).await else {
            ctx.whisper_error(format!("Could not find UUID for {}.", target));
            return Ok(());
        };
        match ctx.state.api.casino_adjust(&target_uuid, amount).await {
            Ok(bal) => ctx.whisper_success(format!("Gave {} to {}. Balance: {}.", chips_str(amount), target, chips_str(bal))),
            Err(_) => ctx.whisper_success("Failed."),
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
                    ctx.whisper_success("Lotto draw triggered.");
                } else {
                    ctx.whisper_success("Draw request failed.");
                }
            }
            Some("jackpot") => {
                if ctx.state.api.casino_fire_jackpot_draw().await {
                    ctx.whisper_success("Jackpot draw triggered.");
                } else {
                    ctx.whisper_success("Draw request failed.");
                }
            }
            _ => ctx.whisper_success("Usage: !draw lotto|jackpot"),
        }
        Ok(())
    })
}
