use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::market::types::now_unix;
use crate::structure::mineflayer::bot::AzaleaState;

use super::{chips_str, fmt_close, calc_payout, sleep_until, deliver};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["flood", "flooding", "noaa"],
    description: "NOAA flood-alert bets. !flood list | !flood <lat> <lon> yes|no <chips> | !flood bets",
    whitelisted: false,
    execute,
};

const NOAA_BASE: &str = "https://api.weather.gov";
const MIN_BET: i64 = 25;
const BET_DURATION_SECS: u64 = 7200;
const POLL_INTERVAL_SECS: u64 = 120;
const MAX_POLL_SECS: u64 = 3600;

#[derive(Debug, Clone)]
pub struct FloodCacheEntry {
    pub area: String,
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Debug, Clone, Default)]
pub struct FloodCache {
    pub fetched_at: u64,
    pub entries: Vec<FloodCacheEntry>,
}

#[derive(Debug, Clone)]
pub struct NOAAFloodingBet {
    pub id: i64,
    pub player: String,
    pub location: String,
    pub latitude: f64,
    pub longitude: f64,
    pub side: String,
    pub price: f64,
    pub stake: i64,
    pub close_time: u64,
}


fn fmt_location(lat: f64, lon: f64) -> String {
    format!("{lat:.3},{lon:.3}")
}

fn compute_odds(currently_flooding: bool) -> (f64, f64) {
    if currently_flooding {
        (0.67, 0.33)
    } else {
        (0.33, 0.67)
    }
}

fn is_flood_related(alert: &serde_json::Value) -> bool {
    let props = &alert["properties"];
    let text = [
        props["event"].as_str(),
        props["headline"].as_str(),
        props["description"].as_str(),
        props["instruction"].as_str(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");
    text.to_lowercase().contains("flood") || text.to_lowercase().contains("storm surge")
}

async fn fetch_active_alerts(client: &reqwest::Client, lat: f64, lon: f64) -> Option<Vec<serde_json::Value>> {
    let url = format!("{NOAA_BASE}/alerts/active?point={lat},{lon}&status=actual");
    let body = client
        .get(&url)
        .header("User-Agent", "ForestBot-RS/1.0")
        .header("Accept", "application/geo+json")
        .send()
        .await
        .ok()?
        .json::<serde_json::Value>()
        .await
        .ok()?;
    body["features"].as_array().cloned()
}

async fn poll_flood_state(client: &reqwest::Client, lat: f64, lon: f64) -> Option<bool> {
    let alerts = fetch_active_alerts(client, lat, lon).await?;
    Some(alerts.iter().any(is_flood_related))
}

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        match ctx.args.first().copied().unwrap_or("") {
            "" => show_usage(&ctx),
            "bets" | "my" => show_bets(&ctx).await?,
            "list" | "ls" => show_list(&ctx).await?,
            "bet" | "b" => place_bet_indexed(&ctx).await?,
            _ => place_bet(&ctx).await?,
        }
        Ok(())
    })
}

fn show_usage(ctx: &CommandContext<'_>) {
    let p = &ctx.runtime.prefix;
    ctx.whisper(format!(
        "NOAA flood bets: {p}flood list | {p}flood bet <#> yes|no [chips] | {p}flood bets | Omit chips for odds preview"
    ));
}

fn centroid_of_ring(ring: &[serde_json::Value]) -> Option<(f64, f64)> {
    if ring.is_empty() { return None; }
    let mut sum_lat = 0.0_f64;
    let mut sum_lon = 0.0_f64;
    let mut count = 0usize;
    for pt in ring {
        sum_lon += pt[0].as_f64()?;
        sum_lat += pt[1].as_f64()?;
        count += 1;
    }
    Some((sum_lat / count as f64, sum_lon / count as f64))
}

fn extract_centroid(geometry: &serde_json::Value) -> Option<(f64, f64)> {
    match geometry["type"].as_str()? {
        "Polygon"      => centroid_of_ring(geometry["coordinates"][0].as_array()?),
        "MultiPolygon" => centroid_of_ring(geometry["coordinates"][0][0].as_array()?),
        "Point"        => Some((geometry["coordinates"][1].as_f64()?, geometry["coordinates"][0].as_f64()?)),
        _              => None,
    }
}

