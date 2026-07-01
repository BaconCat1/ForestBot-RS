use crate::commands::{enqueue_chat, CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::market::types::now_unix;
use crate::structure::mineflayer::bot::AzaleaState;

use super::casino::chips_str;

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["weather", "w"],
    description: "Current weather or rain futures. !weather <city> | !weather bet <city> rain yes/no <chips> <1d/3d/7d/14d> | !weather bets",
    whitelisted: false,
    execute,
};

#[derive(Debug, Clone)]
pub struct WeatherBet {
    pub id: i64,
    pub player: String,
    pub city: String,
    pub latitude: f64,
    pub longitude: f64,
    pub rain_yes: bool,
    pub forecast_prob: u8,
    pub payout_mult: f64,
    pub stake: i64,
    pub closes_unix: u64,
    pub duration_label: String,
}

const MIN_BET: i64 = 50;

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let arg = ctx.args.first().copied().unwrap_or("").to_lowercase();
        match arg.as_str() {
            "bet"  => place_bet(&ctx).await?,
            "bets" => {
                let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
                    ctx.whisper("Could not resolve your UUID.");
                    return Ok(());
                };
                show_bets(&ctx, &player_uuid);
            }
            "odds" => {
                let location = ctx.args[1..].join(" ");
                if location.is_empty() {
                    ctx.whisper("Usage: !weather odds <city>");
                } else {
                    match fetch_odds_preview(&location).await {
                        Some(msg) => ctx.whisper(msg),
                        None => ctx.whisper(format!("Couldn't fetch odds for: {location}")),
                    }
                }
            }
            _ => {
                if ctx.args.is_empty() {
                    ctx.whisper(format!(
                        "Usage: {}weather <city> | {}weather odds <city> | {}weather bet <city> rain yes/no <chips> <1d/3d/7d/14d> | {}weather bets",
                        ctx.runtime.prefix, ctx.runtime.prefix, ctx.runtime.prefix, ctx.runtime.prefix
                    ));
                } else {
                    let location = ctx.args.join(" ");
                    match fetch_weather(&location).await {
                        Some((wx_msg, rain_odds)) => {
                            ctx.chat(wx_msg);
                            if let Some(odds) = rain_odds {
                                ctx.whisper(odds);
                            }
                        }
                        None => ctx.chat(format!("No weather data found for: {location}")),
                    }
                }
            }
        }
        Ok(())
    })
}

// ── Current weather ───────────────────────────────────────────────────────────

