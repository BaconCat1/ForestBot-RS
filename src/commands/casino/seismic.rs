use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::market::types::now_unix;
use crate::structure::mineflayer::bot::AzaleaState;

use super::{chips_str, fmt_close, calc_payout, sleep_until, deliver};

// ── Command definitions ───────────────────────────────────────────────────────

pub const QUAKE_COMMAND: CommandDefinition = CommandDefinition {
    names: &["quake", "earthquake", "eq"],
    description: "Earthquake bets. !quake list | !quake <region> [m<mag>] yes|no [chips] | !quake bets",
    whitelisted: false,
    execute: quake_execute,
};

pub const VOLCANO_COMMAND: CommandDefinition = CommandDefinition {
    names: &["volcano", "vol"],
    description: "Volcano bets. !volcano list | !volcano <name> yes|no [chips] | !volcano bets",
    whitelisted: false,
    execute: volcano_execute,
};

const MIN_BET: i64 = 25;
pub const BET_WINDOW_SECS: u64 = 7 * 24 * 3600; // 7 days
const HOUSE_EDGE: f64 = 0.05;

// ── Quake region definitions ──────────────────────────────────────────────────

pub struct QuakeRegion {
    pub slug: &'static str,
    pub display: &'static str,
    pub lat: f64,
    pub lon: f64,
    pub radius_km: f64,
    pub default_mag: f64,
}

pub const REGIONS: &[QuakeRegion] = &[
    QuakeRegion { slug: "california",  display: "California",   lat: 37.5,   lon: -120.0,  radius_km: 400.0, default_mag: 5.0 },
    QuakeRegion { slug: "alaska",      display: "Alaska",       lat: 62.0,   lon: -150.0,  radius_km: 800.0, default_mag: 5.0 },
    QuakeRegion { slug: "pacific-nw",  display: "Pacific NW",   lat: 47.0,   lon: -122.0,  radius_km: 400.0, default_mag: 5.0 },
    QuakeRegion { slug: "japan",       display: "Japan",        lat: 36.0,   lon:  138.0,  radius_km: 700.0, default_mag: 5.0 },
    QuakeRegion { slug: "indonesia",   display: "Indonesia",    lat: -5.0,   lon:  118.0,  radius_km: 1000.0, default_mag: 5.5 },
    QuakeRegion { slug: "chile",       display: "Chile",        lat: -33.0,  lon:  -70.5,  radius_km: 600.0, default_mag: 5.0 },
    QuakeRegion { slug: "italy",       display: "Italy",        lat: 42.0,   lon:   12.5,  radius_km: 400.0, default_mag: 4.5 },
    QuakeRegion { slug: "turkey",      display: "Turkey",       lat: 39.0,   lon:   35.0,  radius_km: 600.0, default_mag: 5.0 },
    QuakeRegion { slug: "new-zealand", display: "New Zealand",  lat: -41.0,  lon:  174.0,  radius_km: 400.0, default_mag: 4.5 },
];

pub fn resolve_region(slug: &str) -> Option<&'static QuakeRegion> {
    let lower = slug.to_lowercase();
    REGIONS.iter().find(|r| r.slug == lower.as_str() || r.display.to_lowercase() == lower.as_str())
}

// ── Quake bet struct ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct QuakeBet {
    pub id: i64,
    pub player: String,
    pub region_slug: String,
    pub display: String,   // e.g. "M5+ in California"
    pub side: String,      // "yes" | "no"
    pub price: f64,        // probability (win price)
    pub stake: i64,
    pub close_time: u64,
    pub mag: f64,
    pub lat: f64,
    pub lon: f64,
    pub radius_km: f64,
}

// ── Volcano bet struct ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct VolcanoBet {
    pub id: i64,
    pub player: String,
    pub vnum: String,      // USGS volcano number
    pub vname: String,
    pub side: String,      // "yes" | "no"
    pub price: f64,
    pub stake: i64,
    pub close_time: u64,
}

// ── Shared helpers ────────────────────────────────────────────────────────────

// Inflate probability by house-edge factor so actual payout = (1/price) ≈ (1-edge)/p_true.
fn probability_to_price(p: f64) -> f64 {
    (p / (1.0 - HOUSE_EDGE)).clamp(0.02, 0.98)
}

