use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::market::types::now_unix;
use crate::structure::mineflayer::bot::AzaleaState;

use super::{chips_str, fmt_close, calc_payout, sleep_until, deliver, FetchErr, check_resp};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["kalshi", "k"],
    description: "Kalshi prediction markets. !kalshi — categories | !kalshi <category> — markets | !kalshi <#> yes|no <chips> | !kalshi bets",
    whitelisted: false,
    execute,
};

const KALSHI_BASE: &str = "https://external-api.kalshi.com/trade-api/v2";
const CACHE_TTL: u64 = 600;
const MIN_BET: i64 = 25;
const POLL_INTERVAL_SECS: u64 = 60;
const MAX_POLL_SECS: u64 = 3600;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct KalshiMarket {
    pub ticker: String,
    pub title: String,
    pub yes_ask: f64,
    pub no_ask: f64,
    pub close_time: u64,
}

#[derive(Debug, Clone)]
pub struct KalshiBet {
    pub id: i64,
    pub player: String,
    pub ticker: String,
    pub title: String,
    pub side: String,
    pub price: f64,
    pub stake: i64,
    pub close_time: u64,
}

#[derive(Debug, Clone, Default)]
pub struct KalshiCache {
    pub fetched_at: u64,
    pub category: String,
    pub markets: Vec<KalshiMarket>,
}

// ── Category mapping ──────────────────────────────────────────────────────────

fn map_category(kw: &str) -> Option<&'static str> {
    Some(match kw {
        "sports" | "sport" => "Sports",
        "crypto" | "cryptocurrency" => "Crypto",
        "politics" | "political" => "Politics",
        "economics" | "econ" | "economy" => "Economics",
        "entertainment" | "ent" => "Entertainment",
        "tech" | "technology" => "Science and Technology",
        "climate" => "Climate and Weather",
        "finance" | "financials" | "fin" => "Financials",
        "elections" | "election" => "Elections",
        "health" => "Health",
        _ => return None,
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_iso_unix(s: &str) -> Option<u64> {
    use chrono::DateTime;
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.timestamp() as u64)
        .or_else(|| {
            DateTime::parse_from_rfc3339(&format!("{s}Z"))
                .ok()
                .map(|dt| dt.timestamp() as u64)
        })
}

async fn kalshi_get(
    client: &reqwest::Client,
    path: &str,
    params: &[(&str, &str)],
) -> Result<serde_json::Value, FetchErr> {
    let mut url = format!("{KALSHI_BASE}{path}");
    if !params.is_empty() {
        let qs = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v.replace(' ', "%20")))
            .collect::<Vec<_>>()
            .join("&");
        url.push('?');
        url.push_str(&qs);
    }
    let resp = client
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|_| FetchErr::Error)?;
    let resp = check_resp(resp).await?;
    resp.json::<serde_json::Value>().await.map_err(|_| FetchErr::Error)
}

fn parse_market(m: &serde_json::Value) -> Option<KalshiMarket> {
    let ticker = m["ticker"].as_str()?.to_owned();
    let title  = m["title"].as_str().unwrap_or(&ticker).to_owned();
    let yes_ask = m["yes_ask_dollars"].as_str()?.parse::<f64>().ok()?;
    let no_ask  = m["no_ask_dollars"].as_str()?.parse::<f64>().ok()?;
    let close_time = parse_iso_unix(m["close_time"].as_str().unwrap_or(""))?;
    if yes_ask < 0.01 || no_ask < 0.01 { return None; }
    Some(KalshiMarket { ticker, title, yes_ask, no_ask, close_time })
}

async fn fetch_markets(client: &reqwest::Client, category: &str) -> Result<Vec<KalshiMarket>, FetchErr> {
    let sv = kalshi_get(client, "/series", &[("category", category), ("limit", "5")]).await?;
    let tickers: Vec<String> = sv["series"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|s| s["ticker"].as_str().map(|t| t.to_owned()))
                .take(3)
                .collect()
        })
        .unwrap_or_default();

    let mut out: Vec<KalshiMarket> = vec![];
    for t in &tickers {
        let Ok(mv) = kalshi_get(client, "/markets", &[
            ("status", "open"),
            ("mve_filter", "exclude"),
            ("series_ticker", t.as_str()),
            ("limit", "3"),
        ]).await else { continue; };
        if let Some(arr) = mv["markets"].as_array() {
            out.extend(arr.iter().filter_map(parse_market));
        }
        if out.len() >= 5 { break; }
    }
    out.sort_by_key(|m| m.close_time);
    out.truncate(5);
    Ok(out)
}