async fn fetch_weather(location: &str) -> Option<(String, Option<String>)> {
    let client = reqwest::Client::new();
    let (lat, lon, city, country, population) = geocode(&client, location).await?;

    let wx_url = format!(
        "https://api.open-meteo.com/v1/forecast\
        ?latitude={lat:.4}&longitude={lon:.4}\
        &current=temperature_2m,apparent_temperature,weather_code,\
        wind_speed_10m,wind_direction_10m,relative_humidity_2m,precipitation,is_day\
        &daily=precipitation_probability_max&forecast_days=2\
        &wind_speed_unit=kmh&temperature_unit=celsius&timezone=auto"
    );
    let wx: serde_json::Value = client
        .get(&wx_url)
        .header("User-Agent", "ForestBot/1.0")
        .send().await.ok()?.json().await.ok()?;

    let cur = &wx["current"];
    let temp      = cur["temperature_2m"].as_f64()?;
    let feels     = cur["apparent_temperature"].as_f64()?;
    let code      = cur["weather_code"].as_u64().unwrap_or(0);
    let wind_spd  = cur["wind_speed_10m"].as_f64().unwrap_or(0.0);
    let wind_deg  = cur["wind_direction_10m"].as_f64().unwrap_or(0.0);
    let humidity  = cur["relative_humidity_2m"].as_f64().unwrap_or(0.0);
    let precip    = cur["precipitation"].as_f64().unwrap_or(0.0);
    let is_day    = cur["is_day"].as_u64().unwrap_or(1) == 1;

    let desc      = wmo_desc(code);
    let dir       = wind_dir(wind_deg);
    let day_emoji = if is_day { "☀" } else { "🌙" };

    let local_time = wx["utc_offset_seconds"].as_i64().map(|offset| {
        let utc   = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
        let local = utc + offset;
        let h = (local % 86400) / 3600;
        let m = (local % 3600) / 60;
        format!(" [{h:02}:{m:02} local]")
    }).unwrap_or_default();

    let pop_str = population.map(|p| format!(", pop. {p}")).unwrap_or_default();
    let mut msg = format!(
        "{day_emoji} {city}, {country}{pop_str}{local_time}: {temp:.0}°C (feels {feels:.0}°C), {desc} | Wind {wind_spd:.0} km/h {dir} | {humidity:.0}% humidity"
    );
    if precip > 0.0 {
        msg.push_str(&format!(" | {precip:.1} mm precip"));
    }
    if msg.chars().count() > 255 {
        msg = format!("{}...", msg.chars().take(252).collect::<String>());
    }

    // 1d rain odds from tomorrow's forecast (index 1)
    let odds_msg = wx["daily"]["precipitation_probability_max"]
        .as_array()
        .and_then(|arr| arr.get(1))
        .and_then(|v| v.as_f64())
        .map(|p| {
            let prob = (p.round() as u8).clamp(5, 95);
            let yes  = (100.0 / prob as f64 * 100.0).round() / 100.0;
            let no   = (100.0 / (100 - prob) as f64 * 100.0).round() / 100.0;
            format!("[{city} 1d rain odds] {prob}% chance | yes {yes:.2}× / no {no:.2}×")
        });

    Some((msg, odds_msg))
}

async fn fetch_odds_preview(location: &str) -> Option<String> {
    let client = reqwest::Client::new();
    let (lat, lon, city, _country, _pop) = geocode(&client, location).await?;

    let url = format!(
        "https://api.open-meteo.com/v1/forecast\
        ?latitude={lat:.4}&longitude={lon:.4}\
        &daily=precipitation_probability_max&forecast_days=15&timezone=GMT"
    );
    let resp: serde_json::Value = client
        .get(&url)
        .header("User-Agent", "ForestBot/1.0")
        .send().await.ok()?.json().await.ok()?;

    let probs = resp["daily"]["precipitation_probability_max"].as_array()?;

    let mut parts = Vec::new();
    for (label, idx) in [("1d", 1usize), ("3d", 3), ("7d", 7), ("14d", 14)] {
        let prob = probs.get(idx)?.as_f64()? as u8;
        let prob = prob.clamp(5, 95);
        let yes  = (100.0 / prob as f64 * 100.0).round() / 100.0;
        let no   = (100.0 / (100 - prob) as f64 * 100.0).round() / 100.0;
        parts.push(format!("{label} {prob}%→{yes:.2}×/{no:.2}×"));
    }

    Some(format!("[Rain odds] {city}: {}", parts.join(" | ")))
}

// ── Bet placement ─────────────────────────────────────────────────────────────

