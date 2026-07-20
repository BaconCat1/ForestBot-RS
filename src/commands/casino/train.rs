use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::market::types::now_unix;

use super::{chips_str, format_alimony, fmt_close, calc_payout, sleep_until, gtfs_rt, FetchErr, check_resp, SettleDeps};

type TrainBetsMap = std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<TrainBet>>>>;

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["train", "trains"],
    description: "Train delay bets. !train list <country|agency> | !train <country> <code> ontime|delayed [chips] | !train <agency> <route> ontime|delayed [chips] | !train bets",
    whitelisted: false,
    execute,
};

const TRAINS_BASE: &str = "https://trainstracking.com";
const DELAY_THRESHOLD_SECS: i32 = 300; // 5 minutes
const DELAY_THRESHOLD_MINS: i64 = 5;
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

impl super::CasinoBet for TrainBet {
    const TYPE: &'static str = "train";

    fn to_insert_json(&self) -> serde_json::Value {
        serde_json::json!({
            "player_uuid": self.player,
            "country":     self.country,
            "train_code":  self.train_code,
            "train_name":  self.train_name,
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
            country:    item.get("country")?.as_str()?.to_owned(),
            train_code: item.get("train_code")?.as_str()?.to_owned(),
            train_name: item.get("train_name")?.as_str()?.to_owned(),
            side:       item.get("side")?.as_str()?.to_owned(),
            price:      item.get("price")?.as_f64()?,
            stake:      item.get("stake")?.as_i64()?,
            close_time: item.get("close_time")?.as_u64()?,
        })
    }
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
    const RAKE: f64 = 0.03;
    if currently_delayed {
        (0.33 / (1.0 - RAKE), 0.67 / (1.0 - RAKE))
    } else {
        (0.67 / (1.0 - RAKE), 0.33 / (1.0 - RAKE))
    }
}


async fn fetch_trains(client: &reqwest::Client, country: &str) -> Result<Vec<serde_json::Value>, FetchErr> {
    let url = format!("{TRAINS_BASE}/api/live/realtime?source={country}");
    let resp = client
        .get(&url)
        .header("User-Agent", "ForestBot-RS/1.0")
        .send()
        .await
        .map_err(|_| FetchErr::Error)?;
    let resp = check_resp(resp).await?;
    let body: serde_json::Value = resp.json().await.map_err(|_| FetchErr::Error)?;
    body["trains"].as_array().cloned().ok_or(FetchErr::Error)
}

