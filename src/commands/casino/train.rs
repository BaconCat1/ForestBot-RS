use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::market::types::now_unix;
use crate::structure::mineflayer::bot::AzaleaState;

use super::{chips_str, fmt_close, calc_payout, sleep_until, deliver, gtfs_rt};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["train", "trains"],
    description: "Train delay bets. !train list <country|agency> | !train <country> <code> ontime|delayed [chips] | !train <agency> <route> ontime|delayed [chips] | !train bets",
    whitelisted: false,
    execute,
};

const TRAINS_BASE: &str = "https://trainstracking.com";
const DELAY_THRESHOLD_SECS: i32 = 300; // 5 minutes
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
    pub country: String,   // legacy: country slug; GTFS-RT: agency slug (e.g. "mbta")
    pub train_code: String, // legacy: train code; GTFS-RT: trip_id
    pub train_name: String, // legacy: train name; GTFS-RT: "MBTA Red" (display)
    pub side: String,
    pub price: f64,
    pub stake: i64,
    pub close_time: u64,
}

enum PollOutcome {
    Found(bool),  // bool = is_delayed
    Gone,         // not in feed — train arrived/cancelled → refund
    ApiError,
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

fn is_delayed_legacy(delay_mins: i64) -> bool {
    delay_mins > DELAY_THRESHOLD_MINS
}

// Currently delayed → ontime harder; on time → delayed harder.
fn compute_odds(currently_delayed: bool) -> (f64, f64) {
    if currently_delayed {
        (0.33, 0.67)
    } else {
        (0.67, 0.33)
    }
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

async fn poll_train_legacy(client: &reqwest::Client, country: &str, train_code: &str) -> PollOutcome {
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
            PollOutcome::Found(is_delayed_legacy(delay_mins))
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
            "debug" => {
                let allowed = !ctx.runtime.use_whitelist
                    || ctx.runtime.user_whitelist.iter().any(|u| u.eq_ignore_ascii_case(ctx.sender));
                if !allowed { ctx.whisper("Whitelist only."); return Ok(()); }
                gtfs_debug(&ctx, ctx.args.get(1).copied().unwrap_or("")).await?;
            }
            "list" => {
                let target = ctx.args.get(1).copied().unwrap_or("");
                if let Some(agency) = gtfs_rt::resolve_agency(target) {
                    gtfs_show_trains(&ctx, agency).await?;
                } else {
                    show_trains(&ctx, target).await?;
                }
            }
            first => {
                if let Some(agency) = gtfs_rt::resolve_agency(first) {
                    gtfs_place_bet(&ctx, agency).await?;
                } else {
                    place_bet(&ctx).await?;
                }
            }
        }
        Ok(())
    })
}

async fn gtfs_debug(ctx: &CommandContext<'_>, slug: &str) -> anyhow::Result<()> {
    use prost::Message as _;
    let Some(agency) = gtfs_rt::resolve_agency(slug) else {
        ctx.whisper(format!("Unknown agency '{slug}'."));
        return Ok(());
    };
    let client = reqwest::Client::new();
    let resp = client
        .get(agency.tu_url)
        .header("User-Agent", "ForestBot-RS/1.0")
        .send()
        .await;
    let Ok(resp) = resp else {
        ctx.whisper(format!("HTTP request failed: {:?}", resp.unwrap_err()));
        return Ok(());
    };
    let status = resp.status().as_u16();
    let bytes = resp.bytes().await;
    let Ok(bytes) = bytes else {
        ctx.whisper(format!("HTTP {status}, body read failed."));
        return Ok(());
    };
    let byte_len = bytes.len();
    let feed = gtfs_rt::FeedMessage::decode(bytes);
    let Ok(feed) = feed else {
        ctx.whisper(format!("HTTP {status}, {byte_len}B, prost decode FAILED: {:?}", feed.unwrap_err()));
        return Ok(());
    };
    let entity_count = feed.entity.len();
    let with_tu = feed.entity.iter().filter(|e| e.trip_update.is_some()).count();
    let first_entity = feed.entity.first().map(|e| {
        format!("id={:?} has_tu={}", e.id, e.trip_update.is_some())
    }).unwrap_or_else(|| "no entities".to_owned());
    let first_tu = feed.entity.iter().find_map(|e| e.trip_update.as_ref()).map(|tu| {
        let route = tu.trip.as_ref().and_then(|t| t.route_id.as_deref()).unwrap_or("?");
        let stus = tu.stop_time_update.len();
        let first_t = tu.stop_time_update.first()
            .and_then(|s| s.departure.as_ref().or(s.arrival.as_ref()))
            .and_then(|e| e.time);
        format!("route={route} stus={stus} first_t={first_t:?}")
    }).unwrap_or_else(|| "no trip_updates".to_owned());
    ctx.whisper(format!(
        "[debug] {status} {byte_len}B | entities={entity_count} with_tu={with_tu} | first_entity: {first_entity} | first_tu: {first_tu}",
    ));
    Ok(())
}

