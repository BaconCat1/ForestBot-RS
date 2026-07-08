use serde::Deserialize;

use crate::structure::market::types::{Asset, Candle, MarketKind, Quote};

const BASE: &str = "https://api.coingecko.com/api/v3";

fn apply_key(req: reqwest::RequestBuilder, key: &str) -> reqwest::RequestBuilder {
    if !key.is_empty() {
        req.header("x-cg-demo-api-key", key)
    } else {
        req
    }
}

// Known symbol → CoinGecko ID mappings
const KNOWN: &[(&str, &str)] = &[
    ("BTC",   "bitcoin"),
    ("ETH",   "ethereum"),
    ("SOL",   "solana"),
    ("DOGE",  "dogecoin"),
    ("ADA",   "cardano"),
    ("XRP",   "ripple"),
    ("DOT",   "polkadot"),
    ("AVAX",  "avalanche-2"),
    ("LINK",  "chainlink"),
    ("LTC",   "litecoin"),
    ("UNI",   "uniswap"),
    ("ATOM",  "cosmos"),
    ("MATIC", "matic-network"),
    ("PEPE",  "pepe"),
    ("SHIB",  "shiba-inu"),
    ("BNB",   "binancecoin"),
    ("TRX",   "tron"),
    ("TON",   "the-open-network"),
    ("NEAR",  "near"),
    ("APT",   "aptos"),
];

pub fn known_id(symbol: &str) -> Option<&'static str> {
    let up = symbol.to_uppercase();
    KNOWN.iter().find(|(s, _)| *s == up).map(|(_, id)| *id)
}

pub fn is_known_crypto(symbol: &str) -> bool {
    known_id(symbol).is_some()
}

async fn resolve_id(client: &reqwest::Client, symbol: &str, key: &str) -> anyhow::Result<String> {
    if let Some(id) = known_id(symbol) {
        return Ok(id.to_owned());
    }
    // Fall back to search
    let results = search(client, symbol, key).await?;
    results.first()
        .map(|a| a.symbol.to_ascii_lowercase())
        .ok_or_else(|| anyhow::anyhow!("Unknown crypto symbol: {symbol}"))
}

// ── Quote ──────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[allow(dead_code)]
struct CoinMarket {
    id: String,
    symbol: String,
    name: String,
    current_price: f64,
    #[serde(default)]
    price_change_percentage_24h: Option<f64>,
}

pub async fn quote(client: &reqwest::Client, symbol: &str, key: &str) -> anyhow::Result<Quote> {
    let id = resolve_id(client, symbol, key).await?;
    let url = format!(
        "{BASE}/coins/markets?vs_currency=usd&ids={id}&price_change_percentage=24h"
    );
    let raw = apply_key(client.get(&url), key).send().await?;
    if raw.status() == 429 {
        anyhow::bail!("CoinGecko rate limit — set api_keys.coingecko in config.json");
    }
    let mut resp: Vec<CoinMarket> = raw.json().await?;
    let coin = resp.pop().ok_or_else(|| anyhow::anyhow!("No data for {symbol}"))?;

    Ok(Quote {
        symbol: coin.symbol.to_uppercase(),
        name: coin.name,
        price: coin.current_price,
        change_pct: coin.price_change_percentage_24h.unwrap_or(0.0),
        market: MarketKind::Crypto,
    })
}

// ── History ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct MarketChart {
    prices: Vec<(f64, f64)>,
}

pub async fn history(
    client: &reqwest::Client,
    symbol: &str,
    range_days: u32,
    key: &str,
) -> anyhow::Result<Vec<Candle>> {
    let id = resolve_id(client, symbol, key).await?;
    let url = format!("{BASE}/coins/{id}/market_chart?vs_currency=usd&days={range_days}");
    let raw = apply_key(client.get(&url), key).send().await?;
    if raw.status() == 429 {
        anyhow::bail!("CoinGecko rate limit — set api_keys.coingecko in config.json");
    }
    let chart: MarketChart = raw.json().await?;

    let candles: Vec<Candle> = chart.prices.windows(2).map(|w| {
        let (ts, open) = w[0];
        let (_, close) = w[1];
        Candle {
            timestamp: (ts / 1000.0) as u64,
            open,
            high: open.max(close),
            low:  open.min(close),
            close,
        }
    }).collect();

    anyhow::ensure!(!candles.is_empty(), "No history for {symbol}");
    Ok(candles)
}

pub async fn price_at(client: &reqwest::Client, symbol: &str, target_unix: u64, key: &str) -> anyhow::Result<f64> {
    let now = crate::structure::market::types::now_unix();
    let age_secs = now.saturating_sub(target_unix);
    let days = ((age_secs / 86400) + 1).max(1) as u32;
    let id = resolve_id(client, symbol, key).await?;
    let url = format!("{BASE}/coins/{id}/market_chart?vs_currency=usd&days={days}");
    let raw = apply_key(client.get(&url), key).send().await?;
    if raw.status() == 429 {
        anyhow::bail!("CoinGecko rate limit — set api_keys.coingecko in config.json");
    }
    let chart: MarketChart = raw.json().await?;

    let best = chart.prices.iter()
        .min_by_key(|(ts_ms, _)| {
            let ts = (*ts_ms / 1000.0) as u64;
            ts.abs_diff(target_unix)
        });

    let &(_, price) = best.ok_or_else(|| anyhow::anyhow!("No price history for {symbol}"))?;
    Ok(price)
}

// ── Search ─────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct SearchResp {
    coins: Vec<CoinResult>,
}
#[derive(Deserialize)]
#[allow(dead_code)]
struct CoinResult {
    id: String,
    symbol: String,
    name: String,
}

pub async fn search(client: &reqwest::Client, query: &str, key: &str) -> anyhow::Result<Vec<Asset>> {
    let url = format!("{BASE}/search?query={}", urlencoding::encode(query));
    let raw = apply_key(client.get(&url), key).send().await?;
    if raw.status() == 429 {
        anyhow::bail!("CoinGecko rate limit — set api_keys.coingecko in config.json");
    }
    let resp: SearchResp = raw.json().await?;
    Ok(resp.coins.into_iter().take(5).map(|c| Asset {
        symbol: c.symbol.to_uppercase(),
        name: c.name,
        market: MarketKind::Crypto,
    }).collect())
}
