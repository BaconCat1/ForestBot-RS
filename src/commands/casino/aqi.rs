use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::market::types::now_unix;

use super::{chips_str, fmt_close, fmt_odds, calc_payout, sleep_until, FetchErr, check_resp, SettleDeps};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["aqi", "airquality"],
    description: "AQI bets. !aqi <zip> | !aqi <zip> good|unhealthy <chips> | !aqi bets. Requires airnow_api_key.",
    whitelisted: false,
    execute,
};

const MIN_BET: i64 = 25;
const HOUSE_EDGE: f64 = 0.03;
const SETTLE_SECS: u64 = 24 * 3600; // 24h
const AIRNOW_BASE: &str = "https://www.airnowapi.org/aq";
const TIMEOUT_SECS: u64 = 10;

// ── Bet struct ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AqiBet {
    pub id:         i64,
    pub player:     String,
    pub zip:        String,
    pub area:       String, // ReportingArea from API
    pub side:       String, // "good" | "unhealthy"
    pub price:      f64,    // inflated probability (payout = stake / price)
    pub stake:      i64,
    pub close_time: u64,
}

impl super::CasinoBet for AqiBet {
    const TYPE: &'static str = "aqi";

    fn to_insert_json(&self) -> serde_json::Value {
        serde_json::json!({
            "player_uuid": self.player,
            "zip":         self.zip,
            "area":        self.area,
            "side":        self.side,
            "price":       self.price,
            "stake":       self.stake,
            "close_time":  self.close_time,
        })
    }

    fn from_json(item: &serde_json::Value) -> Option<Self> {
        Some(Self {
            id:         item.get("id")?.as_i64()?,
            player:     item.get("player_uuid")?.as_str()?.to_owned(),
            zip:        item.get("zip")?.as_str()?.to_owned(),
            area:       item.get("area")?.as_str()?.to_owned(),
            side:       item.get("side")?.as_str()?.to_owned(),
            price:      item.get("price")?.as_f64()?,
            stake:      item.get("stake")?.as_i64()?,
            close_time: item.get("close_time")?.as_u64()?,
        })
    }
}

// ── AirNow API helpers ────────────────────────────────────────────────────────

#[derive(Debug)]
struct AqiReading {
    parameter: String,
    aqi:       u32,
    cat_num:   u8,
    cat_name:  String,
    area:      String,
}

async fn fetch_forecast(client: &reqwest::Client, zip: &str, key: &str) -> Result<Vec<AqiReading>, FetchErr> {
    let date = tomorrow_date_str();
    let url = format!(
        "{AIRNOW_BASE}/forecast/zipCode/?format=application/json&zipCode={zip}&date={date}&distance=25&API_KEY={key}"
    );
    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
        .send()
        .await
        .map_err(|_| FetchErr::Error)?;
    let resp = check_resp(resp).await?;
    let body: serde_json::Value = resp.json().await.map_err(|_| FetchErr::Error)?;
    parse_readings_for_date(&body, Some(&date)).ok_or(FetchErr::Error)
}

async fn fetch_current(client: &reqwest::Client, zip: &str, key: &str) -> Result<Vec<AqiReading>, FetchErr> {
    let url = format!(
        "{AIRNOW_BASE}/observation/zipCode/current/?format=application/json&zipCode={zip}&distance=25&API_KEY={key}"
    );
    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
        .send()
        .await
        .map_err(|_| FetchErr::Error)?;
    let resp = check_resp(resp).await?;
    let body: serde_json::Value = resp.json().await.map_err(|_| FetchErr::Error)?;
    parse_readings(&body).ok_or(FetchErr::Error)
}

fn parse_readings(body: &serde_json::Value) -> Option<Vec<AqiReading>> {
    parse_readings_for_date(body, None)
}