async fn poll_market_result(client: &reqwest::Client, ticker: &str) -> Option<String> {
    let v = kalshi_get(client, &format!("/markets/{ticker}"), &[]).await.ok()?;
    let result = v["market"]["result"]
        .as_str()
        .or_else(|| v["result"].as_str())
        .unwrap_or("");
    if result == "yes" || result == "no" { Some(result.to_owned()) } else { None }
}

// ── Command entry ─────────────────────────────────────────────────────────────

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let first = ctx.args.first().copied().unwrap_or("");
        match first {
            "" => show_categories(&ctx).await?,
            "bets" | "my" => show_bets(&ctx).await?,
            s if s.chars().next().map_or(false, |c| c.is_ascii_digit()) => place_bet(&ctx).await?,
            _ => show_markets(&ctx).await?,
        }
        Ok(())
    })
}

// ── show_categories ───────────────────────────────────────────────────────────

async fn show_categories(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    ctx.whisper_success(format!(
        "Kalshi: sports, crypto, politics, economics, entertainment, tech, climate, finance, elections, health | {}kalshi <category>",
        ctx.runtime.prefix
    ));
    Ok(())
}

// ── show_markets ──────────────────────────────────────────────────────────────

async fn load_markets(ctx: &CommandContext<'_>, category: &str) -> Result<Vec<KalshiMarket>, FetchErr> {
    let cached = {
        let cache = ctx.state.kalshi_cache.lock().expect("kalshi_cache lock");
        let age = now_unix().saturating_sub(cache.fetched_at);
        if age < CACHE_TTL && cache.category == category && !cache.markets.is_empty() {
            Some(cache.markets.clone())
        } else {
            None
        }
    };
    if let Some(c) = cached { return Ok(c); }

    let client = reqwest::Client::new();
    let markets = fetch_markets(&client, category).await?;
    {
        let mut cache = ctx.state.kalshi_cache.lock().expect("kalshi_cache lock");
        cache.fetched_at = now_unix();
        cache.category   = category.to_owned();
        cache.markets    = markets.clone();
    }
    Ok(markets)
}

async fn show_markets(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let kw = ctx.args.first().copied().unwrap_or("").to_lowercase();
    let Some(category) = map_category(&kw) else {
        ctx.whisper_success("Unknown category. Try: sports, crypto, politics, economics, entertainment, tech, climate, finance, elections, health");
        return Ok(());
    };

    let markets = match load_markets(ctx, category).await {
        Ok(m) => m,
        Err(FetchErr::RateLimit) => {
            ctx.whisper_success("Kalshi API rate limit reached. Try again later.");
            return Ok(());
        }
        Err(_) => {
            ctx.whisper_success(format!("No open markets for {kw} right now."));
            return Ok(());
        }
    };
    if markets.is_empty() {
        ctx.whisper_success(format!("No open markets for {kw} right now."));
        return Ok(());
    }

    let lines: Vec<String> = markets.iter().enumerate().map(|(i, m)| {
        let title = if m.title.len() > 42 { format!("{}...", &m.title[..39]) } else { m.title.clone() };
        format!("#{} {} YES {:.2}x NO {:.2}x {}", i + 1, title, 1.0 / m.yes_ask, 1.0 / m.no_ask, fmt_close(m.close_time))
    }).collect();

    ctx.whisper_success(lines.join(" | "));
    ctx.whisper_success(format!("Bet: {}kalshi <#> yes|no <chips>", ctx.runtime.prefix));
    Ok(())
}

// ── place_bet ─────────────────────────────────────────────────────────────────

