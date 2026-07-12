use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::market::types::now_unix;
use crate::structure::mineflayer::bot::AzaleaState;

use super::{chips_str, fmt_close, calc_payout, sleep_until, deliver, FetchErr, check_resp};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["faa", "airport"],
    description: "Flight conditions bets (2h window). !faa <ICAO> — current conditions + odds | !faa <ICAO> yes|no <chips> — bet IFR/LIFR (yes) or VFR/MVFR (no) | !faa bets",
    whitelisted: false,
    execute,
};

const METAR_BASE: &str = "https://aviationweather.gov/api/data/metar";
const BET_DURATION_SECS: u64 = 7200;
const POLL_INTERVAL_SECS: u64 = 120;
const MAX_POLL_SECS: u64 = 3600;
const MIN_BET: i64 = 25;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FaaAirportBet {
    pub id: i64,
    pub player: String,
    pub airport_code: String,
    pub name: String,
    pub side: String,
    pub price: f64,
    pub stake: i64,
    pub close_time: u64,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

// Convert IATA (3-char, e.g. "JFK") to ICAO by prepending K.
// ICAO (4-char, e.g. "KJFK") passed through unchanged.
fn to_icao(code: &str) -> String {
    if code.len() == 3 { format!("K{code}") } else { code.to_owned() }
}

// IFR or LIFR = instrument conditions (low visibility/ceiling) → delays likely
fn is_ifr(flt_cat: &str) -> bool {
    flt_cat == "IFR" || flt_cat == "LIFR"
}

// Odds based on current flight category.
// Currently IFR:  YES likely to continue → lower payout; NO contrarian → higher
// Currently VFR:  YES risky → higher payout; NO safe → lower
fn compute_odds(currently_ifr: bool) -> (f64, f64) {
    const RAKE: f64 = 0.03;
    if currently_ifr {
        (0.67 / (1.0 - RAKE), 0.33 / (1.0 - RAKE))
    } else {
        (0.33 / (1.0 - RAKE), 0.67 / (1.0 - RAKE))
    }
}

async fn fetch_metar(client: &reqwest::Client, icao: &str) -> Result<serde_json::Value, FetchErr> {
    let url = format!("{METAR_BASE}?ids={icao}&format=json");
    let resp = client.get(&url).send().await.map_err(|_| FetchErr::Error)?;
    let resp = check_resp(resp).await?;
    let arr: serde_json::Value = resp.json().await.map_err(|_| FetchErr::Error)?;
    arr.as_array().and_then(|a| a.first().cloned()).ok_or(FetchErr::Error)
}

async fn poll_flt_cat(client: &reqwest::Client, icao: &str) -> Option<String> {
    let v = fetch_metar(client, icao).await.ok()?;
    v["fltCat"].as_str().map(|s| s.to_owned())
}

// ── Command entry ─────────────────────────────────────────────────────────────

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        match ctx.args.first().copied().unwrap_or("") {
            "" => show_usage(&ctx),
            "bets" | "my" => show_bets(&ctx).await?,
            code if ctx.args.len() == 1 => show_airport(&ctx, &code.to_uppercase()).await?,
            _ => place_bet(&ctx).await?,
        }
        Ok(())
    })
}

fn show_usage(ctx: &CommandContext<'_>) {
    let p = &ctx.runtime.prefix;
    ctx.whisper(format!(
        "Airport conditions bets (2h window, US airports): {p}faa <IATA/ICAO> — conditions+odds | {p}faa <code> yes|no <chips> — bet IFR/worse (yes) or VFR/MVFR (no) | {p}faa bets | Examples: JFK BOS ORD LAX KLAX"
    ));
}

// ── show_airport ──────────────────────────────────────────────────────────────

async fn show_airport(ctx: &CommandContext<'_>, code: &str) -> anyhow::Result<()> {
    let icao = to_icao(code);
    let client = reqwest::Client::new();
    let v = match fetch_metar(&client, &icao).await {
        Ok(v) => v,
        Err(FetchErr::RateLimit) => {
            ctx.whisper("aviationweather.gov API rate limit reached. Try again later.");
            return Ok(());
        }
        Err(_) => {
            ctx.whisper(format!("Could not fetch METAR for {code}. Check IATA/ICAO code."));
            return Ok(());
        }
    };
    let flt_cat = v["fltCat"].as_str().unwrap_or("?");
    let name = v["name"].as_str().unwrap_or(code);
    let currently_ifr = is_ifr(flt_cat);
    let (yes_price, no_price) = compute_odds(currently_ifr);

    ctx.whisper(format!(
        "{name} ({icao}) | {flt_cat} | YES (IFR/LIFR) {:.2}x NO (VFR/MVFR) {:.2}x | {}faa {code} yes|no <chips> (2h)",
        1.0 / yes_price, 1.0 / no_price, ctx.runtime.prefix
    ));
    Ok(())
}

// ── show_bets ─────────────────────────────────────────────────────────────────

