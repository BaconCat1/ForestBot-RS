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

fn default_heartbeat_interval_ms() -> u64 {
    60_000
}

fn default_queue_retry_delay_ms() -> u64 {
    300_000
}

fn default_board_whisper_delay_ms() -> u64 {
    1_000
}

fn default_announce_min_interval_ms() -> u64 {
    900_000
}

fn default_announce_max_interval_ms() -> u64 {
    2_700_000
}

fn default_censor_threshold() -> String {
    "moderate".to_owned()
}

fn default_duplicate_message_window_ms() -> u64 {
    5_000
}

fn default_afk_mention_cooldown_ms() -> u64 {
    60_000
}

fn default_connection_failure_backoff_ms() -> u64 {
    600_000
}

fn default_packet_send_delay_ms() -> u64 {
    25
}

fn default_entity_spawn_greeting_ttl_ms() -> u64 {
    500_000
}

fn default_player_detection_cooldown_ms() -> u64 {
    600_000
}

fn default_smart_censor_timeout_ms() -> u64 {
    5_000
}

fn default_ws_response_timeout_ms() -> u64 {
    5_000
}

fn default_player_list_update_interval_ms() -> u64 {
    60_000
}

fn default_reminder_tick_interval_ms() -> u64 {
    30_000
}

fn default_crouch_max_hold_ms() -> u64 {
    600_000
}

fn default_crouch_toggle_delay_ms() -> u64 {
    50
}

fn default_poll_duration_ms() -> u64 {
    120_000
}

fn default_duel_confirm_window_ms() -> u64 {
    60_000
}

fn default_duel_timeout_ms() -> u64 {
    600_000
}

fn default_marry_confirm_window_ms() -> u64 {
    60_000
}

fn default_trade_propose_cooldown_ms() -> u64 {
    60_000
}

fn default_trade_reject_penalty_ms() -> u64 {
    600_000
}

fn default_roast_timeout_ms() -> u64 {
    8_000
}

fn default_scratch_animation_delay_ms() -> u64 {
    600
}

fn default_slots_animation_delay_ms() -> u64 {
    800
}

fn default_twerk_flash_delay_ms() -> u64 {
    100
}

fn default_market_quote_ttl_ms() -> u64 {
    60_000
}

fn default_market_history_ttl_ms() -> u64 {
    300_000
}

fn default_market_search_ttl_ms() -> u64 {
    86_400_000
}

fn default_market_api_timeout_ms() -> u64 {
    10_000
}

fn default_url_blocklist_timeout_ms() -> u64 {
    30_000
}