async fn place_bet(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    // Syntax: !weather bet <city> rain yes/no <chips> <duration>
    let args = &ctx.args; // [0]="bet" [1]=city [2]="rain" [3]=yes/no [4]=chips [5]=duration
    if args.len() < 6 {
        ctx.whisper("Usage: !weather bet <city> rain yes/no <chips> <1d/3d/7d/14d>");
        return Ok(());
    }

    let city_arg  = args[1].to_owned();
    let bet_type  = args[2].to_lowercase();
    let direction = args[3].to_lowercase();
    let chips_arg = args[4];
    let dur_arg   = args[5].to_lowercase();

    if bet_type != "rain" {
        ctx.whisper("Only 'rain' bets are supported. Usage: !weather bet <city> rain yes/no <chips> <duration>");
        return Ok(());
    }

    let rain_yes = match direction.as_str() {
        "yes" | "y" => true,
        "no"  | "n" => false,
        _ => {
            ctx.whisper("Direction must be 'yes' or 'no'. E.g. !weather bet London rain yes 100 3d");
            return Ok(());
        }
    };

    let stake: i64 = match chips_arg.parse() {
        Ok(n) => n,
        Err(_) => {
            ctx.whisper("Invalid chip amount.");
            return Ok(());
        }
    };

    if stake < MIN_BET {
        ctx.whisper(format!("Min bet is {}.", chips_str(MIN_BET)));
        return Ok(());
    }

    let dur_secs: u64 = match dur_arg.as_str() {
        "1d"  => 86_400,
        "3d"  => 3 * 86_400,
        "7d"  => 7 * 86_400,
        "14d" => 14 * 86_400,
        _ => {
            ctx.whisper("Duration must be 1d, 3d, 7d, or 14d.");
            return Ok(());
        }
    };

    // Geocode
    let client = reqwest::Client::new();
    let (lat, lon, city_name, _country, _pop) = match geocode(&client, &city_arg).await {
        Some(g) => g,
        None => {
            ctx.whisper(format!("Couldn't find city: {city_arg}"));
            return Ok(());
        }
    };

    // Fetch forecast probability for the target date
    let closes_unix = now_unix() + dur_secs;
    let target_date = unix_to_date(closes_unix);
    let forecast_prob = match fetch_precip_probability(&client, lat, lon, &target_date).await {
        Some(p) => p,
        None => {
            ctx.whisper("Couldn't fetch forecast — try again.");
            return Ok(());
        }
    };

    // Clamp to 5-95% so odds are never degenerate
    let prob_clamped = forecast_prob.clamp(5, 95);
    let payout_mult = if rain_yes {
        (100.0 / prob_clamped as f64).min(20.0).max(1.05)
    } else {
        (100.0 / (100 - prob_clamped) as f64).min(20.0).max(1.05)
    };
    let payout_mult = (payout_mult * 100.0).round() / 100.0; // 2 decimal places

    let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
        ctx.whisper("Could not resolve your UUID.");
        return Ok(());
    };

    // Deduct stake
    match ctx.state.api.casino_adjust(&player_uuid, -stake).await {
        Ok(_) => {}
        Err(CasinoAdjustErr::InsufficientFunds(have)) => {
            ctx.whisper(format!("Need {} but have {}.", chips_str(stake), chips_str(have)));
            return Ok(());
        }
        Err(CasinoAdjustErr::NetworkErr) => {
            ctx.whisper("Casino unavailable.");
            return Ok(());
        }
    }

    let mut bet = WeatherBet {
        id: 0i64,
        player: player_uuid.clone(),
        city: city_name.clone(),
        latitude: lat,
        longitude: lon,
        rain_yes,
        forecast_prob: prob_clamped,
        payout_mult,
        stake,
        closes_unix,
        duration_label: dur_arg.clone(),
    };

    match ctx.state.api.casino_weather_bet_insert(&bet).await {
        Some(id) => { bet.id = id; }
        None => {
            ctx.whisper("Failed to save bet. Refunding chips.");
            let _ = ctx.state.api.casino_adjust(&player_uuid, stake).await;
            return Ok(());
        }
    }
    {
        let mut bets = ctx.state.weather_bets.lock().expect("weather_bets lock");
        bets.entry(player_uuid.clone()).or_default().push(bet.clone());
    }

    let dir_str  = if rain_yes { "yes" } else { "no" };
    let odds_str = format!("{payout_mult:.2}x");
    ctx.whisper(format!(
        "[Weather] {city_name} rain {dir_str} | {dur_arg} | {} @ {odds_str} | forecast: {prob_clamped}% rain on {target_date}. Resolves +{}.",
        chips_str(stake), chips_str((stake as f64 * payout_mult).ceil() as i64 - stake)
    ));

    let whisper_cmd = ctx.runtime.whisper_command.clone();
    tokio::spawn(settle_task(ctx.state.clone(), whisper_cmd, bet, dur_secs));

    Ok(())
}

// ── Show bets ─────────────────────────────────────────────────────────────────

