use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::market::types::now_unix;
use crate::structure::mineflayer::bot::AzaleaState;

use super::{MIN_BET, chips_str, to_price, fmt_odds, fmt_time, calc_payout, sleep_until, deliver, FetchErr, check_resp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchBetSide {
    Success,
    OnTime,
}

impl LaunchBetSide {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "success" => Some(Self::Success),
            "ontime"  => Some(Self::OnTime),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self { Self::Success => "success", Self::OnTime => "ontime" }
    }
    pub fn display(self) -> &'static str {
        match self { Self::Success => "SUCCESS", Self::OnTime => "ONTIME" }
    }
}

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["rocket", "launch"],
    description: "Bet on rocket launches. !rocket | !rocket <id> | !rocket <id> success|ontime <chips>",
    whitelisted: false,
    execute,
};

const LOCK_BEFORE_SECS: u64 = 2 * 3600; // lock bets 2h before window_start
const POLL_INTERVAL_SECS: u64 = 3600;    // poll every hour at settlement
const MAX_SETTLE_WAIT_SECS: u64 = 7 * 24 * 3600; // give up after 7 days
const LL2_BASE: &str = "https://ll.thespacedevs.com/2.2.0";
const TIMEOUT_SECS: u64 = 15;

// Status IDs from LL2
const STATUS_SUCCESS: u64 = 3;
const STATUS_FAILURE: u64 = 4;
const STATUS_PARTIAL_FAILURE: u64 = 7;

// ── Structs ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LaunchBet {
    pub id:            Option<i64>,
    pub player:        String,
    pub launch_id:     String,   // LL2 full UUID
    pub launch_name:   String,
    pub lsp_id:        u32,
    pub lsp_name:      String,
    pub side:          LaunchBetSide,
    pub price:         f64,
    pub stake:         i64,
    pub window_start:  u64,      // locked at bet time
    pub close_time:    u64,      // window_end
}

impl super::CasinoBet for LaunchBet {
    const TYPE: &'static str = "launch";

    fn to_insert_json(&self) -> serde_json::Value {
        serde_json::json!({
            "player_uuid":  self.player,
            "launch_id":    self.launch_id,
            "launch_name":  self.launch_name,
            "lsp_id":       self.lsp_id,
            "lsp_name":     self.lsp_name,
            "side":         self.side.as_str(),
            "price":        self.price,
            "stake":        self.stake,
            "window_start": self.window_start,
            "close_time":   self.close_time,
        })
    }

    fn from_json(item: &serde_json::Value) -> Option<Self> {
        Some(Self {
            id:            Some(item.get("id")?.as_i64()?),
            player:        item.get("player_uuid")?.as_str()?.to_owned(),
            launch_id:     item.get("launch_id")?.as_str()?.to_owned(),
            launch_name:   item.get("launch_name")?.as_str()?.to_owned(),
            lsp_id:        item.get("lsp_id")?.as_u64()? as u32,
            lsp_name:      item.get("lsp_name")?.as_str()?.to_owned(),
            side:          LaunchBetSide::from_str(item.get("side")?.as_str()?)?,
            price:         item.get("price")?.as_f64()?,
            stake:         item.get("stake")?.as_i64()?,
            window_start:  item.get("window_start")?.as_u64()?,
            close_time:    item.get("close_time")?.as_u64()?,
        })
    }
}

#[derive(Debug, Clone)]
struct LaunchInfo {
    id:           String,
    name:         String,
    status_id:    u64,
    status_name:  String,
    lsp_id:       u32,
    lsp_name:     String,
    window_start: u64,
    window_end:   u64,
    net:          Option<u64>,
}

const CACHE_TTL: u64 = 3600; // 1h

// ── LL2 helpers ───────────────────────────────────────────────────────────────

