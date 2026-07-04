use crate::commands::{enqueue_chat, CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::market::types::now_unix;
use crate::structure::mineflayer::bot::AzaleaState;

use super::casino::chips_str;

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["weather", "w"],
    description: "Weather and weather bets. !weather <city> | !weather odds <city> | !weather bet <city> rain yes/no <chips> <1d/3d/7d/14d> | !weather bet <city> temp over/under <threshold> <chips> <dur> | !weather bet <city> wind over/under <threshold> <chips> <dur> | !weather bets",
    whitelisted: false,
    execute,
};

// ── Cache ─────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct WeatherCacheEntry {
    pub fetched_at: u64,
    pub value: WeatherCacheValue,
}

#[derive(Clone)]
pub enum WeatherCacheValue {
    Rain(u8),
    Members(Vec<f64>),
}

const CACHE_TTL: u64 = 3600;

// ── Bet struct ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct WeatherBet {
    pub id: i64,
    pub player: String,
    pub bet_type: String,
    pub city: String,
    pub latitude: f64,
    pub longitude: f64,
    pub direction: String,
    pub threshold: Option<f64>,
    pub unit: Option<String>,
    pub forecast_prob: u8,
    pub payout_mult: f64,
    pub stake: i64,
    pub closes_unix: u64,
    pub duration_label: String,
}

const MIN_BET: i64 = 50;