async fn place_bet(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let (Some(&idx_s), Some(&side_s), Some(&amt_s)) =
        (ctx.args.first(), ctx.args.get(1), ctx.args.get(2))
    else {
        ctx.whisper_success(format!("Usage: {}kalshi <#> yes|no <chips>", ctx.runtime.prefix));
        return Ok(());
    };

    let Ok(idx) = idx_s.parse::<usize>().map(|n| n.saturating_sub(1)) else {
        ctx.whisper_success("Market number must be a positive integer.");
        return Ok(());
    };
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

    let market = {
        let cache = ctx.state.kalshi_cache.lock().expect("kalshi_cache lock");
        cache.markets.get(idx).cloned()
    };
    let Some(market) = market else {
        ctx.whisper_success("Market not found. Run !kalshi <category> first.");
        return Ok(());
    };

    let now = now_unix();
    if market.close_time > 0 && now >= market.close_time {
        ctx.whisper_success("That market is already closed.");
        return Ok(());
    }

    let price = if side == "yes" { market.yes_ask } else { market.no_ask };
    if price < 0.01 {
        ctx.whisper_success("No liquidity on that side right now.");
        return Ok(());
    }

    let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
        ctx.whisper_success("Could not resolve your UUID.");
        return Ok(());
    };
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

    let mut bet = KalshiBet {
        id: 0,
        player: player_uuid.clone(),
        ticker: market.ticker.clone(),
        title: market.title.clone(),
        side: side.clone(),
        price,
        stake,
        close_time: market.close_time,
    };

    match ctx.state.api.casino_kalshi_bet_insert(&bet).await {
        Some(id) => { bet.id = id; }
        None => {
            if let Err(e) = ctx.state.api.casino_adjust(&player_uuid, stake).await {
                eprintln!("[Kalshi] refund failed for {player_uuid}: {e:?}");
                ctx.whisper_success("Failed to save bet. Refund also failed — contact an admin.");
            } else {
                ctx.whisper_success("Failed to save bet. Chips refunded.");
            }
            return Ok(());
        }
    }
    {
        let mut bets = ctx.state.kalshi_bets.lock().expect("kalshi_bets lock");
        bets.entry(player_uuid.clone()).or_default().push(bet.clone());
    }

    let payout = (stake as f64 / price).floor() as i64;
    let profit = payout - stake;
    ctx.whisper_success(format!(
        "[Kalshi] {} | {} {:.2}x | {} | profit if win: +{}",
        market.title,
        side.to_uppercase(),
        1.0 / price,
        chips_str(stake),
        chips_str(profit),
    ));

    let wcmd = ctx.runtime.whisper_command.clone();
    tokio::spawn(settle_task(ctx.state.clone(), wcmd, bet));
    Ok(())
}

// ── show_bets ─────────────────────────────────────────────────────────────────

async fn show_bets(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
        ctx.whisper_success("Could not resolve your UUID.");
        return Ok(());
    };
    let all_bets = ctx.state.api.casino_kalshi_bet_list().await;
    let player_bets: Vec<_> = all_bets.into_iter().filter(|b| b.player == player_uuid).collect();
    if player_bets.is_empty() {
        ctx.whisper_success("No open Kalshi bets.");
        return Ok(());
    }
    for bet in &player_bets {
        let payout = (bet.stake as f64 / bet.price).floor() as i64;
        ctx.whisper_success(format!(
            "[Kalshi] {} | {} {:.2}x | {} -> {} | {}",
            bet.title,
            bet.side.to_uppercase(),
            1.0 / bet.price,
            chips_str(bet.stake),
            chips_str(payout),
            fmt_close(bet.close_time),
        ));
    }
    Ok(())
}

// ── settle_task ───────────────────────────────────────────────────────────────

pub async fn settle_task(
    state: AzaleaState,
    whisper_cmd: String,
    bet: KalshiBet,
) {
    sleep_until(bet.close_time).await;

    let claimed = {
        let mut bets = state.kalshi_bets.lock().expect("kalshi_bets lock");
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
    let result: Option<String> = loop {
        match poll_market_result(&client, &bet.ticker).await {
            Some(r) => break Some(r),
            None => {
                if now_unix() >= deadline { break None; }
                tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
            }
        }
    };

    state.api.casino_kalshi_bet_delete(bet.id).await;

    let msg = match result {
        Some(ref winner) if *winner == bet.side => {
            let payout = calc_payout(bet.stake, bet.price);
            if let Err(e) = state.api.casino_adjust(&bet.player, payout).await {
                eprintln!("[Kalshi settle] casino_adjust failed for {}: {e:?}", bet.player);
            }
            format!(
                "[Kalshi] {} — {} wins. WIN +{} ({} @ {:.2}x).",
                bet.title,
                winner.to_uppercase(),
                chips_str(payout - bet.stake),
                chips_str(bet.stake),
                1.0 / bet.price,
            )
        }
        Some(ref winner) => {
            state.api.casino_jackpot_rake(bet.stake).await;
            format!(
                "[Kalshi] {} — {} wins. LOSS -{} (to jackpot).",
                bet.title,
                winner.to_uppercase(),
                chips_str(bet.stake),
            )
        }
        None => {
            if let Err(e) = state.api.casino_adjust(&bet.player, bet.stake).await {
                eprintln!("[Kalshi settle] refund failed for {}: {e:?}", bet.player);
            }
            format!(
                "[Kalshi] {} — result unavailable. {} refunded.",
                bet.title,
                chips_str(bet.stake),
            )
        }
    };

    deliver(&state, &whisper_cmd, &bet.player, msg).await;
}