async fn fetch_upcoming(client: &reqwest::Client) -> Result<Vec<LaunchInfo>, FetchErr> {
    let url = format!("{LL2_BASE}/launch/upcoming/?limit=5&status__in=1,8&format=json");
    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
        .send().await.map_err(|_| FetchErr::Error)?;
    let resp = check_resp(resp).await?;
    let v: serde_json::Value = resp.json().await.map_err(|_| FetchErr::Error)?;
    parse_launches(v.get("results").ok_or(FetchErr::Error)?).ok_or(FetchErr::Error)
}

async fn fetch_single(client: &reqwest::Client, short_id: &str) -> Result<LaunchInfo, FetchErr> {
    let url = format!("{LL2_BASE}/launch/upcoming/?limit=20&status__in=1,8&format=json");
    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
        .send().await.map_err(|_| FetchErr::Error)?;
    let resp = check_resp(resp).await?;
    let v: serde_json::Value = resp.json().await.map_err(|_| FetchErr::Error)?;
    let all = parse_launches(v.get("results").ok_or(FetchErr::Error)?).ok_or(FetchErr::Error)?;
    all.into_iter().find(|l| l.id.starts_with(short_id)).ok_or(FetchErr::Error)
}

async fn fetch_single_by_full_id(client: &reqwest::Client, full_id: &str) -> Option<LaunchInfo> {
    let url = format!("{LL2_BASE}/launch/{full_id}/?format=json");
    let v: serde_json::Value = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
        .send().await.ok()?
        .json().await.ok()?;
    parse_one_launch(&v)
}

fn calc_provider_probs(results: &[serde_json::Value]) -> (f64, f64) {
    let total = results.len();
    if total == 0 { return (0.80, 0.70); }
    let successes = results.iter().filter(|r| {
        r["status"]["id"].as_u64() == Some(STATUS_SUCCESS)
    }).count();
    let on_time = results.iter().filter(|r| {
        let ws  = r.get("window_start").and_then(|s| s.as_str()).and_then(parse_iso);
        let net = r.get("net").and_then(|s| s.as_str()).and_then(parse_iso);
        matches!((ws, net), (Some(a), Some(b)) if b.abs_diff(a) <= 86400)
    }).count();
    let p_success = (successes as f64 / total as f64).max(0.70).min(0.98);
    let p_ontime  = (on_time  as f64 / total as f64).max(0.50).min(0.98);
    (p_success, p_ontime)
}

async fn fetch_provider_history(client: &reqwest::Client, lsp_id: u32) -> Option<(f64, f64)> {
    let url = format!("{LL2_BASE}/launch/previous/?limit=50&lsp__id={lsp_id}&format=json");
    let v: serde_json::Value = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
        .send().await.ok()?
        .json().await.ok()?;
    let results = v.get("results")?.as_array()?;
    Some(calc_provider_probs(results))
}

fn parse_launches(v: &serde_json::Value) -> Option<Vec<LaunchInfo>> {
    let arr = v.as_array()?;
    Some(arr.iter().filter_map(parse_one_launch).collect())
}

fn parse_one_launch(r: &serde_json::Value) -> Option<LaunchInfo> {
    let id          = r["id"].as_str()?.to_owned();
    let name        = r["name"].as_str()?.to_owned();
    let status_id   = r["status"]["id"].as_u64()?;
    let status_name = r["status"]["name"].as_str()?.to_owned();
    let lsp         = &r["launch_service_provider"];
    let lsp_id      = lsp["id"].as_u64()? as u32;
    let lsp_name    = lsp["name"].as_str()?.to_owned();
    let ws          = r["window_start"].as_str().and_then(parse_iso)?;
    let we          = r["window_end"].as_str().and_then(parse_iso).unwrap_or(ws + 3600);
    let net         = r["net"].as_str().and_then(parse_iso);
    Some(LaunchInfo { id, name, status_id, status_name, lsp_id, lsp_name, window_start: ws, window_end: we, net })
}

fn parse_iso(s: &str) -> Option<u64> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.timestamp() as u64)
}

// ── Rates cache ───────────────────────────────────────────────────────────────