async fn show_bets(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
        ctx.whisper("Could not resolve your UUID.");
        return Ok(());
    };
    let all_bets = ctx.state.api.casino_faa_airport_bet_list().await;
    let player_bets: Vec<_> = all_bets.into_iter().filter(|b| b.player == player_uuid).collect();
    if player_bets.is_empty() {
        ctx.whisper("No open airport condition bets.");
        return Ok(());
    }
    for bet in &player_bets {
        let payout = (bet.stake as f64 / bet.price).floor() as i64;
        ctx.whisper(format!(
            "[FAA] {} ({}) {} {:.2}x | {} -> {} | {}",
            bet.name, bet.airport_code,
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
    let (Some(&code_s), Some(&side_s), Some(&amt_s)) =
        (ctx.args.first(), ctx.args.get(1), ctx.args.get(2))
    else {
        show_usage(ctx);
        return Ok(());
    };
    let code = code_s.to_uppercase();
    let icao = to_icao(&code);
    let side = side_s.to_lowercase();
    if side != "yes" && side != "no" {
        ctx.whisper("Side must be yes or no.");
        return Ok(());
    }
    let Ok(stake) = amt_s.parse::<i64>() else {
        ctx.whisper("Chip amount must be a number.");
        return Ok(());
    };
    if stake < MIN_BET {
        ctx.whisper(format!("Minimum bet is {}.", chips_str(MIN_BET)));
        return Ok(());
    }

    let client = reqwest::Client::new();
    let v = match fetch_metar(&client, &icao).await {
        Ok(v) => v,
        Err(FetchErr::RateLimit) => {
            ctx.whisper("aviationweather.gov API rate limit reached. Try again later.");
            return Ok(());
        }
        Err(_) => {
            ctx.whisper(format!("Could not fetch METAR for {code}."));
            return Ok(());
        }
    };
    let flt_cat = v["fltCat"].as_str().unwrap_or("VFR");
    let name = v["name"].as_str().unwrap_or(&code).to_owned();
    let currently_ifr = is_ifr(flt_cat);
    let (yes_price, no_price) = compute_odds(currently_ifr);
    let price = if side == "yes" { yes_price } else { no_price };

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
    let mut bet = FaaAirportBet {
        id: 0,
        player: player_uuid.clone(),
        airport_code: icao.clone(),
        name: name.clone(),
        side: side.clone(),
        price,
        stake,
        close_time,
    };
    match ctx.state.api.casino_faa_airport_bet_insert(&bet).await {
        Some(id) => { bet.id = id; }
        None => {
            if let Err(e) = ctx.state.api.casino_adjust(&player_uuid, stake).await {
                eprintln!("[FAA] refund failed for {player_uuid}: {e:?}");
                ctx.whisper("Failed to save bet. Refund also failed — contact an admin.");
            } else {
                ctx.whisper("Failed to save bet. Chips refunded.");
            }
            return Ok(());
        }
    }
    {
        let mut bets = ctx.state.faa_airport_bets.lock().expect("faa_airport_bets lock");
        bets.entry(player_uuid.clone()).or_default().push(bet.clone());
    }

    let payout = (stake as f64 / price).floor() as i64;
    ctx.whisper(format!(
        "[FAA] {name} ({icao}) | {flt_cat} now | {} {:.2}x | {} | profit if win: +{} | settles in 2h",
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

pub async fn settle_task(
    state: AzaleaState,
    whisper_cmd: String,
    bet: FaaAirportBet,
) {
    sleep_until(bet.close_time).await;

    let claimed = {
        let mut bets = state.faa_airport_bets.lock().expect("faa_airport_bets lock");
        bets.get_mut(&bet.player)
            .map(|v| {
                let pos = v.iter().position(|b| b.id == bet.id);
                pos.map(|i| { v.remove(i); }).is_some()
            })
            .unwrap_or(false)
    };
    if !claimed { return; }

    let client = reqwest::Client::new();

    let deadline = now_unix() + MAX_POLL_SECS;
    let result: Option<String> = loop {
        match poll_flt_cat(&client, &bet.airport_code).await {
            Some(cat) => break Some(cat),
            None => {
                if now_unix() >= deadline { break None; }
                tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
            }
        }
    };

    state.api.casino_faa_airport_bet_delete(bet.id).await;

    let (flt_cat, outcome_is_ifr) = match result {
        Some(ref cat) => (cat.as_str(), is_ifr(cat)),
        None => {
            if let Err(e) = state.api.casino_adjust(&bet.player, bet.stake).await {
                eprintln!("[FAA settle] refund failed for {}: {e:?}", bet.player);
            }
            let msg = format!(
                "[FAA] {} ({}) — METAR unavailable. {} refunded.",
                bet.name, bet.airport_code, chips_str(bet.stake)
            );
            deliver(&state, &whisper_cmd, &bet.player, msg).await;
            return;
        }
    };

    let won = (bet.side == "yes") == outcome_is_ifr;

    let msg = if won {
        let payout = calc_payout(bet.stake, bet.price);
        if let Err(e) = state.api.casino_adjust(&bet.player, payout).await {
            eprintln!("[FAA settle] casino_adjust failed for {}: {e:?}", bet.player);
        }
        format!(
            "[FAA] {} ({}) — {}. {} wins. WIN +{} ({} @ {:.2}x).",
            bet.name, bet.airport_code, flt_cat,
            bet.side.to_uppercase(),
            chips_str(payout - bet.stake),
            chips_str(bet.stake),
            1.0 / bet.price,
        )
    } else {
        state.api.casino_jackpot_rake(bet.stake).await;
        format!(
            "[FAA] {} ({}) — {}. {} loses. LOSS -{} (to jackpot).",
            bet.name, bet.airport_code, flt_cat,
            bet.side.to_uppercase(),
            chips_str(bet.stake),
        )
    };

    deliver(&state, &whisper_cmd, &bet.player, msg).await;
}
