use chrono::Local;
use colored::{ColoredString, Colorize};
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

fn timestamp() -> String {
    Local::now().format("%m/%d/%Y, %I:%M:%S %p").to_string()
}

fn log(tag: ColoredString, message: &str) {
    println!("[{}] - {} | {}", tag, message, timestamp().dimmed());
}

pub fn info(message: impl AsRef<str>) {
    log("info".blue(), message.as_ref());
}

#[allow(dead_code)]
pub fn success(message: impl AsRef<str>) {
    log("success".bright_green(), message.as_ref());
}

pub fn warn(message: impl AsRef<str>) {
    log("warn".yellow(), message.as_ref());
}

#[allow(dead_code)]
pub fn error(message: impl AsRef<str>) {
    log("error".red(), message.as_ref());
}

pub fn chat(message: impl AsRef<str>) {
    log("chat".red(), message.as_ref());
}

pub fn advancement(message: impl AsRef<str>) {
    log("advancement".yellow(), message.as_ref());
}

pub fn death(message: impl AsRef<str>) {
    log("death".cyan(), message.as_ref());
}

pub fn join(message: impl AsRef<str>) {
    log("join".magenta(), message.as_ref());
}

pub fn leave(message: impl AsRef<str>) {
    log("leave".magenta(), message.as_ref());
}

pub fn kick(message: impl AsRef<str>) {
    log("kick".red(), message.as_ref());
}

pub fn login(message: impl AsRef<str>) {
    log("login".green(), message.as_ref());
}

pub fn logout(message: impl AsRef<str>) {
    log("logout".red(), message.as_ref());
}

pub fn spawn(message: impl AsRef<str>) {
    log("spawn".green(), message.as_ref());
}

pub fn world(message: impl AsRef<str>) {
    log("world".yellow(), message.as_ref());
}

pub fn command(message: impl AsRef<str>) {
    log("command".cyan(), message.as_ref());
}

pub fn websocket(message: impl AsRef<str>) {
    log("websocket".yellow(), message.as_ref());
}

static DEBUG_CATEGORIES: OnceLock<RwLock<HashMap<String, bool>>> = OnceLock::new();

fn read_and_merge_debug_json() -> HashMap<String, bool> {
    let example: HashMap<String, bool> = std::fs::read_to_string("./example.debug.json")
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let mut current: HashMap<String, bool> = match std::fs::read_to_string("./debug.json") {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => HashMap::new(),
    };

    let mut added: Vec<String> = Vec::new();
    for (key, value) in &example {
        current.entry(key.clone()).or_insert_with(|| {
            added.push(key.clone());
            *value
        });
    }

    if !added.is_empty() {
        println!(
            "[debug] Auto-merged {} missing categor{} from example.debug.json: {}",
            added.len(),
            if added.len() == 1 { "y" } else { "ies" },
            added.join(", ")
        );
        if let Ok(pretty) = serde_json::to_string_pretty(&current) {
            let _ = std::fs::write("./debug.json", pretty);
        }
    }

    current
}

/// Loads debug.json, creating it fresh from example.debug.json if it doesn't exist yet
/// and auto-merging in any new category keys added to example.debug.json since (same
/// pattern as config.json/example.config.json in config.rs). Categories missing from the
/// map entirely default to enabled -- only explicit `false` entries silence a category.
/// Safe (and expected) to call again later, e.g. from `!reload` -- unlike a plain
/// OnceLock, this actually re-reads the file and overwrites the live categories on every
/// call after the first, instead of silently no-op'ing.
pub fn load_debug_categories() {
    let categories = read_and_merge_debug_json();
    match DEBUG_CATEGORIES.get() {
        Some(lock) => *lock.write().expect("debug categories lock poisoned") = categories,
        None => {
            let _ = DEBUG_CATEGORIES.set(RwLock::new(categories));
        }
    }
}

/// Whether a debug category is currently enabled -- requires the global DEBUG env var
/// AND the category not being explicitly set to `false` in debug.json. Missing/unlisted
/// categories default to enabled. Use this to gate any debug-only behavior, not just log
/// lines (e.g. writing debug artifacts to disk).
pub fn debug_cat_enabled(category: &str) -> bool {
    if std::env::var("DEBUG").is_err() {
        return false;
    }
    DEBUG_CATEGORIES
        .get()
        .and_then(|lock| lock.read().ok().map(|categories| categories.get(category).copied()))
        .flatten()
        .unwrap_or(true)
}

/// Debug logging gated per-category via debug.json, in addition to the global DEBUG env
/// var. A category not listed in debug.json defaults to enabled (so forgetting to add a
/// new one to example.debug.json never silently hides it).
pub fn debug_cat(category: &str, message: impl AsRef<str>) {
    if debug_cat_enabled(category) {
        log("debug".bright_black(), message.as_ref());
    }
}