fn show_usage(ctx: &CommandContext<'_>) {
    let p = &ctx.runtime.prefix;
    ctx.whisper(format!(
        "Train delay bets: {p}train list <country|agency> | {p}train <country> <code> ontime|delayed [chips] | {p}train <agency> <route> ontime|delayed [chips] | {p}train bets | Omit chips for odds preview | Countries: us de fr be ch fi nl no at se it es pl cz my | Agencies: mbta mta mta-ace mta-bdfm mta-nqrw mta-l mta-g mta-jz lirr metro-north"
    ));
}

// ── Legacy: show_trains ───────────────────────────────────────────────────────

async fn show_trains(ctx: &CommandContext<'_>, country_raw: &str) -> anyhow::Result<()> {
    let Some(country) = normalize_country(country_raw) else {
        ctx.whisper(format!(
            "Unknown '{country_raw}'. Countries: us de fr be ch fi nl no at se it es pl cz my | Agencies: mbta mta mta-ace mta-bdfm mta-nqrw mta-l mta-g mta-jz lirr metro-north"
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

// ── GTFS-RT: gtfs_show_trains ─────────────────────────────────────────────────

async fn gtfs_show_trains(
    ctx: &CommandContext<'_>,
    agency: &'static gtfs_rt::AgencyConfig,
) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let Some(trips) = gtfs_rt::fetch_trip_updates(&client, agency.tu_url).await else {
        ctx.whisper(format!("Could not fetch {} feed.", agency.display));
        return Ok(());
    };
    let now = now_unix();
    let by_route = gtfs_rt::rail_trips_by_route(&trips, agency.slug, now);
    if by_route.is_empty() {
        ctx.whisper(format!("No upcoming {} rail departures.", agency.display));
        return Ok(());
    }
    let mut entries: Vec<_> = by_route.into_iter().collect();
    entries.sort_by_key(|(_, s)| s.first_stop_time);
    let items: Vec<String> = entries.iter().take(8).map(|(route, snap)| {
        let mins = (snap.first_stop_time as i64 - now as i64).max(0) / 60;
        let d_str = if snap.first_delay_secs > DELAY_THRESHOLD_SECS {
            format!("+{}m", snap.first_delay_secs / 60)
        } else {
            "ontime".to_owned()
        };
        format!("{route} {mins}m {d_str}")
    }).collect();
    ctx.whisper(format!(
        "[{}] {} | {}train {} <route> ontime|delayed [chips]",
        agency.display,
        items.join(" | "),
        ctx.runtime.prefix,
        agency.slug,
    ));
    Ok(())
}

// ── GTFS-RT: gtfs_place_bet ───────────────────────────────────────────────────

async fn gtfs_place_bet(
    ctx: &CommandContext<'_>,
    agency: &'static gtfs_rt::AgencyConfig,
) -> anyhow::Result<()> {
    // args: <agency_slug> <route...> ontime|delayed [chips]
    if ctx.args.len() < 3 {
        ctx.whisper(format!(
            "Usage: {}train {} <route> ontime|delayed [chips]",
            ctx.runtime.prefix, agency.slug
        ));
        return Ok(());
    }
    let last = ctx.args[ctx.args.len() - 1].to_lowercase();
    let preview = last == "ontime" || last == "delayed";
    let (side_s, amt_s, route_s) = if preview {
        (ctx.args[ctx.args.len() - 1], None, ctx.args[1..ctx.args.len() - 1].join(" "))
    } else {
        if ctx.args.len() < 4 {
            ctx.whisper(format!(
                "Usage: {}train {} <route> ontime|delayed [chips]",
                ctx.runtime.prefix, agency.slug
            ));
            return Ok(());
        }
        (
            ctx.args[ctx.args.len() - 2],
            Some(ctx.args[ctx.args.len() - 1]),
            ctx.args[1..ctx.args.len() - 2].join(" "),
        )
    };

    let side = side_s.to_lowercase();
    if side != "ontime" && side != "delayed" {
        ctx.whisper("Side must be ontime or delayed.");
        return Ok(());
    }

    let client = reqwest::Client::new();
    let Some(trips) = gtfs_rt::fetch_trip_updates(&client, agency.tu_url).await else {
        ctx.whisper(format!("Could not fetch {} feed.", agency.display));
        return Ok(());
    };
    let now = now_unix();
    let Some(snap) = gtfs_rt::find_next_predeparture(&trips, agency.slug, &route_s, now) else {
        ctx.whisper(format!(
            "No upcoming pre-departure trips found for route '{}' in {} feed.",
            route_s, agency.display
        ));
        return Ok(());
    };

    let currently_delayed = snap.first_delay_secs > DELAY_THRESHOLD_SECS;
    let (ontime_price, delayed_price) = compute_odds(currently_delayed);
    let price = if side == "ontime" { ontime_price } else { delayed_price };
    let departs_in_mins = (snap.first_stop_time as i64 - now as i64).max(0) / 60;
    let delay_str = if snap.first_delay_secs > 0 {
        format!("+{}m now", snap.first_delay_secs / 60)
    } else {
        "ontime now".to_owned()
    };
    let train_name = format!("{} {}", agency.display, route_s);

    if preview {
        ctx.whisper(format!(
            "[Train] {} | departs {}m | {} | ontime {:.2}x | delayed {:.2}x | {} {:.2}x selected",
            train_name,
            departs_in_mins,
            delay_str,
            1.0 / ontime_price,
            1.0 / delayed_price,
            side.to_uppercase(),
            1.0 / price,
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

    // close_time = last stop scheduled arrival (settle when trip ends)
    let close_time = snap.last_stop_time.max(now + 120);

    let mut bet = TrainBet {
        id: 0,
        player: player_uuid.clone(),
        country: agency.slug.to_owned(),
        train_code: snap.trip_id.clone(),
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
    let settles_in = if close_time > now { (close_time - now + 59) / 60 } else { 0 };
    ctx.whisper(format!(
        "[Train] {} | {} | departs {}m | {} {:.2}x | {} | profit if win: +{} | settles in ~{}m",
        train_name,
        delay_str,
        departs_in_mins,
        side.to_uppercase(),
        1.0 / price,
        chips_str(stake),
        chips_str(payout - stake),
        settles_in,
    ));

    let wcmd = ctx.runtime.whisper_command.clone();
    tokio::spawn(settle_task(ctx.state.clone(), wcmd, bet));
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

// ── Legacy: place_bet ─────────────────────────────────────────────────────────

async fn place_bet(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
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
            "Unknown '{country_s}'. Countries: us de fr be ch fi nl no at se it es pl cz my | Agencies: mbta mta mta-ace mta-bdfm mta-nqrw mta-l mta-g mta-jz lirr metro-north"
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
    let currently_delayed = is_delayed_legacy(current_delay);
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

// ── settle_task (unified) ─────────────────────────────────────────────────────
//
// Legacy bets: country field matches normalize_country → poll trainstracking API at close_time.
// GTFS-RT bets: country field is an agency slug → wake near close_time, find trip_id in feed.

pub async fn settle_task(state: AzaleaState, whisper_cmd: String, bet: TrainBet) {
    let now = now_unix();
    if normalize_country(&bet.country).is_none() {
        // GTFS-RT path
        gtfs_settle(state, whisper_cmd, bet).await;
    } else {
        // Legacy path
        legacy_settle(state, whisper_cmd, bet, now).await;
    }
}

async fn legacy_settle(state: AzaleaState, whisper_cmd: String, bet: TrainBet, started_at: u64) {
    sleep_until(bet.close_time).await;

    let claimed = claim_bet(&state, &bet);
    if !claimed { return; }

    let client = reqwest::Client::new();

    let deadline = now_unix() + MAX_POLL_SECS;
    let outcome: Option<bool> = loop {
        match poll_train_legacy(&client, &bet.country, &bet.train_code).await {
            PollOutcome::Found(currently_delayed) => break Some(currently_delayed),
            PollOutcome::Gone => break None,
            PollOutcome::ApiError => {
                if now_unix() >= deadline { break None; }
                tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
            }
        }
    };

    let _ = started_at; // suppress unused warning
    state.api.casino_train_bet_delete(bet.id).await;
    let msg = apply_outcome(&state, &bet, outcome).await;
    deliver(&state, &whisper_cmd, &bet.player, msg).await;
}

async fn gtfs_settle(state: AzaleaState, whisper_cmd: String, bet: TrainBet) {
    // Wake 2 minutes before the trip's last stop time so it's still in the feed.
    let wake_at = bet.close_time.saturating_sub(120);
    sleep_until(wake_at).await;

    let claimed = claim_bet(&state, &bet);
    if !claimed { return; }

    let agency = gtfs_rt::resolve_agency(&bet.country);
    let tu_url = agency.map(|a| a.tu_url).unwrap_or("");

    // If the bet is already >1h past close_time the trip is gone from the feed — refund immediately.
    if now_unix() > bet.close_time + 3600 {
        state.api.casino_train_bet_delete(bet.id).await;
        let msg = apply_outcome(&state, &bet, None).await;
        deliver(&state, &whisper_cmd, &bet.player, msg).await;
        return;
    }

    // Poll up to 10 minutes to catch the trip near its last stop.
    let deadline = now_unix() + 600;
    let client = reqwest::Client::new();
    let outcome: Option<bool> = loop {
        let trips = gtfs_rt::fetch_trip_updates(&client, tu_url).await;
        match trips.and_then(|ts| gtfs_rt::find_trip_by_id(&ts, &bet.train_code)) {
            Some(snap) => {
                let delayed = snap.last_delay_secs > DELAY_THRESHOLD_SECS;
                break Some(delayed);
            }
            None => {
                if now_unix() >= deadline { break None; }
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            }
        }
    };

    state.api.casino_train_bet_delete(bet.id).await;
    let msg = apply_outcome(&state, &bet, outcome).await;
    deliver(&state, &whisper_cmd, &bet.player, msg).await;
}

// ── Shared settle helpers ─────────────────────────────────────────────────────

fn claim_bet(state: &AzaleaState, bet: &TrainBet) -> bool {
    let mut bets = state.train_bets.lock().expect("train_bets lock");
    bets.get_mut(&bet.player)
        .map(|v| {
            v.iter().position(|b| b.id == bet.id)
                .map(|i| { v.remove(i); })
                .is_some()
        })
        .unwrap_or(false)
}

async fn apply_outcome(state: &AzaleaState, bet: &TrainBet, outcome: Option<bool>) -> String {
    match outcome {
        Some(is_delayed_result) => {
            let won        = (bet.side == "delayed") == is_delayed_result;
            let outcome_str = if is_delayed_result {
                format!("delayed >{}m", DELAY_THRESHOLD_MINS)
            } else {
                "on time".to_owned()
            };
            if won {
                let payout = calc_payout(bet.stake, bet.price);
                if let Err(e) = state.api.casino_adjust(&bet.player, payout).await {
                    eprintln!("[Train settle] casino_adjust failed for {}: {e:?}", bet.player);
                }
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
                state.api.casino_jackpot_rake(bet.stake).await;
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
            if let Err(e) = state.api.casino_adjust(&bet.player, bet.stake).await {
                eprintln!("[Train settle] refund failed for {}: {e:?}", bet.player);
            }
            format!(
                "[Train] {} ({}) — train not found or API error. {} refunded.",
                bet.train_name,
                bet.train_code,
                chips_str(bet.stake),
            )
        }
    }
}
