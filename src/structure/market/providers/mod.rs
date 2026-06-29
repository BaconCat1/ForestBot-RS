pub mod coingecko;
pub mod yahoo;

use crate::structure::market::types::MarketKind;

pub fn infer_market(symbol: &str) -> MarketKind {
    if coingecko::is_known_crypto(symbol) {
        MarketKind::Crypto
    } else {
        MarketKind::Stock
    }
}
