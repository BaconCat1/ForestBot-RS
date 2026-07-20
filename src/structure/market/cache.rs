use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::types::{Asset, Candle, Quote};

struct Entry<T> {
    value: T,
    expires: Instant,
}

impl<T: Clone> Entry<T> {
    fn get(&self) -> Option<T> {
        if Instant::now() < self.expires { Some(self.value.clone()) } else { None }
    }
}

pub struct Cache {
    quotes:  Mutex<HashMap<String, Entry<Quote>>>,
    history: Mutex<HashMap<String, Entry<Vec<Candle>>>>,
    search:  Mutex<HashMap<String, Entry<Vec<Asset>>>>,
    quote_ttl: Duration,
    history_ttl: Duration,
    search_ttl: Duration,
}

impl Cache {
    pub fn new(quote_ttl_secs: u64, history_ttl_secs: u64, search_ttl_secs: u64) -> Self {
        Self {
            quotes:  Mutex::new(HashMap::new()),
            history: Mutex::new(HashMap::new()),
            search:  Mutex::new(HashMap::new()),
            quote_ttl: Duration::from_secs(quote_ttl_secs),
            history_ttl: Duration::from_secs(history_ttl_secs),
            search_ttl: Duration::from_secs(search_ttl_secs),
        }
    }

    pub fn get_quote(&self, key: &str) -> Option<Quote> {
        self.quotes.lock().ok()?.get(key)?.get()
    }
    pub fn put_quote(&self, key: &str, v: Quote) {
        if let Ok(mut m) = self.quotes.lock() {
            m.insert(key.to_owned(), Entry { value: v, expires: Instant::now() + self.quote_ttl });
        }
    }

    pub fn get_history(&self, key: &str) -> Option<Vec<Candle>> {
        self.history.lock().ok()?.get(key)?.get()
    }
    pub fn put_history(&self, key: &str, v: Vec<Candle>) {
        if let Ok(mut m) = self.history.lock() {
            m.insert(key.to_owned(), Entry { value: v, expires: Instant::now() + self.history_ttl });
        }
    }

    pub fn get_search(&self, key: &str) -> Option<Vec<Asset>> {
        self.search.lock().ok()?.get(key)?.get()
    }
    pub fn put_search(&self, key: &str, v: Vec<Asset>) {
        if let Ok(mut m) = self.search.lock() {
            m.insert(key.to_owned(), Entry { value: v, expires: Instant::now() + self.search_ttl });
        }
    }
}
