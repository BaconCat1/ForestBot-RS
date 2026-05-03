mod config;

use anyhow::Result;
use colored::Colorize;
use config::AppState;

#[tokio::main]
async fn main() -> Result<()> {
    println!(
        "{}",
        r#"
    ███████╗ ██████╗ ██████╗ ███████╗███████╗████████╗██████╗  ██████╗ ████████╗
    ██╔════╝██╔═══██╗██╔══██╗██╔════╝██╔════╝╚══██╔══╝██╔══██╗██╔═══██╗╚══██╔══╝
    █████╗  ██║   ██║██████╔╝█████╗  ███████╗   ██║   ██████╔╝██║   ██║   ██║
    ██╔══╝  ██║   ██║██╔══██╗██╔══╝  ╚════██║   ██║   ██╔══██╗██║   ██║   ██║
    ██║     ╚██████╔╝██║  ██║███████╗███████║   ██║   ██████╔╝╚██████╔╝   ██║
    ╚═╝      ╚═════╝ ╚═╝  ╚═╝╚══════╝╚══════╝   ╚═╝   ╚═════╝  ╚═════╝    ╚═╝
"#
            .red()
    );

    println!("                  Made by Febzey#1854. Ported to Rust by bacon_cat_");

    let state = AppState::load().await?;
    let options = state.options()?;

    println!("{:#?}", options);

    Ok(())
}