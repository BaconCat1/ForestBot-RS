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

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::args().any(|a| a == "--debug") {
        // SAFETY: set before any threads spawn
        unsafe { std::env::set_var("DEBUG", "1") };
    }

    print_banner();

    println!("               Made by Febzey#1854. Ported to Rust by bacon_cat_");

    let state = AppState::load().await?;
    let options = state.options()?;

    let mut api = ApiClient::new(options.api.clone());
    api.init_websocket().await?;
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