async fn poll_train_legacy(client: &reqwest::Client, country: &str, train_code: &str) -> PollOutcome {
    let Ok(trains) = fetch_trains(client, country).await else {
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
                if !allowed { ctx.whisper_success("Whitelist only."); return Ok(()); }
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
        ctx.whisper_error(format!("Unknown agency '{slug}'."));
        return Ok(());
    };
    let client = reqwest::Client::new();
    let resp = client
        .get(agency.tu_url)
        .header("User-Agent", "ForestBot-RS/1.0")
        .send()
        .await;
    let Ok(resp) = resp else {
        ctx.whisper_success(format!("HTTP request failed: {:?}", resp.unwrap_err()));
        return Ok(());
    };
    let status = resp.status().as_u16();
    let bytes = resp.bytes().await;
    let Ok(bytes) = bytes else {
        ctx.whisper_success(format!("HTTP {status}, body read failed."));
        return Ok(());
    };
    let byte_len = bytes.len();
    let feed = gtfs_rt::FeedMessage::decode(bytes);
    let Ok(feed) = feed else {
        ctx.whisper_success(format!("HTTP {status}, {byte_len}B, prost decode FAILED: {:?}", feed.unwrap_err()));
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
    ctx.whisper_success(format!(
        "[debug] {status} {byte_len}B | entities={entity_count} with_tu={with_tu} | first_entity: {first_entity} | first_tu: {first_tu}",
    ));
    Ok(())
}

fn show_usage(ctx: &CommandContext<'_>) {
    let p = &ctx.runtime.prefix;
    ctx.whisper_success(format!(
        "Train delay bets: {p}train list <country|agency> | {p}train <country> <code> ontime|delayed [chips] | {p}train <agency> <route> ontime|delayed [chips] | {p}train bets | Omit chips for odds preview | Countries: us de fr be ch fi nl no at se it es pl cz my | Agencies: mbta mta mta-ace mta-bdfm mta-nqrw mta-l mta-g mta-jz lirr metro-north"
    ));
}

// ── Legacy: show_trains ───────────────────────────────────────────────────────

async fn show_trains(ctx: &CommandContext<'_>, country_raw: &str) -> anyhow::Result<()> {
    let Some(country) = normalize_country(country_raw) else {
        ctx.whisper_error(format!(
            "Unknown '{country_raw}'. Countries: us de fr be ch fi nl no at se it es pl cz my | Agencies: mbta mta mta-ace mta-bdfm mta-nqrw mta-l mta-g mta-jz lirr metro-north"
        ));
        return Ok(());
    };
    let client = reqwest::Client::new();
    let trains = match fetch_trains(&client, country).await {
        Ok(t) => t,
        Err(FetchErr::RateLimit) => {
            ctx.whisper_success("Trains API rate limit reached. Try again later.");
            return Ok(());
        }
        Err(_) => {
            ctx.whisper_success(format!("Could not fetch trains for {country}."));
            return Ok(());
        }
    };
    if trains.is_empty() {
        ctx.whisper_success(format!("No running trains in {country} feed right now."));
        return Ok(());
    }
    let items: Vec<String> = trains.iter().take(8).filter_map(|t| {
        let code  = t["trainCode"].as_str()?;
        let delay = t["delay"].as_i64().unwrap_or(0);
        let d_str = if delay > 0 { format!("+{}m", delay) } else { "ontime".to_owned() };
        Some(format!("{code} {d_str}"))
    }).collect();
    ctx.whisper_success(format!(
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
        ctx.whisper_success(format!("Could not fetch {} feed.", agency.display));
        return Ok(());
    };
    let now = now_unix();
    let by_route = gtfs_rt::rail_trips_by_route(&trips, agency.slug, now);
    if by_route.is_empty() {
        ctx.whisper_success(format!("No upcoming {} rail departures.", agency.display));
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
    ctx.whisper_success(format!(
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
        ctx.whisper_success(format!(
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
            ctx.whisper_success(format!(
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
        ctx.whisper_success("Side must be ontime or delayed.");
        return Ok(());
    }

    let client = reqwest::Client::new();
    let Some(trips) = gtfs_rt::fetch_trip_updates(&client, agency.tu_url).await else {
        ctx.whisper_success(format!("Could not fetch {} feed.", agency.display));
        return Ok(());
    };
    let now = now_unix();
    let Some(snap) = gtfs_rt::find_next_predeparture(&trips, agency.slug, &route_s, now) else {
        ctx.whisper_error(format!(
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
        ctx.whisper_success(format!(
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
    match ctx.state.api.casino_bet_insert(&bet).await {
        Some(id) => { bet.id = id; }
        None => {
            if let Err(e) = ctx.state.api.casino_adjust(&player_uuid, stake).await {
                eprintln!("[Train] refund failed for {player_uuid}: {e:?}");
                ctx.whisper_success("Failed to save bet. Refund also failed — contact an admin.");
            } else {
                ctx.whisper_success("Failed to save bet. Chips refunded.");
            }
            return Ok(());
        }
    }
    {
        let mut bets = ctx.state.train_bets.lock().expect("train_bets lock");
        bets.entry(player_uuid.clone()).or_default().push(bet.clone());
    }

    let payout = calc_payout(stake, price);
    let settles_in = if close_time > now { (close_time - now + 59) / 60 } else { 0 };
    ctx.whisper_success(format!(
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
    tokio::spawn(settle_task(SettleDeps::from(ctx.state), ctx.state.train_bets.clone(), wcmd, bet));
    Ok(())
}

// ── show_bets ─────────────────────────────────────────────────────────────────

async fn show_bets(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let Some(player_uuid) = ctx.require_player_uuid().await else { return Ok(()); };
    let all_bets = ctx.state.api.casino_bet_list::<TrainBet>().await;
    let player_bets: Vec<_> = all_bets.into_iter().filter(|b| b.player == player_uuid).collect();
    if player_bets.is_empty() {
        ctx.whisper_success("No open train bets.");
        return Ok(());
    }
    for bet in &player_bets {
        let payout = calc_payout(bet.stake, bet.price);
        ctx.whisper_success(format!(
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
        ctx.whisper_error(format!(
            "Unknown '{country_s}'. Countries: us de fr be ch fi nl no at se it es pl cz my | Agencies: mbta mta mta-ace mta-bdfm mta-nqrw mta-l mta-g mta-jz lirr metro-north"
        ));
        return Ok(());
    };
    let side = side_s.to_lowercase();
    if side != "ontime" && side != "delayed" {
        ctx.whisper_success("Side must be ontime or delayed.");
        return Ok(());
    }

    let client = reqwest::Client::new();
    let trains = match fetch_trains(&client, country).await {
        Ok(t) => t,
        Err(FetchErr::RateLimit) => {
            ctx.whisper_success("Trains API rate limit reached. Try again later.");
            return Ok(());
        }
        Err(_) => {
            ctx.whisper_success(format!("Could not fetch trains for {country}."));
            return Ok(());
        }
    };
    let Some(train) = trains.iter().find(|t| {
        t["trainCode"].as_str()
            .map(|c| c.eq_ignore_ascii_case(&code_s))
            .unwrap_or(false)
    }) else {
        ctx.whisper_error(format!("Train '{code_s}' not found in {country} realtime feed."));
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
        ctx.whisper_success(format!(
            "[Train] {train_name} ({train_code}) | {delay_str} | ontime {:.2}x | delayed {:.2}x | {} {:.2}x selected",
            1.0 / ontime_price, 1.0 / delayed_price, side.to_uppercase(), 1.0 / price,
        ));
        return Ok(());
    }

    let amt_s = amt_s.unwrap();
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

    let close_time = now_unix() + ctx.runtime.train_bet_duration_ms / 1000;
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
    match ctx.state.api.casino_bet_insert(&bet).await {
        Some(id) => { bet.id = id; }
        None => {
            if let Err(e) = ctx.state.api.casino_adjust(&player_uuid, stake).await {
                eprintln!("[Train] refund failed for {player_uuid}: {e:?}");
                ctx.whisper_success("Failed to save bet. Refund also failed — contact an admin.");
            } else {
                ctx.whisper_success("Failed to save bet. Chips refunded.");
            }
            return Ok(());
        }
    }
    {
        let mut bets = ctx.state.train_bets.lock().expect("train_bets lock");
        bets.entry(player_uuid.clone()).or_default().push(bet.clone());
    }

    let payout = calc_payout(stake, price);
    ctx.whisper_success(format!(
        "[Train] {train_name} ({train_code}) | {delay_str} | {} {:.2}x | {} | profit if win: +{} | settles in 2h",
        side.to_uppercase(),
        1.0 / price,
        chips_str(stake),
        chips_str(payout - stake),
    ));

    let wcmd = ctx.runtime.whisper_command.clone();
    tokio::spawn(settle_task(SettleDeps::from(ctx.state), ctx.state.train_bets.clone(), wcmd, bet));
    Ok(())
}

// ── settle_task (unified) ─────────────────────────────────────────────────────
//
// Legacy bets: country field matches normalize_country → poll trainstracking API at close_time.
// GTFS-RT bets: country field is an agency slug → wake near close_time, find trip_id in feed.

pub async fn settle_task(deps: SettleDeps, bets_map: TrainBetsMap, whisper_cmd: String, bet: TrainBet) {
    let now = now_unix();
    if normalize_country(&bet.country).is_none() {
        // GTFS-RT path
        gtfs_settle(deps, bets_map, whisper_cmd, bet).await;
    } else {
        // Legacy path
        legacy_settle(deps, bets_map, whisper_cmd, bet, now).await;
    }
}

async fn legacy_settle(deps: SettleDeps, bets_map: TrainBetsMap, whisper_cmd: String, bet: TrainBet, started_at: u64) {
    sleep_until(bet.close_time).await;

    let claimed = claim_bet(&bets_map, &bet);
    if !claimed { return; }

    let client = reqwest::Client::new();

    let (max_poll_ms, poll_interval_ms) = {
        let runtime = deps.runtime.read().expect("runtime lock");
        (runtime.train_max_poll_ms, runtime.train_poll_interval_ms)
    };
    let deadline = now_unix() + max_poll_ms / 1000;
    let outcome: Option<bool> = loop {
        match poll_train_legacy(&client, &bet.country, &bet.train_code).await {
            PollOutcome::Found(currently_delayed) => break Some(currently_delayed),
            PollOutcome::Gone => break None,
            PollOutcome::ApiError => {
                if now_unix() >= deadline { break None; }
                tokio::time::sleep(std::time::Duration::from_millis(poll_interval_ms)).await;
            }
        }
    };

    let _ = started_at; // suppress unused warning
    deps.api.casino_bet_delete::<TrainBet>(bet.id).await;
    let msg = apply_outcome(&deps, &bet, outcome).await;
    deps.deliver(&whisper_cmd, &bet.player, msg).await;
}

async fn gtfs_settle(deps: SettleDeps, bets_map: TrainBetsMap, whisper_cmd: String, bet: TrainBet) {
    // Wake 2 minutes before the trip's last stop time so it's still in the feed.
    let wake_at = bet.close_time.saturating_sub(120);
    sleep_until(wake_at).await;

    let claimed = claim_bet(&bets_map, &bet);
    if !claimed { return; }

    let agency = gtfs_rt::resolve_agency(&bet.country);
    let tu_url = agency.map(|a| a.tu_url).unwrap_or("");

    // If the bet is already >1h past close_time the trip is gone from the feed — refund immediately.
    if now_unix() > bet.close_time + 3600 {
        deps.api.casino_bet_delete::<TrainBet>(bet.id).await;
        let msg = apply_outcome(&deps, &bet, None).await;
        deps.deliver(&whisper_cmd, &bet.player, msg).await;
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

    deps.api.casino_bet_delete::<TrainBet>(bet.id).await;
    let msg = apply_outcome(&deps, &bet, outcome).await;
    deps.deliver(&whisper_cmd, &bet.player, msg).await;
}

// ── Shared settle helpers ─────────────────────────────────────────────────────

fn claim_bet(bets_map: &TrainBetsMap, bet: &TrainBet) -> bool {
    let mut bets = bets_map.lock().expect("train_bets lock");
    bets.get_mut(&bet.player)
        .map(|v| {
            v.iter().position(|b| b.id == bet.id)
                .map(|i| { v.remove(i); })
                .is_some()
        })
        .unwrap_or(false)
}

async fn apply_outcome(deps: &SettleDeps, bet: &TrainBet, outcome: Option<bool>) -> String {
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
                match deps.api.casino_win(&bet.player, payout).await {
                    Ok(win) => {
                        let alimony_note = format_alimony(win.alimony_paid);
                        format!(
                            "[Train] {} ({}) — {}. {} wins. WIN +{}{alimony_note} ({} @ {:.2}x).",
                            bet.train_name,
                            bet.train_code,
                            outcome_str,
                            bet.side.to_uppercase(),
                            chips_str(payout - bet.stake),
                            chips_str(bet.stake),
                            1.0 / bet.price,
                        )
                    }
                    Err(e) => {
                        eprintln!("[Train settle] casino_win failed for {}: {e:?}", bet.player);
                        format!("[Train] {} ({}) — {} wins but payout failed. Contact an admin.", bet.train_name, bet.train_code, bet.side.to_uppercase())
                    }
                }
            } else {
                deps.api.casino_jackpot_rake(bet.stake).await;
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
            match deps.api.casino_adjust(&bet.player, bet.stake).await {
                Ok(_) => format!(
                    "[Train] {} ({}) — train not found or API error. {} refunded.",
                    bet.train_name,
                    bet.train_code,
                    chips_str(bet.stake),
                ),
                Err(e) => {
                    eprintln!("[Train settle] refund failed for {}: {e:?}", bet.player);
                    format!("[Train] {} ({}) — train not found or API error. Refund failed — contact an admin.", bet.train_name, bet.train_code)
                }
            }
        }
    }
}
