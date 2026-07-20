use serde_json::json;

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::market::types::now_unix;

use super::{MIN_BET, chips_str, format_alimony, to_price, fmt_odds, sleep_until, FetchErr, SettleDeps};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["gas", "gasbuddy", "gasprice"],
    description: "Bet on tomorrow's gas price. !gas [zip] | !gas <zip> up|down <chips> | !gas bets",
    whitelisted: false,
    execute,
};

const SETTLE_SECS: u64 = 24 * 3600;
const TIMEOUT_SECS: u64 = 20;
const CACHE_TTL: u64 = 3600;
const TOKEN_CACHE_PATH: &str = "./gasbuddy_token.json";
const GQL_URL: &str = "https://www.gasbuddy.com/graphql";
const HOME_URL: &str = "https://www.gasbuddy.com/home";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36";

// ── Structs ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct GasBet {
    pub id:            Option<i64>,
    pub player:        String,
    pub region:        String,
    pub zip:           String,
    pub side:          String,
    pub baseline:      i64,  // thousandths of a dollar (e.g. $3.459 → 3459)
    pub price:         i64,  // ten-thousandths / basis points (e.g. 2.0192 → 20192)
    pub stake:         i64,
    pub close_time:    u64,
}

impl super::CasinoBet for GasBet {
    const TYPE: &'static str = "gas";

    fn to_insert_json(&self) -> serde_json::Value {
        serde_json::json!({
            "player_uuid": self.player,
            "region":      self.region,
            "zip":         self.zip,
            "side":        self.side,
            "baseline":    self.baseline as f64 / 1000.0,
            "price":       self.price as f64 / 10000.0,
            "stake":       self.stake,
            "close_time":  self.close_time,
        })
    }

    fn from_json(item: &serde_json::Value) -> Option<Self> {
        Some(Self {
            id:         Some(item.get("id")?.as_i64()?),
            player:     item.get("player_uuid")?.as_str()?.to_owned(),
            region:     item.get("region")?.as_str()?.to_owned(),
            zip:        item.get("zip")?.as_str()?.to_owned(),
            side:       item.get("side")?.as_str()?.to_owned(),
            baseline:   (item.get("baseline")?.as_f64()? * 1000.0).round() as i64,
            price:      (item.get("price")?.as_f64()? * 10000.0).round() as i64,
            stake:      item.get("stake")?.as_i64()?,
            close_time: item.get("close_time")?.as_u64()?,
        })
    }
}

// ── Token cache ───────────────────────────────────────────────────────────────

pub async fn load_cached_token() -> Option<String> {
    let data = tokio::fs::read_to_string(TOKEN_CACHE_PATH).await.ok()?;
    serde_json::from_str::<serde_json::Value>(&data).ok()?
        .get("token")?.as_str().map(|s| s.to_owned())
}

async fn save_token(token: &str) {
    let body = json!({"token": token}).to_string();
    let tmp = format!("{TOKEN_CACHE_PATH}.tmp");
    if tokio::fs::write(&tmp, body).await.is_ok() {
        let _ = tokio::fs::rename(&tmp, TOKEN_CACHE_PATH).await;
    }
}

// ── GasBuddy fetch ────────────────────────────────────────────────────────────

fn extract_csrf(html: &str) -> Option<String> {
    let marker = "window.gbcsrf = \"";
    let start = html.find(marker)? + marker.len();
    let end = html[start..].find('"')? + start;
    Some(html[start..end].to_owned())
}

async fn fetch_csrf_raw(client: &reqwest::Client) -> Option<String> {
    let resp = client
        .get(HOME_URL)
        .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
        .header("User-Agent", UA)
        .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .send().await.ok()?;
    if !resp.status().is_success() { return None; }
    let html = resp.text().await.ok()?;
    if html.contains("cf-browser-verification") || html.contains("Just a moment") { return None; }
    extract_csrf(&html)
}

async fn fetch_csrf_via_solver(client: &reqwest::Client, solver_url: &str) -> Option<String> {
    let solve: serde_json::Value = client
        .post(solver_url)
        .timeout(std::time::Duration::from_secs(60))
        .json(&json!({"cmd": "request.get", "url": HOME_URL, "maxTimeout": 30000}))
        .send().await.ok()?.json().await.ok()?;
    extract_csrf(solve["solution"]["response"].as_str()?)
}

async fn fetch_csrf(client: &reqwest::Client, solver_url: &str) -> Option<String> {
    if let Some(t) = fetch_csrf_raw(client).await { return Some(t); }
    if !solver_url.is_empty() { return fetch_csrf_via_solver(client, solver_url).await; }
    None
}