// ── Earthquake odds (Poisson from FDSN catalog) ───────────────────────────────

const FDSN_BASE: &str = "https://earthquake.usgs.gov/fdsnws/event/1/query";
// 3-year lookback: 2023-01-01 to 2026-01-01 (static window for stable base rates)
const LOOKBACK_DAYS: f64 = 3.0 * 365.0;
const LOOKBACK_START: &str = "2023-01-01";
const LOOKBACK_END:   &str = "2026-01-01";

async fn fetch_quake_count(
    client: &reqwest::Client,
    lat: f64,
    lon: f64,
    radius_km: f64,
    min_mag: f64,
) -> Option<u64> {
    let url = format!(
        "{FDSN_BASE}?format=geojson&starttime={LOOKBACK_START}&endtime={LOOKBACK_END}\
         &minmagnitude={min_mag}&latitude={lat}&longitude={lon}&maxradiuskm={radius_km}\
         &limit=20000&eventtype=earthquake"
    );
    let body: serde_json::Value = client
        .get(&url)
        .header("User-Agent", "ForestBot-RS/1.0")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    body["features"].as_array().map(|a| a.len() as u64)
}

fn poisson_probability(count: u64, window_days: f64) -> f64 {
    let lambda_per_day = count as f64 / LOOKBACK_DAYS;
    let expected = lambda_per_day * window_days;
    1.0 - (-expected).exp()
}

// ── Earthquake: check if event occurred in window ────────────────────────────

async fn quake_occurred(
    client: &reqwest::Client,
    lat: f64,
    lon: f64,
    radius_km: f64,
    min_mag: f64,
    after_unix: u64,
    before_unix: u64,
) -> Option<bool> {
    // Use FDSN event API with time window
    let start_iso = unix_to_iso(after_unix);
    let end_iso   = unix_to_iso(before_unix);
    let url = format!(
        "{FDSN_BASE}?format=geojson&starttime={start_iso}&endtime={end_iso}\
         &minmagnitude={min_mag}&latitude={lat}&longitude={lon}&maxradiuskm={radius_km}\
         &limit=1&eventtype=earthquake"
    );
    let body: serde_json::Value = client
        .get(&url)
        .header("User-Agent", "ForestBot-RS/1.0")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    let count = body["metadata"]["count"].as_u64().unwrap_or(0);
    Some(count > 0)
}

fn unix_to_iso(ts: u64) -> String {
    // Minimal ISO-8601 formatter without chrono dependency on UTC datetime
    let secs = ts;
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;
    let h = time_of_day / 3600;
    let m = (time_of_day % 3600) / 60;
    let s = time_of_day % 60;
    let (y, mo, d) = days_to_ymd(days_since_epoch as i64);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}")
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

// ── Volcano API ───────────────────────────────────────────────────────────────

const VHP_STATUS: &str = "https://volcanoes.usgs.gov/vsc/api/volcanoApi/vhpstatus";

#[derive(Debug)]
struct VolcanoStatus {
    vnum: String,
    vname: String,
    alert_level: String,
    color_code: String,
}

async fn fetch_all_volcano_status(client: &reqwest::Client) -> Option<Vec<VolcanoStatus>> {
    let body: serde_json::Value = client
        .get(VHP_STATUS)
        .header("User-Agent", "ForestBot-RS/1.0")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    let arr = body.as_array()?;
    Some(arr.iter().filter_map(|v| {
        let vnum   = v["vnum"].as_str()?.to_owned();
        let vname  = v["vName"].as_str()?.to_owned();
        let alert  = v["alertLevel"].as_str().unwrap_or("UNASSIGNED").to_owned();
        let color  = v["colorCode"].as_str().unwrap_or("UNASSIGNED").to_owned();
        Some(VolcanoStatus { vnum, vname, alert_level: alert, color_code: color })
    }).collect())
}