async fn show_list(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let url = format!("{NOAA_BASE}/alerts/active?event=Flood%20Warning&status=actual");
    let Ok(resp) = client
        .get(&url)
        .header("User-Agent", "ForestBot-RS/1.0")
        .header("Accept", "application/geo+json")
        .send().await
    else {
        ctx.whisper("Could not reach NOAA API.");
        return Ok(());
    };
    let Ok(body) = resp.json::<serde_json::Value>().await else {
        ctx.whisper("Could not parse NOAA response.");
        return Ok(());
    };
    let Some(features) = body["features"].as_array() else {
        ctx.whisper("No active flood alerts right now.");
        return Ok(());
    };

    let mut entries: Vec<FloodCacheEntry> = Vec::new();
    for feat in features {
        if entries.len() >= 4 { break; }
        let Some((lat, lon)) = extract_centroid(&feat["geometry"]) else { continue; };
        let area = feat["properties"]["areaDesc"].as_str().unwrap_or("Unknown");
        let area_short = area.split(';').next().unwrap_or(area).trim().to_string();
        entries.push(FloodCacheEntry { area: area_short, latitude: lat, longitude: lon });
    }

    if entries.is_empty() {
        ctx.whisper("No active flood alerts with location data. Check alerts.weather.gov.");
        return Ok(());
    }

    {
        let mut cache = ctx.state.flood_cache.lock().expect("flood_cache lock");
        cache.fetched_at = now_unix();
        cache.entries = entries.clone();
    }

    let items: Vec<String> = entries.iter().enumerate()
        .map(|(i, e)| format!("#{} {}", i + 1, e.area))
        .collect();
    let p = &ctx.runtime.prefix;
    ctx.whisper(format!(
        "[Flood Alerts] {} | {p}flood bet <#> yes|no <chips>",
        items.join(" | ")
    ));
    Ok(())
}

async fn show_bets(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
        ctx.whisper("Could not resolve your UUID.");
        return Ok(());
    };
    let all_bets = ctx.state.api.casino_noaa_flooding_bet_list().await;
    let player_bets: Vec<_> = all_bets.into_iter().filter(|b| b.player == player_uuid).collect();
    if player_bets.is_empty() {
        ctx.whisper("No open NOAA flood bets.");
        return Ok(());
    }
    for bet in &player_bets {
        let payout = (bet.stake as f64 / bet.price).floor() as i64;
        ctx.whisper(format!(
            "[NOAA Flood] {} | {} {:.2}x | {} -> {} | {}",
            bet.location,
            bet.side.to_uppercase(),
            1.0 / bet.price,
            chips_str(bet.stake),
            chips_str(payout),
            fmt_close(bet.close_time),
        ));
    }
    Ok(())
}

async fn place_bet_indexed(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    // args: bet <#> yes|no [chips]  → ctx.args = ["bet", "#", "yes|no", "chips"?]
    let (Some(&idx_s), Some(&side_s)) = (ctx.args.get(1), ctx.args.get(2)) else {
        ctx.whisper(format!("Usage: {}flood bet <#> yes|no <chips> | Omit chips for odds preview", ctx.runtime.prefix));
        return Ok(());
    };
    let Ok(idx) = idx_s.parse::<usize>().map(|n| n.saturating_sub(1)) else {
        ctx.whisper("Event number must be a positive integer.");
        return Ok(());
    };
    let entry = {
        let cache = ctx.state.flood_cache.lock().expect("flood_cache lock");
        cache.entries.get(idx).map(|e| (e.area.clone(), e.latitude, e.longitude))
    };
    let Some((area, latitude, longitude)) = entry else {
        ctx.whisper("Event not found. Run !flood list to refresh.");
        return Ok(());
    };

    // preview mode — no chips provided
    let Some(&amt_s) = ctx.args.get(3) else {
        let client = reqwest::Client::new();
        let flooding = fetch_active_alerts(&client, latitude, longitude)
            .await.map(|a| a.iter().any(is_flood_related)).unwrap_or(false);
        let (yes_price, no_price) = compute_odds(flooding);
        ctx.whisper(format!(
            "[NOAA Flood] {area} | {} | yes {:.2}x | no {:.2}x",
            if flooding { "flood alert active" } else { "no alert" },
            1.0 / yes_price, 1.0 / no_price,
        ));
        return Ok(());
    };

    place_bet_inner(ctx, area, latitude, longitude, side_s, amt_s).await
}

async fn place_bet(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let (Some(&lat_s), Some(&lon_s), Some(&side_s), Some(&amt_s)) =
        (ctx.args.first(), ctx.args.get(1), ctx.args.get(2), ctx.args.get(3))
    else {
        show_usage(ctx);
        return Ok(());
    };
    let Ok(latitude) = lat_s.parse::<f64>() else {
        ctx.whisper("Latitude must be numeric.");
        return Ok(());
    };
    let Ok(longitude) = lon_s.parse::<f64>() else {
        ctx.whisper("Longitude must be numeric.");
        return Ok(());
    };
    place_bet_inner(ctx, fmt_location(latitude, longitude), latitude, longitude, side_s, amt_s).await
}