async fn get_rates(
    client: &reqwest::Client,
    cache: &Arc<Mutex<HashMap<u32, (f64, f64, u64)>>>,
    lsp_id: u32,
) -> (f64, f64) {
    {
        let c = cache.lock().unwrap();
        if let Some(&(ps, po, fetched)) = c.get(&lsp_id) {
            if now_unix() - fetched < CACHE_TTL {
                return (ps, po);
            }
        }
    }
    let rates = fetch_provider_history(client, lsp_id).await.unwrap_or((0.80, 0.70));
    cache.lock().unwrap().insert(lsp_id, (rates.0, rates.1, now_unix()));
    rates
}

// ── Command ───────────────────────────────────────────────────────────────────

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let arg0 = ctx.args.first().copied().unwrap_or("").to_ascii_lowercase();
        let arg1 = ctx.args.get(1).copied().unwrap_or("").to_ascii_lowercase();
        let arg2 = ctx.args.get(2).copied().unwrap_or("");

        match arg0.as_str() {
            "" | "list" => show_list(ctx).await?,
            "bets" | "my" => show_bets(ctx).await?,
            _ => {
                match LaunchBetSide::from_str(&arg1) {
                    Some(side) => place_bet(ctx, &arg0, side, arg2).await?,
                    None       => show_launch(ctx, &arg0).await?,
                }
            }
        }
        Ok(())
    })
}

async fn show_list(ctx: CommandContext<'_>) -> anyhow::Result<()> {
    let launches = match fetch_upcoming(&ctx.state.http).await {
        Ok(l) if !l.is_empty() => l,
        Err(FetchErr::RateLimit) => {
            ctx.whisper_success("Launch Library API rate limit reached. Try again later.");
            return Ok(());
        }
        _ => {
            ctx.whisper_success("No upcoming Go/TBC launches found.");
            return Ok(());
        }
    };
    let p = &ctx.runtime.prefix;
    ctx.whisper_success(format!("Upcoming launches (use {p}rocket <id> for odds):"));
    for l in launches.iter().take(5) {
        let short = &l.id[..8];
        let in_ = fmt_time(l.window_start);
        ctx.whisper_success(format!("[{}] {} — {} | T-{}", short, &l.name[..l.name.len().min(35)], l.lsp_name, in_));
    }
    Ok(())
}

async fn show_bets(ctx: CommandContext<'_>) -> anyhow::Result<()> {
    let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
        ctx.whisper_success("Could not resolve your UUID.");
        return Ok(());
    };
    let bets = {
        let map = ctx.state.launch_bets.lock().unwrap();
        map.get(&player_uuid).cloned().unwrap_or_default()
    };
    if bets.is_empty() {
        ctx.whisper_success("No open launch bets.");
        return Ok(());
    }
    for b in &bets {
        ctx.whisper_success(format!(
            "[ROCKET] {} {} {} — {:.2}× | closes in {}",
            &b.launch_name[..b.launch_name.len().min(25)],
            b.side.display(),
            chips_str(b.stake),
            1.0 / b.price,
            fmt_time(b.close_time),
        ));
    }
    Ok(())
}

async fn show_launch(ctx: CommandContext<'_>, short_id: &str) -> anyhow::Result<()> {
    let l = match fetch_single(&ctx.state.http, short_id).await {
        Ok(l) => l,
        Err(FetchErr::RateLimit) => {
            ctx.whisper_success("Launch Library API rate limit reached. Try again later.");
            return Ok(());
        }
        Err(_) => {
            ctx.whisper_error(format!("Launch '{short_id}' not found. Use !rocket to list upcoming."));
            return Ok(());
        }
    };

    let now = now_unix();
    let lock_at = l.window_start.saturating_sub(LOCK_BEFORE_SECS);
    if now >= lock_at {
        ctx.whisper_success(format!(
            "[{}] {} — bets locked (T-2h window passed).",
            &l.id[..8], l.name
        ));
        return Ok(());
    }

    let (p_s, p_o) = get_rates(&ctx.state.http, &ctx.state.launch_cache, l.lsp_id).await;
    let price_success = to_price(p_s);
    let price_ontime  = to_price(p_o);
    let p = &ctx.runtime.prefix;

    ctx.whisper_success(format!(
        "[{}] {} | {} | T-{} | Success: {} | On-time: {} | {p}rocket {} success|ontime <chips> | Min: {}",
        &l.id[..8],
        &l.name[..l.name.len().min(30)],
        l.lsp_name,
        fmt_time(l.window_start),
        fmt_odds(price_success),
        fmt_odds(price_ontime),
        &l.id[..8],
        chips_str(MIN_BET),
    ));
    Ok(())
}