enum GqlResult { Ok(f64, String), RateLimit, TokenBad, NoData }

async fn gql_price(client: &reqwest::Client, zip: &str, token: &str) -> GqlResult {
    let query = r#"query LocationBySearchTerm($brandId: Int, $cursor: String, $fuel: Int, $lat: Float, $lng: Float, $maxAge: Int, $search: String) {
  locationBySearchTerm(lat: $lat, lng: $lng, search: $search, priority: "locality") {
    displayName
    stations(brandId: $brandId, cursor: $cursor, fuel: $fuel, lat: $lat, lng: $lng, maxAge: $maxAge, priority: "locality") {
      results { prices { credit { price } fuelProduct } }
    }
  }
}"#;

    let resp = match client
        .post(GQL_URL)
        .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
        .header("Content-Type", "application/json")
        .header("gbcsrf", token)
        .header("apollo-require-preflight", "true")
        .header("Origin", "https://www.gasbuddy.com")
        .header("Referer", HOME_URL)
        .header("Sec-Fetch-Mode", "cors")
        .header("Sec-Fetch-Site", "same-origin")
        .header("User-Agent", UA)
        .json(&json!({"operationName":"LocationBySearchTerm","query":query,"variables":{"maxAge":0,"search":zip}}))
        .send().await
    {
        Ok(r) => r,
        Err(_) => return GqlResult::NoData,
    };

    if resp.status().as_u16() == 429 { return GqlResult::RateLimit; }
    if !resp.status().is_success() { return GqlResult::TokenBad; }

    let v: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => return GqlResult::TokenBad,
    };

    let loc = &v["data"]["locationBySearchTerm"];
    let display = loc["displayName"].as_str().unwrap_or(zip).to_owned();
    let Some(results) = loc["stations"]["results"].as_array() else { return GqlResult::NoData; };

    let prices: Vec<f64> = results.iter()
        .flat_map(|s| s["prices"].as_array().into_iter().flatten())
        .filter(|p| p["fuelProduct"].as_str() == Some("regular_gas"))
        .filter_map(|p| p["credit"]["price"].as_f64())
        .filter(|&p| p > 0.0)
        .collect();

    if prices.is_empty() { return GqlResult::NoData; }
    let avg = prices.iter().sum::<f64>() / prices.len() as f64;
    GqlResult::Ok(avg, display)
}

async fn fetch_gas_price(
    http: &reqwest::Client,
    gasbuddy_csrf: &std::sync::Arc<std::sync::Mutex<Option<String>>>,
    zip: &str,
    solver_url: &str,
    readonly: bool,
) -> Result<(f64, String), FetchErr> {
    let token = gasbuddy_csrf.lock().unwrap().clone().unwrap_or_default();

    match gql_price(http, zip, &token).await {
        GqlResult::Ok(p, n) => Ok((p, n)),
        GqlResult::RateLimit => Err(FetchErr::RateLimit),
        GqlResult::TokenBad => {
            let new_token = fetch_csrf(http, solver_url).await.ok_or(FetchErr::Error)?;
            if !readonly { save_token(&new_token).await; }
            *gasbuddy_csrf.lock().unwrap() = Some(new_token.clone());
            match gql_price(http, zip, &new_token).await {
                GqlResult::Ok(p, n) => Ok((p, n)),
                GqlResult::RateLimit => Err(FetchErr::RateLimit),
                _ => Err(FetchErr::Error),
            }
        }
        GqlResult::NoData => Err(FetchErr::Error),
    }
}

// ── Probability / pricing ─────────────────────────────────────────────────────

fn base_probs() -> (f64, f64) { (0.48, 0.52) } // (p_up, p_down)

fn gas_outcome(side: &str, new_price: f64, baseline: i64) -> bool {
    let new_mills = (new_price * 1000.0).round() as i64;
    match side {
        "up"   => new_mills > baseline,
        "down" => new_mills < baseline,
        _      => false,
    }
}

// ── Command ───────────────────────────────────────────────────────────────────

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let arg0 = ctx.args.first().copied().unwrap_or("").to_ascii_lowercase();
        let arg1 = ctx.args.get(1).copied().unwrap_or("").to_ascii_lowercase();
        let arg2 = ctx.args.get(2).copied().unwrap_or("");

        match arg0.as_str() {
            "" => {
                let p = &ctx.runtime.prefix;
                ctx.whisper_success(format!(
                    "Gas price bets (24h window): {p}gas <zip> | {p}gas <zip> up|down <chips> | {p}gas bets"
                ));
            }
            "bets" | "my" => show_bets(ctx).await?,
            _ => place_or_preview(ctx, &arg0, &arg1, arg2).await?,
        }
        Ok(())
    })
}

