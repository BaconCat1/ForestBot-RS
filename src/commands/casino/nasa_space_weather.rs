use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::market::types::now_unix;
use crate::structure::mineflayer::bot::AzaleaState;

use super::{chips_str, format_alimony, fmt_close, sleep_until, SettleDeps};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["spaceweather", "sw"],
    description: "Space weather bets (settles midnight UTC). !spaceweather — list types | !spaceweather cme|xflare|gstorm <chips> | !spaceweather bets",
    whitelisted: false,
    execute,
};

const NASA_BASE: &str = "https://api.nasa.gov";
const SWPC_KP_URL: &str = "https://services.swpc.noaa.gov/products/noaa-planetary-k-index-forecast.json";
const MIN_BET: i64 = 25;
const HOUSE_EDGE: f64 = 0.97;
const DONKI_WINDOW_DAYS: i64 = 27;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct NasaSpaceWeatherBet {
    pub id: i64,
    pub player: String,
    pub bet_type: String,
    pub stake: i64,
    pub multiplier: f64,
    pub settle_at: u64,
}

impl super::CasinoBet for NasaSpaceWeatherBet {
    const TYPE: &'static str = "nasa";

    fn to_insert_json(&self) -> serde_json::Value {
        serde_json::json!({
            "player_uuid": self.player,
            "bet_type":    self.bet_type,
            "stake":       self.stake,
            "multiplier":  self.multiplier,
            "settle_at":   self.settle_at,
        })
    }

