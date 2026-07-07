use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::market::types::now_unix;
use crate::structure::mineflayer::bot::AzaleaState;

use super::{chips_str, sleep_until, deliver};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["sports", "sb"],
    description: "Sports betting. !sports — categories | !sports <sport> — events | !sports bet <#> home|away|draw <chips> | !sports bets",
    whitelisted: false,
    execute,
};

const SHARPAPI_BASE: &str = "https://api.sharpapi.io/api/v1";
const CACHE_TTL: u64 = 600; // 10 min
const MIN_BET: i64 = 50;
const POLL_INTERVAL_SECS: u64 = 300; // poll every 5 min for result
const MAX_POLL_SECS: u64 = 18_000;   // 5 hours max

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SportsBet {
    pub id: i64,
    pub player: String,
    pub event_id: String,
    pub sport: String,
    pub home_team: String,
    pub away_team: String,
    pub selection: String, // "home" | "away" | "draw"
    pub payout_mult: f64,
    pub stake: i64,
    pub start_unix: u64,
}

#[derive(Debug, Clone)]
pub struct EventDisplay {
    pub event_id: String,
    pub sport: String,
    pub home_team: String,
    pub away_team: String,
    pub start_unix: u64,
    pub home_odds: f64,
    pub away_odds: f64,
    pub draw_odds: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct SportsCache {
    pub fetched_at: u64,
    pub events: Vec<EventDisplay>,
}

// ── SharpAPI helpers ──────────────────────────────────────────────────────────

async fn sharp_get(client: &reqwest::Client, key: &str, path: &str) -> Option<serde_json::Value> {
    client
        .get(format!("{SHARPAPI_BASE}{path}"))
        .header("X-API-Key", key)
        .header("Accept", "application/json")
        .send().await.ok()?
        .json::<serde_json::Value>().await.ok()
}

fn strip_book_suffix(id: &str) -> &str {
    if let Some(pos) = id.rfind("_b") {
        if id[pos + 2..].chars().all(|c| c.is_ascii_digit()) {
            return &id[..pos];
        }
    }
    id
}

fn date_from_base_id(base: &str) -> u64 {
    let len = base.len();
    if len < 10 { return 0; }
    let date_str = &base[len - 10..];
    if date_str.as_bytes().get(4) == Some(&b'-') && date_str.as_bytes().get(7) == Some(&b'-') {
        parse_iso_unix(&format!("{date_str}T12:00:00Z")).unwrap_or(0)
    } else {
        0
    }
}

async fn build_event_cache(client: &reqwest::Client, key: &str) -> Vec<EventDisplay> {
    let Some(odds_val) = sharp_get(client, key, "/odds/best?market_type=moneyline&limit=200").await else {
        return vec![];
    };
    let Some(odds_arr) = odds_val["data"].as_array() else {
        return vec![];
    };

    let mut event_map: std::collections::HashMap<String, EventDisplay> = std::collections::HashMap::new();

    for item in odds_arr {
        let eid_raw = item["event_id"].as_str().unwrap_or("");
        let base = strip_book_suffix(eid_raw).to_string();
        if base.is_empty() { continue; }

        let sel = item["selection"].as_str().unwrap_or("").to_string();
        let od = item["best_odds"]["decimal"].as_f64().unwrap_or(0.0);
        if od < 1.01 { continue; }

        let ev = event_map.entry(base.clone()).or_insert_with(|| {
            let name = item["event_name"].as_str().unwrap_or("");
            let sport = item["sport"].as_str().unwrap_or("Sports").to_string();
            // event_name format: "Away Team @ Home Team"
            let (home, away) = if let Some(pos) = name.find(" @ ") {
                (name[pos + 3..].to_string(), name[..pos].to_string())
            } else {
                (String::new(), String::new())
            };
            EventDisplay {
                event_id: eid_raw.to_string(),
                sport,
                home_team: home,
                away_team: away,
                start_unix: date_from_base_id(&base),
                home_odds: 0.0,
                away_odds: 0.0,
                draw_odds: None,
            }
        });

        if ev.home_team.is_empty() { continue; }

        let sel_lower = sel.to_lowercase();
        if matches!(sel_lower.as_str(), "draw" | "tie") {
            ev.draw_odds = Some(ev.draw_odds.unwrap_or(0.0).max(od));
        } else if sel_lower == ev.home_team.to_lowercase() {
            ev.home_odds = ev.home_odds.max(od);
        } else if sel_lower == ev.away_team.to_lowercase() {
            ev.away_odds = ev.away_odds.max(od);
        }
    }

    let mut out: Vec<EventDisplay> = event_map.into_values()
        .filter(|ev| !ev.home_team.is_empty() && !ev.away_team.is_empty()
               && ev.home_odds >= 1.01 && ev.away_odds >= 1.01)
        .collect();
    out.sort_by_key(|ev| ev.start_unix);
    out.truncate(15);
    out
}

enum EventStatus {
    InProgress,
    Completed(String),
    Cancelled,
}

enum EventFetch {
    Found(serde_json::Value),
    NotFound, // 404 — event gone from API, refund immediately
    Error,    // network/parse failure — retry
}

async fn fetch_event(client: &reqwest::Client, key: &str, event_id: &str) -> EventFetch {
    let Ok(resp) = client
        .get(format!("{SHARPAPI_BASE}/events/{event_id}"))
        .header("X-API-Key", key)
        .header("Accept", "application/json")
        .send()
        .await
    else {
        return EventFetch::Error;
    };
    if resp.status() == 404 {
        return EventFetch::NotFound;
    }
    match resp.json::<serde_json::Value>().await {
        Ok(v) => EventFetch::Found(v),
        Err(_) => EventFetch::Error,
    }
}

async fn poll_event_result(client: &reqwest::Client, key: &str, event_id: &str) -> EventStatus {
    let v = match fetch_event(client, key, event_id).await {
        EventFetch::Found(v) => v,
        EventFetch::NotFound => return EventStatus::Cancelled,
        EventFetch::Error    => return EventStatus::InProgress,
    };
    let data = if v["data"].is_object() { &v["data"] } else { &v };
    let status = data["status"].as_str().unwrap_or("").to_lowercase();
    match status.as_str() {
        "completed" | "settled" | "closed" | "finished" | "ended" => {
            let result = &data["result"];
            let winner = result["winner"].as_str()
                .map(|s| s.to_lowercase())
                .or_else(|| {
                    let hs = data["home_score"].as_f64().or_else(|| result["home_score"].as_f64())?;
                    let as_ = data["away_score"].as_f64().or_else(|| result["away_score"].as_f64())?;
                    Some(if hs > as_ { "home".into() } else if as_ > hs { "away".into() } else { "draw".into() })
                });
            match winner {
                Some(w) => EventStatus::Completed(w),
                None => EventStatus::InProgress,
            }
        }
        "cancelled" | "postponed" | "abandoned" => EventStatus::Cancelled,
        _ => EventStatus::InProgress,
    }
}

fn parse_iso_unix(s: &str) -> Option<u64> {
    use chrono::DateTime;
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.timestamp() as u64)
        .or_else(|| DateTime::parse_from_rfc3339(&format!("{s}Z")).ok().map(|dt| dt.timestamp() as u64))
        .or_else(|| DateTime::parse_from_rfc3339(&s.replace(' ', "T")).ok().map(|dt| dt.timestamp() as u64))
}