fn parse_readings_for_date(body: &serde_json::Value, date_filter: Option<&str>) -> Option<Vec<AqiReading>> {
    let arr = body.as_array()?;
    if arr.is_empty() { return None; }
    let readings: Vec<AqiReading> = arr.iter().filter_map(|item| {
        if let Some(date) = date_filter {
            let item_date = item.get("DateForecast").and_then(|d| d.as_str()).unwrap_or("");
            if item_date != date { return None; }
        }
        let parameter = item["ParameterName"].as_str()?.to_owned();
        let aqi       = item["AQI"].as_i64().unwrap_or(-1).max(0) as u32;
        let cat_num   = item["Category"]["Number"].as_u64()? as u8;
        let cat_name  = item["Category"]["Name"].as_str()?.to_owned();
        let area      = item["ReportingArea"].as_str()?.to_owned();
        Some(AqiReading { parameter, aqi, cat_num, cat_name, area })
    }).collect();
    if readings.is_empty() { None } else { Some(readings) }
}

fn worst(readings: &[AqiReading]) -> Option<&AqiReading> {
    readings.iter().max_by_key(|r| r.aqi)
}

// ── Probability / pricing ─────────────────────────────────────────────────────

fn p_good(cat: u8) -> f64 {
    match cat {
        1 => 0.80,
        2 => 0.30,
        3 => 0.10,
        _ => 0.05,
    }
}

fn p_unhealthy(cat: u8) -> f64 {
    match cat {
        1 => 0.05,
        2 => 0.15,
        3 => 0.50,
        4 => 0.75,
        _ => 0.90,
    }
}

fn to_price(p: f64) -> f64 {
    (p / (1.0 - HOUSE_EDGE)).clamp(0.02, 0.98)
}

// ── Date helpers ──────────────────────────────────────────────────────────────

fn tomorrow_date_str() -> String {
    let days = (now_unix() + 86400) / 86400;
    let (y, m, d) = days_to_ymd(days as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

fn days_to_ymd(mut days: i64) -> (i32, u32, u32) {
    let mut y = 1970i32;
    loop {
        let dy: i64 = if is_leap(y) { 366 } else { 365 };
        if days < dy { break; }
        days -= dy;
        y += 1;
    }
    let months: [i64; 12] = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut mo = 1u32;
    for dm in months {
        if days < dm { break; }
        days -= dm;
        mo += 1;
    }
    (y, mo, days as u32 + 1)
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

// ── Command ───────────────────────────────────────────────────────────────────

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let key = ctx.runtime.airnow_api_key.trim().to_owned();
        if key.is_empty() {
            ctx.whisper_success("AQI betting is not configured (missing airnow_api_key).");
            return Ok(());
        }

        let subcmd = ctx.args.first().copied().unwrap_or("").to_ascii_lowercase();
        match subcmd.as_str() {
            "" => {
                let p = &ctx.runtime.prefix;
                ctx.whisper_success(format!(
                    "AQI bets (24h): {p}aqi <zip> | {p}aqi <zip> good|unhealthy <chips> | {p}aqi bets"
                ));
            }
            "bets" | "my" => show_bets(ctx).await?,
            _ => place_or_preview(ctx, key).await?,
        }
        Ok(())
    })
}

async fn show_bets(ctx: CommandContext<'_>) -> anyhow::Result<()> {
    let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
        ctx.whisper_success("Could not resolve your UUID.");
        return Ok(());
    };
    let bets = {
        let map = ctx.state.aqi_bets.lock().unwrap();
        map.get(&player_uuid).cloned().unwrap_or_default()
    };
    if bets.is_empty() {
        ctx.whisper_success("No open AQI bets.");
        return Ok(());
    }
    for b in &bets {
        ctx.whisper_success(format!(
            "[AQI] {} {} {} — {:.2}× payout on {} | closes in {}",
            b.zip, b.side.to_uppercase(),
            chips_str(b.stake),
            1.0 / b.price,
            b.area,
            fmt_close(b.close_time),
        ));
    }
    Ok(())
}