async fn place_bet(ctx: CommandContext<'_>, short_id: &str, side: LaunchBetSide, chips_str_arg: &str) -> anyhow::Result<()> {
    let chips = match chips_str_arg.parse::<i64>() {
        Ok(n) if n >= MIN_BET => n,
        Ok(_) => { ctx.whisper_success(format!("Min bet: {} chips.", MIN_BET)); return Ok(()); }
        Err(_) => { ctx.whisper_success(format!("Usage: !rocket <id> success|ontime <chips>")); return Ok(()); }
    };

    let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
        ctx.whisper_success("Could not resolve your UUID.");
        return Ok(());
    };

    let l = match fetch_single(&ctx.state.http, short_id).await {
        Ok(l) => l,
        Err(FetchErr::RateLimit) => {
            ctx.whisper_success("Launch Library API rate limit reached. Try again later.");
            return Ok(());
        }
        Err(_) => {
            ctx.whisper_error(format!("Launch '{short_id}' not found."));
            return Ok(());
        }
    };

    let now = now_unix();
    let lock_at = l.window_start.saturating_sub(LOCK_BEFORE_SECS);
    if now >= lock_at {
        ctx.whisper_success("Bets locked (within 2h of launch window).");
        return Ok(());
    }

    let (p_s, p_o) = get_rates(&ctx.state.http, &ctx.state.launch_cache, l.lsp_id).await;
    let price = match side {
        LaunchBetSide::Success => to_price(p_s),
        LaunchBetSide::OnTime  => to_price(p_o),
    };

    match ctx.state.api.casino_adjust(&player_uuid, -chips).await {
        Err(CasinoAdjustErr::InsufficientFunds(have)) => {
            ctx.whisper_success(format!("Not enough chips (have {}).", chips_str(have)));
            return Ok(());
        }
        Err(e) => { ctx.whisper_success(format!("Error: {e:?}")); return Ok(()); }
        Ok(_) => {}
    }

    let mut bet = LaunchBet {
        id: None,
        player:       player_uuid.clone(),
        launch_id:    l.id.clone(),
        launch_name:  l.name.clone(),
        lsp_id:       l.lsp_id,
        lsp_name:     l.lsp_name.clone(),
        side,
        price,
        stake:        chips,
        window_start: l.window_start,
        close_time:   l.window_end,
    };

    match ctx.state.api.casino_bet_insert(&bet).await {
        Some(i) => bet.id = Some(i),
        None => {
            if let Err(e) = ctx.state.api.casino_adjust(&player_uuid, chips).await {
                eprintln!("[Launch] refund failed for {player_uuid}: {e:?}");
                ctx.whisper_success("Failed to record bet. Refund also failed — contact an admin.");
            } else {
                ctx.whisper_success("Failed to record bet. Chips refunded.");
            }
            return Ok(());
        }
    }

    let payout = calc_payout(chips, price);
    ctx.whisper_success(format!(
        "[ROCKET] {} {} {} — pays {} if {} | T-{} | bets lock T-2h",
        &l.name[..l.name.len().min(25)],
        side.display(),
        chips_str(chips),
        chips_str(payout),
        side.as_str(),
        fmt_time(l.window_start),
    ));

    ctx.state.launch_bets.lock().unwrap()
        .entry(player_uuid)
        .or_default()
        .push(bet.clone());

    let state      = ctx.state.clone();
    let whisper_cmd = ctx.runtime.whisper_command.clone();
    tokio::spawn(launch_settle_task(state, whisper_cmd, bet));

    Ok(())
}