// ── Command entry ─────────────────────────────────────────────────────────────

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        match ctx.args.first().copied().unwrap_or("") {
            "bet" | "b" => place_bet(&ctx).await?,
            "bets" | "my" => show_bets(&ctx).await?,
            _ => show_events(&ctx).await?,
        }
        Ok(())
    })
}

// ── show_events ───────────────────────────────────────────────────────────────

async fn load_events(ctx: &CommandContext<'_>) -> Option<Vec<EventDisplay>> {
    let key = ctx.runtime.sharpapi_key.clone();
    if key.is_empty() {
        ctx.whisper("Sports betting is not configured (missing sharpapi_key).");
        return None;
    }
    let cached = {
        let cache = ctx.state.sports_cache.lock().expect("sports_cache lock");
        let age = now_unix().saturating_sub(cache.fetched_at);
        (age < CACHE_TTL && !cache.events.is_empty()).then(|| cache.events.clone())
    };
    if let Some(c) = cached {
        return Some(c);
    }
    let client = reqwest::Client::new();
    let evs = build_event_cache(&client, &key).await;
    let mut cache = ctx.state.sports_cache.lock().expect("sports_cache lock");
    cache.fetched_at = now_unix();
    cache.events = evs.clone();
    Some(evs)
}

async fn show_events(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let Some(events) = load_events(ctx).await else { return Ok(()); };

    if events.is_empty() {
        ctx.whisper("No upcoming events found. Try again in a few minutes.");
        return Ok(());
    }

    let sport_arg = ctx.args.first().copied().unwrap_or("").to_lowercase();

    if sport_arg.is_empty() {
        let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for ev in &events {
            *counts.entry(ev.sport.to_lowercase()).or_default() += 1;
        }
        let list = counts.iter()
            .map(|(s, n)| format!("{s} ({n})"))
            .collect::<Vec<_>>()
            .join(", ");
        ctx.whisper(format!("Sports: {list} | {}sports <sport>", ctx.runtime.prefix));
    } else {
        let matching: Vec<(usize, &EventDisplay)> = events.iter().enumerate()
            .filter(|(_, ev)| ev.sport.to_lowercase() == sport_arg)
            .collect();

        if matching.is_empty() {
            let known: std::collections::BTreeSet<String> =
                events.iter().map(|ev| ev.sport.to_lowercase()).collect();
            ctx.whisper(format!(
                "No '{sport_arg}' events. Available: {}",
                known.into_iter().collect::<Vec<_>>().join(", ")
            ));
            return Ok(());
        }

        let now = now_unix() as i64;
        let lines = matching.iter().take(5).map(|(i, ev)| {
            let draw = ev.draw_odds.map(|d| format!(" d{d:.2}")).unwrap_or_default();
            let date_label = if ev.start_unix > 0 {
                use chrono::{DateTime, Utc, Datelike};
                let dt = DateTime::<Utc>::from_timestamp(ev.start_unix as i64, 0).unwrap_or_default();
                let today = DateTime::<Utc>::from_timestamp(now, 0).unwrap_or_default().date_naive();
                if dt.date_naive() == today {
                    " [today]".to_string()
                } else {
                    format!(" [{} {}]", dt.format("%b"), dt.day())
                }
            } else {
                String::new()
            };
            format!("#{} {} v {} h{:.2}{draw} a{:.2}{date_label}", i + 1, ev.home_team, ev.away_team, ev.home_odds, ev.away_odds)
        }).collect::<Vec<_>>().join(" | ");

        ctx.whisper(lines);
        ctx.whisper(format!("Bet: {}sports bet <#> home|away|draw <chips> | Odds preview: omit <chips>", ctx.runtime.prefix));
    }
    Ok(())
}