async fn fetch_volcano_status_by_vnum(client: &reqwest::Client, vnum: &str) -> Option<VolcanoStatus> {
    let url = format!("{VHP_STATUS}/{vnum}");
    let body: serde_json::Value = client
        .get(&url)
        .header("User-Agent", "ForestBot-RS/1.0")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    // API returns an array even for single volcano
    let item = if body.is_array() {
        body.get(0)?.clone()
    } else {
        body
    };
    let vname  = item["vName"].as_str()?.to_owned();
    let alert  = item["alertLevel"].as_str().unwrap_or("UNASSIGNED").to_owned();
    let color  = item["colorCode"].as_str().unwrap_or("UNASSIGNED").to_owned();
    Some(VolcanoStatus { vnum: vnum.to_owned(), vname, alert_level: alert, color_code: color })
}

fn is_elevated(vs: &VolcanoStatus) -> bool {
    !matches!(
        vs.alert_level.to_uppercase().as_str(),
        "NORMAL" | "UNASSIGNED" | ""
    )
}

// Probability that volcano reaches/stays at Warning (RED) within 7 days.
// Input: current alert level.
fn volcano_yes_probability(alert_level: &str) -> f64 {
    match alert_level.to_uppercase().as_str() {
        "WARNING"  => 0.70, // already erupting/imminent — likely stays at WARNING
        "WATCH"    => 0.20, // heightened unrest — escalation to WARNING possible
        "ADVISORY" => 0.05, // elevated unrest — escalation unlikely but nonzero
        _          => 0.02, // effectively no signal
    }
}

fn alert_level_tag(alert: &str) -> &'static str {
    match alert.to_uppercase().as_str() {
        "WARNING"  => "[!!!]",
        "WATCH"    => "[!!]",
        "ADVISORY" => "[!]",
        _          => "[-]",
    }
}

// ── Quake command ─────────────────────────────────────────────────────────────

fn quake_execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let subcmd = ctx.args.first().copied().unwrap_or("").to_owned();
        match subcmd.as_str() {
            "" => quake_usage(&ctx),
            "bets" | "my" => quake_show_bets(ctx).await?,
            "list" | "ls" => quake_list(ctx).await?,
            _ => quake_place_bet(ctx).await?,
        }
        Ok(())
    })
}

fn quake_usage(ctx: &CommandContext<'_>) {
    let p = &ctx.runtime.prefix;
    ctx.whisper(format!(
        "Earthquake bets (7-day window): \
         {p}quake list | \
         {p}quake <region> [m<mag>] yes|no [chips] | \
         {p}quake bets | \
         Regions: california alaska pacific-nw japan indonesia chile italy turkey new-zealand | \
         Default mag M5+ (override: m4.5 m5.5 m6 etc)"
    ));
}

async fn quake_list(ctx: CommandContext<'_>) -> anyhow::Result<()> {
    use futures_util::future::join_all;
    let client = reqwest::Client::new();
    let shown = &REGIONS[..5.min(REGIONS.len())];
    let fetches: Vec<_> = shown.iter().map(|region| {
        let c = client.clone();
        async move {
            let count = fetch_quake_count(&c, region.lat, region.lon, region.radius_km, region.default_mag)
                .await
                .unwrap_or(0);
            let raw_p = poisson_probability(count, 7.0);
            format!("{} M{}+ {:.0}%", region.display, region.default_mag, raw_p * 100.0)
        }
    }).collect();
    let parts: Vec<String> = join_all(fetches).await;
    let p = &ctx.runtime.prefix;
    ctx.whisper(format!(
        "[Earthquake] 7-day probability | {} | {p}quake <region> [m<mag>] yes|no <chips>",
        parts.join(" | ")
    ));
    Ok(())
}

async fn quake_show_bets(ctx: CommandContext<'_>) -> anyhow::Result<()> {
    let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
        ctx.whisper("Could not resolve your UUID.");
        return Ok(());
    };
    let all_bets = ctx.state.api.casino_quake_bet_list().await;
    let player_bets: Vec<_> = all_bets.into_iter().filter(|b| b.player == player_uuid).collect();
    if player_bets.is_empty() {
        ctx.whisper("No open earthquake bets.");
        return Ok(());
    }
    for bet in &player_bets {
        let payout = (bet.stake as f64 / bet.price).floor() as i64;
        ctx.whisper(format!(
            "[Quake] {} | {} {:.2}x | {} → {} | {}",
            bet.display,
            bet.side.to_uppercase(),
            (1.0 / (bet.price)),
            chips_str(bet.stake),
            chips_str(payout),
            fmt_close(bet.close_time),
        ));
    }
    Ok(())
}

