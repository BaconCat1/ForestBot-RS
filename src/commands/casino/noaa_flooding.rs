use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::market::types::now_unix;

use super::{chips_str, format_alimony, fmt_close, calc_payout, sleep_until, FetchErr, check_resp, SettleDeps};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["flood", "flooding", "noaa"],
    description: "NOAA flood-alert bets. !flood list | !flood <lat> <lon> yes|no <chips> | !flood bets",
    whitelisted: false,
    execute,
};

const NOAA_BASE: &str = "https://api.weather.gov";
const MIN_BET: i64 = 25;

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

impl super::CasinoBet for NOAAFloodingBet {
    const TYPE: &'static str = "noaa";

    fn to_insert_json(&self) -> serde_json::Value {
        serde_json::json!({
            "player_uuid": self.player,
            "location":    self.location,
            "latitude":    self.latitude,
            "longitude":   self.longitude,
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
            location:   item.get("location")?.as_str()?.to_owned(),
            latitude:   item.get("latitude")?.as_f64()?,
            longitude:  item.get("longitude")?.as_f64()?,
            side:       item.get("side")?.as_str()?.to_owned(),
            price:      item.get("price")?.as_f64()?,
            stake:      item.get("stake")?.as_i64()?,
            close_time: item.get("close_time")?.as_u64()?,
        })
    }
}


fn fmt_location(lat: f64, lon: f64) -> String {
    format!("{lat:.3},{lon:.3}")
}

