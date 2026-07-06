use crate::commands::{enqueue_chat, CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::market::types::now_unix;
use crate::structure::mineflayer::bot::AzaleaState;

use super::chips_str;

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["spaceweather", "sw"],
    description: "Space weather bets (settles midnight UTC). !spaceweather — list types | !spaceweather cme|xflare|gstorm <chips> | !spaceweather bets",
    whitelisted: false,
    execute,
};

const NASA_BASE: &str = "https://api.nasa.gov";
const MIN_BET: i64 = 25;
const POLL_INTERVAL_SECS: u64 = 600;
const MAX_POLL_SECS: u64 = 7200;
const SETTLE_BUFFER_SECS: u64 = 3600;

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

struct BetKind {
    slug: &'static str,
    label: &'static str,
    multiplier: f64,
}

const BET_KINDS: &[BetKind] = &[
    BetKind { slug: "cme",    label: "Coronal Mass Ejection today",  multiplier: 1.9  },
    BetKind { slug: "xflare", label: "X-class solar flare today",    multiplier: 12.0 },
    BetKind { slug: "gstorm", label: "Geomagnetic storm today",      multiplier: 5.0  },
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

fn fmt_close(settle_at: u64) -> String {
    let now = now_unix();
    if settle_at <= now { return "settling".into(); }
    let secs = settle_at - now;
    if secs < 3600       { format!("{}m", secs / 60) }
    else if secs < 86400 { format!("{}h", secs / 3600) }
    else                 { format!("{}d", secs / 86400) }
}

async fn donki_array(client: &reqwest::Client, endpoint: &str, date: &str, nasa_key: &str) -> Option<Vec<serde_json::Value>> {
    let url = format!("{NASA_BASE}/DONKI/{endpoint}?startDate={date}&endDate={date}&api_key={nasa_key}");
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
            let arr = donki_array(client, "CME", date, nasa_key).await?;
            Some(!arr.is_empty())
        }
        "xflare" => {
            let arr = donki_array(client, "FLR", date, nasa_key).await?;
            Some(arr.iter().any(|e| {
                e["classType"].as_str().map_or(false, |c| c.starts_with('X'))
            }))
        }
        "gstorm" => {
            let arr = donki_array(client, "GST", date, nasa_key).await?;
            Some(!arr.is_empty())
        }
        _ => None,
    }
}

// ── Command entry ─────────────────────────────────────────────────────────────

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let first = ctx.args.first().copied().unwrap_or("");
        match first {
            "" => show_types(&ctx),
            "bets" | "my" => show_bets(&ctx).await?,
            _ => place_bet(&ctx).await?,
        }
        Ok(())
    })
}

fn show_types(ctx: &CommandContext<'_>) {
    let p = &ctx.runtime.prefix;
    let kinds: Vec<String> = BET_KINDS.iter()
        .map(|k| format!("{} {:.1}x", k.slug, k.multiplier))
        .collect();
    ctx.whisper(format!(
        "Space Weather bets: {} | Settles midnight UTC | {p}spaceweather <type> <chips>",
        kinds.join(", ")
    ));
}

// ── show_bets ─────────────────────────────────────────────────────────────────