// ── place_bet ─────────────────────────────────────────────────────────────────

async fn place_bet(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let tail = &ctx.args[1..]; // skip "bet"
    let (Some(&idx_s), Some(&sel_s)) = (tail.first(), tail.get(1)) else {
        ctx.whisper(format!("Usage: {}sports bet <#> home|away|draw <chips> | Omit chips for odds preview", ctx.runtime.prefix));
        return Ok(());
    };
    let amt_s = tail.get(2).copied();

    let Ok(idx) = idx_s.parse::<usize>().map(|n| n.saturating_sub(1)) else {
        ctx.whisper("Event number must be a positive integer.");
        return Ok(());
    };
    let sel = match sel_s.to_lowercase().as_str() {
        "h" | "home" => "home",
        "a" | "away" => "away",
        "d" | "draw" => "draw",
        _ => {
            ctx.whisper("Selection must be: home/h, away/a, or draw/d.");
            return Ok(());
        }
    }.to_string();
    let ev = {
        let cache = ctx.state.sports_cache.lock().expect("sports_cache lock");
        cache.events.get(idx).cloned()
    };
    let Some(ev) = ev else {
        ctx.whisper("Event not found. Run !sports to refresh the list.");
        return Ok(());
    };

    let payout_mult = match sel.as_str() {
        "home" => ev.home_odds,
        "away" => ev.away_odds,
        "draw" => match ev.draw_odds {
            Some(d) => d,
            None => { ctx.whisper("Draw is not available for this event."); return Ok(()); }
        },
        _ => unreachable!(),
    };

    // preview mode — no chip amount provided
    let Some(amt_s) = amt_s else {
        let draw_str = ev.draw_odds.map(|d| format!(" | draw {d:.2}x")).unwrap_or_default();
        ctx.whisper(format!(
            "[Sports] {} v {} | home {:.2}x | away {:.2}x{draw_str} | {} {:.2}x selected",
            ev.home_team, ev.away_team, ev.home_odds, ev.away_odds, sel, payout_mult
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
        Err(CasinoAdjustErr::NetworkErr) => { ctx.whisper("Casino unavailable."); return Ok(()); }
    }

    let mut bet = SportsBet {
        id: 0,
        player: player_uuid.clone(),
        event_id: ev.event_id.clone(),
        sport: ev.sport.clone(),
        home_team: ev.home_team.clone(),
        away_team: ev.away_team.clone(),
        selection: sel.clone(),
        payout_mult,
        stake,
        start_unix: ev.start_unix,
    };

    match ctx.state.api.casino_sports_bet_insert(&bet).await {
        Some(id) => { bet.id = id; }
        None => {
            ctx.whisper("Failed to save bet. Refunding chips.");
            let _ = ctx.state.api.casino_adjust(&player_uuid, stake).await;
            return Ok(());
        }
    }
    {
        let mut bets = ctx.state.sports_bets.lock().expect("sports_bets lock");
        bets.entry(player_uuid.clone()).or_default().push(bet.clone());
    }

    let profit = (stake as f64 * payout_mult).ceil() as i64 - stake;
    ctx.whisper(format!(
        "[Sports] {} vs {} | {} {:.2}x | {} | profit if win: +{}",
        ev.home_team, ev.away_team, sel, payout_mult, chips_str(stake), chips_str(profit)
    ));

    let key = ctx.runtime.sharpapi_key.clone();
    let wcmd = ctx.runtime.whisper_command.clone();
    tokio::spawn(settle_task(ctx.state.clone(), wcmd, key, bet));
    Ok(())
}

// ── show_bets ─────────────────────────────────────────────────────────────────

async fn show_bets(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
        ctx.whisper("Could not resolve your UUID.");
        return Ok(());
    };
    let all_bets = ctx.state.api.casino_sports_bet_list().await;
    let player_bets: Vec<_> = all_bets.into_iter().filter(|b| b.player == player_uuid).collect();
    if player_bets.is_empty() {
        ctx.whisper("No open sports bets.");
        return Ok(());
    }
    let now = now_unix();
    for bet in &player_bets {
        let payout = (bet.stake as f64 * bet.payout_mult).ceil() as i64;
        let t = bet.start_unix.saturating_sub(now);
        let when = if t == 0 { "in progress".into() } else if t < 3600 { format!("{}m", t / 60) } else { format!("{}h", t / 3600) };
        ctx.whisper(format!(
            "[Sports] {} vs {} | {} {:.2}x | {} -> {} | {}",
            bet.home_team, bet.away_team, bet.selection, bet.payout_mult,
            chips_str(bet.stake), chips_str(payout), when
        ));
    }
    Ok(())
}

// ── settle_task ───────────────────────────────────────────────────────────────

pub async fn settle_task(
    state: AzaleaState,
    whisper_cmd: String,
    api_key: String,
    bet: SportsBet,
) {
    sleep_until(bet.start_unix).await;

    let claimed = {
        let mut bets = state.sports_bets.lock().expect("sports_bets lock");
        bets.get_mut(&bet.player)
            .map(|v| { let pos = v.iter().position(|b| b.id == bet.id); pos.map(|i| { v.remove(i); }).is_some() })
            .unwrap_or(false)
    };
    if !claimed { return; }

    let client = reqwest::Client::new();

    let deadline = now_unix() + MAX_POLL_SECS;
    let outcome: Option<String> = loop {
        match poll_event_result(&client, &api_key, &bet.event_id).await {
            EventStatus::Completed(w) => break Some(w),
            EventStatus::Cancelled => break None,
            EventStatus::InProgress => {
                if now_unix() >= deadline { break None; }
                tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
            }
        }
    };

    state.api.casino_sports_bet_delete(bet.id).await;

    let msg = match outcome {
        Some(ref winner) => {
            if *winner == bet.selection {
                let payout = (bet.stake as f64 * bet.payout_mult).ceil() as i64;
                if let Err(e) = state.api.casino_adjust(&bet.player, payout).await {
                    eprintln!("[Sports settle] casino_adjust failed for {}: {e:?}", bet.player);
                }
                format!("[Sports] {} vs {} — {winner} wins. WIN +{} ({} @ {:.2}x).",
                    bet.home_team, bet.away_team,
                    chips_str(payout - bet.stake), chips_str(bet.stake), bet.payout_mult)
            } else {
                state.api.casino_jackpot_rake(bet.stake).await;
                format!("[Sports] {} vs {} — {winner} wins. LOSS -{} (to jackpot).",
                    bet.home_team, bet.away_team, chips_str(bet.stake))
            }
        }
        None => {
            if let Err(e) = state.api.casino_adjust(&bet.player, bet.stake).await {
                eprintln!("[Sports settle] refund failed for {}: {e:?}", bet.player);
            }
            format!("[Sports] {} vs {} — result unavailable. {} refunded.",
                bet.home_team, bet.away_team, chips_str(bet.stake))
        }
    };
    deliver(&state, &whisper_cmd, &bet.player, msg).await;
}