// ── Settlement ────────────────────────────────────────────────────────────────

fn determine_launch_win(side: LaunchBetSide, status_id: u64, net: Option<u64>, window_start: u64) -> bool {
    match side {
        LaunchBetSide::Success => status_id == STATUS_SUCCESS,
        LaunchBetSide::OnTime  => status_id == STATUS_SUCCESS
            && net.map_or(false, |n| n.abs_diff(window_start) <= 86400),
    }
}

pub async fn launch_settle_task(state: AzaleaState, whisper_cmd: String, bet: LaunchBet) {
    sleep_until(bet.close_time).await;

    let give_up = now_unix() + MAX_SETTLE_WAIT_SECS;

    loop {
        if now_unix() > give_up { break; }

        let info = fetch_single_by_full_id(&state.http, &bet.launch_id).await;
        if let Some(ref l) = info {
            let finished = matches!(l.status_id, STATUS_SUCCESS | STATUS_FAILURE | STATUS_PARTIAL_FAILURE);
            if finished {
                settle(&state, &whisper_cmd, &bet, l).await;
                return;
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
    }

    // Give-up path: refund
    remove_bet(&state, &bet);
    state.api.casino_bet_delete::<LaunchBet>(bet.id.unwrap()).await;
    let msg = if let Err(e) = state.api.casino_adjust(&bet.player, bet.stake).await {
        eprintln!("[Launch settle] refund failed for {}: {e:?}", bet.player);
        format!(
            "[ROCKET] {} {} — no final status after 7 days. Refund failed — contact an admin.",
            bet.launch_name, bet.side.display()
        )
    } else {
        format!(
            "[ROCKET] {} {} — no final status after 7 days. {} refunded.",
            bet.launch_name, bet.side.display(), chips_str(bet.stake)
        )
    };
    deliver(&state, &whisper_cmd, &bet.player, msg).await;
}

async fn settle(state: &AzaleaState, whisper_cmd: &str, bet: &LaunchBet, l: &LaunchInfo) {
    remove_bet(state, bet);
    state.api.casino_bet_delete::<LaunchBet>(bet.id.unwrap()).await;

    let won = determine_launch_win(bet.side, l.status_id, l.net, bet.window_start);

    let msg = if won {
        let payout = calc_payout(bet.stake, bet.price);
        if let Err(e) = state.api.casino_adjust(&bet.player, payout).await {
            eprintln!("[Launch settle] payout failed for {}: {e:?}", bet.player);
            format!(
                "[ROCKET] {} {} — {}. Win detected but payout failed — contact an admin.",
                bet.launch_name, bet.side.display(), l.status_name
            )
        } else {
            format!(
                "[ROCKET] {} {} — {}. WIN +{} ({} @ {:.2}×).",
                bet.launch_name, bet.side.display(), l.status_name,
                chips_str(payout - bet.stake),
                chips_str(bet.stake),
                1.0 / bet.price,
            )
        }
    } else {
        let _ = state.api.casino_jackpot_rake(bet.stake).await;
        format!(
            "[ROCKET] {} {} — {}. LOSS -{} (to jackpot).",
            bet.launch_name, bet.side.display(), l.status_name,
            chips_str(bet.stake),
        )
    };

    deliver(state, whisper_cmd, &bet.player, msg).await;
}

fn remove_bet(state: &AzaleaState, bet: &LaunchBet) {
    let mut bets = state.launch_bets.lock().unwrap();
    if let Some(v) = bets.get_mut(&bet.player) {
        v.retain(|b| b.id != bet.id);
    }
}