async fn place_bet_inner(
    ctx: &CommandContext<'_>,
    location: String,
    latitude: f64,
    longitude: f64,
    side_s: &str,
    amt_s: &str,
) -> anyhow::Result<()> {
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

    let client = reqwest::Client::new();
    let currently_flooding = fetch_active_alerts(&client, latitude, longitude)
        .await
        .map(|alerts| alerts.iter().any(is_flood_related))
        .unwrap_or(false);
    let (yes_price, no_price) = compute_odds(currently_flooding);
    let price = if side == "yes" { yes_price } else { no_price };

    let close_time = now_unix() + BET_DURATION_SECS;
    let mut bet = NOAAFloodingBet {
        id: 0,
        player: player_uuid.clone(),
        location: location.clone(),
        latitude,
        longitude,
        side: side.clone(),
        price,
        stake,
        close_time,
    };
    match ctx.state.api.casino_noaa_flooding_bet_insert(&bet).await {
        Some(id) => { bet.id = id; }
        None => {
            ctx.whisper("Failed to save bet. Refunding chips.");
            let _ = ctx.state.api.casino_adjust(&player_uuid, stake).await;
            return Ok(());
        }
    }
    {
        let mut bets = ctx.state.noaa_flooding_bets.lock().expect("noaa_flooding_bets lock");
        bets.entry(player_uuid.clone()).or_default().push(bet.clone());
    }

    let payout = (stake as f64 / price).floor() as i64;
    ctx.whisper(format!(
        "[NOAA Flood] {location} | {} {} | {:.2}x | {} | profit if win: +{} | settles in 2h",
        side.to_uppercase(),
        if currently_flooding { "flood alert now" } else { "no alert now" },
        1.0 / price,
        chips_str(stake),
        chips_str(payout - stake),
    ));

    let wcmd = ctx.runtime.whisper_command.clone();
    tokio::spawn(settle_task(ctx.state.clone(), wcmd, bet));
    Ok(())
}

pub async fn settle_task(
    state: AzaleaState,
    whisper_cmd: String,
    bet: NOAAFloodingBet,
) {
    sleep_until(bet.close_time).await;

    let claimed = {
        let mut bets = state.noaa_flooding_bets.lock().expect("noaa_flooding_bets lock");
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
    let result: Option<bool> = loop {
        match poll_flood_state(&client, bet.latitude, bet.longitude).await {
            Some(flooding) => break Some(flooding),
            None => {
                if now_unix() >= deadline { break None; }
                tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
            }
        }
    };

    state.api.casino_noaa_flooding_bet_delete(bet.id).await;

    let msg = match result {
        Some(was_flooding) => {
            let won = (bet.side == "yes") == was_flooding;
            if won {
                let payout = calc_payout(bet.stake, bet.price);
                if let Err(e) = state.api.casino_adjust(&bet.player, payout).await {
                    eprintln!("[NOAA Flood settle] casino_adjust failed for {}: {e:?}", bet.player);
                }
                format!(
                    "[NOAA Flood] {} — {}. {} wins. WIN +{} ({} @ {:.2}x).",
                    bet.location,
                    if was_flooding { "flood alert" } else { "no alert" },
                    bet.side.to_uppercase(),
                    chips_str(payout - bet.stake),
                    chips_str(bet.stake),
                    1.0 / bet.price,
                )
            } else {
                state.api.casino_jackpot_rake(bet.stake).await;
                format!(
                    "[NOAA Flood] {} — {}. {} loses. LOSS -{} (to jackpot).",
                    bet.location,
                    if was_flooding { "flood alert" } else { "no alert" },
                    bet.side.to_uppercase(),
                    chips_str(bet.stake),
                )
            }
        }
        None => {
            if let Err(e) = state.api.casino_adjust(&bet.player, bet.stake).await {
                eprintln!("[NOAA Flood settle] refund failed for {}: {e:?}", bet.player);
            }
            format!(
                "[NOAA Flood] {} — NOAA API unavailable. {} refunded.",
                bet.location,
                chips_str(bet.stake),
            )
        }
    };

    deliver(&state, &whisper_cmd, &bet.player, msg).await;
}

#[cfg(test)]
mod tests {
    use super::is_flood_related;
    use serde_json::json;

    #[test]
    fn recognizes_flood_alerts() {
        let alert = json!({"properties": {"event": "Flash Flood Warning", "headline": "Flash Flood Warning"}});
        assert!(is_flood_related(&alert));
    }

    #[test]
    fn ignores_non_flood_alerts() {
        let alert = json!({"properties": {"event": "Severe Thunderstorm Warning", "headline": "Severe Thunderstorm Warning"}});
        assert!(!is_flood_related(&alert));
    }
}