async fn quake_place_bet(ctx: CommandContext<'_>) -> anyhow::Result<()> {
    // Syntax: !quake <region> [m<mag>] yes|no [chips]
    // args[0] = region, then optional "m<mag>", then side, then optional chips
    if ctx.args.is_empty() { quake_usage(&ctx); return Ok(()); }

    let region_s = ctx.args.first().copied().unwrap_or("");
    let Some(region) = resolve_region(region_s) else {
        ctx.whisper(format!(
            "Unknown region '{region_s}'. Regions: california alaska pacific-nw japan indonesia chile italy turkey new-zealand"
        ));
        return Ok(());
    };

    // Optional magnitude override: "m5.5" "m6" etc.
    let mut idx = 1usize;
    let mut mag = region.default_mag;
    if let Some(next) = ctx.args.get(idx).copied() {
        if next.to_lowercase().starts_with('m') {
            if let Ok(parsed) = next[1..].parse::<f64>() {
                mag = parsed.clamp(3.0, 8.0);
                idx += 1;
            }
        }
    }

    // Side: yes|no
    let side_s = ctx.args.get(idx).copied().unwrap_or("");
    let side = side_s.to_lowercase();
    if side != "yes" && side != "no" {
        // Odds preview (no side given means we just show current probability)
        let client = reqwest::Client::new();
        let count = fetch_quake_count(&client, region.lat, region.lon, region.radius_km, mag)
            .await
            .unwrap_or(0);
        let raw_p = poisson_probability(count, 7.0);
        let p_yes = probability_to_price(raw_p);
        let p_no  = probability_to_price(1.0 - raw_p);
        ctx.whisper(format!(
            "[Quake] {} M{}+ | 7d probability {:.1}% | yes {:.2}x | no {:.2}x | {}{} {} yes|no <chips>",
            region.display, mag, raw_p * 100.0,
            (1.0 / (p_yes)),
            (1.0 / (p_no)),
            ctx.runtime.prefix, region.slug, mag,
        ));
        return Ok(());
    }

    // Optional chips
    let amt_s = ctx.args.get(idx + 1).copied().unwrap_or("");

    let client = reqwest::Client::new();
    let count = fetch_quake_count(&client, region.lat, region.lon, region.radius_km, mag).await;
    let Some(count) = count else {
        ctx.whisper("FDSN API unavailable. Try again later.");
        return Ok(());
    };
    let p_yes = probability_to_price(poisson_probability(count, 7.0));
    let p_no  = probability_to_price(1.0 - poisson_probability(count, 7.0));
    let price = if side == "yes" { p_yes } else { p_no };

    // Preview if no chips
    if amt_s.is_empty() {
        ctx.whisper(format!(
            "[Quake] {} M{}+ | 7d yes {:.2}x | no {:.2}x | {} selected {:.2}x",
            region.display, mag,
            (1.0 / (p_yes)),
            (1.0 / (p_no)),
            side.to_uppercase(),
            (1.0 / (price)),
        ));
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

    let display = format!("M{}+ in {} (7d)", mag, region.display);
    let close_time = now_unix() + BET_WINDOW_SECS;
    let placed_at  = now_unix();

    let mut bet = QuakeBet {
        id: 0,
        player: player_uuid.clone(),
        region_slug: region.slug.to_owned(),
        display: display.clone(),
        side: side.clone(),
        price,
        stake,
        close_time,
        mag,
        lat: region.lat,
        lon: region.lon,
        radius_km: region.radius_km,
    };
    match ctx.state.api.casino_quake_bet_insert(&bet, placed_at).await {
        Some(id) => { bet.id = id; }
        None => {
            ctx.whisper("Failed to save bet. Refunding chips.");
            let _ = ctx.state.api.casino_adjust(&player_uuid, stake).await;
            return Ok(());
        }
    }
    {
        let mut bets = ctx.state.quake_bets.lock().expect("quake_bets lock");
        bets.entry(player_uuid.clone()).or_default().push(bet.clone());
    }

    let payout = (stake as f64 / price).floor() as i64;
    ctx.whisper(format!(
        "[Quake] {} | {} {:.2}x | {} | profit if win: +{} | settles in 7d",
        display,
        side.to_uppercase(),
        (1.0 / (price)),
        chips_str(stake),
        chips_str(payout - stake),
    ));

    let wcmd = ctx.runtime.whisper_command.clone();
    tokio::spawn(quake_settle_task(ctx.state.clone(), wcmd, bet, placed_at));
    Ok(())
}

// ── Quake settlement ──────────────────────────────────────────────────────────

pub async fn quake_settle_task(state: AzaleaState, whisper_cmd: String, bet: QuakeBet, placed_at: u64) {
    sleep_until(bet.close_time).await;

    let claimed = {
        let mut bets = state.quake_bets.lock().expect("quake_bets lock");
        bets.get_mut(&bet.player)
            .map(|v| v.iter().position(|b| b.id == bet.id).map(|i| { v.remove(i); }).is_some())
            .unwrap_or(false)
    };
    if !claimed { return; }

    let client = reqwest::Client::new();
    let result = quake_occurred(&client, bet.lat, bet.lon, bet.radius_km, bet.mag, placed_at, bet.close_time).await;

    state.api.casino_quake_bet_delete(bet.id).await;

    let msg = match result {
        Some(occurred) => {
            let won = (bet.side == "yes") == occurred;
            if won {
                let payout = calc_payout(bet.stake, bet.price);
                if let Err(e) = state.api.casino_adjust(&bet.player, payout).await {
                    eprintln!("[Quake settle] casino_adjust failed for {}: {e:?}", bet.player);
                }
                format!(
                    "[Quake] {} — {}. {} wins. WIN +{} ({} @ {:.2}x).",
                    bet.display,
                    if occurred { "event occurred" } else { "no event" },
                    bet.side.to_uppercase(),
                    chips_str(payout - bet.stake),
                    chips_str(bet.stake),
                    (1.0 / (bet.price)),
                )
            } else {
                state.api.casino_jackpot_rake(bet.stake).await;
                format!(
                    "[Quake] {} — {}. {} loses. LOSS -{} (to jackpot).",
                    bet.display,
                    if occurred { "event occurred" } else { "no event" },
                    bet.side.to_uppercase(),
                    chips_str(bet.stake),
                )
            }
        }
        None => {
            if let Err(e) = state.api.casino_adjust(&bet.player, bet.stake).await {
                eprintln!("[Quake settle] refund failed for {}: {e:?}", bet.player);
            }
            format!(
                "[Quake] {} — FDSN API unavailable. {} refunded.",
                bet.display,
                chips_str(bet.stake),
            )
        }
    };

    deliver(&state, &whisper_cmd, &bet.player, msg).await;
}

// ── Volcano command ───────────────────────────────────────────────────────────

fn volcano_execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let subcmd = ctx.args.first().copied().unwrap_or("").to_owned();
        match subcmd.as_str() {
            "" => volcano_usage(&ctx),
            "bets" | "my" => volcano_show_bets(ctx).await?,
            "list" | "ls" => volcano_list(ctx).await?,
            _ => volcano_place_bet(ctx).await?,
        }
        Ok(())
    })
}