async fn show_bets(ctx: CommandContext<'_>) -> anyhow::Result<()> {
    let Some(player_uuid) = ctx.require_player_uuid().await else { return Ok(()); };
    let bets = {
        let map = ctx.state.gas_bets.lock().unwrap();
        map.get(&player_uuid).cloned().unwrap_or_default()
    };
    if bets.is_empty() {
        ctx.whisper_success("No open gas bets.");
        return Ok(());
    }
    for b in &bets {
        let secs_left = b.close_time.saturating_sub(now_unix());
        ctx.whisper_success(format!(
            "[GAS] {} {} {} — baseline ${:.3}/gal | {:.2}× | closes in {}h",
            b.region, b.side.to_uppercase(), chips_str(b.stake),
            b.baseline as f64 / 1000.0, 10000.0 / b.price as f64, secs_left / 3600,
        ));
    }
    Ok(())
}

async fn place_or_preview(ctx: CommandContext<'_>, zip: &str, side: &str, chips_str_arg: &str) -> anyhow::Result<()> {
    let solver_url = ctx.runtime.gasbuddy_solver_url.clone();
    let readonly   = ctx.runtime.gasbuddy_csrf_readonly;

    let cached = {
        let cache = ctx.state.gas_price_cache.lock().unwrap();
        cache.get(zip).and_then(|(p, r, t)| {
            if now_unix() - t < CACHE_TTL { Some((*p, r.clone())) } else { None }
        })
    };

    let (price, region) = if let Some(hit) = cached {
        hit
    } else {
        match fetch_gas_price(&ctx.state.http, &ctx.state.gasbuddy_csrf, zip, &solver_url, readonly).await {
            Ok(r) => {
                ctx.state.gas_price_cache.lock().unwrap()
                    .insert(zip.to_owned(), (r.0, r.1.clone(), now_unix()));
                r
            }
            Err(FetchErr::RateLimit) => {
                ctx.whisper_success("GasBuddy API rate limit reached. Try again later.");
                return Ok(());
            }
            Err(_) => {
                ctx.whisper_success("Could not fetch gas price — GasBuddy unavailable or zip not found.");
                return Ok(());
            }
        }
    };

    let (p_up, p_down) = base_probs();
    let price_up   = to_price(p_up);
    let price_down = to_price(p_down);

    ctx.whisper_success(format!(
        "[GAS: {region}] ${price:.3}/gal avg regular | Up tomorrow: {} | Down tomorrow: {}",
        fmt_odds(price_up), fmt_odds(price_down),
    ));

    if side.is_empty() { return Ok(()); }

    let bet_price: i64 = match side {
        "up"   => (price_up   * 10000.0).round() as i64,
        "down" => (price_down * 10000.0).round() as i64,
        _ => { ctx.whisper_success("Side must be 'up' or 'down'."); return Ok(()); }
    };

    let chips = match chips_str_arg.parse::<i64>() {
        Ok(n) if n >= MIN_BET => n,
        Ok(_) => { ctx.whisper_success(format!("Min bet: {} chips.", MIN_BET)); return Ok(()); }
        Err(_) => { ctx.whisper_success("Usage: !gas <zip> up|down <chips>"); return Ok(()); }
    };

    let Some(player_uuid) = ctx.require_player_uuid().await else { return Ok(()); };

    match ctx.state.api.casino_adjust(&player_uuid, -chips).await {
        Err(CasinoAdjustErr::InsufficientFunds(have)) => {
            ctx.whisper_success(format!("Not enough chips (have {}).", chips_str(have)));
            return Ok(());
        }
        Err(e) => { ctx.whisper_success(format!("Error: {e:?}")); return Ok(()); }
        Ok(_)  => {}
    }

    let close_time = now_unix() + SETTLE_SECS;
    let mut bet = GasBet {
        id: None,
        player:     player_uuid.clone(),
        region:     region.clone(),
        zip:        zip.to_owned(),
        side:       side.to_owned(),
        baseline:   (price * 1000.0).round() as i64,
        price:      bet_price,
        stake:      chips,
        close_time,
    };

    match ctx.state.api.casino_bet_insert(&bet).await {
        Some(i) => bet.id = Some(i),
        None => {
            if let Err(e) = ctx.state.api.casino_adjust(&player_uuid, chips).await {
                eprintln!("[Gas] refund failed for {player_uuid}: {e:?}");
                ctx.whisper_success("Failed to record bet. Refund also failed — contact an admin.");
            } else {
                ctx.whisper_success("Failed to record bet. Chips refunded.");
            }
            return Ok(());
        }
    }

    let payout = (chips as f64 * 10000.0 / bet_price as f64).floor() as i64;
    ctx.whisper_success(format!(
        "[GAS] {} {} {} — pays {} if price goes {} from ${price:.3} | closes in 24h",
        region, side.to_uppercase(), chips_str(chips), chips_str(payout), side,
    ));

    ctx.state.gas_bets.lock().unwrap()
        .entry(player_uuid)
        .or_default()
        .push(bet.clone());

    let deps = SettleDeps::from(ctx.state);
    let bets_map = ctx.state.gas_bets.clone();
    let http = ctx.state.http.clone();
    let gasbuddy_csrf = ctx.state.gasbuddy_csrf.clone();
    let whisper_cmd = ctx.runtime.whisper_command.clone();
    tokio::spawn(gas_settle_task(deps, bets_map, http, gasbuddy_csrf, whisper_cmd, bet));

    Ok(())
}

