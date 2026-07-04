use crate::commands::{enqueue_chat, CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::market::types::now_unix;
use crate::structure::mineflayer::bot::AzaleaState;

use super::chips_str;

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["train", "trains"],
    description: "Train delay bets (2h window). !train list <country> | !train <country> <code> ontime|delayed <chips> | !train bets",
    whitelisted: false,
    execute,
};

const TRAINS_BASE: &str = "https://trainstracking.com";
const DELAY_THRESHOLD_MINS: i64 = 5;
const BET_DURATION_SECS: u64 = 7200;
const POLL_INTERVAL_SECS: u64 = 120;
const MAX_POLL_SECS: u64 = 3600;
const MIN_BET: i64 = 25;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TrainBet {
    pub id: i64,
    pub player: String,
    pub country: String,
    pub train_code: String,
    pub train_name: String,
    pub side: String,
    pub price: f64,
    pub stake: i64,
    pub close_time: u64,
}

enum PollOutcome {
    Found(bool),  // bool = is_delayed (delay > DELAY_THRESHOLD_MINS)
    Gone,         // not in realtime list — train arrived/cancelled → refund
    ApiError,     // network or parse failure → retry
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn normalize_country(s: &str) -> Option<&'static str> {
    match s.to_lowercase().as_str() {
        "us" | "usa" | "amtrak" | "united-states" | "unitedstates" | "america" => Some("united-states"),
        "de" | "germany" | "deutsche" | "db" => Some("germany"),
        "fr" | "france" | "sncf" => Some("france"),
        "be" | "belgium" | "belgie" | "irail" => Some("belgium"),
        "ch" | "switzerland" | "swiss" | "sbb" => Some("switzerland"),
        "fi" | "finland" => Some("finland"),
        "nl" | "netherlands" | "holland" | "ns" => Some("netherlands"),
        "no" | "norway" | "norge" => Some("norway"),
        "at" | "austria" | "obb" => Some("austria"),
        "se" | "sweden" | "sverige" | "sj" => Some("sweden"),
        "it" | "italy" | "italia" | "trenitalia" => Some("italy"),
        "es" | "spain" | "espana" | "renfe" => Some("spain"),
        "pl" | "poland" | "polska" => Some("poland"),
        "cz" | "czech" | "czech-republic" | "czechia" => Some("czech-republic"),
        "my" | "malaysia" | "ktm" => Some("malaysia"),
        _ => None,
    }
}

fn is_delayed(delay_mins: i64) -> bool {
    delay_mins > DELAY_THRESHOLD_MINS
}

// Currently delayed → ontime harder (3.03×); delayed easier (1.49×)
fn compute_odds(currently_delayed: bool) -> (f64, f64) {
    if currently_delayed {
        (0.33, 0.67) // (ontime_price, delayed_price)
    } else {
        (0.67, 0.33)
    }
}

fn fmt_close(close_time: u64) -> String {
    let now = now_unix();
    if close_time <= now { return "settling".into(); }
    let secs = close_time - now;
    if secs < 3600       { format!("{}m", secs / 60) }
    else if secs < 86400 { format!("{}h", secs / 3600) }
    else                 { format!("{}d", secs / 86400) }
}

async fn fetch_trains(client: &reqwest::Client, country: &str) -> Option<Vec<serde_json::Value>> {
    let url = format!("{TRAINS_BASE}/api/live/realtime?source={country}");
    let body: serde_json::Value = client
        .get(&url)
        .header("User-Agent", "ForestBot-RS/1.0")
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    body["trains"].as_array().cloned()
}

async fn poll_train(client: &reqwest::Client, country: &str, train_code: &str) -> PollOutcome {
    let Some(trains) = fetch_trains(client, country).await else {
        return PollOutcome::ApiError;
    };
    match trains.iter().find(|t| {
        t["trainCode"].as_str()
            .map(|c| c.eq_ignore_ascii_case(train_code))
            .unwrap_or(false)
    }) {
        Some(t) => {
            let delay_mins = t["delay"].as_i64().unwrap_or(0);
            PollOutcome::Found(is_delayed(delay_mins))
        }
        None => PollOutcome::Gone,
    }
}

// ── Command entry ─────────────────────────────────────────────────────────────

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        match ctx.args.first().copied().unwrap_or("") {
            "" => show_usage(&ctx),
            "bets" | "my" => show_bets(&ctx).await?,
            "list" => show_trains(&ctx, ctx.args.get(1).copied().unwrap_or("")).await?,
            _ => place_bet(&ctx).await?,
        }
        Ok(())
    })
}

fn show_usage(ctx: &CommandContext<'_>) {
    let p = &ctx.runtime.prefix;
    ctx.whisper(format!(
        "Train delay bets (2h): {p}train list <country> | {p}train <country> <code> ontime|delayed [chips] | {p}train bets | Omit chips for odds preview | Countries: us de fr be ch fi nl no at se it es pl cz my"
    ));
}

