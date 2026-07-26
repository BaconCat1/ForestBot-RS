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

    if crate::structure::logger::debug_cat_enabled("packets") {
        // Azalea's own trace!() logging (e.g. the raw packet-bytes dump in
        // azalea-protocol's read.rs) needs a tracing subscriber to go anywhere --
        // ForestBot-RS otherwise never touches the `tracing` crate at all, so this
        // was previously a silent no-op regardless of RUST_LOG. Scoped to just the
        // packet-read target to avoid flooding the log with every other crate's
        // trace-level noise. Written to its own file, not stdout -- one packet per
        // read floods the console/pm2 log otherwise, drowning out the normal
        // debug_cat lines.
        let packet_log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("azalea-packets.log")
            .expect("failed to open azalea-packets.log");
        tracing_subscriber::fmt()
            .with_env_filter("azalea_protocol::read=trace")
            .with_writer(std::sync::Mutex::new(packet_log))
            .with_ansi(false)
            .init();
    }

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