fn volcano_usage(ctx: &CommandContext<'_>) {
    let p = &ctx.runtime.prefix;
    ctx.whisper(format!(
        "Volcano bets (7-day window, resolves YES if reaches Warning/Red): \
         {p}volcano list | \
         {p}volcano <name> yes|no [chips] | \
         {p}volcano bets | \
         Bet YES = volcano reaches Warning level within 7 days"
    ));
}

async fn volcano_list(ctx: CommandContext<'_>) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let Some(statuses) = fetch_all_volcano_status(&client).await else {
        ctx.whisper("VHP API unavailable.");
        return Ok(());
    };
    let elevated: Vec<_> = statuses.iter().filter(|v| is_elevated(v)).collect();
    if elevated.is_empty() {
        ctx.whisper("No elevated US volcanoes right now.");
        return Ok(());
    }
    let items: Vec<String> = elevated.iter().take(6).map(|v| {
        let p = volcano_yes_probability(&v.alert_level);
        format!(
            "{} {} {:.0}%",
            alert_level_tag(&v.alert_level),
            v.vname,
            p * 100.0,
        )
    }).collect();
    let p = &ctx.runtime.prefix;
    ctx.whisper(format!(
        "[Volcanoes] 7d yes % | {} | {p}volcano <name> yes|no <chips>",
        items.join(" | ")
    ));
    Ok(())
}