fn compute_odds(currently_flooding: bool) -> (f64, f64) {
    const RAKE: f64 = 0.03;
    if currently_flooding {
        (0.67 / (1.0 - RAKE), 0.33 / (1.0 - RAKE))
    } else {
        (0.33 / (1.0 - RAKE), 0.67 / (1.0 - RAKE))
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

async fn fetch_active_alerts(client: &reqwest::Client, lat: f64, lon: f64) -> Result<Vec<serde_json::Value>, FetchErr> {
    let url = format!("{NOAA_BASE}/alerts/active?point={lat},{lon}&status=actual");
    let resp = client
        .get(&url)
        .header("User-Agent", "ForestBot-RS/1.0")
        .header("Accept", "application/geo+json")
        .send()
        .await
        .map_err(|_| FetchErr::Error)?;
    let resp = check_resp(resp).await?;
    let body = resp.json::<serde_json::Value>().await.map_err(|_| FetchErr::Error)?;
    body["features"].as_array().cloned().ok_or(FetchErr::Error)
}

async fn poll_flood_state(client: &reqwest::Client, lat: f64, lon: f64) -> Option<bool> {
    let alerts = fetch_active_alerts(client, lat, lon).await.ok()?;
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
    ctx.whisper_success(format!(
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
        ctx.whisper_success("Could not reach NOAA API.");
        return Ok(());
    };
    let Ok(body) = resp.json::<serde_json::Value>().await else {
        ctx.whisper_success("Could not parse NOAA response.");
        return Ok(());
    };
    let Some(features) = body["features"].as_array() else {
        ctx.whisper_success("No active flood alerts right now.");
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
        ctx.whisper_success("No active flood alerts with location data. Check alerts.weather.gov.");
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
    ctx.whisper_success(format!(
        "[Flood Alerts] {} | {p}flood bet <#> yes|no <chips>",
        items.join(" | ")
    ));
    Ok(())
}

async fn show_bets(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let Some(player_uuid) = ctx.require_player_uuid().await else { return Ok(()); };
    let all_bets = ctx.state.api.casino_bet_list::<NOAAFloodingBet>().await;
    let player_bets: Vec<_> = all_bets.into_iter().filter(|b| b.player == player_uuid).collect();
    if player_bets.is_empty() {
        ctx.whisper_success("No open NOAA flood bets.");
        return Ok(());
    }
    for bet in &player_bets {
        let payout = calc_payout(bet.stake, bet.price);
        ctx.whisper_success(format!(
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
        ctx.whisper_success(format!("Usage: {}flood bet <#> yes|no <chips> | Omit chips for odds preview", ctx.runtime.prefix));
        return Ok(());
    };
    let Ok(idx) = idx_s.parse::<usize>().map(|n| n.saturating_sub(1)) else {
        ctx.whisper_success("Event number must be a positive integer.");
        return Ok(());
    };
    let entry = {
        let cache = ctx.state.flood_cache.lock().expect("flood_cache lock");
        cache.entries.get(idx).map(|e| (e.area.clone(), e.latitude, e.longitude))
    };
    let Some((area, latitude, longitude)) = entry else {
        ctx.whisper_success("Event not found. Run !flood list to refresh.");
        return Ok(());
    };

    // preview mode — no chips provided
    let Some(&amt_s) = ctx.args.get(3) else {
        let client = reqwest::Client::new();
        let flooding = match fetch_active_alerts(&client, latitude, longitude).await {
            Ok(alerts) => alerts.iter().any(is_flood_related),
            Err(FetchErr::RateLimit) => {
                ctx.whisper_success("NOAA API rate limit reached. Try again later.");
                return Ok(());
            }
            Err(_) => false,
        };
        let (yes_price, no_price) = compute_odds(flooding);
        ctx.whisper_success(format!(
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
        ctx.whisper_success("Latitude must be numeric.");
        return Ok(());
    };
    let Ok(longitude) = lon_s.parse::<f64>() else {
        ctx.whisper_success("Longitude must be numeric.");
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
        ctx.whisper_success("Side must be yes or no.");
        return Ok(());
    }
    let Ok(stake) = amt_s.parse::<i64>() else {
        ctx.whisper_success("Chip amount must be a number.");
        return Ok(());
    };
    if stake < MIN_BET {
        ctx.whisper_success(format!("Minimum bet is {}.", chips_str(MIN_BET)));
        return Ok(());
    }

    let Some(player_uuid) = ctx.require_player_uuid().await else { return Ok(()); };
    match ctx.state.api.casino_adjust(&player_uuid, -stake).await {
        Ok(_) => {}
        Err(CasinoAdjustErr::InsufficientFunds(have)) => {
            ctx.whisper_success(format!("Need {} but only have {}.", chips_str(stake), chips_str(have)));
            return Ok(());
        }
        Err(CasinoAdjustErr::NetworkErr) => {
            ctx.whisper_success("Casino unavailable.");
            return Ok(());
        }
    }

    let client = reqwest::Client::new();
    let currently_flooding = match fetch_active_alerts(&client, latitude, longitude).await {
        Ok(alerts) => alerts.iter().any(is_flood_related),
        Err(FetchErr::RateLimit) => {
            if let Err(e) = ctx.state.api.casino_adjust(&player_uuid, stake).await {
                eprintln!("[NOAA] refund failed for {player_uuid}: {e:?}");
                ctx.whisper_success("NOAA API rate limit reached. Refund also failed — contact an admin.");
            } else {
                ctx.whisper_success("NOAA API rate limit reached. Chips refunded.");
            }
            return Ok(());
        }
        Err(_) => false,
    };
    let (yes_price, no_price) = compute_odds(currently_flooding);
    let price = if side == "yes" { yes_price } else { no_price };

    let close_time = now_unix() + ctx.runtime.noaa_flooding_bet_duration_ms / 1000;
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
    match ctx.state.api.casino_bet_insert(&bet).await {
        Some(id) => { bet.id = id; }
        None => {
            if let Err(e) = ctx.state.api.casino_adjust(&player_uuid, stake).await {
                eprintln!("[NOAA] refund failed for {player_uuid}: {e:?}");
                ctx.whisper_success("Failed to save bet. Refund also failed — contact an admin.");
            } else {
                ctx.whisper_success("Failed to save bet. Chips refunded.");
            }
            return Ok(());
        }
    }
    {
        let mut bets = ctx.state.noaa_flooding_bets.lock().expect("noaa_flooding_bets lock");
        bets.entry(player_uuid.clone()).or_default().push(bet.clone());
    }

    let payout = calc_payout(stake, price);
    ctx.whisper_success(format!(
        "[NOAA Flood] {location} | {} {} | {:.2}x | {} | profit if win: +{} | settles in 2h",
        side.to_uppercase(),
        if currently_flooding { "flood alert now" } else { "no alert now" },
        1.0 / price,
        chips_str(stake),
        chips_str(payout - stake),
    ));

    let wcmd = ctx.runtime.whisper_command.clone();
    tokio::spawn(settle_task(SettleDeps::from(ctx.state), ctx.state.noaa_flooding_bets.clone(), wcmd, bet));
    Ok(())
}

pub async fn settle_task(
    deps: SettleDeps,
    bets_map: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<NOAAFloodingBet>>>>,
    whisper_cmd: String,
    bet: NOAAFloodingBet,
) {
    sleep_until(bet.close_time).await;

    let claimed = {
        let mut bets = bets_map.lock().expect("noaa_flooding_bets lock");
        bets.get_mut(&bet.player)
            .map(|v| {
                let pos = v.iter().position(|b| b.id == bet.id);
                pos.map(|i| { v.remove(i); }).is_some()
            })
            .unwrap_or(false)
    };
    if !claimed { return; }

    let client = reqwest::Client::new();

    let (max_poll_ms, poll_interval_ms) = {
        let runtime = deps.runtime.read().expect("runtime lock");
        (runtime.noaa_flooding_max_poll_ms, runtime.noaa_flooding_poll_interval_ms)
    };
    let deadline = now_unix() + max_poll_ms / 1000;
    let result: Option<bool> = loop {
        match poll_flood_state(&client, bet.latitude, bet.longitude).await {
            Some(flooding) => break Some(flooding),
            None => {
                if now_unix() >= deadline { break None; }
                tokio::time::sleep(std::time::Duration::from_millis(poll_interval_ms)).await;
            }
        }
    };

    deps.api.casino_bet_delete::<NOAAFloodingBet>(bet.id).await;

    let msg = match result {
        Some(was_flooding) => {
            let won = (bet.side == "yes") == was_flooding;
            if won {
                let payout = calc_payout(bet.stake, bet.price);
                match deps.api.casino_win(&bet.player, payout).await {
                    Ok(win) => {
                        let alimony_note = format_alimony(win.alimony_paid);
                        format!(
                            "[NOAA Flood] {} — {}. {} wins. WIN +{}{alimony_note} ({} @ {:.2}x).",
                            bet.location,
                            if was_flooding { "flood alert" } else { "no alert" },
                            bet.side.to_uppercase(),
                            chips_str(payout - bet.stake),
                            chips_str(bet.stake),
                            1.0 / bet.price,
                        )
                    }
                    Err(e) => {
                        eprintln!("[NOAA Flood settle] casino_win failed for {}: {e:?}", bet.player);
                        format!("[NOAA Flood] {} — {} wins but payout failed. Contact an admin.", bet.location, bet.side.to_uppercase())
                    }
                }
            } else {
                deps.api.casino_jackpot_rake(bet.stake).await;
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
            match deps.api.casino_adjust(&bet.player, bet.stake).await {
                Ok(_) => format!(
                    "[NOAA Flood] {} — NOAA API unavailable. {} refunded.",
                    bet.location,
                    chips_str(bet.stake),
                ),
                Err(e) => {
                    eprintln!("[NOAA Flood settle] refund failed for {}: {e:?}", bet.player);
                    format!("[NOAA Flood] {} — NOAA API unavailable. Refund failed — contact an admin.", bet.location)
                }
            }
        }
    };

    deps.deliver(&whisper_cmd, &bet.player, msg).await;
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