// ── show_trains ───────────────────────────────────────────────────────────────

async fn show_trains(ctx: &CommandContext<'_>, country_raw: &str) -> anyhow::Result<()> {
    let Some(country) = normalize_country(country_raw) else {
        ctx.whisper(format!(
            "Unknown country '{country_raw}'. Use: us de fr be ch fi nl no at se it es pl cz my"
        ));
        return Ok(());
    };
    let client = reqwest::Client::new();
    let Some(trains) = fetch_trains(&client, country).await else {
        ctx.whisper(format!("Could not fetch trains for {country}."));
        return Ok(());
    };
    if trains.is_empty() {
        ctx.whisper(format!("No running trains in {country} feed right now."));
        return Ok(());
    }
    let items: Vec<String> = trains.iter().take(8).filter_map(|t| {
        let code  = t["trainCode"].as_str()?;
        let delay = t["delay"].as_i64().unwrap_or(0);
        let d_str = if delay > 0 { format!("+{}m", delay) } else { "ontime".to_owned() };
        Some(format!("{code} {d_str}"))
    }).collect();
    ctx.whisper(format!(
        "[{country}] {} | {}train <country> <code> ontime|delayed <chips>",
        items.join(" | "),
        ctx.runtime.prefix,
    ));
    Ok(())
}

// ── show_bets ─────────────────────────────────────────────────────────────────

async fn show_bets(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
        ctx.whisper("Could not resolve your UUID.");
        return Ok(());
    };
    let all_bets = ctx.state.api.casino_train_bet_list().await;
    let player_bets: Vec<_> = all_bets.into_iter().filter(|b| b.player == player_uuid).collect();
    if player_bets.is_empty() {
        ctx.whisper("No open train bets.");
        return Ok(());
    }
    for bet in &player_bets {
        let payout = (bet.stake as f64 / bet.price).floor() as i64;
        ctx.whisper(format!(
            "[Train] {} ({}) {} {:.2}x | {} -> {} | {}",
            bet.train_name,
            bet.train_code,
            bet.side.to_uppercase(),
            1.0 / bet.price,
            chips_str(bet.stake),
            chips_str(payout),
            fmt_close(bet.close_time),
        ));
    }
    Ok(())
}

// ── place_bet ─────────────────────────────────────────────────────────────────

async fn place_bet(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    // args: <country> <code...> <ontime|delayed> [chips]
    // If last arg is ontime|delayed, it's a preview (no chips).
    if ctx.args.len() < 3 {
        show_usage(ctx);
        return Ok(());
    }
    let country_s = ctx.args[0];
    let last = ctx.args[ctx.args.len() - 1].to_lowercase();
    let preview = last == "ontime" || last == "delayed";
    let (side_s, amt_s, code_s) = if preview {
        (ctx.args[ctx.args.len() - 1], None, ctx.args[1..ctx.args.len() - 1].join(" "))
    } else {
        if ctx.args.len() < 4 { show_usage(ctx); return Ok(()); }
        (ctx.args[ctx.args.len() - 2], Some(ctx.args[ctx.args.len() - 1]), ctx.args[1..ctx.args.len() - 2].join(" "))
    };

    let Some(country) = normalize_country(country_s) else {
        ctx.whisper(format!(
            "Unknown country '{country_s}'. Use: us de fr be ch fi nl no at se it es pl cz my"
        ));
        return Ok(());
    };
    let side = side_s.to_lowercase();
    if side != "ontime" && side != "delayed" {
        ctx.whisper("Side must be ontime or delayed.");
        return Ok(());
    }

    let client = reqwest::Client::new();
    let Some(trains) = fetch_trains(&client, country).await else {
        ctx.whisper(format!("Could not fetch trains for {country}."));
        return Ok(());
    };
    let Some(train) = trains.iter().find(|t| {
        t["trainCode"].as_str()
            .map(|c| c.eq_ignore_ascii_case(&code_s))
            .unwrap_or(false)
    }) else {
        ctx.whisper(format!("Train '{code_s}' not found in {country} realtime feed."));
        return Ok(());
    };

    let train_code        = train["trainCode"].as_str().unwrap_or(&code_s).to_owned();
    let train_name        = train["name"].as_str().unwrap_or(&train_code).to_owned();
    let current_delay     = train["delay"].as_i64().unwrap_or(0);
    let currently_delayed = is_delayed(current_delay);
    let (ontime_price, delayed_price) = compute_odds(currently_delayed);
    let price = if side == "ontime" { ontime_price } else { delayed_price };

    if preview {
        let delay_str = if current_delay > 0 { format!("+{}m now", current_delay) } else { "on time now".to_owned() };
        ctx.whisper(format!(
            "[Train] {train_name} ({train_code}) | {delay_str} | ontime {:.2}x | delayed {:.2}x | {} {:.2}x selected",
            1.0 / ontime_price, 1.0 / delayed_price, side.to_uppercase(), 1.0 / price,
        ));
        return Ok(());
    }

    let amt_s = amt_s.unwrap();
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
            ctx.whisper(format!("Need {} but only have {}.", chips_str(stake), chips_str(have)));
            return Ok(());
        }
        Err(CasinoAdjustErr::NetworkErr) => {
            ctx.whisper("Casino unavailable.");
            return Ok(());
        }
    }

    let close_time = now_unix() + BET_DURATION_SECS;
    let delay_str  = if current_delay > 0 {
        format!("+{}m now", current_delay)
    } else {
        "on time now".to_owned()
    };

    let mut bet = TrainBet {
        id: 0,
        player: player_uuid.clone(),
        country: country.to_owned(),
        train_code: train_code.clone(),
        train_name: train_name.clone(),
        side: side.clone(),
        price,
        stake,
        close_time,
    };
    match ctx.state.api.casino_train_bet_insert(&bet).await {
        Some(id) => { bet.id = id; }
        None => {
            ctx.whisper("Failed to save bet. Refunding chips.");
            let _ = ctx.state.api.casino_adjust(&player_uuid, stake).await;
            return Ok(());
        }
    }
    {
        let mut bets = ctx.state.train_bets.lock().expect("train_bets lock");
        bets.entry(player_uuid.clone()).or_default().push(bet.clone());
    }

    let payout = (stake as f64 / price).floor() as i64;
    ctx.whisper(format!(
        "[Train] {train_name} ({train_code}) | {delay_str} | {} {:.2}x | {} | profit if win: +{} | settles in 2h",
        side.to_uppercase(),
        1.0 / price,
        chips_str(stake),
        chips_str(payout - stake),
    ));

    let wcmd = ctx.runtime.whisper_command.clone();
    tokio::spawn(settle_task(ctx.state.clone(), wcmd, bet));
    Ok(())
}

