use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use tokio::fs;

fn default_azalea_version() -> String {
    "1.21.11".to_owned()
}

fn default_server_version() -> String {
    "1.21.11".to_owned()
}

fn default_viaversion_target_version() -> String {
    "1.21.11".to_owned()
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub mc_server: String,
    pub host: String,
    pub port: u16,
    pub version: String,
    #[serde(default = "default_server_version", alias = "server-version")]
    pub server_version: String,
    #[serde(
        default = "default_azalea_version",
        alias = "azalea-version",
        alias = "azaleaVersion"
    )]
    pub azalea_version: String,
    #[serde(default, alias = "enable-viaversion", alias = "enableViaVersion")]
    pub enable_viaversion: bool,
    #[serde(
        default = "default_viaversion_target_version",
        alias = "viaversion-target-version",
        alias = "viaVersionTargetVersion"
    )]
    pub viaversion_target_version: String,
    #[serde(default, alias = "disable-chat-signing")]
    #[serde(rename = "disableChatSigning")]
    pub disable_chat_signing: bool,
    pub api_url: String,
    pub websocket_url: String,

    #[serde(rename = "useLogger")]
    #[allow(dead_code)]
    pub use_logger: bool,
    pub prefix: String,
    #[serde(rename = "useCustomChatPrefix")]
    pub use_custom_chat_prefix: bool,
    #[serde(rename = "customChatPrefix")]
    pub custom_chat_prefix: String,
    #[serde(rename = "whisperCommand")]
    pub whisper_command: String,
    #[allow(dead_code)]
    pub announce: bool,
    #[allow(dead_code)]
    pub antiafk: bool,
    pub use_mc_whitelist: bool,
    pub reconnect_time: u64,
    #[allow(dead_code)]
    pub anti_spam_cooldown: u64,
    #[serde(default)]
    pub anti_spam_global_cooldown: u64,
    #[allow(dead_code)]
    pub anti_spam_msg_limit: u32,
    #[serde(default)]
    pub command_cooldowns: HashMap<String, CommandCooldownConfig>,
    pub welcome_messages: bool,
    #[serde(rename = "useCommands")]
    pub use_commands: bool,
    pub disabled_events: Vec<String>,
    pub allow_chatbridge_input: bool,
    #[serde(rename = "rotateHeadOnJoin")]
    #[allow(dead_code)]
    pub rotate_head_on_join: bool,
    pub smart_censoring: bool,
    pub together_api_key: String,
    #[serde(default)]
    pub wolfram_app_id: String,
    #[serde(rename = "useLegacyChat")]
    #[allow(dead_code)]
    pub use_legacy_chat: bool,
    #[serde(rename = "useCustomChatFormatParser")]
    pub use_custom_chat_format_parser: bool,
    #[serde(rename = "customChatFormats")]
    pub custom_chat_formats: Vec<String>,
    pub commands: HashMap<String, bool>,
    #[serde(rename = "usePViewer")]
    #[allow(dead_code)]
    pub use_p_viewer: bool,
    #[serde(rename = "pViewerPort")]
    #[allow(dead_code)]
    pub p_viewer_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandCooldownConfig {
    #[serde(default)]
    pub cooldown_ms: u64,
    #[serde(default)]
    pub increment_ms: u64,
    #[serde(default = "default_cooldown_reset_multiplier")]
    pub reset_multiplier: u64,
    #[serde(default)]
    pub max_cooldown_ms: u64,
}

fn default_cooldown_reset_multiplier() -> u64 {
    2
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Colors {
    pub red: String,
    pub green: String,
    pub purple: String,
    pub yellow: String,
    pub gray: String,
    pub pink: String,
    pub blue: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserList {
    users: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WordList {
    words: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineMessage {
    pub sender: String,
    pub recipient: String,
    pub message: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct BotConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub version: String,
    pub server_version: String,
    pub azalea_version: String,
    pub enable_viaversion: bool,
    pub viaversion_target_version: String,
    pub disable_chat_signing: bool,
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
    #[allow(dead_code)]
    pub colors: Colors,
    pub config: Config,
    pub mc_whitelist: Vec<String>,
    pub mc_blacklist: Vec<String>,
}

pub async fn load_offline_messages() -> Result<Vec<OfflineMessage>> {
    read_json("./json/offline_messages.json").await
}

pub async fn save_offline_messages(messages: &[OfflineMessage]) -> Result<()> {
    let data = serde_json::to_string_pretty(messages)?;
    fs::write("./json/offline_messages.json", data)
        .await
        .context("Failed to write ./json/offline_messages.json")
}

pub async fn load_user_list(path: &str) -> Result<Vec<String>> {
    Ok(read_json::<UserList>(path).await?.users)
}

pub async fn save_user_list(path: &str, users: &[String]) -> Result<()> {
    let data = serde_json::to_string_pretty(&UserList {
        users: users.to_vec(),
    })?;
    fs::write(path, data)
        .await
        .with_context(|| format!("Failed to write {path}"))
}

pub async fn load_word_list(path: &str) -> Result<Vec<String>> {
    Ok(read_json::<WordList>(path).await?.words)
}

pub async fn save_word_list(path: &str, words: &[String]) -> Result<()> {
    let data = serde_json::to_string_pretty(&WordList {
        words: words.to_vec(),
    })?;
    fs::write(path, data)
        .await
        .with_context(|| format!("Failed to write {path}"))
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
        require_env_any(&["API_KEY", "apiKey"])?;

        Ok(Self {
            colors,
            config,
            mc_whitelist: whitelist.users,
            mc_blacklist: blacklist.users,
        })
    }

    #[allow(dead_code)]
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
                server_version: self.config.server_version.clone(),
                azalea_version: self.config.azalea_version.clone(),
                enable_viaversion: self.config.enable_viaversion,
                viaversion_target_version: self.config.viaversion_target_version.clone(),
                disable_chat_signing: self.config.disable_chat_signing,
            },

            api: ApiConfig {
                api_url: self.config.api_url.clone(),
                websocket_url: self.config.websocket_url.clone(),
                api_key: env_any(&["API_KEY", "apiKey"])?,
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

    serde_json::from_str(&data).with_context(|| format!("Failed to parse JSON from {path}"))
}

fn require_env(name: &str) -> Result<String> {
    env::var(name)
        .with_context(|| format!("Missing environment variable `{name}`. Check your .env/config."))
}

fn env_any(names: &[&str]) -> Result<String> {
    names
        .iter()
        .find_map(|name| env::var(name).ok())
        .with_context(|| format!("Missing one of these environment variables: {names:?}"))
}

fn require_env_any(names: &[&str]) -> Result<String> {
    env_any(names)
}