async fn volcano_show_bets(ctx: CommandContext<'_>) -> anyhow::Result<()> {
    let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
        ctx.whisper("Could not resolve your UUID.");
        return Ok(());
    };
    let all_bets = ctx.state.api.casino_volcano_bet_list().await;
    let player_bets: Vec<_> = all_bets.into_iter().filter(|b| b.player == player_uuid).collect();
    if player_bets.is_empty() {
        ctx.whisper("No open volcano bets.");
        return Ok(());
    }
    for bet in &player_bets {
        let payout = (bet.stake as f64 / bet.price).floor() as i64;
        ctx.whisper(format!(
            "[Volcano] {} | {} {:.2}x | {} → {} | {}",
            bet.vname,
            bet.side.to_uppercase(),
            (1.0 / (bet.price)),
            chips_str(bet.stake),
            chips_str(payout),
            fmt_close(bet.close_time),
        ));
    }
    Ok(())
}

async fn volcano_place_bet(ctx: CommandContext<'_>) -> anyhow::Result<()> {
    // Syntax: !volcano <name...> yes|no [chips]
    // "yes" bets on volcano reaching Warning within 7 days
    if ctx.args.is_empty() { volcano_usage(&ctx); return Ok(()); }

    // last arg is chips (number) or side (yes/no), second-to-last is side
    let last = ctx.args.last().copied().unwrap_or("");
    let is_chips = last.parse::<i64>().is_ok();
    let (chips_s, side_idx) = if is_chips {
        (Some(last), ctx.args.len().saturating_sub(2))
    } else {
        (None, ctx.args.len().saturating_sub(1))
    };
    let side_s = ctx.args.get(side_idx).copied().unwrap_or("");
    let side = side_s.to_lowercase();
    let name_query = ctx.args[..side_idx].join(" ").to_lowercase();

    if name_query.is_empty() {
        volcano_usage(&ctx);
        return Ok(());
    }

    if side != "yes" && side != "no" {
        // Odds preview
        let client = reqwest::Client::new();
        let Some(statuses) = fetch_all_volcano_status(&client).await else {
            ctx.whisper("VHP API unavailable.");
            return Ok(());
        };
        let Some(vs) = statuses.iter().find(|v| v.vname.to_lowercase().contains(&name_query)) else {
            ctx.whisper(format!("Volcano '{name_query}' not found. Use !volcano list to see elevated volcanoes."));
            return Ok(());
        };
        let raw_p = volcano_yes_probability(&vs.alert_level);
        let p_yes = probability_to_price(raw_p);
        let p_no  = probability_to_price(1.0 - raw_p);
        ctx.whisper(format!(
            "[Volcano] {} {} | yes {:.2}x | no {:.2}x | Bet resolves YES if reaches Warning within 7d",
            vs.vname,
            alert_level_tag(&vs.alert_level),
            (1.0 / (p_yes)),
            (1.0 / (p_no)),
        ));
        return Ok(());
    }

    let client = reqwest::Client::new();
    let Some(statuses) = fetch_all_volcano_status(&client).await else {
        ctx.whisper("VHP API unavailable.");
        return Ok(());
    };
    let Some(vs) = statuses.iter().find(|v| v.vname.to_lowercase().contains(&name_query)) else {
        ctx.whisper(format!("Volcano '{name_query}' not found. Use !volcano list to see elevated volcanoes."));
        return Ok(());
    };
    if !is_elevated(vs) {
        ctx.whisper(format!(
            "{} is currently {} — no signal to bet on. Use !volcano list for elevated volcanoes.",
            vs.vname, vs.alert_level
        ));
        return Ok(());
    }

    let raw_p = volcano_yes_probability(&vs.alert_level);
    let p_yes = probability_to_price(raw_p);
    let p_no  = probability_to_price(1.0 - raw_p);
    let price = if side == "yes" { p_yes } else { p_no };

    // Preview if no chips
    let Some(amt_s) = chips_s else {
        ctx.whisper(format!(
            "[Volcano] {} {} | {} {:.2}x | yes {:.2}x | no {:.2}x",
            vs.vname,
            alert_level_tag(&vs.alert_level),
            side.to_uppercase(),
            (1.0 / (price)),
            (1.0 / (p_yes)),
            (1.0 / (p_no)),
        ));
        return Ok(());
    };

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

    let close_time = now_unix() + BET_WINDOW_SECS;
    let mut bet = VolcanoBet {
        id: 0,
        player: player_uuid.clone(),
        vnum: vs.vnum.clone(),
        vname: vs.vname.clone(),
        side: side.clone(),
        price,
        stake,
        close_time,
    };
    match ctx.state.api.casino_volcano_bet_insert(&bet).await {
        Some(id) => { bet.id = id; }
        None => {
            ctx.whisper("Failed to save bet. Refunding chips.");
            let _ = ctx.state.api.casino_adjust(&player_uuid, stake).await;
            return Ok(());
        }
    }
    {
        let mut bets = ctx.state.volcano_bets.lock().expect("volcano_bets lock");
        bets.entry(player_uuid.clone()).or_default().push(bet.clone());
    }

    let payout = (bet.stake as f64 / price).floor() as i64;
    ctx.whisper(format!(
        "[Volcano] {} {} | {} {:.2}x | {} | profit if win: +{} | settles in 7d",
        vs.vname,
        alert_level_tag(&vs.alert_level),
        side.to_uppercase(),
        (1.0 / (price)),
        chips_str(stake),
        chips_str(payout - stake),
    ));

    let wcmd = ctx.runtime.whisper_command.clone();
    tokio::spawn(volcano_settle_task(ctx.state.clone(), wcmd, bet));
    Ok(())
}

