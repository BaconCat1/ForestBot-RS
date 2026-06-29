use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarketKind {
    Stock,
    Crypto,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Long,
    Short,
}

impl Direction {
    pub fn label(self) -> &'static str {
        match self { Direction::Long => "LONG", Direction::Short => "SHORT" }
    }
}

#[derive(Clone, Debug)]
pub struct Quote {
    pub symbol: String,
    pub name: String,
    pub price: f64,
    pub change_pct: f64,
    pub market: MarketKind,
}

#[derive(Clone, Debug)]
pub struct Candle {
    pub timestamp: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}

#[derive(Clone, Debug)]
pub struct Asset {
    pub symbol: String,
    pub name: String,
    pub market: MarketKind,
}

#[derive(Clone, Debug)]
pub struct MarketBet {
    pub id: Uuid,
    pub player: String,
    pub symbol: String,
    pub market: MarketKind,
    pub direction: Direction,
    pub entry_price: f64,
    pub stake: i64,
    pub closes_unix: u64,
    pub duration_label: String,
}

#[derive(Clone, Debug)]
pub struct PortfolioPosition {
    pub id: uuid::Uuid,
    pub player: String,
    pub symbol: String,
    pub market: MarketKind,
    pub entry_price: f64,
    pub stake: i64,
    pub opened_unix: u64,
}

pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn parse_duration(s: &str) -> Option<u64> {
    let s = s.to_ascii_lowercase();
    if let Some(n) = s.strip_suffix('d') {
        return n.parse::<u64>().ok().map(|n| n * 86400).filter(|&d| d <= 86400);
    }
    if let Some(n) = s.strip_suffix('h') {
        return n.parse::<u64>().ok().map(|n| n * 3600).filter(|&d| d >= 60 && d <= 86400);
    }
    if let Some(n) = s.strip_suffix('m') {
        return n.parse::<u64>().ok().map(|n| n * 60).filter(|&d| d >= 60 && d <= 86400);
    }
    None
}

pub fn format_remaining(secs: u64) -> String {
    if secs < 60 { format!("{}s", secs) }
    else if secs < 3600 { format!("{}m {}s", secs / 60, secs % 60) }
    else { format!("{}h {}m", secs / 3600, (secs % 3600) / 60) }
}

pub fn fmt_price(p: f64) -> String {
    if p >= 1000.0 { format!("${:.2}", p) }
    else if p >= 1.0 { format!("${:.2}", p) }
    else { format!("${:.6}", p) }
}
