use serde::Deserialize;

use crate::structure::market::types::{Asset, Candle, MarketKind, Quote};

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/124 Safari/537.36";

// ── Quote ──────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ChartResp {
    chart: ChartBody,
}
#[derive(Deserialize)]
struct ChartBody {
    result: Option<Vec<ChartResult>>,
}
#[derive(Deserialize)]
struct ChartResult {
    meta: ChartMeta,
    timestamp: Option<Vec<i64>>,
    indicators: Option<Indicators>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChartMeta {
    symbol: String,
    #[serde(default)]
    long_name: Option<String>,
    #[serde(default)]
    short_name: Option<String>,
    regular_market_price: f64,
    #[serde(default)]
    previous_close: Option<f64>,
}
#[derive(Deserialize)]
struct Indicators {
    quote: Vec<QuoteIndicator>,
}
#[derive(Deserialize)]
struct QuoteIndicator {
    open:  Option<Vec<Option<f64>>>,
    high:  Option<Vec<Option<f64>>>,
    low:   Option<Vec<Option<f64>>>,
    close: Option<Vec<Option<f64>>>,
}

pub async fn quote(client: &reqwest::Client, symbol: &str) -> anyhow::Result<Quote> {
    let url = format!(
        "https://query2.finance.yahoo.com/v8/finance/chart/{}?interval=1d&range=2d",
        symbol
    );
    let resp: ChartResp = client.get(&url).header("User-Agent", UA).send().await?.json().await?;
    let result = resp.chart.result
        .and_then(|mut v| if v.is_empty() { None } else { Some(v.remove(0)) })
        .ok_or_else(|| anyhow::anyhow!("No data for {symbol}"))?;

    let meta = result.meta;
    let prev = meta.previous_close.unwrap_or(meta.regular_market_price);
    let change_pct = if prev != 0.0 { (meta.regular_market_price - prev) / prev * 100.0 } else { 0.0 };
    let name = meta.long_name.or(meta.short_name).unwrap_or_else(|| meta.symbol.clone());

    Ok(Quote {
        symbol: meta.symbol,
        name,
        price: meta.regular_market_price,
        change_pct,
        market: MarketKind::Stock,
    })
}

pub async fn history(
    client: &reqwest::Client,
    symbol: &str,
    range_days: u32,
) -> anyhow::Result<Vec<Candle>> {
    let (interval, range) = match range_days {
        1       => ("1h",  "1d"),
        2..=7   => ("1d",  "7d"),
        8..=31  => ("1d",  "1mo"),
        _       => ("1wk", "1y"),
    };
    let url = format!(
        "https://query2.finance.yahoo.com/v8/finance/chart/{}?interval={}&range={}",
        symbol, interval, range
    );
    let resp: ChartResp = client.get(&url).header("User-Agent", UA).send().await?.json().await?;
    let result = resp.chart.result
        .and_then(|mut v| if v.is_empty() { None } else { Some(v.remove(0)) })
        .ok_or_else(|| anyhow::anyhow!("No history for {symbol}"))?;

    let timestamps = result.timestamp.unwrap_or_default();
    let qi = result.indicators
        .and_then(|i| i.quote.into_iter().next())
        .unwrap_or(QuoteIndicator { open: None, high: None, low: None, close: None });

    let open  = qi.open.unwrap_or_default();
    let high  = qi.high.unwrap_or_default();
    let low   = qi.low.unwrap_or_default();
    let close = qi.close.unwrap_or_default();

    let candles: Vec<Candle> = timestamps.iter().enumerate()
        .filter_map(|(i, &ts)| {
            Some(Candle {
                timestamp: ts as u64,
                open:  open.get(i)?.as_ref().copied()?,
                high:  high.get(i)?.as_ref().copied()?,
                low:   low.get(i)?.as_ref().copied()?,
                close: close.get(i)?.as_ref().copied()?,
            })
        })
        .collect();

    anyhow::ensure!(!candles.is_empty(), "No candles for {symbol}");
    Ok(candles)
}

pub async fn price_at(client: &reqwest::Client, symbol: &str, target_unix: u64) -> anyhow::Result<f64> {
    let now = crate::structure::market::types::now_unix();
    let age = now.saturating_sub(target_unix);
    let (interval, range) = match age {
        0..=3_600      => ("1m",  "1d"),
        3_601..=604_800 => ("5m", "5d"),
        _              => ("1h",  "60d"),
    };
    let url = format!(
        "https://query2.finance.yahoo.com/v8/finance/chart/{}?interval={}&range={}",
        symbol, interval, range
    );
    let resp: ChartResp = client.get(&url).header("User-Agent", UA).send().await?.json().await?;
    let result = resp.chart.result
        .and_then(|mut v| if v.is_empty() { None } else { Some(v.remove(0)) })
        .ok_or_else(|| anyhow::anyhow!("No data for {symbol}"))?;

    let timestamps = result.timestamp.unwrap_or_default();
    let close = result.indicators
        .and_then(|i| i.quote.into_iter().next())
        .and_then(|qi| qi.close)
        .unwrap_or_default();

    let best = timestamps.iter().enumerate()
        .filter_map(|(i, &ts)| close.get(i)?.as_ref().map(|&c| (ts as u64, c)))
        .min_by_key(|&(ts, _)| ts.abs_diff(target_unix));

    let (_, price) = best.ok_or_else(|| anyhow::anyhow!("No candles near target for {symbol}"))?;
    Ok(price)
}

// ── Search ─────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct SearchResp {
    quotes: Vec<SearchQuote>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchQuote {
    symbol: String,
    #[serde(default)]
    short_name: Option<String>,
    #[serde(default)]
    long_name: Option<String>,
}

pub async fn search(client: &reqwest::Client, query: &str) -> anyhow::Result<Vec<Asset>> {
    let url = format!(
        "https://query2.finance.yahoo.com/v1/finance/search?q={}&quotesCount=5&newsCount=0",
        urlencoding::encode(query)
    );
    let resp: SearchResp = client.get(&url).header("User-Agent", UA).send().await?.json().await?;
    Ok(resp.quotes.into_iter().map(|q| Asset {
        symbol: q.symbol.clone(),
        name: q.long_name.or(q.short_name).unwrap_or(q.symbol),
        market: MarketKind::Stock,
    }).collect())
}