    fn from_json(item: &serde_json::Value) -> Option<Self> {
        Some(Self {
            id:         item.get("id")?.as_i64()?,
            player:     item.get("player_uuid")?.as_str()?.to_owned(),
            bet_type:   item.get("bet_type")?.as_str()?.to_owned(),
            stake:      item.get("stake")?.as_i64()?,
            multiplier: item.get("multiplier")?.as_f64()?,
            settle_at:  item.get("settle_at")?.as_u64()?,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SwOdds {
    pub cme: f64,
    pub xflare: f64,
    pub gstorm: f64,
}

impl SwOdds {
    fn for_type(&self, slug: &str) -> f64 {
        match slug {
            "cme" => self.cme,
            "xflare" => self.xflare,
            "gstorm" => self.gstorm,
            _ => 2.0,
        }
    }
}

const FALLBACK_ODDS: SwOdds = SwOdds { cme: 1.76, xflare: 12.13, gstorm: 4.85 };

struct BetKind {
    slug: &'static str,
    label: &'static str,
}

const BET_KINDS: &[BetKind] = &[
    BetKind { slug: "cme",    label: "Coronal Mass Ejection today"  },
    BetKind { slug: "xflare", label: "X-class solar flare today"    },
    BetKind { slug: "gstorm", label: "Geomagnetic storm today"      },
];

fn find_kind(slug: &str) -> Option<&'static BetKind> {
    BET_KINDS.iter().find(|k| k.slug.eq_ignore_ascii_case(slug))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn next_midnight_utc() -> u64 {
    let now = now_unix();
    now - (now % 86400) + 86400
}

fn unix_to_ymd(unix: u64) -> String {
    use chrono::{TimeZone, Utc};
    Utc.timestamp_opt(unix as i64, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "1970-01-01".to_owned())
}

fn today_utc_ymd() -> String {
    unix_to_ymd(now_unix())
}

fn prob_to_mult(p: f64) -> f64 {
    let p = p.clamp(0.03, 0.95);
    (HOUSE_EDGE / p * 100.0).round() / 100.0
}

async fn donki_array(
    client: &reqwest::Client,
    endpoint: &str,
    start_date: &str,
    end_date: &str,
    nasa_key: &str,
) -> Option<Vec<serde_json::Value>> {
    let url = format!("{NASA_BASE}/DONKI/{endpoint}?startDate={start_date}&endDate={end_date}&api_key={nasa_key}");
    client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .ok()?
        .json::<serde_json::Value>()
        .await
        .ok()?
        .as_array()
        .cloned()
}

async fn poll_event_occurred(client: &reqwest::Client, bet_type: &str, date: &str, nasa_key: &str) -> Option<bool> {
    match bet_type {
        "cme" => {
            let arr = donki_array(client, "CME", date, date, nasa_key).await?;
            Some(!arr.is_empty())
        }
        "xflare" => {
            let arr = donki_array(client, "FLR", date, date, nasa_key).await?;
            Some(arr.iter().any(|e| {
                e["classType"].as_str().map_or(false, |c| c.starts_with('X'))
            }))
        }
        "gstorm" => {
            let arr = donki_array(client, "GST", date, date, nasa_key).await?;
            Some(!arr.is_empty())
        }
        _ => None,
    }
}

// ── Live odds ─────────────────────────────────────────────────────────────────

async fn fetch_gstorm_prob(client: &reqwest::Client) -> Option<f64> {
    let arr: Vec<serde_json::Value> = client
        .get(SWPC_KP_URL)
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    let today = today_utc_ymd();
    let today_entries: Vec<&serde_json::Value> = arr.iter()
        .filter(|e| e["time_tag"].as_str().map_or(false, |t| t.starts_with(&today)))
        .collect();

    // Storm already occurred today — high confidence win.
    if today_entries.iter().any(|e| {
        e["observed"].as_str() == Some("observed")
            && e["kp"].as_f64().map_or(false, |kp| kp >= 5.0)
    }) {
        return Some(0.93);
    }

    let uncertain: Vec<&&serde_json::Value> = today_entries.iter().filter(|e| {
        matches!(e["observed"].as_str(), Some("predicted") | Some("estimated"))
    }).collect();

    if uncertain.is_empty() {
        // All slots observed, none hit Kp 5 — storm not happening today.
        return Some(0.03);
    }

    let storm_slots = uncertain.iter()
        .filter(|e| e["kp"].as_f64().map_or(false, |kp| kp >= 5.0))
        .count();

    Some((storm_slots as f64 / uncertain.len() as f64).max(0.03))
}

async fn donki_base_rate<F>(
    client: &reqwest::Client,
    endpoint: &str,
    filter: F,
    nasa_key: &str,
) -> Option<f64>
where
    F: Fn(&serde_json::Value) -> bool,
{
    use chrono::{Duration, Utc};
    let end = Utc::now();
    let start = end - Duration::days(DONKI_WINDOW_DAYS);
    let start_str = start.format("%Y-%m-%d").to_string();
    let end_str = end.format("%Y-%m-%d").to_string();
    let arr = donki_array(client, endpoint, &start_str, &end_str, nasa_key).await?;
    let count = arr.iter().filter(|e| filter(e)).count();
    Some(count as f64 / DONKI_WINDOW_DAYS as f64)
}

async fn fetch_sw_odds_live(client: &reqwest::Client, nasa_key: &str) -> SwOdds {
    let gstorm_p = fetch_gstorm_prob(client).await.unwrap_or(0.2);

    let cme_p = donki_base_rate(client, "CME", |_| true, nasa_key)
        .await
        .unwrap_or(0.55);

    let xflare_p = donki_base_rate(
        client, "FLR",
        |e| e["classType"].as_str().map_or(false, |c| c.starts_with('X')),
        nasa_key,
    )
    .await
    .unwrap_or(0.08);

    SwOdds {
        cme: prob_to_mult(cme_p),
        xflare: prob_to_mult(xflare_p),
        gstorm: prob_to_mult(gstorm_p),
    }
}

async fn load_odds(state: &AzaleaState, client: &reqwest::Client, nasa_key: &str) -> SwOdds {
    if nasa_key.trim().is_empty() {
        return FALLBACK_ODDS;
    }

    {
        let lock = state.sw_odds_cache.lock().expect("sw_odds_cache lock");
        if let Some((odds, fetched_at)) = *lock {
            let odds_cache_ttl_secs = state
                .runtime
                .read()
                .expect("runtime config lock poisoned")
                .nasa_space_weather_odds_cache_ttl_ms
                / 1000;
            if now_unix().saturating_sub(fetched_at) < odds_cache_ttl_secs {
                return odds;
            }
        }
    }

    let odds = fetch_sw_odds_live(client, nasa_key).await;
    {
        let mut lock = state.sw_odds_cache.lock().expect("sw_odds_cache lock");
        *lock = Some((odds, now_unix()));
    }
    odds
}

// ── Command entry ─────────────────────────────────────────────────────────────

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let first = ctx.args.first().copied().unwrap_or("");
        if first.is_empty() {
            show_types(&ctx).await;
        } else if first == "bets" || first == "my" {
            show_bets(&ctx).await?;
        } else {
            place_bet(&ctx).await?;
        }
        Ok(())
    })
}

async fn show_types(ctx: &CommandContext<'_>) {
    let p = &ctx.runtime.prefix;
    let nasa_key = ctx.runtime.nasa_api_key.clone();
    let odds = load_odds(ctx.state, &ctx.state.http, &nasa_key).await;
    let kinds: Vec<String> = BET_KINDS.iter()
        .map(|k| format!("{} {:.2}x", k.slug, odds.for_type(k.slug)))
        .collect();
    ctx.whisper_success(format!(
        "Space Weather bets: {} | Settles midnight UTC | {p}spaceweather <type> <chips>",
        kinds.join(", ")
    ));
}

// ── show_bets ─────────────────────────────────────────────────────────────────

async fn show_bets(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let Some(player_uuid) = ctx.require_player_uuid().await else { return Ok(()); };
    let all_bets = ctx.state.api.casino_bet_list::<NasaSpaceWeatherBet>().await;
    let player_bets: Vec<_> = all_bets.into_iter().filter(|b| b.player == player_uuid).collect();
    if player_bets.is_empty() {
        ctx.whisper_success("No open space weather bets.");
        return Ok(());
    }
    for bet in &player_bets {
        let payout = (bet.stake as f64 * bet.multiplier) as i64;
        let label = find_kind(&bet.bet_type).map_or(bet.bet_type.as_str(), |k| k.label);
        ctx.whisper_success(format!(
            "[SpaceWX] {} | {} -> {} | {}",
            label,
            chips_str(bet.stake),
            chips_str(payout),
            fmt_close(bet.settle_at),
        ));
    }
    Ok(())
}

// ── place_bet ─────────────────────────────────────────────────────────────────

async fn place_bet(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    if ctx.runtime.nasa_api_key.trim().is_empty() {
        ctx.whisper_success("Space weather bets are not configured on this server.");
        return Ok(());
    }
    let (Some(&type_s), Some(&amt_s)) = (ctx.args.first(), ctx.args.get(1)) else {
        show_types(ctx).await;
        return Ok(());
    };
    let Some(kind) = find_kind(type_s) else {
        ctx.whisper_success(format!(
            "Unknown type. Valid: {}",
            BET_KINDS.iter().map(|k| k.slug).collect::<Vec<_>>().join(", ")
        ));
        return Ok(());
    };
    let Ok(stake) = amt_s.parse::<i64>() else {
        ctx.whisper_success("Chip amount must be a number.");
        return Ok(());
    };
    let limit = ctx.bet_limit("nasa_space_weather", MIN_BET, None);
    if stake < limit.min {
        ctx.whisper_success(format!("Minimum bet is {}.", chips_str(limit.min)));
        return Ok(());
    }

    let nasa_key = ctx.runtime.nasa_api_key.clone();
    let odds = load_odds(ctx.state, &ctx.state.http, &nasa_key).await;
    let multiplier = odds.for_type(kind.slug);

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
    let settle_at = next_midnight_utc();
    let mut bet = NasaSpaceWeatherBet {
        id: 0,
        player: player_uuid.clone(),
        bet_type: kind.slug.to_owned(),
        stake,
        multiplier,
        settle_at,
    };
    match ctx.state.api.casino_bet_insert(&bet).await {
        Some(id) => { bet.id = id; }
        None => {
            if let Err(e) = ctx.state.api.casino_adjust(&player_uuid, stake).await {
                eprintln!("[NASA] refund failed for {player_uuid}: {e:?}");
                ctx.whisper_success("Failed to save bet. Refund also failed — contact an admin.");
            } else {
                ctx.whisper_success("Failed to save bet. Chips refunded.");
            }
            return Ok(());
        }
    }
    {
        let mut bets = ctx.state.nasa_space_weather_bets.lock().expect("nasa_space_weather_bets lock");
        bets.entry(player_uuid.clone()).or_default().push(bet.clone());
    }
    let payout = (stake as f64 * multiplier) as i64;
    ctx.whisper_success(format!(
        "[SpaceWX] {} | {} | {:.2}x | profit if YES: +{} | settles {}",
        kind.label,
        chips_str(stake),
        multiplier,
        chips_str(payout - stake),
        fmt_close(settle_at),
    ));
    let wcmd = ctx.runtime.whisper_command.clone();
    let deps = SettleDeps::from(ctx.state);
    let bets_map = ctx.state.nasa_space_weather_bets.clone();
    let http = ctx.state.http.clone();
    tokio::spawn(settle_task(deps, bets_map, http, wcmd, nasa_key, bet));
    Ok(())
}

// ── settle_task ───────────────────────────────────────────────────────────────

pub async fn settle_task(
    deps: SettleDeps,
    bets_map: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<NasaSpaceWeatherBet>>>>,
    http: reqwest::Client,
    whisper_cmd: String,
    nasa_api_key: String,
    bet: NasaSpaceWeatherBet,
) {
    sleep_until(bet.settle_at).await;
    // Buffer: give NASA 1h after midnight to log events.
    // Credit elapsed time against buffer so stale bets don't spin forever through bot restarts.
    let elapsed_past_close = now_unix().saturating_sub(bet.settle_at);
    let settle_buffer_secs = deps
        .runtime
        .read()
        .expect("runtime lock")
        .nasa_space_weather_settle_buffer_ms
        / 1000;
    let remaining_buffer = settle_buffer_secs.saturating_sub(elapsed_past_close);
    if remaining_buffer > 0 {
        tokio::time::sleep(std::time::Duration::from_secs(remaining_buffer)).await;
    }

    let claimed = {
        let mut bets = bets_map.lock().expect("nasa_space_weather_bets lock");
        bets.get_mut(&bet.player)
            .map(|v| {
                let pos = v.iter().position(|b| b.id == bet.id);
                pos.map(|i| { v.remove(i); }).is_some()
            })
            .unwrap_or(false)
    };
    if !claimed { return; }

    let client = &http;

    // Settle date = the day before midnight (i.e. the day the bet was placed on)
    let settle_date = unix_to_ymd(bet.settle_at - 86400);

    let (max_poll_ms, poll_interval_ms) = {
        let runtime = deps.runtime.read().expect("runtime lock");
        (runtime.nasa_space_weather_max_poll_ms, runtime.nasa_space_weather_poll_interval_ms)
    };
    let deadline = now_unix() + max_poll_ms / 1000;
    let result: Option<bool> = loop {
        match poll_event_occurred(client, &bet.bet_type, &settle_date, &nasa_api_key).await {
            Some(r) => break Some(r),
            None => {
                if now_unix() >= deadline { break None; }
                tokio::time::sleep(std::time::Duration::from_millis(poll_interval_ms)).await;
            }
        }
    };

    deps.api.casino_bet_delete::<NasaSpaceWeatherBet>(bet.id).await;

    let label = find_kind(&bet.bet_type).map_or(bet.bet_type.as_str(), |k| k.label);

    let msg = match result {
        Some(true) => {
            let payout = (bet.stake as f64 * bet.multiplier) as i64;
            match deps.api.casino_win(&bet.player, payout).await {
                Ok(win) => {
                    let alimony_note = format_alimony(win.alimony_paid);
                    format!(
                        "[SpaceWX] {} — YES. WIN +{}{alimony_note} ({} @ {:.2}x).",
                        label,
                        chips_str(payout - bet.stake),
                        chips_str(bet.stake),
                        bet.multiplier,
                    )
                }
                Err(e) => {
                    eprintln!("[SpaceWX settle] casino_win failed for {}: {e:?}", bet.player);
                    format!("[SpaceWX] {} — YES, but payout failed. Contact an admin.", label)
                }
            }
        }
        Some(false) => {
            deps.api.casino_jackpot_rake(bet.stake).await;
            format!(
                "[SpaceWX] {} — NO. LOSS -{} (to jackpot).",
                label,
                chips_str(bet.stake),
            )
        }
        None => {
            match deps.api.casino_adjust(&bet.player, bet.stake).await {
                Ok(_) => format!(
                    "[SpaceWX] {} — NASA API unavailable. {} refunded.",
                    label,
                    chips_str(bet.stake),
                ),
                Err(e) => {
                    eprintln!("[SpaceWX settle] refund failed for {}: {e:?}", bet.player);
                    format!("[SpaceWX] {} — NASA API unavailable. Refund failed — contact an admin.", label)
                }
            }
        }
    };

    deps.deliver(&whisper_cmd, &bet.player, msg).await;
}