// ── settle_task ───────────────────────────────────────────────────────────────

pub async fn settle_task(state: AzaleaState, whisper_cmd: String, bet: TrainBet) {
    let now = now_unix();
    if bet.close_time > now {
        tokio::time::sleep(std::time::Duration::from_secs(bet.close_time - now)).await;
    }

    let claimed = {
        let mut bets = state.train_bets.lock().expect("train_bets lock");
        bets.get_mut(&bet.player)
            .map(|v| {
                let pos = v.iter().position(|b| b.id == bet.id);
                pos.map(|i| { v.remove(i); }).is_some()
            })
            .unwrap_or(false)
    };
    if !claimed { return; }

    let client = reqwest::Client::new();
    let online_username = state.players.read().ok()
        .and_then(|pl| pl.values().find(|s| s.uuid == bet.player).map(|s| s.username.clone()));

    let deadline = now_unix() + MAX_POLL_SECS;
    let outcome: Option<bool> = loop {
        match poll_train(&client, &bet.country, &bet.train_code).await {
            PollOutcome::Found(currently_delayed) => break Some(currently_delayed),
            PollOutcome::Gone => break None,
            PollOutcome::ApiError => {
                if now_unix() >= deadline { break None; }
                tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
            }
        }
    };

    state.api.casino_train_bet_delete(bet.id).await;

    let msg = match outcome {
        Some(is_delayed_result) => {
            let won        = (bet.side == "delayed") == is_delayed_result;
            let outcome_str = if is_delayed_result {
                format!("delayed >{}m", DELAY_THRESHOLD_MINS)
            } else {
                "on time".to_owned()
            };
            if won {
                let payout = (bet.stake as f64 / bet.price).floor() as i64;
                let _ = state.api.casino_adjust(&bet.player, payout).await;
                format!(
                    "[Train] {} ({}) — {}. {} wins. WIN +{} ({} @ {:.2}x).",
                    bet.train_name,
                    bet.train_code,
                    outcome_str,
                    bet.side.to_uppercase(),
                    chips_str(payout - bet.stake),
                    chips_str(bet.stake),
                    1.0 / bet.price,
                )
            } else {
                let _ = state.api.casino_jackpot_rake(bet.stake).await;
                format!(
                    "[Train] {} ({}) — {}. {} loses. LOSS -{} (to jackpot).",
                    bet.train_name,
                    bet.train_code,
                    outcome_str,
                    bet.side.to_uppercase(),
                    chips_str(bet.stake),
                )
            }
        }
        None => {
            let _ = state.api.casino_adjust(&bet.player, bet.stake).await;
            format!(
                "[Train] {} ({}) — train not found or API error. {} refunded.",
                bet.train_name,
                bet.train_code,
                chips_str(bet.stake),
            )
        }
    };

    if let Some(ref username) = online_username {
        enqueue_chat(&state, format!("/{whisper_cmd} {username} {msg}"));
    } else {
        state.api.casino_add_notification(&bet.player, &msg).await;
    }
}