fn show_bets(ctx: &CommandContext<'_>, player_uuid: &str) {
    let bets = ctx.state.weather_bets.lock().expect("weather_bets lock");
    let player_bets = bets.get(player_uuid).cloned().unwrap_or_default();
    if player_bets.is_empty() {
        ctx.whisper("No open weather bets.");
        return;
    }
    for bet in &player_bets {
        let dir     = if bet.rain_yes { "rain yes" } else { "rain no" };
        let date    = unix_to_date(bet.closes_unix);
        let payout  = (bet.stake as f64 * bet.payout_mult).ceil() as i64;
        ctx.whisper(format!(
            "[Weather] {} {} | {} | {} @ {:.2}x = {} | resolves {}",
            bet.city, dir, bet.duration_label, chips_str(bet.stake), bet.payout_mult, chips_str(payout), date
        ));
    }
}

// ── Settle task ───────────────────────────────────────────────────────────────

pub async fn settle_task(state: AzaleaState, whisper_cmd: String, bet: WeatherBet, dur_secs: u64) {
    tokio::time::sleep(std::time::Duration::from_secs(dur_secs)).await;

    // Bail if bet was already removed
    {
        let bets = state.weather_bets.lock().expect("weather_bets lock");
        let still_open = bets.get(&bet.player)
            .map(|v| v.iter().any(|b| b.id == bet.id))
            .unwrap_or(false);
        if !still_open { return; }
    }

    let target_date = unix_to_date(bet.closes_unix);
    let client = reqwest::Client::new();

    let display_player = state.players.read().ok()
        .and_then(|pl| pl.values().find(|s| s.uuid == bet.player).map(|s| s.username.clone()))
        .unwrap_or_else(|| bet.player.clone());

    let rained = match fetch_precipitation(&client, bet.latitude, bet.longitude, &target_date).await {
        Some(p) => p > 0.0,
        None => {
            // Refund on API failure
            let _ = state.api.casino_adjust(&bet.player, bet.stake).await;
            remove_bet(&state, &bet.player, bet.id);
            state.api.casino_weather_bet_delete(bet.id).await;
            enqueue_chat(&state, format!(
                "/{whisper_cmd} {} [Weather] Forecast data unavailable — {} refunded.",
                display_player, chips_str(bet.stake)
            ));
            return;
        }
    };

    let won = rained == bet.rain_yes;
    remove_bet(&state, &bet.player, bet.id);
    state.api.casino_weather_bet_delete(bet.id).await;

    let dir_str = if bet.rain_yes { "rain yes" } else { "rain no" };
    let result_str = if rained { "It rained" } else { "No rain" };

    if won {
        let payout = (bet.stake as f64 * bet.payout_mult).ceil() as i64;
        let net    = payout - bet.stake;
        let _ = state.api.casino_adjust(&bet.player, payout).await;
        enqueue_chat(&state, format!(
            "/{whisper_cmd} {} [Weather] {} {} — {}. WIN +{} ({} @ {:.2}x).",
            display_player, bet.city, dir_str, result_str, chips_str(net), chips_str(bet.stake), bet.payout_mult
        ));
    } else {
        let _ = state.api.casino_jackpot_rake(bet.stake).await;
        enqueue_chat(&state, format!(
            "/{whisper_cmd} {} [Weather] {} {} — {}. LOSS -{} (to jackpot).",
            display_player, bet.city, dir_str, result_str, chips_str(bet.stake)
        ));
    }
}

fn remove_bet(state: &AzaleaState, player: &str, id: i64) {
    let mut bets = state.weather_bets.lock().expect("weather_bets lock");
    if let Some(v) = bets.get_mut(player) {
        v.retain(|b| b.id != id);
    }
}

// ── Open-Meteo helpers ────────────────────────────────────────────────────────

