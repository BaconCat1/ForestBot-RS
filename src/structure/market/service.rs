use std::sync::Arc;

use super::cache::Cache;
use super::providers::{coingecko, infer_market, yahoo};
use super::types::{Asset, Candle, MarketKind, Quote};

pub struct MarketService {
    cache: Arc<Cache>,
    client: reqwest::Client,
    coingecko_key: String,
}

impl MarketService {
    pub fn new(
        coingecko_key: String,
        quote_ttl_secs: u64,
        history_ttl_secs: u64,
        search_ttl_secs: u64,
        api_timeout_secs: u64,
    ) -> Self {
        Self {
            cache: Arc::new(Cache::new(quote_ttl_secs, history_ttl_secs, search_ttl_secs)),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(api_timeout_secs))
                .build()
                .expect("reqwest client"),
            coingecko_key,
        }
    }

    pub async fn quote(&self, symbol: &str) -> anyhow::Result<Quote> {
        let key = symbol.to_uppercase();
        if let Some(cached) = self.cache.get_quote(&key) {
            return Ok(cached);
        }
        let q = match infer_market(symbol) {
            MarketKind::Stock  => yahoo::quote(&self.client, &key).await?,
            MarketKind::Crypto => coingecko::quote(&self.client, &key, &self.coingecko_key).await?,
        };
        self.cache.put_quote(&key, q.clone());
        Ok(q)
    }

    pub async fn history(&self, symbol: &str, range_days: u32) -> anyhow::Result<Vec<Candle>> {
        let key = format!("{}:{}", symbol.to_uppercase(), range_days);
        if let Some(cached) = self.cache.get_history(&key) {
            return Ok(cached);
        }
        let candles = match infer_market(symbol) {
            MarketKind::Stock  => yahoo::history(&self.client, &symbol.to_uppercase(), range_days).await?,
            MarketKind::Crypto => coingecko::history(&self.client, symbol, range_days, &self.coingecko_key).await?,
        };
        self.cache.put_history(&key, candles.clone());
        Ok(candles)
    }

    pub async fn price_at(&self, symbol: &str, target_unix: u64) -> anyhow::Result<f64> {
        let key = symbol.to_uppercase();
        match infer_market(symbol) {
            MarketKind::Stock  => yahoo::price_at(&self.client, &key, target_unix).await,
            MarketKind::Crypto => coingecko::price_at(&self.client, &key, target_unix, &self.coingecko_key).await,
        }
    }

    pub async fn search(&self, query: &str) -> anyhow::Result<Vec<Asset>> {
        let key = query.to_ascii_lowercase();
        if let Some(cached) = self.cache.get_search(&key) {
            return Ok(cached);
        }
        // Search both providers and merge
        let mut results = Vec::new();
        if let Ok(mut r) = yahoo::search(&self.client, query).await { results.append(&mut r); }
        if let Ok(mut r) = coingecko::search(&self.client, query, &self.coingecko_key).await { results.append(&mut r); }
        results.dedup_by(|a, b| a.symbol == b.symbol);
        results.truncate(6);
        self.cache.put_search(&key, results.clone());
        Ok(results)
    }
}
