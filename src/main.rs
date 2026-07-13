mod commands;
mod config;
mod constants;
mod events;
mod functions;
mod structure;

use anyhow::Result;
use colored::Colorize;
use config::AppState;
use structure::{endpoints::endpoints::ApiClient, mineflayer::bot::Bot};

// Windows default main-thread stack is 1MB vs Linux's 8MB, causing overflows in
// Azalea's async event loop. Spawn the runtime on a thread with an explicit 8MB stack.
fn main() -> Result<()> {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(run)?
        .join()
        .unwrap()
}

#[tokio::main]
async fn run() -> Result<()> {
    if std::env::args().any(|a| a == "--debug") {
        // SAFETY: set before any threads spawn
        unsafe { std::env::set_var("DEBUG", "1") };
    }
    if std::env::args().any(|a| a == "--announcefast") {
        unsafe { std::env::set_var("ANNOUNCE_FAST", "1") };
    }

    print_banner();

    println!("               Made by Febzey#1854. Ported to Rust by bacon_cat_");

    crate::structure::logger::load_debug_categories();

    let state = AppState::load().await?;
    let options = state.options()?;

    let mut api = ApiClient::new(options.api.clone());
    api.init_websocket().await?;
    tokio::task::spawn_blocking(crate::commands::askgod::preload_all_corpora)
        .await
        .ok();
    let mut bot = Bot::new(options.bot, &state, api);
    bot.start().await?;

    Ok(())
}

fn print_banner() {
    let forestbot = [
        "    ███████╗ ██████╗ ██████╗ ███████╗███████╗████████╗██████╗  ██████╗ ████████╗",
        "    ██╔════╝██╔═══██╗██╔══██╗██╔════╝██╔════╝╚══██╔══╝██╔══██╗██╔═══██╗╚══██╔══╝",
        "    █████╗  ██║   ██║██████╔╝█████╗  ███████╗   ██║   ██████╔╝██║   ██║   ██║   ",
        "    ██╔══╝  ██║   ██║██╔══██╗██╔══╝  ╚════██║   ██║   ██╔══██╗██║   ██║   ██║   ",
        "    ██║     ╚██████╔╝██║  ██║███████╗███████║   ██║   ██████╔╝╚██████╔╝   ██║   ",
        "    ╚═╝      ╚═════╝ ╚═╝  ╚═╝╚══════╝╚══════╝   ╚═╝   ╚═════╝  ╚═════╝    ╚═╝   ",
    ];

    let rs = [
        "       ██████╗ ███████╗",
        "       ██╔══██╗██╔════╝",
        "█████╗ ██████╔╝███████╗",
        "╚════╝ ██╔══██╗╚════██║",
        "       ██║  ██║███████║",
        "       ╚═╝  ╚═╝╚══════╝",
    ];

    for (left, right) in forestbot.iter().zip(rs.iter()) {
        println!("{}{}", left.green(), right.red());
    }
}