// ── Settlement ────────────────────────────────────────────────────────────────

pub async fn gas_settle_task(
    deps: SettleDeps,
    bets_map: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<GasBet>>>>,
    http: reqwest::Client,
    gasbuddy_csrf: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    whisper_cmd: String,
    bet: GasBet,
) {
    sleep_until(bet.close_time).await;

    {
        let mut bets = bets_map.lock().unwrap();
        if let Some(v) = bets.get_mut(&bet.player) {
            v.retain(|b| b.id != bet.id);
        }
    }

    let (solver_url, readonly) = {
        let rt = deps.runtime.read().expect("runtime lock");
        (rt.gasbuddy_solver_url.clone(), rt.gasbuddy_csrf_readonly)
    };

    let current = fetch_gas_price(&http, &gasbuddy_csrf, &bet.zip, &solver_url, readonly).await.ok();

    deps.api.casino_bet_delete::<GasBet>(bet.id.unwrap()).await;

    let msg = match current {
        Some((new_price, _)) => {
            let base_display = bet.baseline as f64 / 1000.0;
            let mult_display = 10000.0 / bet.price as f64;
            if gas_outcome(&bet.side, new_price, bet.baseline) {
                let payout = (bet.stake as f64 * 10000.0 / bet.price as f64).floor() as i64;
                match deps.api.casino_win(&bet.player, payout).await {
                    Ok(win) => {
                        let alimony_note = format_alimony(win.alimony_paid);
                        format!(
                            "[GAS] {} {} — ${:.3}→${:.3}. {} WIN +{}{alimony_note} ({} @ {:.2}×).",
                            bet.region, bet.side.to_uppercase(),
                            base_display, new_price,
                            bet.side.to_uppercase(),
                            chips_str(payout - bet.stake),
                            chips_str(bet.stake),
                            mult_display,
                        )
                    }
                    Err(e) => {
                        eprintln!("[GAS settle] casino_win failed for {}: {e:?}", bet.player);
                        format!("[GAS] {} {} wins but payout failed. Contact an admin.", bet.region, bet.side.to_uppercase())
                    }
                }
            } else {
                deps.api.casino_jackpot_rake(bet.stake).await;
                format!(
                    "[GAS] {} {} — ${:.3}→${:.3}. {} LOSS -{} (to jackpot).",
                    bet.region, bet.side.to_uppercase(),
                    base_display, new_price,
                    bet.side.to_uppercase(),
                    chips_str(bet.stake),
                )
            }
        }
        None => {
            match deps.api.casino_adjust(&bet.player, bet.stake).await {
                Ok(_) => format!(
                    "[GAS] {} {} — GasBuddy unavailable at settlement. {} refunded.",
                    bet.region, bet.side.to_uppercase(), chips_str(bet.stake),
                ),
                Err(e) => {
                    eprintln!("[GAS settle] refund failed for {}: {e:?}", bet.player);
                    format!("[GAS] {} {} — GasBuddy unavailable. Refund failed — contact an admin.", bet.region, bet.side.to_uppercase())
                }
            }
        }
    };

    deps.deliver(&whisper_cmd, &bet.player, msg).await;
}
