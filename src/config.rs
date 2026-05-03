use anyhow::{Context, Result};
use serde::Deserialize;
use std::env;
use tokio::fs;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub version: String,

    pub api_url: String,
    pub websocket_url: String,
    pub mc_server: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Colors {
    // TODO: match colors.json
}

#[derive(Debug, Clone, Deserialize)]
struct UserList {
    users: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct BotConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub version: String,
}

#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub api_url: String,
    pub websocket_url: String,
    pub api_key: String,
    pub mc_server: String,

    pub is_bot_client: bool,
    pub log_errors: bool,
    pub use_websocket: bool,
}

#[derive(Debug, Clone)]
pub struct Options {
    pub bot: BotConfig,
    pub api: ApiConfig,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub colors: Colors,
    pub config: Config,
    pub mc_whitelist: Vec<String>,
    pub mc_blacklist: Vec<String>,
}

impl AppState {
    pub async fn load() -> Result<Self> {
        dotenvy::dotenv().ok();

        let colors: Colors = read_json("./json/colors.json").await?;
        let config: Config = read_json("./config.json").await?;
        let whitelist: UserList = read_json("./json/mc_whitelist.json").await?;
        let blacklist: UserList = read_json("./json/mc_blacklist.json").await?;

        require_env("MC_USER")?;
        require_env("MC_PASS")?;
        require_env("API_KEY")?;

        Ok(Self {
            colors,
            config,
            mc_whitelist: whitelist.users,
            mc_blacklist: blacklist.users,
        })
    }

    pub async fn reload_config(&mut self) -> Result<()> {
        self.config = read_json("./config.json").await?;
        self.mc_whitelist = read_json::<UserList>("./json/mc_whitelist.json")
            .await?
            .users;
        self.mc_blacklist = read_json::<UserList>("./json/mc_blacklist.json")
            .await?
            .users;

        println!("Config reloaded successfully.");
        Ok(())
    }

    pub fn options(&self) -> Result<Options> {
        Ok(Options {
            bot: BotConfig {
                host: self.config.host.clone(),
                port: self.config.port,
                username: env::var("MC_USER")?,
                password: env::var("MC_PASS")?,
                version: self.config.version.clone(),
            },

            api: ApiConfig {
                api_url: self.config.api_url.clone(),
                websocket_url: self.config.websocket_url.clone(),
                api_key: env::var("API_KEY")?,
                mc_server: self.config.mc_server.clone(),

                is_bot_client: true,
                log_errors: true,
                use_websocket: true,
            },
        })
    }
}

async fn read_json<T>(path: &str) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let data = fs::read_to_string(path)
        .await
        .with_context(|| format!("Failed to read {path}"))?;

    serde_json::from_str(&data)
        .with_context(|| format!("Failed to parse JSON from {path}"))
}

fn require_env(name: &str) -> Result<String> {
    env::var(name)
        .with_context(|| format!("Missing environment variable `{name}`. Check your .env/config."))
}