async fn geocode(client: &reqwest::Client, location: &str) -> Option<(f64, f64, String, String, Option<String>)> {
    let url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=en",
        percent_encode(location)
    );
    let geo: serde_json::Value = client
        .get(&url)
        .header("User-Agent", "ForestBot/1.0")
        .send().await.ok()?.json().await.ok()?;

    let result  = geo["results"].get(0)?;
    let lat     = result["latitude"].as_f64()?;
    let lon     = result["longitude"].as_f64()?;
    let city    = result["name"].as_str().unwrap_or(location).to_owned();
    let country = result["country_code"].as_str().unwrap_or("").to_owned();
    let pop     = result["population"].as_u64().map(format_pop);
    Some((lat, lon, city, country, pop))
}

async fn fetch_precip_probability(client: &reqwest::Client, lat: f64, lon: f64, date: &str) -> Option<u8> {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast\
        ?latitude={lat:.4}&longitude={lon:.4}\
        &daily=precipitation_probability_max\
        &start_date={date}&end_date={date}&timezone=GMT"
    );
    let resp: serde_json::Value = client
        .get(&url)
        .header("User-Agent", "ForestBot/1.0")
        .send().await.ok()?.json().await.ok()?;

    resp["daily"]["precipitation_probability_max"]
        .as_array()?
        .first()?
        .as_f64()
        .map(|p| p.round() as u8)
}

async fn fetch_precipitation(client: &reqwest::Client, lat: f64, lon: f64, date: &str) -> Option<f64> {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast\
        ?latitude={lat:.4}&longitude={lon:.4}\
        &daily=precipitation_sum\
        &start_date={date}&end_date={date}&timezone=GMT"
    );
    let resp: serde_json::Value = client
        .get(&url)
        .header("User-Agent", "ForestBot/1.0")
        .send().await.ok()?.json().await.ok()?;

    resp["daily"]["precipitation_sum"]
        .as_array()?
        .first()?
        .as_f64()
}

fn unix_to_date(unix: u64) -> String {
    let secs  = unix as i64;
    let days  = secs / 86_400;
    let epoch = chrono_days_to_date(days);
    epoch
}

fn chrono_days_to_date(days_since_epoch: i64) -> String {
    // Simple Gregorian calendar calculation from Unix epoch (1970-01-01)
    let mut d = days_since_epoch;
    let mut year = 1970i32;
    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if d < days_in_year { break; }
        d -= days_in_year;
        year += 1;
    }
    let months = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1u32;
    for &m in &months {
        if d < m { break; }
        d -= m;
        month += 1;
    }
    format!("{year}-{month:02}-{:02}", d + 1)
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

// ── WMO / misc helpers (unchanged from original) ──────────────────────────────

fn wmo_desc(code: u64) -> &'static str {
    match code {
        0  => "Clear sky",
        1  => "Mainly clear",
        2  => "Partly cloudy",
        3  => "Overcast",
        45 => "Fog",
        48 => "Icy fog",
        51 => "Light drizzle",
        53 => "Drizzle",
        55 => "Heavy drizzle",
        56 | 57 => "Freezing drizzle",
        61 => "Light rain",
        63 => "Rain",
        65 => "Heavy rain",
        66 | 67 => "Freezing rain",
        71 => "Light snow",
        73 => "Snow",
        75 => "Heavy snow",
        77 => "Snow grains",
        80 => "Light showers",
        81 => "Showers",
        82 => "Heavy showers",
        85 => "Snow showers",
        86 => "Heavy snow showers",
        95 => "Thunderstorm",
        96 => "Thunderstorm + hail",
        99 => "Thunderstorm + heavy hail",
        _  => "Unknown",
    }
}

fn wind_dir(deg: f64) -> &'static str {
    let idx = ((deg + 22.5) / 45.0) as usize % 8;
    ["N", "NE", "E", "SE", "S", "SW", "W", "NW"][idx]
}

fn format_pop(p: u64) -> String {
    if p >= 1_000_000 {
        format!("{:.1}M", p as f64 / 1_000_000.0)
    } else if p >= 1_000 {
        format!("{:.0}K", p as f64 / 1_000.0)
    } else {
        p.to_string()
    }
}

fn percent_encode(value: &str) -> String {
    value.bytes().flat_map(|byte| match byte {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => vec![byte as char],
        _ => format!("%{byte:02X}").chars().collect(),
    }).collect()
}