async fn show_bets(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
        ctx.whisper("Could not resolve your UUID.");
        return Ok(());
    };
    let all_bets = ctx.state.api.casino_nasa_space_weather_bet_list().await;
    let player_bets: Vec<_> = all_bets.into_iter().filter(|b| b.player == player_uuid).collect();
    if player_bets.is_empty() {
        ctx.whisper("No open space weather bets.");
        return Ok(());
    }
    for bet in &player_bets {
        let payout = (bet.stake as f64 * bet.multiplier) as i64;
        let label = find_kind(&bet.bet_type).map_or(bet.bet_type.as_str(), |k| k.label);
        ctx.whisper(format!(
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
        ctx.whisper("Space weather bets are not configured on this server.");
        return Ok(());
    }
    let (Some(&type_s), Some(&amt_s)) = (ctx.args.first(), ctx.args.get(1)) else {
        show_types(ctx);
        return Ok(());
    };
    let Some(kind) = find_kind(type_s) else {
        ctx.whisper(format!(
            "Unknown type. Valid: {}",
            BET_KINDS.iter().map(|k| k.slug).collect::<Vec<_>>().join(", ")
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
    let settle_at = next_midnight_utc();
    let mut bet = NasaSpaceWeatherBet {
        id: 0,
        player: player_uuid.clone(),
        bet_type: kind.slug.to_owned(),
        stake,
        multiplier: kind.multiplier,
        settle_at,
    };
    match ctx.state.api.casino_nasa_space_weather_bet_insert(&bet).await {
        Some(id) => { bet.id = id; }
        None => {
            ctx.whisper("Failed to save bet. Refunding chips.");
            let _ = ctx.state.api.casino_adjust(&player_uuid, stake).await;
            return Ok(());
        }
    }
    {
        let mut bets = ctx.state.nasa_space_weather_bets.lock().expect("nasa_space_weather_bets lock");
        bets.entry(player_uuid.clone()).or_default().push(bet.clone());
    }
    let payout = (stake as f64 * kind.multiplier) as i64;
    ctx.whisper(format!(
        "[SpaceWX] {} | {} | {:.1}x | profit if YES: +{} | settles {}",
        kind.label,
        chips_str(stake),
        kind.multiplier,
        chips_str(payout - stake),
        fmt_close(settle_at),
    ));
    let wcmd = ctx.runtime.whisper_command.clone();
    let nasa_key = ctx.runtime.nasa_api_key.clone();
    let secs = settle_at.saturating_sub(now_unix());
    tokio::spawn(settle_task(ctx.state.clone(), wcmd, nasa_key, bet, secs));
    Ok(())
}

// ── settle_task ───────────────────────────────────────────────────────────────

pub async fn settle_task(
    state: AzaleaState,
    whisper_cmd: String,
    nasa_api_key: String,
    bet: NasaSpaceWeatherBet,
    secs_to_close: u64,
) {
    if secs_to_close > 0 {
        tokio::time::sleep(std::time::Duration::from_secs(secs_to_close)).await;
    }
    // Buffer: give NASA 1h after midnight to log events.
    // If the bet's close time is already in the past, credit elapsed time against the buffer
    // so stale bets don't spin forever through bot restarts.
    let elapsed_past_close = now_unix().saturating_sub(bet.settle_at);
    let remaining_buffer = SETTLE_BUFFER_SECS.saturating_sub(elapsed_past_close);
    if remaining_buffer > 0 {
        tokio::time::sleep(std::time::Duration::from_secs(remaining_buffer)).await;
    }

    let claimed = {
        let mut bets = state.nasa_space_weather_bets.lock().expect("nasa_space_weather_bets lock");
        bets.get_mut(&bet.player)
            .map(|v| {
                let pos = v.iter().position(|b| b.id == bet.id);
                pos.map(|i| { v.remove(i); }).is_some()
            })
            .unwrap_or(false)
    };
    if !claimed { return; }

    let client = reqwest::Client::new();
    let online_username = state.players.read().ok()
        .and_then(|pl| pl.values().find(|s| s.uuid == bet.player).map(|s| s.username.clone()));

    // Settle date = the day before midnight (i.e. the day the bet was placed on)
    let settle_date = unix_to_ymd(bet.settle_at - 86400);

    let deadline = now_unix() + MAX_POLL_SECS;
    let result: Option<bool> = loop {
        match poll_event_occurred(&client, &bet.bet_type, &settle_date, &nasa_api_key).await {
            Some(r) => break Some(r),
            None => {
                if now_unix() >= deadline { break None; }
                tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
            }
        }
    };

    state.api.casino_nasa_space_weather_bet_delete(bet.id).await;

    let label = find_kind(&bet.bet_type).map_or(bet.bet_type.as_str(), |k| k.label);

    let msg = match result {
        Some(true) => {
            let payout = (bet.stake as f64 * bet.multiplier) as i64;
            let _ = state.api.casino_adjust(&bet.player, payout).await;
            format!(
                "[SpaceWX] {} — YES. WIN +{} ({} @ {:.1}x).",
                label,
                chips_str(payout - bet.stake),
                chips_str(bet.stake),
                bet.multiplier,
            )
        }
        Some(false) => {
            let _ = state.api.casino_jackpot_rake(bet.stake).await;
            format!(
                "[SpaceWX] {} — NO. LOSS -{} (to jackpot).",
                label,
                chips_str(bet.stake),
            )
        }
        None => {
            let _ = state.api.casino_adjust(&bet.player, bet.stake).await;
            format!(
                "[SpaceWX] {} — NASA API unavailable. {} refunded.",
                label,
                chips_str(bet.stake),
            )
        }
    };

    if let Some(ref username) = online_username {
        enqueue_chat(&state, format!("/{whisper_cmd} {username} {msg}"));
    } else {
        state.api.casino_add_notification(&bet.player, &msg).await;
    }
}