async fn place_or_preview(ctx: CommandContext<'_>, key: String) -> anyhow::Result<()> {
    let zip  = ctx.args.first().copied().unwrap_or("").to_owned();
    let side = ctx.args.get(1).copied().unwrap_or("").to_ascii_lowercase();
    let chips_str_arg = ctx.args.get(2).copied().unwrap_or("");

    if zip.is_empty() {
        ctx.whisper_success(format!("Usage: {}aqi <zip> [good|unhealthy] [chips]", ctx.runtime.prefix));
        return Ok(());
    }

    let client = reqwest::Client::new();

    let (current_result, forecast_result) = tokio::join!(
        fetch_current(&client, &zip, &key),
        fetch_forecast(&client, &zip, &key),
    );

    let forecast_readings = match forecast_result {
        Ok(r) => r,
        Err(FetchErr::RateLimit) => {
            ctx.whisper_success("AirNow API rate limit reached. Try again later.");
            return Ok(());
        }
        Err(_) => {
            ctx.whisper_error(format!("AirNow returned no forecast for zip {zip}. Check the zip code."));
            return Ok(());
        }
    };
    let Some(forecast_worst) = worst(&forecast_readings) else {
        ctx.whisper_success("No forecast data available.");
        return Ok(());
    };

    let area = forecast_worst.area.clone();
    let f_cat = forecast_worst.cat_num;

    let price_good      = to_price(p_good(f_cat));
    let price_unhealthy = to_price(p_unhealthy(f_cat));

    // Info display (always show, whether preview or placing)
    let current_str = current_result.ok()
        .as_ref()
        .and_then(|r| worst(r))
        .map(|w| format!("{} {} ({})", w.parameter, w.aqi, w.cat_name))
        .unwrap_or_else(|| "unavailable".to_owned());

    let forecast_str = format!("{} {} ({})", forecast_worst.parameter, forecast_worst.aqi, forecast_worst.cat_name);

    ctx.whisper_success(format!(
        "[AQI: {zip} / {area}] Now: {current_str} | Tomorrow forecast: {forecast_str} | \
         Good(≤50): {} | Unhealthy(>100): {}",
        fmt_odds(price_good),
        fmt_odds(price_unhealthy),
    ));

    if side.is_empty() { return Ok(()); }

    let price = match side.as_str() {
        "good"      => price_good,
        "unhealthy" => price_unhealthy,
        _ => {
            ctx.whisper_success("Side must be 'good' or 'unhealthy'.");
            return Ok(());
        }
    };

    let chips = match chips_str_arg.parse::<i64>() {
        Ok(n) if n >= MIN_BET => n,
        Ok(_) => {
            ctx.whisper_success(format!("Minimum bet: {} chips.", MIN_BET));
            return Ok(());
        }
        Err(_) => return Ok(()), // no chips = preview only
    };

    let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
        ctx.whisper_success("Could not resolve your UUID.");
        return Ok(());
    };

    // Deduct chips
    match ctx.state.api.casino_adjust(&player_uuid, -chips).await {
        Err(CasinoAdjustErr::InsufficientFunds(have)) => {
            ctx.whisper_success(format!("Not enough chips (have {}).", chips_str(have)));
            return Ok(());
        }
        Err(e) => { ctx.whisper_success(format!("Error: {e:?}")); return Ok(()); }
        Ok(_) => {}
    }

    let close_time = now_unix() + SETTLE_SECS;
    let mut bet = AqiBet {
        id: 0,
        player: player_uuid.clone(),
        zip: zip.clone(),
        area: area.clone(),
        side: side.clone(),
        price,
        stake: chips,
        close_time,
    };

    let id = ctx.state.api.casino_bet_insert(&bet).await;
    match id {
        Some(i) => bet.id = i,
        None => {
            if let Err(e) = ctx.state.api.casino_adjust(&player_uuid, chips).await {
                eprintln!("[AQI] refund failed for {player_uuid}: {e:?}");
                ctx.whisper_success("Failed to record bet. Refund also failed — contact an admin.");
            } else {
                ctx.whisper_success("Failed to record bet. Chips refunded.");
            }
            return Ok(());
        }
    }

    let payout = calc_payout(chips, price);
    ctx.whisper_success(format!(
        "[AQI] Bet placed: {} {} {} — pays {} if {} AQI ≤50 tomorrow | closes in 24h",
        zip,
        side.to_uppercase(),
        chips_str(chips),
        chips_str(payout),
        area,
    ));

    ctx.state.aqi_bets.lock().unwrap()
        .entry(player_uuid.clone())
        .or_default()
        .push(bet.clone());

    let deps = SettleDeps::from(ctx.state);
    let bets_map = ctx.state.aqi_bets.clone();
    let http = ctx.state.http.clone();
    let whisper_cmd = ctx.runtime.whisper_command.clone();
    tokio::spawn(aqi_settle_task(deps, bets_map, http, whisper_cmd, bet));

    Ok(())
}