// ── Command entry ─────────────────────────────────────────────────────────────

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let arg = ctx.args.first().copied().unwrap_or("").to_lowercase();
        match arg.as_str() {
            "bet" => place_bet(&ctx).await?,
            "bets" => {
                let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
                    ctx.whisper("Could not resolve your UUID.");
                    return Ok(());
                };
                show_bets(&ctx, &player_uuid).await;
            }
            "odds" => {
                let location = ctx.args[1..].join(" ");
                if location.is_empty() {
                    ctx.whisper(format!("Usage: {}weather odds <city>", ctx.runtime.prefix));
                } else {
                    let msgs = show_odds(&location).await;
                    if msgs.is_empty() {
                        ctx.whisper(format!("Couldn't fetch odds for: {location}"));
                    } else {
                        for msg in msgs { ctx.whisper(msg); }
                    }
                }
            }
            _ => {
                if ctx.args.is_empty() {
                    ctx.whisper(format!(
                        "Usage: {p}weather <city> | {p}weather odds <city> | {p}weather bet <city> rain yes/no <chips> <1d/3d/7d/14d> | {p}weather bet <city> temp over/under <threshold> <chips> <dur> | {p}weather bets",
                        p = ctx.runtime.prefix
                    ));
                } else {
                    let location = ctx.args.join(" ");
                    match fetch_weather(&location).await {
                        Some((wx_msg, rain_odds)) => {
                            ctx.chat(wx_msg);
                            if let Some(odds) = rain_odds { ctx.whisper(odds); }
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

// ── Odds preview ──────────────────────────────────────────────────────────────

async fn show_odds(location: &str) -> Vec<String> {
    let client = reqwest::Client::new();
    let Some((lat, lon, city, _country, _pop)) = geocode(&client, location).await else {
        return vec![];
    };

    let mut msgs = Vec::new();

    // Rain odds (15-day forecast)
    let rain_url = format!(
        "https://api.open-meteo.com/v1/forecast\
        ?latitude={lat:.4}&longitude={lon:.4}\
        &daily=precipitation_probability_max&forecast_days=15&timezone=GMT"
    );
    if let Ok(resp) = client.get(&rain_url).header("User-Agent", "ForestBot/1.0").send().await {
        if let Ok(v) = resp.json::<serde_json::Value>().await {
            if let Some(probs) = v["daily"]["precipitation_probability_max"].as_array() {
                let mut parts = Vec::new();
                for (label, idx) in [("1d", 1usize), ("3d", 3), ("7d", 7), ("14d", 14)] {
                    if let Some(p) = probs.get(idx).and_then(|v| v.as_f64()) {
                        let prob = (p as u8).clamp(5, 95);
                        let yes = (100.0 / prob as f64 * 100.0).round() / 100.0;
                        let no  = (100.0 / (100 - prob) as f64 * 100.0).round() / 100.0;
                        parts.push(format!("{label} {prob}%→{yes:.2}×/{no:.2}×"));
                    }
                }
                if !parts.is_empty() {
                    msgs.push(format!("[Rain odds] {city}: {}", parts.join(" | ")));
                }
            }
        }
    }

    // Temp + wind ensemble for tomorrow
    let tomorrow = unix_to_date(now_unix() + 86_400);
    let ens_url = format!(
        "https://ensemble-api.open-meteo.com/v1/ensemble\
        ?latitude={lat:.4}&longitude={lon:.4}\
        &hourly=temperature_2m,wind_speed_10m&models=gfs_seamless&forecast_days=3&timezone=GMT"
    );
    if let Ok(resp) = client.get(&ens_url).header("User-Agent", "ForestBot/1.0").send().await {
        if let Ok(v) = resp.json::<serde_json::Value>().await {
            let hourly = &v["hourly"];
            if let Some(times) = hourly["time"].as_array() {
                let date_indices: Vec<usize> = times.iter().enumerate()
                    .filter_map(|(i, t)| t.as_str().filter(|s| s.starts_with(&tomorrow)).map(|_| i))
                    .collect();

                if !date_indices.is_empty() {
                    let temp_members = collect_members(hourly, &date_indices, "temperature_2m");
                    let wind_members = collect_members(hourly, &date_indices, "wind_speed_10m");

                    if temp_members.len() >= 5 {
                        let median = ensemble_median(&temp_members);
                        let p_over = ensemble_prob_over(&temp_members, median).max(0.05).min(0.95);
                        let p_under = (1.0 - ensemble_prob_over(&temp_members, median)).max(0.05).min(0.95);
                        msgs.push(format!(
                            "[Temp odds] {city} {tomorrow}: median {median:.1}°C | over {:.2}× / under {:.2}×",
                            (1.0 / p_over).min(20.0), (1.0 / p_under).min(20.0)
                        ));
                    }

                    if wind_members.len() >= 5 {
                        let median = ensemble_median(&wind_members);
                        let p_over = ensemble_prob_over(&wind_members, median).max(0.05).min(0.95);
                        let p_under = (1.0 - ensemble_prob_over(&wind_members, median)).max(0.05).min(0.95);
                        msgs.push(format!(
                            "[Wind odds] {city} {tomorrow}: median {median:.1} km/h | over {:.2}× / under {:.2}×",
                            (1.0 / p_over).min(20.0), (1.0 / p_under).min(20.0)
                        ));
                    }
                }
            }
        }
    }

    msgs
}

// ── Bet placement ─────────────────────────────────────────────────────────────

async fn place_bet(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let args = &ctx.args;
    if args.len() < 3 {
        ctx.whisper(format!(
            "Usage: {p}weather bet <city> rain yes/no <chips> <1d/3d/7d/14d> | {p}weather bet <city> temp over/under <threshold> <chips> <dur> | {p}weather bet <city> wind over/under <threshold> <chips> <dur>",
            p = ctx.runtime.prefix
        ));
        return Ok(());
    }

    let city_arg = args[1].to_owned();
    let bet_type = args[2].to_lowercase();

    match bet_type.as_str() {
        "rain" => place_rain_bet(ctx, &city_arg, &args[3..]).await?,
        "temp" => place_ensemble_bet(ctx, &city_arg, "temp", "temperature_2m", "°C", &args[3..]).await?,
        "wind" => place_ensemble_bet(ctx, &city_arg, "wind", "wind_speed_10m", "km/h", &args[3..]).await?,
        _ => ctx.whisper("Bet type must be rain, temp, or wind."),
    }
    Ok(())
}

async fn place_rain_bet(ctx: &CommandContext<'_>, city_arg: &str, tail: &[&str]) -> anyhow::Result<()> {
    // tail: [yes/no, chips, dur]
    if tail.len() < 3 {
        ctx.whisper(format!("Usage: {}weather bet <city> rain yes/no <chips> <1d/3d/7d/14d>", ctx.runtime.prefix));
        return Ok(());
    }

    let dir_raw = tail[0].to_lowercase();
    let direction = match dir_raw.as_str() {
        "yes" | "y" => "yes",
        "no"  | "n" => "no",
        _ => { ctx.whisper("Direction must be yes or no."); return Ok(()); }
    }.to_owned();
    let rain_yes = direction == "yes";

    let stake: i64 = match tail[1].parse() {
        Ok(n) => n,
        Err(_) => { ctx.whisper("Invalid chip amount."); return Ok(()); }
    };
    if stake < MIN_BET {
        ctx.whisper(format!("Min bet is {}.", chips_str(MIN_BET)));
        return Ok(());
    }

    let dur_arg = tail[2].to_lowercase();
    let dur_secs: u64 = match dur_arg.as_str() {
        "1d" => 86_400, "3d" => 3 * 86_400, "7d" => 7 * 86_400, "14d" => 14 * 86_400,
        _ => { ctx.whisper("Duration must be 1d, 3d, 7d, or 14d."); return Ok(()); }
    };

    let client = reqwest::Client::new();
    let (lat, lon, city_name, _country, _pop) = match geocode(&client, city_arg).await {
        Some(g) => g,
        None => { ctx.whisper(format!("Couldn't find city: {city_arg}")); return Ok(()); }
    };

    let closes_unix = now_unix() + dur_secs;
    let target_date = unix_to_date(closes_unix);

    let forecast_prob = match get_rain_prob_cached(&ctx.state, &client, lat, lon, &target_date).await {
        Some(p) => p,
        None => { ctx.whisper("Couldn't fetch forecast — try again."); return Ok(()); }
    };
    let prob_clamped = forecast_prob.clamp(5, 95);
    let payout_mult = if rain_yes {
        ((100.0 / prob_clamped as f64).min(20.0).max(1.05) * 100.0).round() / 100.0
    } else {
        ((100.0 / (100 - prob_clamped) as f64).min(20.0).max(1.05) * 100.0).round() / 100.0
    };

    let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
        ctx.whisper("Could not resolve your UUID.");
        return Ok(());
    };

    match ctx.state.api.casino_adjust(&player_uuid, -stake).await {
        Ok(_) => {}
        Err(CasinoAdjustErr::InsufficientFunds(have)) => {
            ctx.whisper(format!("Need {} but have {}.", chips_str(stake), chips_str(have)));
            return Ok(());
        }
        Err(CasinoAdjustErr::NetworkErr) => { ctx.whisper("Casino unavailable."); return Ok(()); }
    }

    let mut bet = WeatherBet {
        id: 0,
        player: player_uuid.clone(),
        bet_type: "rain".to_owned(),
        city: city_name.clone(),
        latitude: lat,
        longitude: lon,
        direction: direction.clone(),
        threshold: None,
        unit: None,
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

    ctx.whisper(format!(
        "[Weather] {city_name} rain {direction} | {dur_arg} | {} @ {payout_mult:.2}x | forecast: {prob_clamped}% rain on {target_date}. Profit if win: +{}.",
        chips_str(stake), chips_str((stake as f64 * payout_mult).ceil() as i64 - stake)
    ));

    let whisper_cmd = ctx.runtime.whisper_command.clone();
    tokio::spawn(settle_task(ctx.state.clone(), whisper_cmd, bet, dur_secs));

    Ok(())
}

async fn place_ensemble_bet(
    ctx: &CommandContext<'_>,
    city_arg: &str,
    bet_type: &str,
    variable: &str,
    unit: &str,
    tail: &[&str],
) -> anyhow::Result<()> {
    // tail: [over/under, threshold, chips, dur]
    if tail.len() < 4 {
        ctx.whisper(format!(
            "Usage: {}weather bet <city> {bet_type} over/under <threshold> <chips> <1d/3d/7d/14d>",
            ctx.runtime.prefix
        ));
        return Ok(());
    }

    let direction = tail[0].to_lowercase();
    if direction != "over" && direction != "under" {
        ctx.whisper("Direction must be over or under.");
        return Ok(());
    }

    let threshold: f64 = match tail[1].parse() {
        Ok(v) => v,
        Err(_) => {
            ctx.whisper(format!("Invalid threshold. E.g.: {}weather bet London {bet_type} over 25 100 3d", ctx.runtime.prefix));
            return Ok(());
        }
    };

    let stake: i64 = match tail[2].parse() {
        Ok(n) => n,
        Err(_) => { ctx.whisper("Invalid chip amount."); return Ok(()); }
    };
    if stake < MIN_BET {
        ctx.whisper(format!("Min bet is {}.", chips_str(MIN_BET)));
        return Ok(());
    }

    let dur_arg = tail[3].to_lowercase();
    let dur_secs: u64 = match dur_arg.as_str() {
        "1d" => 86_400, "3d" => 3 * 86_400, "7d" => 7 * 86_400, "14d" => 14 * 86_400,
        _ => { ctx.whisper("Duration must be 1d, 3d, 7d, or 14d."); return Ok(()); }
    };

    let client = reqwest::Client::new();
    let (lat, lon, city_name, _country, _pop) = match geocode(&client, city_arg).await {
        Some(g) => g,
        None => { ctx.whisper(format!("Couldn't find city: {city_arg}")); return Ok(()); }
    };

    let closes_unix = now_unix() + dur_secs;
    let target_date = unix_to_date(closes_unix);

    let members = match get_ensemble_cached(&ctx.state, &client, lat, lon, &target_date, bet_type, variable).await {
        Some(m) if m.len() >= 5 => m,
        _ => { ctx.whisper("Couldn't fetch ensemble data — try again."); return Ok(()); }
    };

    let median = ensemble_median(&members);
    let p_raw = if direction == "over" {
        ensemble_prob_over(&members, threshold)
    } else {
        1.0 - ensemble_prob_over(&members, threshold)
    };
    let p = p_raw.max(0.05).min(0.95);
    let payout_mult = ((1.0 / p).min(20.0).max(1.05) * 100.0).round() / 100.0;
    let forecast_prob = (p * 100.0).round() as u8;

    let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
        ctx.whisper("Could not resolve your UUID.");
        return Ok(());
    };

    match ctx.state.api.casino_adjust(&player_uuid, -stake).await {
        Ok(_) => {}
        Err(CasinoAdjustErr::InsufficientFunds(have)) => {
            ctx.whisper(format!("Need {} but have {}.", chips_str(stake), chips_str(have)));
            return Ok(());
        }
        Err(CasinoAdjustErr::NetworkErr) => { ctx.whisper("Casino unavailable."); return Ok(()); }
    }

    let mut bet = WeatherBet {
        id: 0,
        player: player_uuid.clone(),
        bet_type: bet_type.to_owned(),
        city: city_name.clone(),
        latitude: lat,
        longitude: lon,
        direction: direction.clone(),
        threshold: Some(threshold),
        unit: Some(unit.to_owned()),
        forecast_prob,
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

    ctx.whisper(format!(
        "[Weather] {city_name} {bet_type} {direction} {threshold}{unit} | {dur_arg} | {} @ {payout_mult:.2}x | median: {median:.1}{unit} | resolves {target_date}. Profit if win: +{}.",
        chips_str(stake), chips_str((stake as f64 * payout_mult).ceil() as i64 - stake)
    ));

    let whisper_cmd = ctx.runtime.whisper_command.clone();
    tokio::spawn(settle_task(ctx.state.clone(), whisper_cmd, bet, dur_secs));

    Ok(())
}

// ── Show bets ─────────────────────────────────────────────────────────────────

async fn show_bets(ctx: &CommandContext<'_>, player_uuid: &str) {
    let all_bets = ctx.state.api.casino_weather_bet_list().await;
    let player_bets: Vec<_> = all_bets.into_iter().filter(|b| b.player == player_uuid).collect();
    if player_bets.is_empty() {
        ctx.whisper("No open weather bets.");
        return;
    }
    for bet in &player_bets {
        let type_str = match bet.bet_type.as_str() {
            "rain" => format!("rain {}", bet.direction),
            _ => format!(
                "{} {} {:.1}{}",
                bet.bet_type, bet.direction,
                bet.threshold.unwrap_or(0.0),
                bet.unit.as_deref().unwrap_or("")
            ),
        };
        let date   = unix_to_date(bet.closes_unix);
        let payout = (bet.stake as f64 * bet.payout_mult).ceil() as i64;
        ctx.whisper(format!(
            "[Weather] {} {} | {} | {} @ {:.2}x = {} | resolves {}",
            bet.city, type_str, bet.duration_label, chips_str(bet.stake), bet.payout_mult, chips_str(payout), date
        ));
    }
}

// ── Settle task ───────────────────────────────────────────────────────────────

pub async fn settle_task(state: AzaleaState, whisper_cmd: String, bet: WeatherBet, dur_secs: u64) {
    tokio::time::sleep(std::time::Duration::from_secs(dur_secs)).await;

    // Atomically claim the bet — only one task wins if two race here.
    let claimed = {
        let mut bets = state.weather_bets.lock().expect("weather_bets lock");
        bets.get_mut(&bet.player)
            .map(|v| { let pos = v.iter().position(|b| b.id == bet.id); pos.map(|i| { v.remove(i); }).is_some() })
            .unwrap_or(false)
    };
    if !claimed { return; }

    let target_date = unix_to_date(bet.closes_unix);
    let client = reqwest::Client::new();
    let threshold = bet.threshold.unwrap_or(0.0);
    let unit = bet.unit.as_deref().unwrap_or("");

    let online_username = state.players.read().ok()
        .and_then(|pl| pl.values().find(|s| s.uuid == bet.player).map(|s| s.username.clone()));

    let (won, result_str) = match fetch_actual(&client, &bet, &target_date).await {
        Some(actual) => {
            let won = match bet.bet_type.as_str() {
                "rain" => (actual > 0.0) == (bet.direction == "yes"),
                _      => (actual > threshold) == (bet.direction == "over"),
            };
            let result_str = match bet.bet_type.as_str() {
                "rain" => if actual > 0.0 { "It rained".to_owned() } else { "No rain".to_owned() },
                _      => format!("Actual: {actual:.1}{unit}"),
            };
            (won, result_str)
        }
        None => {
            state.api.casino_weather_bet_delete(bet.id).await;
            let _ = state.api.casino_adjust(&bet.player, bet.stake).await;
            let msg = format!("[Weather] Data unavailable — {} refunded.", chips_str(bet.stake));
            if let Some(ref username) = online_username {
                enqueue_chat(&state, format!("/{whisper_cmd} {username} {msg}"));
            } else {
                state.api.casino_add_notification(&bet.player, &msg).await;
            }
            return;
        }
    };

    let type_str = match bet.bet_type.as_str() {
        "rain" => format!("rain {}", bet.direction),
        _      => format!("{} {} {:.1}{}", bet.bet_type, bet.direction, threshold, unit),
    };

    state.api.casino_weather_bet_delete(bet.id).await;

    let msg = if won {
        let payout = (bet.stake as f64 * bet.payout_mult).ceil() as i64;
        let net    = payout - bet.stake;
        let _ = state.api.casino_adjust(&bet.player, payout).await;
        format!("[Weather] {} {} — {}. WIN +{} ({} @ {:.2}x).",
            bet.city, type_str, result_str, chips_str(net), chips_str(bet.stake), bet.payout_mult)
    } else {
        let _ = state.api.casino_jackpot_rake(bet.stake).await;
        format!("[Weather] {} {} — {}. LOSS -{} (to jackpot).",
            bet.city, type_str, result_str, chips_str(bet.stake))
    };
    if let Some(ref username) = online_username {
        enqueue_chat(&state, format!("/{whisper_cmd} {username} {msg}"));
    } else {
        state.api.casino_add_notification(&bet.player, &msg).await;
    }
}

// ── Open-Meteo helpers ────────────────────────────────────────────────────────

async fn fetch_actual(client: &reqwest::Client, bet: &WeatherBet, date: &str) -> Option<f64> {
    match bet.bet_type.as_str() {
        "rain" => fetch_precipitation(client, bet.latitude, bet.longitude, date).await,
        "temp" => fetch_temp_max(client, bet.latitude, bet.longitude, date).await,
        "wind" => fetch_wind_max(client, bet.latitude, bet.longitude, date).await,
        _      => None,
    }
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

async fn fetch_temp_max(client: &reqwest::Client, lat: f64, lon: f64, date: &str) -> Option<f64> {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast\
        ?latitude={lat:.4}&longitude={lon:.4}\
        &daily=temperature_2m_max&start_date={date}&end_date={date}&timezone=GMT"
    );
    let resp: serde_json::Value = client
        .get(&url)
        .header("User-Agent", "ForestBot/1.0")
        .send().await.ok()?.json().await.ok()?;

    resp["daily"]["temperature_2m_max"].as_array()?.first()?.as_f64()
}

async fn fetch_wind_max(client: &reqwest::Client, lat: f64, lon: f64, date: &str) -> Option<f64> {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast\
        ?latitude={lat:.4}&longitude={lon:.4}\
        &daily=wind_speed_10m_max&start_date={date}&end_date={date}&timezone=GMT&wind_speed_unit=kmh"
    );
    let resp: serde_json::Value = client
        .get(&url)
        .header("User-Agent", "ForestBot/1.0")
        .send().await.ok()?.json().await.ok()?;

    resp["daily"]["wind_speed_10m_max"].as_array()?.first()?.as_f64()
}

async fn fetch_ensemble_members(client: &reqwest::Client, lat: f64, lon: f64, date: &str, variable: &str) -> Option<Vec<f64>> {
    let url = format!(
        "https://ensemble-api.open-meteo.com/v1/ensemble\
        ?latitude={lat:.4}&longitude={lon:.4}\
        &hourly={variable}&models=gfs_seamless&forecast_days=16&timezone=GMT"
    );
    let resp: serde_json::Value = client
        .get(&url)
        .header("User-Agent", "ForestBot/1.0")
        .send().await.ok()?.json().await.ok()?;

    let hourly = resp.get("hourly")?;
    let times  = hourly["time"].as_array()?;
    let date_indices: Vec<usize> = times.iter().enumerate()
        .filter_map(|(i, t)| t.as_str().filter(|s| s.starts_with(date)).map(|_| i))
        .collect();

    if date_indices.is_empty() { return None; }
    let members = collect_members(hourly, &date_indices, variable);
    if members.is_empty() { None } else { Some(members) }
}

fn collect_members(hourly: &serde_json::Value, date_indices: &[usize], variable: &str) -> Vec<f64> {
    let mut members = Vec::new();
    for m in 1usize..=50 {
        let key = format!("{variable}_member{m:02}");
        let Some(arr) = hourly.get(&key).and_then(|v| v.as_array()) else { break; };
        let max = date_indices.iter()
            .filter_map(|&i| arr.get(i)?.as_f64())
            .fold(f64::NEG_INFINITY, f64::max);
        if max.is_finite() { members.push(max); }
    }
    members
}

fn ensemble_median(members: &[f64]) -> f64 {
    let mut sorted = members.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = sorted.len();
    if n == 0 { return 0.0; }
    if n % 2 == 0 { (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0 } else { sorted[n / 2] }
}

fn ensemble_prob_over(members: &[f64], threshold: f64) -> f64 {
    if members.is_empty() { return 0.5; }
    members.iter().filter(|&&v| v > threshold).count() as f64 / members.len() as f64
}

// ── Cache helpers ─────────────────────────────────────────────────────────────

async fn get_rain_prob_cached(state: &AzaleaState, client: &reqwest::Client, lat: f64, lon: f64, date: &str) -> Option<u8> {
    let key = format!("rain_{lat:.4}_{lon:.4}_{date}");
    let now = now_unix();
    {
        if let Ok(cache) = state.weather_odds_cache.lock() {
            if let Some(entry) = cache.get(&key) {
                if now - entry.fetched_at < CACHE_TTL {
                    if let WeatherCacheValue::Rain(p) = entry.value { return Some(p); }
                }
            }
        }
    }
    let prob = fetch_precip_probability(client, lat, lon, date).await?;
    if let Ok(mut cache) = state.weather_odds_cache.lock() {
        cache.insert(key, WeatherCacheEntry { fetched_at: now, value: WeatherCacheValue::Rain(prob) });
    }
    Some(prob)
}

async fn get_ensemble_cached(
    state: &AzaleaState,
    client: &reqwest::Client,
    lat: f64, lon: f64,
    date: &str,
    bet_type: &str,
    variable: &str,
) -> Option<Vec<f64>> {
    let key = format!("{bet_type}_{lat:.4}_{lon:.4}_{date}");
    let now = now_unix();
    {
        if let Ok(cache) = state.weather_odds_cache.lock() {
            if let Some(entry) = cache.get(&key) {
                if now - entry.fetched_at < CACHE_TTL {
                    if let WeatherCacheValue::Members(ref m) = entry.value { return Some(m.clone()); }
                }
            }
        }
    }
    let members = fetch_ensemble_members(client, lat, lon, date, variable).await?;
    if let Ok(mut cache) = state.weather_odds_cache.lock() {
        cache.insert(key, WeatherCacheEntry { fetched_at: now, value: WeatherCacheValue::Members(members.clone()) });
    }
    Some(members)
}

// ── Geocoding ─────────────────────────────────────────────────────────────────

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

// ── Date helpers ──────────────────────────────────────────────────────────────

fn unix_to_date(unix: u64) -> String {
    chrono_days_to_date(unix as i64 / 86_400)
}

fn chrono_days_to_date(days_since_epoch: i64) -> String {
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

// ── WMO / misc helpers ────────────────────────────────────────────────────────

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