fn default_websocket_keepalive_ms() -> u64 {
    5_000
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ApiKeys {
    #[serde(default)]
    pub nasa: String,
    #[serde(default)]
    pub together: String,
    #[serde(default)]
    pub wolfram: String,
    #[serde(default)]
    pub azure_key: String,
    #[serde(default)]
    pub azure_region: String,
    #[serde(default)]
    pub sharpapi: String,
    #[serde(default)]
    pub airnow: String,
    #[serde(default)]
    pub coingecko: String,
    #[serde(default)]
    pub gasbuddy_solver_url: String,
    #[serde(default)]
    pub gasbuddy_csrf_readonly: bool,
    #[serde(default)]
    pub google_safe_browsing: String,
    #[serde(default)]
    pub ai_gemini: String,
    #[serde(default)]
    pub ai_github_models: String,
    #[serde(default)]
    pub ai_groq: String,
    #[serde(default)]
    pub ai_cerebras: String,
    #[serde(default)]
    pub ai_ollama_cloud: String,
    #[serde(default)]
    pub ai_mistral: String,
    #[serde(default)]
    pub ai_nvidia_nim: String,
    #[serde(default)]
    pub ai_nscale: String,
    #[serde(default)]
    pub ai_sambanova: String,
    #[serde(default)]
    pub ai_modelscope: String,
    #[serde(default)]
    pub ai_chutes: String,
    #[serde(default)]
    pub ai_venice: String,
    #[serde(default)]
    pub ai_zhipu: String,
    #[serde(default)]
    pub ai_siliconflow: String,
    #[serde(default)]
    pub ai_openrouter: String,
    #[serde(default)]
    pub ai_cohere: String,
    #[serde(default)]
    pub ai_cloudflare_key: String,
    #[serde(default)]
    pub ai_cloudflare_account_id: String,
    #[serde(default)]
    pub ai_huggingface: String,
    #[serde(default)]
    pub ai_deepseek: String,
    #[serde(default)]
    pub ai_llm7: String,
    #[serde(default)]
    pub ai_nebius: String,
    #[serde(default)]
    pub ai_glhf: String,
    #[serde(default)]
    pub ai_ovhcloud: String,
    #[serde(default)]
    pub ai_kilo_code: String,
    #[serde(default)]
    pub ai_agnes_ai: String,
    #[serde(default)]
    pub ai_opencode_zen: String,
    #[serde(default)]
    pub ai_alibaba: String,
    #[serde(default)]
    pub ai_xai: String,
}

impl ApiKeys {
    pub fn get_ai_key(&self, field: &str) -> &str {
        match field {
            "ai_gemini" => &self.ai_gemini,
            "ai_github_models" => &self.ai_github_models,
            "ai_groq" => &self.ai_groq,
            "ai_cerebras" => &self.ai_cerebras,
            "ai_ollama_cloud" => &self.ai_ollama_cloud,
            "ai_mistral" => &self.ai_mistral,
            "ai_nvidia_nim" => &self.ai_nvidia_nim,
            "ai_nscale" => &self.ai_nscale,
            "ai_sambanova" => &self.ai_sambanova,
            "ai_modelscope" => &self.ai_modelscope,
            "ai_chutes" => &self.ai_chutes,
            "ai_venice" => &self.ai_venice,
            "ai_zhipu" => &self.ai_zhipu,
            "ai_siliconflow" => &self.ai_siliconflow,
            "ai_openrouter" => &self.ai_openrouter,
            "ai_cohere" => &self.ai_cohere,
            "ai_cloudflare_key" => &self.ai_cloudflare_key,
            "ai_huggingface" => &self.ai_huggingface,
            "ai_deepseek" => &self.ai_deepseek,
            "ai_llm7" => &self.ai_llm7,
            "ai_nebius" => &self.ai_nebius,
            "ai_glhf" => &self.ai_glhf,
            "ai_ovhcloud" => &self.ai_ovhcloud,
            "ai_kilo_code" => &self.ai_kilo_code,
            "ai_agnes_ai" => &self.ai_agnes_ai,
            "ai_opencode_zen" => &self.ai_opencode_zen,
            "ai_alibaba" => &self.ai_alibaba,
            "ai_xai" => &self.ai_xai,
            _ => "",
        }
    }
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
    pub reconnect_time_ms: u64,
    #[allow(dead_code)]
    pub anti_spam_cooldown_ms: u64,
    #[serde(default)]
    pub anti_spam_global_cooldown_ms: u64,
    #[allow(dead_code)]
    pub anti_spam_msg_limit: u32,
    #[serde(default)]
    pub command_cooldowns: HashMap<String, CommandCooldownConfig>,
    pub welcome_messages: bool,
    #[serde(rename = "useCommands")]
    pub use_commands: bool,
    pub disabled_events: Vec<String>,
    pub allow_chatbridge_input: bool,
    #[serde(default)]
    pub use_live_time_query: bool,
    #[serde(rename = "rotateHeadOnJoin")]
    #[allow(dead_code)]
    pub rotate_head_on_join: bool,
    pub smart_censoring: bool,
    // Minimum rustrict severity that gets censored in outbound chat: "mild", "moderate", or
    // "severe". Data-driven so it can be tuned without a recompile -- see profanity_filter.rs's
    // censor_threshold_from_config().
    #[serde(default = "default_censor_threshold")]
    pub censor_threshold: String,
    #[serde(default)]
    pub api_keys: ApiKeys,
    #[serde(rename = "useLegacyChat")]
    #[allow(dead_code)]
    pub use_legacy_chat: bool,
    #[serde(rename = "useCustomChatFormatParser")]
    pub use_custom_chat_format_parser: bool,
    #[serde(rename = "customChatFormats")]
    pub custom_chat_formats: Vec<String>,
    pub commands: HashMap<String, bool>,
    #[serde(default)]
    pub url_blocklist_sources: Vec<String>,
    #[serde(default)]
    pub url_whitelist_file: String,
    #[serde(rename = "usePViewer")]
    #[allow(dead_code)]
    pub use_p_viewer: bool,
    #[serde(rename = "pViewerPort")]
    #[allow(dead_code)]
    pub p_viewer_port: u16,
    #[serde(default)]
    pub heartbeat_url: String,
    #[serde(default = "default_heartbeat_interval_ms")]
    pub heartbeat_interval_ms: u64,
    // Command only the real server recognizes (e.g. "/lag") -- used to detect when the
    // bot has landed on a queue proxy instead of the real server. Empty disables the
    // whole queue-detection feature.
    #[serde(default)]
    pub queue_probe_command: String,
    #[serde(default = "default_queue_retry_delay_ms")]
    pub queue_retry_delay_ms: u64,
    // Gap between whisper lines when sending a multi-line game board (battleship,
    // checkers, chess, connect four, mines, reversi, wordle). Tune per target server --
    // RV's anti-spam filter kicks the bot on rapid-fire whispers at the default queue
    // drain rate (~1 msg/tick).
    #[serde(default = "default_board_whisper_delay_ms")]
    pub board_whisper_delay_ms: u64,
    // Announce loop picks a random wait in [min, max) before each command-usage tip.
    // Defaults match the original hardcoded 15-45min range.
    #[serde(default = "default_announce_min_interval_ms")]
    pub announce_min_interval_ms: u64,
    #[serde(default = "default_announce_max_interval_ms")]
    pub announce_max_interval_ms: u64,

    // Config-driven-gating scan (2026-07-19/20, todo.md:114) -- hardcoded timing/threshold
    // constants surfaced by grep, moved here so ops can tune per-server without a recompile.
    // All timing fields are milliseconds, no _secs fields anywhere in this struct.
    #[serde(default = "default_duplicate_message_window_ms")]
    pub duplicate_message_window_ms: u64,
    #[serde(default = "default_afk_mention_cooldown_ms")]
    pub afk_mention_cooldown_ms: u64,
    #[serde(default = "default_connection_failure_backoff_ms")]
    pub connection_failure_backoff_ms: u64,
    #[serde(default = "default_packet_send_delay_ms")]
    pub packet_send_delay_ms: u64,
    #[serde(default = "default_entity_spawn_greeting_ttl_ms")]
    pub entity_spawn_greeting_ttl_ms: u64,
    #[serde(default = "default_player_detection_cooldown_ms")]
    pub player_detection_cooldown_ms: u64,
    #[serde(default = "default_smart_censor_timeout_ms")]
    pub smart_censor_timeout_ms: u64,
    // Shared by run_queue_probe and resolve_and_check_bridge_sender -- both are bounded
    // waits on a oneshot channel for a websocket round trip, same structural role.
    #[serde(default = "default_ws_response_timeout_ms")]
    pub ws_response_timeout_ms: u64,
    #[serde(default = "default_player_list_update_interval_ms")]
    pub player_list_update_interval_ms: u64,
    #[serde(default = "default_reminder_tick_interval_ms")]
    pub reminder_tick_interval_ms: u64,
    #[serde(default = "default_crouch_max_hold_ms")]
    pub crouch_max_hold_ms: u64,
    #[serde(default = "default_crouch_toggle_delay_ms")]
    pub crouch_toggle_delay_ms: u64,
    #[serde(default = "default_poll_duration_ms")]
    pub poll_duration_ms: u64,
    #[serde(default = "default_duel_confirm_window_ms")]
    pub duel_confirm_window_ms: u64,
    #[serde(default = "default_duel_timeout_ms")]
    pub duel_timeout_ms: u64,
    #[serde(default = "default_marry_confirm_window_ms")]
    pub marry_confirm_window_ms: u64,
    #[serde(default = "default_trade_propose_cooldown_ms")]
    pub trade_propose_cooldown_ms: u64,
    #[serde(default = "default_trade_reject_penalty_ms")]
    pub trade_reject_penalty_ms: u64,
    #[serde(default = "default_roast_timeout_ms")]
    pub roast_timeout_ms: u64,
    #[serde(default = "default_scratch_animation_delay_ms")]
    pub scratch_animation_delay_ms: u64,
    #[serde(default = "default_slots_animation_delay_ms")]
    pub slots_animation_delay_ms: u64,
    #[serde(default = "default_twerk_flash_delay_ms")]
    pub twerk_flash_delay_ms: u64,
    // Constructor-time only (MarketService::new/Cache::new, build_blocklist,
    // WebsocketClient::connect) -- not in RuntimeConfig since nothing re-reads them
    // per-message, only once at startup.
    #[serde(default = "default_market_quote_ttl_ms")]
    pub market_quote_ttl_ms: u64,
    #[serde(default = "default_market_history_ttl_ms")]
    pub market_history_ttl_ms: u64,
    #[serde(default = "default_market_search_ttl_ms")]
    pub market_search_ttl_ms: u64,
    #[serde(default = "default_market_api_timeout_ms")]
    pub market_api_timeout_ms: u64,
    #[serde(default = "default_url_blocklist_timeout_ms")]
    pub url_blocklist_timeout_ms: u64,
    #[serde(default = "default_websocket_keepalive_ms")]
    pub websocket_keepalive_ms: u64,
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

fn default_censor_flag() -> bool {
    true
}

// Per-command opt-out of profanity censoring, keyed by the command's canonical (first)
// name. `success` covers a command's normal/computed output; `error` covers messages that
// echo back raw/unresolved player input (e.g. "no user found matching X"). Kept as two
// separate flags rather than one, since a command's success output can be pure computed
// data (safe to bypass) while its error path still echoes untrusted input (must stay
// censored) -- see json/commands_censorship.json. Missing command or missing key defaults
// to censored (true), so anything not explicitly reviewed fails safe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandCensorship {
    #[serde(default = "default_censor_flag")]
    pub success: bool,
    #[serde(default = "default_censor_flag")]
    pub error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineMessage {
    pub sender: String,
    pub recipient: String,
    pub message: String,
    pub timestamp: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deliver_at: Option<u64>,
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
    pub websocket_keepalive_ms: u64,
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
    pub command_censorship: HashMap<String, CommandCensorship>,
}

async fn merge_config_from_example() -> Result<()> {
    let config_str = fs::read_to_string("./config.json")
        .await
        .context("Failed to read ./config.json")?;

    let example_str = match fs::read_to_string("./example.config.json").await {
        Ok(s) => s,
        Err(_) => return Ok(()),
    };

    let mut config: serde_json::Value =
        serde_json::from_str(&config_str).context("Failed to parse ./config.json")?;
    let example: serde_json::Value =
        serde_json::from_str(&example_str).context("Failed to parse ./example.config.json")?;

    let mut added: Vec<String> = Vec::new();
    merge_missing(&mut config, &example, "", &mut added);

    let mut removed: Vec<String> = Vec::new();
    prune_extra(&mut config, &example, "", &mut removed);

    if !added.is_empty() || !removed.is_empty() {
        if !added.is_empty() {
            println!(
                "[config] Auto-merged {} missing key(s) from example.config.json: {}",
                added.len(),
                added.join(", ")
            );
        }
        if !removed.is_empty() {
            println!(
                "[config] Pruned {} key(s) not present in example.config.json: {}",
                removed.len(),
                removed.join(", ")
            );
        }
        let updated = serde_json::to_string_pretty(&config)?;
        fs::write("./config.json", updated)
            .await
            .context("Failed to write ./config.json")?;
    }

    Ok(())
}

fn merge_missing(
    target: &mut serde_json::Value,
    source: &serde_json::Value,
    prefix: &str,
    added: &mut Vec<String>,
) {
    let (Some(target_obj), Some(source_obj)) = (target.as_object_mut(), source.as_object()) else {
        return;
    };
    for (key, value) in source_obj {
        let full_key = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        if !target_obj.contains_key(key) {
            target_obj.insert(key.clone(), value.clone());
            added.push(full_key);
        } else if value.is_object() {
            merge_missing(target_obj.get_mut(key).unwrap(), value, &full_key, added);
        }
    }
}

// example.config.json is the schema authority -- any key present in config.json but not
// there is either stale (renamed/removed field) or was never a real Config field to begin
// with, so it's dropped rather than left inert forever (config.rs's Deserialize isn't
// deny_unknown_fields, so orphaned keys would otherwise sit silently unused). Only
// descends into keys present on both sides so it can prune nested extras (e.g. an orphaned
// api_keys entry) without touching arrays or scalars, mirroring merge_missing's shape.
fn prune_extra(
    target: &mut serde_json::Value,
    source: &serde_json::Value,
    prefix: &str,
    removed: &mut Vec<String>,
) {
    let (Some(target_obj), Some(source_obj)) = (target.as_object_mut(), source.as_object()) else {
        return;
    };
    let extra_keys: Vec<String> = target_obj
        .keys()
        .filter(|key| !source_obj.contains_key(*key))
        .cloned()
        .collect();
    for key in extra_keys {
        let full_key = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        target_obj.remove(&key);
        removed.push(full_key);
    }
    for (key, source_value) in source_obj {
        if !source_value.is_object() {
            continue;
        }
        if let Some(target_value) = target_obj.get_mut(key) {
            let full_key = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            prune_extra(target_value, source_value, &full_key, removed);
        }
    }
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

        merge_config_from_example().await?;

        let colors: Colors = read_json("./json/colors.json").await?;
        let config: Config = read_json("./config.json").await?;
        let whitelist: UserList = read_json("./json/mc_whitelist.json").await?;
        let blacklist: UserList = read_json("./json/mc_blacklist.json").await?;
        let command_censorship: HashMap<String, CommandCensorship> =
            read_json("./json/commands_censorship.json").await?;

        require_env("MC_USER")?;
        require_env("MC_PASS")?;
        require_env_any(&["API_KEY", "apiKey"])?;

        Ok(Self {
            colors,
            config,
            mc_whitelist: whitelist.users,
            mc_blacklist: blacklist.users,
            command_censorship,
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
        self.command_censorship = read_json("./json/commands_censorship.json").await?;

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
                websocket_keepalive_ms: self.config.websocket_keepalive_ms,
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