// ── Settlement task ───────────────────────────────────────────────────────────

pub async fn aqi_settle_task(
    deps: SettleDeps,
    bets_map: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<AqiBet>>>>,
    http: reqwest::Client,
    whisper_cmd: String,
    bet: AqiBet,
) {
    sleep_until(bet.close_time).await;

    let claimed = {
        let mut bets = bets_map.lock().unwrap();
        bets.get_mut(&bet.player)
            .map(|v| v.iter().position(|b| b.id == bet.id).map(|i| { v.remove(i); }).is_some())
            .unwrap_or(false)
    };
    if !claimed { return; }

    let key = deps.runtime.read().expect("runtime lock").airnow_api_key.clone();
    let readings = fetch_current(&http, &bet.zip, &key).await.ok();

    deps.api.casino_bet_delete::<AqiBet>(bet.id).await;

    let msg = match readings.and_then(|r| worst(&r).map(|w| w.aqi)) {
        Some(actual_aqi) => {
            let won = match bet.side.as_str() {
                "good"      => actual_aqi <= 50,
                "unhealthy" => actual_aqi > 100,
                _           => false,
            };
            if won {
                let payout = calc_payout(bet.stake, bet.price);
                match deps.api.casino_win(&bet.player, payout).await {
                    Ok(win) => {
                        let alimony_note = if win.alimony_paid > 0 { format!(" (-{} alimony)", chips_str(win.alimony_paid)) } else { String::new() };
                        format!(
                            "[AQI] {} {} — actual AQI {}. {} wins. WIN +{}{alimony_note} ({} @ {:.2}×).",
                            bet.zip, bet.side.to_uppercase(), actual_aqi,
                            bet.side.to_uppercase(),
                            chips_str(payout - bet.stake),
                            chips_str(bet.stake),
                            1.0 / bet.price,
                        )
                    }
                    Err(e) => {
                        eprintln!("[AQI settle] casino_win failed for {}: {e:?}", bet.player);
                        format!("[AQI] {} {} wins but payout failed. Contact an admin.", bet.zip, bet.side.to_uppercase())
                    }
                }
            } else {
                deps.api.casino_jackpot_rake(bet.stake).await;
                format!(
                    "[AQI] {} {} — actual AQI {}. {} loses. LOSS -{} (to jackpot).",
                    bet.zip, bet.side.to_uppercase(), actual_aqi,
                    bet.side.to_uppercase(),
                    chips_str(bet.stake),
                )
            }
        }
        None => {
            match deps.api.casino_adjust(&bet.player, bet.stake).await {
                Ok(_) => format!(
                    "[AQI] {} {} — AirNow API unavailable at settlement. {} refunded.",
                    bet.zip, bet.side.to_uppercase(),
                    chips_str(bet.stake),
                ),
                Err(e) => {
                    eprintln!("[AQI settle] refund failed for {}: {e:?}", bet.player);
                    format!("[AQI] {} {} — AirNow API unavailable. Refund failed — contact an admin.", bet.zip, bet.side.to_uppercase())
                }
            }
        }
    };

    deps.deliver(&whisper_cmd, &bet.player, msg).await;
}