// ── Volcano settlement ────────────────────────────────────────────────────────

pub async fn volcano_settle_task(state: AzaleaState, whisper_cmd: String, bet: VolcanoBet) {
    sleep_until(bet.close_time).await;

    let claimed = {
        let mut bets = state.volcano_bets.lock().expect("volcano_bets lock");
        bets.get_mut(&bet.player)
            .map(|v| v.iter().position(|b| b.id == bet.id).map(|i| { v.remove(i); }).is_some())
            .unwrap_or(false)
    };
    if !claimed { return; }

    let client = reqwest::Client::new();

    // Resolve: check if current alert level is WARNING/RED
    let result: Option<bool> = match fetch_volcano_status_by_vnum(&client, &bet.vnum).await {
        Some(vs) => Some(vs.color_code.to_uppercase() == "RED" || vs.alert_level.to_uppercase() == "WARNING"),
        None => None,
    };

    state.api.casino_volcano_bet_delete(bet.id).await;

    let msg = match result {
        Some(at_warning) => {
            let won = (bet.side == "yes") == at_warning;
            if won {
                let payout = calc_payout(bet.stake, bet.price);
                if let Err(e) = state.api.casino_adjust(&bet.player, payout).await {
                    eprintln!("[Volcano settle] casino_adjust failed for {}: {e:?}", bet.player);
                }
                format!(
                    "[Volcano] {} — {}. {} wins. WIN +{} ({} @ {:.2}x).",
                    bet.vname,
                    if at_warning { "Warning/Red" } else { "below Warning" },
                    bet.side.to_uppercase(),
                    chips_str(payout - bet.stake),
                    chips_str(bet.stake),
                    (1.0 / (bet.price)),
                )
            } else {
                state.api.casino_jackpot_rake(bet.stake).await;
                format!(
                    "[Volcano] {} — {}. {} loses. LOSS -{} (to jackpot).",
                    bet.vname,
                    if at_warning { "Warning/Red" } else { "below Warning" },
                    bet.side.to_uppercase(),
                    chips_str(bet.stake),
                )
            }
        }
        None => {
            if let Err(e) = state.api.casino_adjust(&bet.player, bet.stake).await {
                eprintln!("[Volcano settle] refund failed for {}: {e:?}", bet.player);
            }
            format!(
                "[Volcano] {} — VHP API unavailable. {} refunded.",
                bet.vname,
                chips_str(bet.stake),
            )
        }
    };

    deliver(&state, &whisper_cmd, &bet.player, msg).await;
}
