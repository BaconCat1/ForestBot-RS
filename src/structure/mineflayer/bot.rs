use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Context;
use chrono::Utc;
use azalea::ClientInformation;
use azalea::protocol::packets::game::ClientboundGamePacket;
use azalea::app::PluginGroup;
use azalea::bot::DefaultBotPlugins;
use azalea::chat_signing::ChatSigningPlugin;
use azalea::client_chat::ChatPacket;
use azalea::prelude::*;
use azalea_viaversion::ViaVersionPlugin;
use rustrict::Trie;
use serde_json::json;
use uuid::Uuid;

use crate::commands::stat_history;
use crate::config::{
    AppState, BotConfig, CommandCooldownConfig, load_offline_messages,
    save_offline_messages,
};
use crate::structure::{
    endpoints::endpoints::{
        ApiClient, DiscordChatMessage, FadvAwardsEvent, MinecraftAdvancementMessage, MinecraftChatMessage,
        MinecraftPlayerDeathMessage, MinecraftPlayerJoinMessage, MinecraftPlayerLeaveMessage,
        Player as WebsocketPlayer, WebsocketClient, WebsocketEvent,
    },
    logger,
    mineflayer::utils::{announce, anti_afk, chat_format_parser, command_handler, profanity_filter, whisper_parser},
};

const TOGETHER_MODEL: &str = "ServiceNow-AI/Apriel-1.6-15b-Thinker";
const SMART_CENSOR_TIMEOUT_MS: u64 = 5_000;
const SMART_CENSOR_MAX_INPUT_LENGTH: usize = 280;
const SMART_CENSOR_MAX_OUTPUT_LENGTH: usize = 280;

static WARNED_MISSING_TOGETHER_KEY: AtomicBool = AtomicBool::new(false);
static WARNED_SMART_CENSOR_FAILURE: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone)]
pub struct Command {
    #[allow(dead_code)]
    pub names: Vec<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Player {
    pub username: String,
    pub uuid: String,
    pub latency: i32,
    pub server: String,
}

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub prefix: String,
    pub whisper_command: String,
    pub use_commands: bool,
    pub anti_spam_global_cooldown_ms: u64,
    pub command_cooldowns: HashMap<String, CommandCooldownConfig>,
    pub use_whitelist: bool,
    pub user_whitelist: HashSet<String>,
    pub user_blacklist: HashSet<String>,
    pub custom_chat_formats: Vec<String>,
    pub command_toggles: HashMap<String, bool>,
    pub disabled_events: HashSet<String>,
    pub allow_chatbridge_input: bool,
    pub use_live_time_query: bool,
    pub welcome_messages: bool,
    pub use_custom_chat_prefix: bool,
    pub custom_chat_prefix: String,
    pub smart_censoring: bool,
    pub censor_threshold: String,
    pub command_censorship: HashMap<String, crate::config::CommandCensorship>,
    pub together_api_key: String,
    pub wolfram_app_id: String,
    pub azure_translator_key: String,
    pub azure_translator_region: String,
    pub sharpapi_key: String,
    pub nasa_api_key: String,
    pub airnow_api_key: String,
    pub gasbuddy_solver_url: String,
    pub gasbuddy_csrf_readonly: bool,
    pub google_safe_browsing_key: String,
    pub queue_probe_command: String,
    pub queue_retry_delay_ms: u64,
}

#[derive(Debug, Clone)]
pub struct PlayerSnapshot {
    pub username: String,
    pub uuid: String,
    pub entity_uuid: Uuid,
    pub latency: i32,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Bot {
    pub options: BotConfig,
    pub use_whitelist: bool,
    pub welcome_messages: bool,
    pub mc_server: String,
    pub user_whitelist: HashSet<String>,
    pub user_blacklist: HashSet<String>,
    #[allow(dead_code)]
    pub commands: HashMap<String, Command>,
    pub command_toggles: HashMap<String, bool>,
    pub disabled_events: HashSet<String>,
    pub use_commands: bool,
    pub prefix: String,
    pub whisper_command: String,
    pub custom_chat_formats: Vec<String>,
    pub use_custom_chat_prefix: bool,
    pub custom_chat_prefix: String,
    pub allow_chatbridge_input: bool,
    pub use_live_time_query: bool,
    pub smart_censoring: bool,
    pub censor_threshold: String,
    pub command_censorship: HashMap<String, crate::config::CommandCensorship>,
    pub together_api_key: String,
    pub wolfram_app_id: String,
    pub azure_translator_key: String,
    pub azure_translator_region: String,
    pub sharpapi_key: String,
    pub nasa_api_key: String,
    pub airnow_api_key: String,
    pub coingecko_api_key: String,
    pub gasbuddy_solver_url: String,
    pub gasbuddy_csrf_readonly: bool,
    pub google_safe_browsing_key: String,
    pub url_blocklist_sources: Vec<String>,
    pub url_whitelist_file: String,
    pub anti_spam_global_cooldown_ms: u64,
    pub command_cooldowns: HashMap<String, CommandCooldownConfig>,
    pub reconnect_time_ms: u64,
    pub restart_count: u32,
    #[allow(dead_code)]
    pub is_connected: bool,
    pub allow_connection: bool,
    pub api: ApiClient,
    pub antiafk: bool,
    pub announce: bool,
    pub heartbeat_url: String,
    pub heartbeat_interval_ms: u64,
    pub api_keys: crate::config::ApiKeys,
    pub queue_probe_command: String,
    pub queue_retry_delay_ms: u64,
}

impl Bot {
    pub fn new(options: BotConfig, state: &AppState, api: ApiClient) -> Self {
        Self {
            options,
            use_whitelist: state.config.use_mc_whitelist,
            welcome_messages: state.config.welcome_messages,
            mc_server: state.config.mc_server.clone(),
            user_whitelist: state.mc_whitelist.iter().cloned().collect(),
            user_blacklist: state.mc_blacklist.iter().cloned().collect(),
            commands: HashMap::new(),
            command_toggles: state.config.commands.clone(),
            disabled_events: state.config.disabled_events.iter().cloned().collect(),
            use_commands: state.config.use_commands,
            prefix: state.config.prefix.clone(),
            whisper_command: state.config.whisper_command.clone(),
            custom_chat_formats: if state.config.use_custom_chat_format_parser {
                state.config.custom_chat_formats.clone()
            } else {
                Vec::new()
            },
            use_custom_chat_prefix: state.config.use_custom_chat_prefix,
            custom_chat_prefix: state.config.custom_chat_prefix.clone(),
            allow_chatbridge_input: state.config.allow_chatbridge_input,
            use_live_time_query: state.config.use_live_time_query,
            smart_censoring: state.config.smart_censoring,
            censor_threshold: state.config.censor_threshold.clone(),
            command_censorship: state.command_censorship.clone(),
            together_api_key: state.config.api_keys.together.clone(),
            wolfram_app_id: state.config.api_keys.wolfram.clone(),
            azure_translator_key: state.config.api_keys.azure_key.clone(),
            azure_translator_region: state.config.api_keys.azure_region.clone(),
            sharpapi_key: state.config.api_keys.sharpapi.clone(),
            nasa_api_key: state.config.api_keys.nasa.clone(),
            airnow_api_key: state.config.api_keys.airnow.clone(),
            coingecko_api_key: state.config.api_keys.coingecko.clone(),
            gasbuddy_solver_url: state.config.api_keys.gasbuddy_solver_url.clone(),
            gasbuddy_csrf_readonly: state.config.api_keys.gasbuddy_csrf_readonly,
            google_safe_browsing_key: state.config.api_keys.google_safe_browsing.clone(),
            url_blocklist_sources: state.config.url_blocklist_sources.clone(),
            url_whitelist_file: state.config.url_whitelist_file.clone(),
            anti_spam_global_cooldown_ms: state.config.anti_spam_global_cooldown,
            command_cooldowns: state.config.command_cooldowns.clone(),
            reconnect_time_ms: state.config.reconnect_time,
            restart_count: 0,
            is_connected: false,
            allow_connection: true,
            api,
            antiafk: state.config.antiafk,
            announce: state.config.announce,
            heartbeat_url: state.config.heartbeat_url.clone(),
            heartbeat_interval_ms: state.config.heartbeat_interval_ms,
            api_keys: state.config.api_keys.clone(),
            queue_probe_command: state.config.queue_probe_command.clone(),
            queue_retry_delay_ms: state.config.queue_retry_delay_ms,
        }
    }

    pub async fn start(&mut self) -> anyhow::Result<()> {
        if !self.allow_connection {
            logger::warn("Connection skipped because allow_connection is false.");
            return Ok(());
        }

        self.restart_count += 1;
        logger::login(format!(
            "Starting Azalea bot for {} on {}:{} using configured server version {}",
            self.options.username,
            self.options.host,
            self.options.port,
            self.options.server_version
        ));
        logger::login(format!(
            "Azalea protocol crate target: {}",
            self.options.azalea_version
        ));

        let account = Account::microsoft(&self.options.username)
            .await
            .context("Failed to authenticate Microsoft account with Azalea")?;
        let address = format!("{}:{}", self.options.host, self.options.port);
        let ai_providers = crate::commands::ai::load_ai_providers(
            "json/ai_providers.json",
            &self.api_keys,
        ).await;
        let bridge_unsafe_commands = crate::commands::load_bridge_unsafe_commands(
            "json/bridge_unsafe_commands.json",
        ).await;
        let state = AzaleaState {
            mc_server: self.mc_server.clone(),
            api: Arc::new(self.api.clone()),
            runtime: Arc::new(RwLock::new(RuntimeConfig {
                prefix: self.prefix.clone(),
                whisper_command: self.whisper_command.clone(),
                use_commands: self.use_commands,
                anti_spam_global_cooldown_ms: self.anti_spam_global_cooldown_ms,
                command_cooldowns: self.command_cooldowns.clone(),
                use_whitelist: self.use_whitelist,
                user_whitelist: self.user_whitelist.clone(),
                user_blacklist: self.user_blacklist.clone(),
                custom_chat_formats: self.custom_chat_formats.clone(), // already gated by use_custom_chat_format_parser in Bot::new()
                command_toggles: self.command_toggles.clone(),
                disabled_events: self.disabled_events.clone(),
                allow_chatbridge_input: self.api.options.use_websocket
                    && self.allow_chatbridge_input,
                use_live_time_query: self.use_live_time_query,
                welcome_messages: self.welcome_messages,
                use_custom_chat_prefix: self.use_custom_chat_prefix,
                custom_chat_prefix: self.custom_chat_prefix.clone(),
                smart_censoring: self.smart_censoring,
                censor_threshold: self.censor_threshold.clone(),
                command_censorship: self.command_censorship.clone(),
                together_api_key: self.together_api_key.clone(),
                wolfram_app_id: self.wolfram_app_id.clone(),
                azure_translator_key: self.azure_translator_key.clone(),
                azure_translator_region: self.azure_translator_region.clone(),
                sharpapi_key: self.sharpapi_key.clone(),
                nasa_api_key: self.nasa_api_key.clone(),
                airnow_api_key: self.airnow_api_key.clone(),
                gasbuddy_solver_url: self.gasbuddy_solver_url.clone(),
                gasbuddy_csrf_readonly: self.gasbuddy_csrf_readonly,
                google_safe_browsing_key: self.google_safe_browsing_key.clone(),
                queue_probe_command: self.queue_probe_command.clone(),
                queue_retry_delay_ms: self.queue_retry_delay_ms,
            })),
            players: Arc::new(RwLock::new(HashMap::new())),
            outbound_chat: Arc::new(Mutex::new(VecDeque::new())),
            background_tasks_started: Arc::new(Mutex::new(false)),
            last_command_at: Arc::new(Mutex::new(None)),
            player_command_cooldowns: Arc::new(Mutex::new(HashMap::new())),
            seen_entity_spawns: Arc::new(RwLock::new(HashSet::new())),
            next_entity_spawn_scan_at: Arc::new(RwLock::new(0)),
            seen_player_detections: Arc::new(RwLock::new(HashSet::new())),
            next_player_detection_scan_at: Arc::new(RwLock::new(0)),
            trade_cooldowns: Arc::new(Mutex::new(HashMap::new())),
            free_scratch_cooldowns: Arc::new(Mutex::new(HashMap::new())),
            initial_spawn_done: Arc::new(AtomicBool::new(false)),
            antiafk: self.antiafk,
            antiafk_active: Arc::new(AtomicBool::new(false)),
            announce: self.announce,
            announce_active: Arc::new(AtomicBool::new(false)),
            reminder_active: Arc::new(AtomicBool::new(false)),
            consecutive_failures: Arc::new(AtomicU32::new(0)),
            seen_advancements: Arc::new(Mutex::new(HashMap::new())),
            nick_cache: Arc::new(RwLock::new(HashMap::new())),
            world_time_ticks: Arc::new(RwLock::new(0)),
            active_trivia: Arc::new(Mutex::new(None)),
            casino_sessions: Arc::new(Mutex::new(HashMap::new())),
            market_service: Arc::new(crate::structure::market::service::MarketService::new(self.coingecko_api_key.clone())),
            market_bets: Arc::new(Mutex::new(HashMap::new())),
            portfolio_positions: Arc::new(Mutex::new(HashMap::new())),
            weather_bets: Arc::new(Mutex::new(HashMap::new())),
            weather_odds_cache: Arc::new(Mutex::new(HashMap::new())),
            sports_bets: Arc::new(Mutex::new(HashMap::new())),
            sports_cache: Arc::new(Mutex::new(crate::commands::casino::sports::SportsCache::default())),
            kalshi_bets: Arc::new(Mutex::new(HashMap::new())),
            kalshi_cache: Arc::new(Mutex::new(crate::commands::casino::kalshi::KalshiCache::default())),
            nasa_space_weather_bets: Arc::new(Mutex::new(HashMap::new())),
            sw_odds_cache: Arc::new(Mutex::new(None)),
            faa_airport_bets: Arc::new(Mutex::new(HashMap::new())),
            noaa_flooding_bets: Arc::new(Mutex::new(HashMap::new())),
            flood_cache: Arc::new(Mutex::new(crate::commands::casino::noaa_flooding::FloodCache::default())),
            train_bets: Arc::new(Mutex::new(HashMap::new())),
            quake_bets: Arc::new(Mutex::new(HashMap::new())),
            volcano_bets: Arc::new(Mutex::new(HashMap::new())),
            duels: Arc::new(Mutex::new(Vec::new())),
            wordle_games: Arc::new(Mutex::new(HashMap::new())),
            checkers_games: Arc::new(Mutex::new(HashMap::new())),
            reversi_games: Arc::new(Mutex::new(HashMap::new())),
            battleship_games: Arc::new(Mutex::new(HashMap::new())),
            mines_games: Arc::new(Mutex::new(HashMap::new())),
            aqi_bets: Arc::new(Mutex::new(HashMap::new())),
            launch_bets: Arc::new(Mutex::new(HashMap::new())),
            launch_cache: Arc::new(Mutex::new(HashMap::new())),
            gas_bets: Arc::new(Mutex::new(HashMap::new())),
            gas_price_cache: Arc::new(Mutex::new(HashMap::new())),
            gasbuddy_csrf: Arc::new(Mutex::new(None)),
            http: reqwest::Client::new(),
            url_blocklist: Arc::new(RwLock::new(None)),
            tps_time_samples: Arc::new(Mutex::new(VecDeque::new())),
            afk_messages: Arc::new(RwLock::new(HashMap::new())),
            afk_cooldowns: Arc::new(Mutex::new(HashMap::new())),
            recent_whispers: Arc::new(Mutex::new(HashMap::new())),
            active_poll: Arc::new(Mutex::new(None)),
            ai_providers: Arc::new(RwLock::new(ai_providers)),
            ai_model_cache: Arc::new(Mutex::new(HashMap::new())),
            last_ai_at: Arc::new(Mutex::new(None)),
            bridge_unsafe_commands: Arc::new(RwLock::new(bridge_unsafe_commands)),
            pending_time_query: Arc::new(Mutex::new(None)),
            day_ticks_accum: Arc::new(Mutex::new(0.0)),
            day_clock_rate: Arc::new(Mutex::new(1.0)),
            pending_queue_probe: Arc::new(Mutex::new(None)),
            queue_disconnect_pending: Arc::new(AtomicBool::new(false)),
            pending_discord_resolutions: Arc::new(Mutex::new(HashMap::new())),
            profanity_trie: Arc::new(RwLock::new(None)),
        };

        // Must run synchronously, before any spawned task can touch censoring --
        // see doc comment on strip_false_positive_leetspeak for why.
        profanity_filter::strip_false_positive_leetspeak();

        // Heartbeat watchdog — pings external dead-man's switch (e.g. healthchecks.io) while alive
        if !self.heartbeat_url.is_empty() {
            let url = self.heartbeat_url.clone();
            let interval = self.heartbeat_interval_ms;
            let http = state.http.clone();
            tokio::spawn(async move {
                loop {
                    match http.get(&url).send().await {
                        Ok(_) => crate::structure::logger::debug_cat("heartbeat", "Heartbeat sent."),
                        Err(e) => crate::structure::logger::debug_cat("heartbeat", format!("Heartbeat failed: {e}")),
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(interval)).await;
                }
            });
        }

        // Build URL blocklist in background
        {
            let blocklist_arc = state.url_blocklist.clone();
            let sources = self.url_blocklist_sources.clone();
            let whitelist = self.url_whitelist_file.clone();
            tokio::spawn(async move {
                let set = crate::structure::mineflayer::url_blocklist::build_blocklist(&sources, &whitelist).await;
                *blocklist_arc.write().expect("url_blocklist write") = Some(set);
            });
        }

        // Build profanity filter trie in background (rustrict built-in dictionary + custom lists)
        {
            let trie_arc = state.profanity_trie.clone();
            tokio::spawn(async move {
                let trie = profanity_filter::build_trie().await;
                *trie_arc.write().expect("profanity_trie write") = Some(trie);
            });
        }

        // Load cached GasBuddy CSRF token
        if let Some(token) = crate::commands::casino::gas::load_cached_token().await {
            *state.gasbuddy_csrf.lock().expect("gasbuddy_csrf") = Some(token);
        }

        // Recover market bets that were open when the bot last shut down
        {
            let open_bets = state.api.casino_market_bet_list().await;
            if !open_bets.is_empty() {
                let whisper_cmd = state.runtime.read().expect("runtime lock").whisper_command.clone();
                let now = crate::structure::market::types::now_unix();
                {
                    let mut bets = state.market_bets.lock().expect("market_bets lock");
                    for bet in &open_bets {
                        bets.entry(bet.player.clone()).or_default().push(bet.clone());
                    }
                }
                for bet in open_bets {
                    let remaining = bet.closes_unix.saturating_sub(now);
                    tokio::spawn(crate::commands::market::settle_task(
                        state.clone(),
                        bet.player.clone(),
                        whisper_cmd.clone(),
                        bet,
                        remaining,
                    ));
                }
            }
        }

        // Recover weather bets that were open when the bot last shut down
        {
            let open_bets = state.api.casino_weather_bet_list().await;
            if !open_bets.is_empty() {
                let whisper_cmd = state.runtime.read().expect("runtime lock").whisper_command.clone();
                let now = crate::structure::market::types::now_unix();
                {
                    let mut bets = state.weather_bets.lock().expect("weather_bets lock");
                    for bet in &open_bets {
                        bets.entry(bet.player.clone()).or_default().push(bet.clone());
                    }
                }
                for bet in open_bets {
                    let remaining = bet.closes_unix.saturating_sub(now);
                    tokio::spawn(crate::commands::weather::settle_task(
                        state.clone(),
                        whisper_cmd.clone(),
                        bet,
                        remaining,
                    ));
                }
            }
        }

        // Recover sports bets that were open when the bot last shut down
        {
            let open_bets = state.api.casino_sports_bet_list().await;
            if !open_bets.is_empty() {
                let whisper_cmd = state.runtime.read().expect("runtime lock").whisper_command.clone();
                let api_key = state.runtime.read().expect("runtime lock").sharpapi_key.clone();
                {
                    let mut bets = state.sports_bets.lock().expect("sports_bets lock");
                    for bet in &open_bets {
                        bets.entry(bet.player.clone()).or_default().push(bet.clone());
                    }
                }
                for bet in open_bets {
                    tokio::spawn(crate::commands::casino::sports::settle_task(
                        state.clone(),
                        whisper_cmd.clone(),
                        api_key.clone(),
                        bet,
                    ));
                }
            }
        }

        // Recover Kalshi bets that were open when the bot last shut down
        {
            let open_bets = state.api.casino_kalshi_bet_list().await;
            if !open_bets.is_empty() {
                let whisper_cmd = state.runtime.read().expect("runtime lock").whisper_command.clone();
                {
                    let mut bets = state.kalshi_bets.lock().expect("kalshi_bets lock");
                    for bet in &open_bets {
                        bets.entry(bet.player.clone()).or_default().push(bet.clone());
                    }
                }
                for bet in open_bets {
                    tokio::spawn(crate::commands::casino::kalshi::settle_task(
                        state.clone(),
                        whisper_cmd.clone(),
                        bet,
                    ));
                }
            }
        }

        // Recover NASA space weather bets open when bot last shut down
        {
            let open_bets = state.api.casino_nasa_space_weather_bet_list().await;
            if !open_bets.is_empty() {
                let (whisper_cmd, nasa_api_key) = {
                    let rt = state.runtime.read().expect("runtime lock");
                    (rt.whisper_command.clone(), rt.nasa_api_key.clone())
                };
                {
                    let mut bets = state.nasa_space_weather_bets.lock().expect("nasa_space_weather_bets lock");
                    for bet in &open_bets {
                        bets.entry(bet.player.clone()).or_default().push(bet.clone());
                    }
                }
                for bet in open_bets {
                    tokio::spawn(crate::commands::casino::nasa_space_weather::settle_task(
                        state.clone(),
                        whisper_cmd.clone(),
                        nasa_api_key.clone(),
                        bet,
                    ));
                }
            }
        }

        // Recover FAA airport bets open when bot last shut down
        {
            let open_bets = state.api.casino_faa_airport_bet_list().await;
            if !open_bets.is_empty() {
                let whisper_cmd = state.runtime.read().expect("runtime lock").whisper_command.clone();
                let now = crate::structure::market::types::now_unix();
                {
                    let mut bets = state.faa_airport_bets.lock().expect("faa_airport_bets lock");
                    for bet in &open_bets {
                        bets.entry(bet.player.clone()).or_default().push(bet.clone());
                    }
                }
                for bet in open_bets {
                    // close_time may already be past if bot was down; settle_task handles that
                    let _ = now; // suppress unused warning
                    tokio::spawn(crate::commands::casino::faa_airport::settle_task(
                        state.clone(),
                        whisper_cmd.clone(),
                        bet,
                    ));
                }
            }
        }

        // Recover NOAA flooding bets open when bot last shut down
        {
            let open_bets = state.api.casino_noaa_flooding_bet_list().await;
            if !open_bets.is_empty() {
                let whisper_cmd = state.runtime.read().expect("runtime lock").whisper_command.clone();
                let now = crate::structure::market::types::now_unix();
                {
                    let mut bets = state.noaa_flooding_bets.lock().expect("noaa_flooding_bets lock");
                    for bet in &open_bets {
                        bets.entry(bet.player.clone()).or_default().push(bet.clone());
                    }
                }
                for bet in open_bets {
                    let _ = now;
                    tokio::spawn(crate::commands::casino::noaa_flooding::settle_task(
                        state.clone(),
                        whisper_cmd.clone(),
                        bet,
                    ));
                }
            }
        }

        // Recover train bets open when bot last shut down
        {
            let open_bets = state.api.casino_train_bet_list().await;
            if !open_bets.is_empty() {
                let whisper_cmd = state.runtime.read().expect("runtime lock").whisper_command.clone();
                let now = crate::structure::market::types::now_unix();
                {
                    let mut bets = state.train_bets.lock().expect("train_bets lock");
                    for bet in &open_bets {
                        bets.entry(bet.player.clone()).or_default().push(bet.clone());
                    }
                }
                for bet in open_bets {
                    let _ = now;
                    tokio::spawn(crate::commands::casino::train::settle_task(
                        state.clone(),
                        whisper_cmd.clone(),
                        bet,
                    ));
                }
            }
        }

        // Recover quake bets open when bot last shut down
        {
            let open_bets = state.api.casino_quake_bet_list().await;
            if !open_bets.is_empty() {
                let whisper_cmd = state.runtime.read().expect("runtime lock").whisper_command.clone();
                {
                    let mut bets = state.quake_bets.lock().expect("quake_bets lock");
                    for bet in &open_bets {
                        bets.entry(bet.player.clone()).or_default().push(bet.clone());
                    }
                }
                for bet in open_bets {
                    let placed_at = bet.close_time.saturating_sub(crate::commands::casino::seismic::BET_WINDOW_SECS);
                    tokio::spawn(crate::commands::casino::seismic::quake_settle_task(
                        state.clone(),
                        whisper_cmd.clone(),
                        bet,
                        placed_at,
                    ));
                }
            }
        }

        // Recover volcano bets open when bot last shut down
        {
            let open_bets = state.api.casino_volcano_bet_list().await;
            if !open_bets.is_empty() {
                let whisper_cmd = state.runtime.read().expect("runtime lock").whisper_command.clone();
                {
                    let mut bets = state.volcano_bets.lock().expect("volcano_bets lock");
                    for bet in &open_bets {
                        bets.entry(bet.player.clone()).or_default().push(bet.clone());
                    }
                }
                for bet in open_bets {
                    tokio::spawn(crate::commands::casino::seismic::volcano_settle_task(
                        state.clone(),
                        whisper_cmd.clone(),
                        bet,
                    ));
                }
            }
        }

        // Recover AQI bets open when bot last shut down
        {
            let open_bets = state.api.casino_aqi_bet_list().await;
            if !open_bets.is_empty() {
                let whisper_cmd = state.runtime.read().expect("runtime lock").whisper_command.clone();
                {
                    let mut bets = state.aqi_bets.lock().expect("aqi_bets lock");
                    for bet in &open_bets {
                        bets.entry(bet.player.clone()).or_default().push(bet.clone());
                    }
                }
                for bet in open_bets {
                    tokio::spawn(crate::commands::casino::aqi::aqi_settle_task(
                        state.clone(),
                        whisper_cmd.clone(),
                        bet,
                    ));
                }
            }
        }

        // Recover launch bets open when bot last shut down
        {
            let open_bets = state.api.casino_launch_bet_list().await;
            if !open_bets.is_empty() {
                let whisper_cmd = state.runtime.read().expect("runtime lock").whisper_command.clone();
                {
                    let mut bets = state.launch_bets.lock().expect("launch_bets lock");
                    for bet in &open_bets {
                        bets.entry(bet.player.clone()).or_default().push(bet.clone());
                    }
                }
                for bet in open_bets {
                    tokio::spawn(crate::commands::casino::launch::launch_settle_task(
                        state.clone(),
                        whisper_cmd.clone(),
                        bet,
                    ));
                }
            }
        }

        // Recover gas bets open when bot last shut down
        {
            let open_bets = state.api.casino_gas_bet_list().await;
            if !open_bets.is_empty() {
                let whisper_cmd = state.runtime.read().expect("runtime lock").whisper_command.clone();
                {
                    let mut bets = state.gas_bets.lock().expect("gas_bets lock");
                    for bet in &open_bets {
                        bets.entry(bet.player.clone()).or_default().push(bet.clone());
                    }
                }
                for bet in open_bets {
                    tokio::spawn(crate::commands::casino::gas::gas_settle_task(
                        state.clone(),
                        whisper_cmd.clone(),
                        bet,
                    ));
                }
            }
        }

        // Load portfolio positions into memory
        {
            let open_positions = state.api.casino_portfolio_list().await;
            if !open_positions.is_empty() {
                let mut map = state.portfolio_positions.lock().expect("portfolio lock");
                for pos in open_positions {
                    map.entry(pos.player.clone()).or_default().push(pos);
                }
            }
        }

        let mut builder = if self.options.disable_chat_signing {
            logger::info("Chat signing disabled.");
            ClientBuilder::new_without_plugins()
                .add_plugins(
                    azalea::DefaultPlugins
                        .build()
                        .disable::<ChatSigningPlugin>(),
                )
                .add_plugins(DefaultBotPlugins)
        } else {
            ClientBuilder::new()
        }
        .set_handler(handle_azalea_event)
        .set_state(state)
        .reconnect_after(Duration::from_millis(self.reconnect_time_ms));

        if self.options.enable_viaversion {
            logger::login(format!(
                "Using ViaVersion target version: {}",
                self.options.viaversion_target_version
            ));
            builder = builder.add_plugins(
                ViaVersionPlugin::start(&self.options.viaversion_target_version).await,
            );
        }

        builder.start(account, address).await;

        Ok(())
    }
    #[allow(dead_code)]
    pub async fn end_and_restart(&mut self) -> anyhow::Result<()> {
        self.is_connected = false;
        self.start().await
    }

    #[allow(dead_code)]
    pub fn get_players(&self) -> Vec<Player> {
        Vec::new()
    }
}

#[derive(Clone, Component)]
pub struct AzaleaState {
    pub mc_server: String,
    pub api: Arc<ApiClient>,
    pub runtime: Arc<RwLock<RuntimeConfig>>,
    pub players: Arc<RwLock<HashMap<String, PlayerSnapshot>>>,
    pub outbound_chat: Arc<Mutex<VecDeque<String>>>,
    pub background_tasks_started: Arc<Mutex<bool>>,
    pub last_command_at: Arc<Mutex<Option<Instant>>>,
    pub player_command_cooldowns: Arc<Mutex<HashMap<String, PlayerCommandCooldown>>>,
    pub seen_entity_spawns: Arc<RwLock<HashSet<String>>>,
    pub next_entity_spawn_scan_at: Arc<RwLock<i64>>,
    pub seen_player_detections: Arc<RwLock<HashSet<String>>>,
    pub next_player_detection_scan_at: Arc<RwLock<i64>>,
    pub trade_cooldowns: Arc<Mutex<HashMap<String, Instant>>>,
    pub free_scratch_cooldowns: Arc<Mutex<HashMap<String, Instant>>>,
    pub initial_spawn_done: Arc<AtomicBool>,
    pub antiafk: bool,
    pub antiafk_active: Arc<AtomicBool>,
    pub announce: bool,
    pub announce_active: Arc<AtomicBool>,
    pub reminder_active: Arc<AtomicBool>,
    pub consecutive_failures: Arc<AtomicU32>,
    pub seen_advancements: Arc<Mutex<HashMap<String, HashSet<String>>>>,
    pub nick_cache: Arc<RwLock<HashMap<String, String>>>,
    pub world_time_ticks: Arc<RwLock<u64>>,
    pub active_trivia: Arc<Mutex<Option<TriviaRound>>>,
    pub casino_sessions: Arc<Mutex<HashMap<String, CasinoSession>>>,
    pub market_service: Arc<crate::structure::market::service::MarketService>,
    pub market_bets: Arc<Mutex<HashMap<String, Vec<crate::structure::market::types::MarketBet>>>>,
    pub portfolio_positions: Arc<Mutex<HashMap<String, Vec<crate::structure::market::types::PortfolioPosition>>>>,
    pub weather_bets: Arc<Mutex<HashMap<String, Vec<crate::commands::weather::WeatherBet>>>>,
    pub weather_odds_cache: Arc<Mutex<HashMap<String, crate::commands::weather::WeatherCacheEntry>>>,
    pub sports_bets: Arc<Mutex<HashMap<String, Vec<crate::commands::casino::sports::SportsBet>>>>,
    pub sports_cache: Arc<Mutex<crate::commands::casino::sports::SportsCache>>,
    pub kalshi_bets: Arc<Mutex<HashMap<String, Vec<crate::commands::casino::kalshi::KalshiBet>>>>,
    pub kalshi_cache: Arc<Mutex<crate::commands::casino::kalshi::KalshiCache>>,
    pub nasa_space_weather_bets: Arc<Mutex<HashMap<String, Vec<crate::commands::casino::nasa_space_weather::NasaSpaceWeatherBet>>>>,
    pub sw_odds_cache: Arc<Mutex<Option<(crate::commands::casino::nasa_space_weather::SwOdds, u64)>>>,
    pub faa_airport_bets: Arc<Mutex<HashMap<String, Vec<crate::commands::casino::faa_airport::FaaAirportBet>>>>,
    pub noaa_flooding_bets: Arc<Mutex<HashMap<String, Vec<crate::commands::casino::noaa_flooding::NOAAFloodingBet>>>>,
    pub flood_cache: Arc<Mutex<crate::commands::casino::noaa_flooding::FloodCache>>,
    pub train_bets: Arc<Mutex<HashMap<String, Vec<crate::commands::casino::train::TrainBet>>>>,
    pub quake_bets: Arc<Mutex<HashMap<String, Vec<crate::commands::casino::seismic::QuakeBet>>>>,
    pub volcano_bets: Arc<Mutex<HashMap<String, Vec<crate::commands::casino::seismic::VolcanoBet>>>>,
    pub duels: Arc<Mutex<Vec<crate::commands::duel::Duel>>>,
    pub wordle_games: Arc<Mutex<std::collections::HashMap<String, crate::commands::wordle::WordleSession>>>,
    pub checkers_games: Arc<Mutex<std::collections::HashMap<String, crate::commands::checkers::CheckersSession>>>,
    pub reversi_games: Arc<Mutex<std::collections::HashMap<String, crate::commands::reversi::ReversiSession>>>,
    pub battleship_games: Arc<Mutex<std::collections::HashMap<String, crate::commands::battleship::BattleshipSession>>>,
    pub mines_games: Arc<Mutex<std::collections::HashMap<String, crate::commands::casino::mines::MinesGame>>>,
    pub aqi_bets: Arc<Mutex<std::collections::HashMap<String, Vec<crate::commands::casino::aqi::AqiBet>>>>,
    pub launch_bets: Arc<Mutex<std::collections::HashMap<String, Vec<crate::commands::casino::launch::LaunchBet>>>>,
    pub launch_cache: Arc<Mutex<std::collections::HashMap<u32, (f64, f64, u64)>>>,
    pub gas_bets: Arc<Mutex<std::collections::HashMap<String, Vec<crate::commands::casino::gas::GasBet>>>>,
    // (price, display_name, fetched_at)
    pub gas_price_cache: Arc<Mutex<std::collections::HashMap<String, (f64, String, u64)>>>,
    pub gasbuddy_csrf: Arc<Mutex<Option<String>>>,
    pub http: reqwest::Client,
    pub url_blocklist: Arc<RwLock<Option<HashSet<String>>>>,
    // (game_ticks, real_time_ms) samples from SetTime packets — used to compute server TPS
    pub tps_time_samples: Arc<Mutex<VecDeque<(u64, u64)>>>,
    // key = lowercase username, value = AFK message; cleared on speak or leave
    pub afk_messages: Arc<RwLock<HashMap<String, String>>>,
    // key = triggering username (lowercase), value = when they last triggered an AFK reply
    pub afk_cooldowns: Arc<Mutex<HashMap<String, Instant>>>,
    // key = whisper target (lowercase), value = (message body, sent_at) — lets the chat
    // handler recognize the server echoing our own outgoing whisper back as if the
    // target had spoken it, and suppress that instead of treating it as real chat
    pub recent_whispers: Arc<Mutex<HashMap<String, (String, Instant)>>>,
    pub active_poll: Arc<Mutex<Option<crate::commands::poll::PollState>>>,
    pub ai_providers: Arc<RwLock<Vec<crate::commands::ai::AiProviderEntry>>>,
    pub ai_model_cache: Arc<Mutex<HashMap<String, String>>>,
    pub last_ai_at: Arc<Mutex<Option<Instant>>>,
    // Defense-in-depth mirror of json/bridge_unsafe_commands.json, checked at dispatch
    // time in handle_inbound_discord_chat — separate from the copy pushed to Hub for
    // discordbot's own client-side relay filter.
    pub bridge_unsafe_commands: Arc<RwLock<HashSet<String>>>,
    // Set by daynight.rs right before sending "/time query day" when use_live_time_query
    // is on; fulfilled by the Event::Chat handler when the server's command-feedback
    // system message (no real sender) arrives. None when no query is in flight.
    pub pending_time_query: Arc<Mutex<Option<tokio::sync::oneshot::Sender<u64>>>>,
    // Free-running local estimate of the "overworld" WorldClock's raw tick count --
    // fallback path for !day/!night when use_live_time_query is off/denied/timed out.
    // Snapped to the authoritative value on every real SetTime packet, incremented by
    // day_clock_rate on every local Event::Tick in between (mirrors how real Minecraft
    // clients keep ClientWorld.timeOfDay live between server corrections). f64 so a
    // fractional rate (e.g. 0.5x) accumulates correctly instead of truncating to 0 each tick.
    pub day_ticks_accum: Arc<Mutex<f64>>,
    pub day_clock_rate: Arc<Mutex<f32>>,
    // Set right before sending `queue_probe_command`; fulfilled with `true` from the
    // Event::Chat no-sender branch if the response is vanilla's "Unknown or incomplete
    // command" error, meaning we're actually on the queue proxy, not the real server.
    pub pending_queue_probe: Arc<Mutex<Option<tokio::sync::oneshot::Sender<bool>>>>,
    // Set to true right before calling bot.disconnect() when queue detection fires, so
    // the Event::Disconnect handler knows to sleep queue_retry_delay_ms (instead of the
    // normal short reconnect_time_ms) before letting Azalea's own reconnect proceed.
    pub queue_disconnect_pending: Arc<AtomicBool>,
    // Keyed by a per-request UUID so concurrent chat-bridge commands from different
    // Discord users don't clobber each other -- unlike pending_time_query/pending_queue_probe
    // (only ever one in flight at a time), multiple resolve_discord_username round trips
    // can be outstanding simultaneously. Fulfilled from the WebsocketEvent handlers for
    // resolve_discord_username_result / resolve_discord_username_unavailable.
    pub pending_discord_resolutions: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<DiscordResolution>>>>,
    // rustrict trie merging its built-in dictionary with json/bad_words.json (PROFANE|SEVERE)
    // and json/word_whitelist.json (SAFE overrides). None until the background build in
    // Bot::start() finishes; rebuilt in place by profanity_filter::rebuild() on !censor,
    // !wordwhitelist, and !reload.
    pub profanity_trie: Arc<RwLock<Option<&'static Trie>>>,
}

// Result of asking discordbot (via Hub) to resolve a chat-bridge "server username" to a
// real Discord snowflake ID -- see pending_discord_resolutions above.
#[derive(Debug, Clone)]
pub enum DiscordResolution {
    Found(String),
    NotFound,
    Unavailable,
}

#[derive(Debug, Clone)]
pub enum CasinoSession {
    Craps {
        bet: i64,
        pass_line: bool,
        point: u32,
    },
    Hilo {
        stake: i64,
        deck: Vec<u8>,
        current_card: u8,
        multiplier: f64,
        guesses: u32,
    },
    Blackjack {
        bet: i64,
        player_hand: Vec<u8>,
        dealer_hand: Vec<u8>,
    },
    Poker {
        stake: i64,
        opponent_name: &'static str,
        aggression: f64,
        game: Box<crate::commands::casino::poker::game::state::GameState>,
    },
    ConnectFour {
        stake: i64,
        opponent_name: &'static str,
        difficulty: connect_four_ai::Difficulty,
        position: connect_four_ai::Position,
    },
    Chess {
        bet: i64,
        player_color: shakmaty::Color,
        position: Box<shakmaty::Chess>,
        opponent_name: &'static str,
        ai_depth: u32,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum TriviaPhase {
    Joining,
    Open,
    Closed,
}

#[derive(Debug, Clone)]
pub struct TriviaRound {
    // Question data (fetched at start, revealed after join window)
    pub correct_answer: String,
    pub is_boolean: bool,
    pub letter_map: Vec<(char, String)>,
    pub correct_letter: Option<char>,
    #[allow(dead_code)]
    pub question_msg: String,
    // Phase + wagering
    pub phase: TriviaPhase,
    pub stake: i64,
    pub participants: HashSet<String>,
    // Results
    pub correct_players: Vec<String>,
    pub wrong_players: Vec<String>,
    pub answered: HashSet<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct PlayerCommandCooldown {
    pub last_success_at: Instant,
    pub cooldown_ms: u64,
}

impl Default for AzaleaState {
    fn default() -> Self {
        Self {
            mc_server: String::new(),
            api: Arc::new(ApiClient::new(crate::config::ApiConfig {
                api_url: String::new(),
                websocket_url: String::new(),
                api_key: String::new(),
                mc_server: String::new(),
                is_bot_client: true,
                log_errors: false,
                use_websocket: false,
            })),
            runtime: Arc::new(RwLock::new(RuntimeConfig {
                prefix: "!".to_owned(),
                whisper_command: "msg".to_owned(),
                use_commands: true,
                anti_spam_global_cooldown_ms: 1_000,
                command_cooldowns: HashMap::new(),
                use_whitelist: false,
                user_whitelist: HashSet::new(),
                user_blacklist: HashSet::new(),
                custom_chat_formats: Vec::new(),
                command_toggles: HashMap::new(),
                disabled_events: HashSet::new(),
                allow_chatbridge_input: false,
                use_live_time_query: false,
                welcome_messages: false,
                use_custom_chat_prefix: false,
                custom_chat_prefix: String::new(),
                smart_censoring: false,
                censor_threshold: "moderate".to_owned(),
                command_censorship: HashMap::new(),
                together_api_key: String::new(),
                wolfram_app_id: String::new(),
                azure_translator_key: String::new(),
                azure_translator_region: String::new(),
                sharpapi_key: String::new(),
                nasa_api_key: String::new(),
                airnow_api_key: String::new(),
                gasbuddy_solver_url: String::new(),
                gasbuddy_csrf_readonly: false,
                google_safe_browsing_key: String::new(),
                queue_probe_command: String::new(),
                queue_retry_delay_ms: 300_000,
            })),
            players: Arc::new(RwLock::new(HashMap::new())),
            outbound_chat: Arc::new(Mutex::new(VecDeque::new())),
            background_tasks_started: Arc::new(Mutex::new(false)),
            last_command_at: Arc::new(Mutex::new(None)),
            player_command_cooldowns: Arc::new(Mutex::new(HashMap::new())),
            seen_entity_spawns: Arc::new(RwLock::new(HashSet::new())),
            next_entity_spawn_scan_at: Arc::new(RwLock::new(0)),
            seen_player_detections: Arc::new(RwLock::new(HashSet::new())),
            next_player_detection_scan_at: Arc::new(RwLock::new(0)),
            trade_cooldowns: Arc::new(Mutex::new(HashMap::new())),
            free_scratch_cooldowns: Arc::new(Mutex::new(HashMap::new())),
            initial_spawn_done: Arc::new(AtomicBool::new(false)),
            antiafk: false,
            antiafk_active: Arc::new(AtomicBool::new(false)),
            announce: false,
            announce_active: Arc::new(AtomicBool::new(false)),
            reminder_active: Arc::new(AtomicBool::new(false)),
            consecutive_failures: Arc::new(AtomicU32::new(0)),
            seen_advancements: Arc::new(Mutex::new(HashMap::new())),
            nick_cache: Arc::new(RwLock::new(HashMap::new())),
            world_time_ticks: Arc::new(RwLock::new(0)),
            active_trivia: Arc::new(Mutex::new(None)),
            casino_sessions: Arc::new(Mutex::new(HashMap::new())),
            market_service: Arc::new(crate::structure::market::service::MarketService::new(String::new())),
            market_bets: Arc::new(Mutex::new(HashMap::new())),
            portfolio_positions: Arc::new(Mutex::new(HashMap::new())),
            weather_bets: Arc::new(Mutex::new(HashMap::new())),
            weather_odds_cache: Arc::new(Mutex::new(HashMap::new())),
            sports_bets: Arc::new(Mutex::new(HashMap::new())),
            sports_cache: Arc::new(Mutex::new(crate::commands::casino::sports::SportsCache::default())),
            kalshi_bets: Arc::new(Mutex::new(HashMap::new())),
            kalshi_cache: Arc::new(Mutex::new(crate::commands::casino::kalshi::KalshiCache::default())),
            nasa_space_weather_bets: Arc::new(Mutex::new(HashMap::new())),
            sw_odds_cache: Arc::new(Mutex::new(None)),
            faa_airport_bets: Arc::new(Mutex::new(HashMap::new())),
            noaa_flooding_bets: Arc::new(Mutex::new(HashMap::new())),
            flood_cache: Arc::new(Mutex::new(crate::commands::casino::noaa_flooding::FloodCache::default())),
            train_bets: Arc::new(Mutex::new(HashMap::new())),
            quake_bets: Arc::new(Mutex::new(HashMap::new())),
            volcano_bets: Arc::new(Mutex::new(HashMap::new())),
            duels: Arc::new(Mutex::new(Vec::new())),
            wordle_games: Arc::new(Mutex::new(HashMap::new())),
            checkers_games: Arc::new(Mutex::new(HashMap::new())),
            reversi_games: Arc::new(Mutex::new(HashMap::new())),
            battleship_games: Arc::new(Mutex::new(HashMap::new())),
            mines_games: Arc::new(Mutex::new(HashMap::new())),
            aqi_bets: Arc::new(Mutex::new(HashMap::new())),
            launch_bets: Arc::new(Mutex::new(HashMap::new())),
            launch_cache: Arc::new(Mutex::new(HashMap::new())),
            gas_bets: Arc::new(Mutex::new(HashMap::new())),
            gas_price_cache: Arc::new(Mutex::new(HashMap::new())),
            gasbuddy_csrf: Arc::new(Mutex::new(None)),
            http: reqwest::Client::new(),
            url_blocklist: Arc::new(RwLock::new(None)),
            tps_time_samples: Arc::new(Mutex::new(VecDeque::new())),
            afk_messages: Arc::new(RwLock::new(HashMap::new())),
            afk_cooldowns: Arc::new(Mutex::new(HashMap::new())),
            recent_whispers: Arc::new(Mutex::new(HashMap::new())),
            active_poll: Arc::new(Mutex::new(None)),
            ai_providers: Arc::new(RwLock::new(Vec::new())),
            ai_model_cache: Arc::new(Mutex::new(HashMap::new())),
            last_ai_at: Arc::new(Mutex::new(None)),
            bridge_unsafe_commands: Arc::new(RwLock::new(HashSet::new())),
            pending_time_query: Arc::new(Mutex::new(None)),
            day_ticks_accum: Arc::new(Mutex::new(0.0)),
            day_clock_rate: Arc::new(Mutex::new(1.0)),
            pending_queue_probe: Arc::new(Mutex::new(None)),
            queue_disconnect_pending: Arc::new(AtomicBool::new(false)),
            pending_discord_resolutions: Arc::new(Mutex::new(HashMap::new())),
            profanity_trie: Arc::new(RwLock::new(None)),
        }
    }
}

async fn handle_azalea_event(bot: Client, event: Event, state: AzaleaState) -> anyhow::Result<()> {
    match event {
        Event::Init => {
            if event_disabled(&state, &["init"]) {
                return Ok(());
            }
            logger::login("Azalea client initialized.");

            bot.set_client_information(ClientInformation {
                view_distance: 2,
                ..Default::default()
            });
        }
        Event::Login => {
            if event_disabled(&state, &["login"]) {
                return Ok(());
            }
            logger::login("Logged into Minecraft server.");
        }
        Event::Spawn => {
            if event_disabled(&state, &["spawn"]) {
                return Ok(());
            }
            logger::spawn(format!("Spawned on {}.", state.mc_server));
            state.consecutive_failures.store(0, Ordering::Relaxed);
            state.initial_spawn_done.store(true, Ordering::Relaxed);
            if mark_background_tasks_started(&state) {
                spawn_websocket_event_task(bot.clone(), state.clone());
                spawn_player_list_update_task(state.clone());
                let push_state = state.clone();
                tokio::spawn(async move {
                    let unsafe_names = crate::commands::load_bridge_unsafe_commands(
                        "json/bridge_unsafe_commands.json",
                    )
                    .await;
                    let list = crate::commands::build_bridge_command_list(&unsafe_names);
                    push_state.api.push_bridge_commands(&list).await;
                });
            }
            send_player_list_update(&state).await;
            if state.antiafk {
                state.antiafk_active.store(true, Ordering::Relaxed);
                anti_afk::spawn_antiafk_loop(bot.clone(), Arc::clone(&state.antiafk_active));
                logger::info("AntiAFK started.");
            }
            if state.announce {
                state.announce_active.store(true, Ordering::Relaxed);
                announce::spawn_announce_loop(state.clone(), Arc::clone(&state.announce_active));
                logger::info("Announce loop started.");
            }
            state.reminder_active.store(true, Ordering::Relaxed);
            spawn_reminder_tick_task(state.clone(), Arc::clone(&state.reminder_active));

            // Queue detection: fires on every spawn (including after reconnects), so
            // it keeps re-checking until the probe command actually works, meaning
            // we've made it past the queue onto the real server. Empty command
            // disables the whole feature.
            let probe_command = state
                .runtime
                .read()
                .expect("runtime config lock poisoned")
                .queue_probe_command
                .clone();
            if !probe_command.is_empty() {
                let probe_bot = bot.clone();
                let probe_state = state.clone();
                tokio::spawn(async move {
                    if run_queue_probe(&probe_state, &probe_command).await {
                        logger::debug_cat(
                            "queue",
                            "Probe command came back unknown -- we're on the queue proxy, disconnecting to retry later",
                        );
                        disconnect_for_queue_retry(&probe_bot, &probe_state);
                    }
                });
            }
        }
        Event::Chat(message) => {
            if event_disabled(&state, &["message", "messagestr", "chat"]) {
                return Ok(());
            }
            let (sender, content) = parse_chat_message(&message, &state);
            if sender.is_none() && is_server_presence_message(&content) {
                return Ok(());
            }

            logger::chat(match &sender {
                Some(sender) => format!("{sender}: {content}"),
                None => format!("Chat: {content}"),
            });

            if sender.is_none() {
                // If a live "/time query day" is in flight, this is very likely its
                // command-feedback response (system message, no real player sender).
                // Filter down to just the digits rather than matching exact wording --
                // the tick count is the only number in the response either way.
                // Locks are scoped to plain blocks (not just drop()) so the MutexGuard
                // (not Send) provably can't still be alive across the .await below.
                let matched_ticks: Option<u64> = {
                    let pending = state
                        .pending_time_query
                        .lock()
                        .expect("pending_time_query lock poisoned");
                    if pending.is_some() {
                        let digits: String =
                            content.chars().filter(|c| c.is_ascii_digit()).collect();
                        digits.parse::<u64>().ok()
                    } else {
                        None
                    }
                };

                if let Some(ticks) = matched_ticks {
                    let mut pending = state
                        .pending_time_query
                        .lock()
                        .expect("pending_time_query lock poisoned");
                    if let Some(tx) = pending.take() {
                        let _ = tx.send(ticks);
                    }
                    return Ok(());
                }

                // If a queue probe command is in flight, vanilla's generic "unknown
                // command" error is the tell that we're actually on the queue proxy
                // rather than the real server -- any other response (or none at all,
                // handled by the timeout on the receiving end) means we're through.
                let queue_probe_matched: bool = {
                    let pending = state
                        .pending_queue_probe
                        .lock()
                        .expect("pending_queue_probe lock poisoned");
                    pending.is_some() && content.to_lowercase().contains("unknown or incomplete command")
                };
                if queue_probe_matched {
                    let mut pending = state
                        .pending_queue_probe
                        .lock()
                        .expect("pending_queue_probe lock poisoned");
                    if let Some(tx) = pending.take() {
                        let _ = tx.send(true);
                    }
                    return Ok(());
                }

                handle_fallback_message(&bot, &state, &content).await;
                return Ok(());
            }

            if let Some(sender) = sender {
                if sender == bot.username() {
                    let allow_bridge = state
                        .runtime
                        .read()
                        .expect("runtime config lock poisoned")
                        .allow_chatbridge_input;
                    if allow_bridge && !content.starts_with('!') {
                        send_minecraft_chat_message(&state, &sender, &content, &"").await;
                    }
                    return Ok(());
                }

                let sender_lower = sender.to_lowercase();

                // Self-echo suppression: the server echoes our own outgoing whispers
                // back into chat, and the generic "{username}: {message}" custom
                // format matches that echo, misreading it as the whisper target
                // speaking. If this (sender, content) matches a whisper we just sent
                // that target, it's our own echo — drop it before AFK/mention/command
                // handling ever sees it.
                {
                    let mut recent = state.recent_whispers.lock().expect("recent_whispers lock poisoned");
                    if let Some((sent_content, sent_at)) = recent.get(&sender_lower) {
                        if *sent_content == content && sent_at.elapsed() < Duration::from_secs(5) {
                            recent.remove(&sender_lower);
                            return Ok(());
                        }
                    }
                }

                // AFK self-clear: if this sender was AFK, clear and notify them
                {
                    let cleared = state
                        .afk_messages
                        .write()
                        .expect("afk_messages lock")
                        .remove(&sender_lower)
                        .is_some();
                    if cleared {
                        let wcmd = state.runtime.read().expect("runtime config lock poisoned").whisper_command.clone();
                        crate::commands::enqueue_chat(&state, format!("/{wcmd} {sender} AFK cleared."));
                    }
                }

                // AFK mention check: whisper sender about any AFK players they mentioned
                {
                    let content_lower = content.to_lowercase();
                    let hits: Vec<(String, String)> = state
                        .afk_messages
                        .read()
                        .expect("afk_messages lock")
                        .iter()
                        .filter(|(k, _)| content_lower.contains(k.as_str()))
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();

                    if !hits.is_empty() {
                        let now = Instant::now();
                        let on_cd = state
                            .afk_cooldowns
                            .lock()
                            .expect("afk_cooldowns lock")
                            .get(&sender_lower)
                            .map_or(false, |&t| now.duration_since(t) < Duration::from_secs(60));

                        if !on_cd {
                            let wcmd = state.runtime.read().expect("runtime config lock poisoned").whisper_command.clone();
                            let (fname, fmsg) = &hits[0];
                            let rest: Vec<&str> = hits[1..].iter().map(|(k, _)| k.as_str()).collect();
                            let main = format!("{fname} is afk: {fmsg}");
                            let suffix = match rest.len() {
                                0 => String::new(),
                                1 => format!(" | {} is also afk.", rest[0]),
                                _ => {
                                    let (last, init) = rest.split_last().unwrap();
                                    format!(" | {} and {} are also afk.", init.join(", "), last)
                                }
                            };
                            let max = 200usize;
                            let main_len = main.chars().count();
                            let suffix_len = suffix.chars().count();
                            let response = if main_len + suffix_len > max {
                                let allowed = max.saturating_sub(suffix_len + 3);
                                let truncated: String = main.chars().take(allowed).collect();
                                format!("{truncated}...{suffix}")
                            } else {
                                format!("{main}{suffix}")
                            };
                            crate::commands::enqueue_chat(&state, format!("/{wcmd} {sender} {response}"));
                            state.afk_cooldowns.lock().expect("afk_cooldowns lock").insert(sender_lower.clone(), now);
                        }
                    }
                }

                let prefix = state
                    .runtime
                    .read()
                    .expect("runtime config lock poisoned")
                    .prefix
                    .clone();

                if content.starts_with(&prefix) {
                    if resolve_sender_uuid(&state, &sender).is_some() {
                        if sender_allowed_for_command(&state, &sender, &content) {
                            command_handler::handle(&bot, &state, &sender, &content).await;
                        }
                        return Ok(());
                    }

                    // No resolvable UUID -- can only mean this "sender" isn't a real,
                    // currently-connected MC player, i.e. this arrived through an official
                    // (non-craftbot) Discord chat bridge. Live-resolve to a Discord snowflake
                    // and check that directly against the blacklist before ever dispatching.
                    match resolve_and_check_bridge_sender(&state, &sender).await {
                        BridgeSenderStatus::Allowed => {
                            command_handler::handle(&bot, &state, &sender, &content).await;
                        }
                        BridgeSenderStatus::Blacklisted => {
                            // Silent drop -- same backstop posture as the bridge_unsafe_commands
                            // block: never post to public chat, log only.
                            logger::command(format!(
                                "Bridge command blocked (blacklisted via resolved Discord ID): {sender}"
                            ));
                        }
                        BridgeSenderStatus::NotFound => {
                            enqueue_outbound_chat(
                                &state,
                                format!("{sender}, your Discord username was not found."),
                            );
                        }
                        BridgeSenderStatus::Unavailable => {
                            enqueue_outbound_chat(
                                &state,
                                format!(
                                    "{sender}, Discord username resolution unavailable, please try again later."
                                ),
                            );
                        }
                    }
                    return Ok(());
                }

                let uuid = state
                    .players
                    .read()
                    .expect("player cache lock poisoned")
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(&sender))
                    .map(|(_, player)| player.uuid.clone());

                if let Some(uuid) = uuid {
                    let blacklisted = state
                        .runtime
                        .read()
                        .expect("runtime config lock poisoned")
                        .user_blacklist
                        .contains(&uuid);
                    if !blacklisted {
                        send_minecraft_chat_message(&state, &sender, &content, &uuid).await;
                    }
                }
            }
        }
        Event::AddPlayer(player) => {
            if event_disabled(&state, &["playerJoined", "playerJoin", "addPlayer"]) {
                return Ok(());
            }
            logger::join(format!("Player joined: {}", player.profile.name));
            let username = player.profile.name.clone();
            let uuid = player.profile.uuid.to_string();
            let latency = player.latency;
            let display_name = player.display_name.as_deref().map(|d| d.to_string());
            if let Some(dn) = &display_name {
                state
                    .nick_cache
                    .write()
                    .expect("nick cache lock poisoned")
                    .insert(dn.clone(), uuid.clone());
            }
            state
                .players
                .write()
                .expect("player cache lock poisoned")
                .insert(
                    username.clone(),
                    PlayerSnapshot {
                        username: username.clone(),
                        uuid: uuid.clone(),
                        entity_uuid: player.profile.uuid,
                        latency,
                        display_name,
                    },
                );
            send_player_list_update(&state).await;
            // Fire-and-forget so /mosaic + /tablist stay populated without a manual
            // backfill; Hub skips the fetch entirely if a fresh head is already cached.
            {
                let ensure_state = state.clone();
                let ensure_uuid = uuid.clone();
                tokio::spawn(async move {
                    ensure_state.api.ensure_head_cached(&ensure_uuid).await;
                });
            }
            deliver_offline_messages(&state, &username).await;
            deliver_casino_notifications(&state, &uuid, &username).await;
            if state.initial_spawn_done.load(Ordering::Relaxed) {
                send_player_join(&state, &username, &uuid).await;
                fire_greeting_if_due(&state, &username).await;
            }
        }
        Event::UpdatePlayer(player) => {
            if event_disabled(&state, &["updatePlayer"]) {
                return Ok(());
            }
            if let Some(dn) = player.display_name.as_deref() {
                state
                    .nick_cache
                    .write()
                    .expect("nick cache lock poisoned")
                    .insert(dn.to_string(), player.profile.uuid.to_string());
            }
            state
                .players
                .write()
                .expect("player cache lock poisoned")
                .insert(
                    player.profile.name.clone(),
                    PlayerSnapshot {
                        username: player.profile.name,
                        uuid: player.profile.uuid.to_string(),
                        entity_uuid: player.profile.uuid,
                        latency: player.latency,
                        display_name: player.display_name.as_deref().map(|d| d.to_string()),
                    },
                );
        }
        Event::RemovePlayer(player) => {
            if event_disabled(&state, &["playerLeft", "playerLeave", "removePlayer"]) {
                return Ok(());
            }
            logger::leave(format!("Player left: {}", player.profile.name));
            let username = player.profile.name.clone();
            let uuid = player.profile.uuid.to_string();
            state
                .players
                .write()
                .expect("player cache lock poisoned")
                .remove(&username);
            stat_history::clear_delete_faq_pending(&username);
            crate::commands::duel::handle_disconnect(&state, &username).await;
            state.afk_messages.write().expect("afk_messages lock").remove(&username.to_lowercase());
            send_player_leave(&state, &username, &uuid).await;
            send_player_list_update(&state).await;
        }
        Event::Disconnect(reason) => {
            if event_disabled(&state, &["end", "disconnect"]) {
                return Ok(());
            }
            let reason_str = reason
                .map(|r| r.to_string())
                .unwrap_or_else(|| "Unknown".to_owned());
            logger::kick(format!("Kicked/disconnected: {reason_str}"));
            logger::logout("Bot has ended, attempting to restart soon.");
            state.initial_spawn_done.store(false, Ordering::Relaxed);
            state.antiafk_active.store(false, Ordering::Relaxed);
            state.announce_active.store(false, Ordering::Relaxed);
            state.reminder_active.store(false, Ordering::Relaxed);
            send_session_flush_leave(&state).await;

            // Queue detection triggered this disconnect -- wait the configured (long)
            // delay before letting Azalea's own reconnect_after proceed, instead of the
            // normal short reconnect_time_ms. swap() atomically reads-and-clears so this
            // only fires once per queue detection, not on every subsequent disconnect.
            if state
                .queue_disconnect_pending
                .swap(false, Ordering::Relaxed)
            {
                let delay_ms = state
                    .runtime
                    .read()
                    .expect("runtime config lock poisoned")
                    .queue_retry_delay_ms;
                logger::debug_cat("queue", format!("Queue-triggered disconnect -- waiting {delay_ms}ms before retrying"));
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }

            let failures = state.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;
            if failures >= 10 {
                state.consecutive_failures.store(0, Ordering::Relaxed);
                logger::warn("Server unreachable after 10 consecutive failures. Waiting 10 minutes before reconnect.");
                tokio::time::sleep(Duration::from_secs(10 * 60)).await;
            }
        }
        Event::ConnectionFailed(error) => {
            if event_disabled(&state, &["error", "kicked", "connectionFailed"]) {
                return Ok(());
            }
            logger::warn(format!("Connection failed: {error}"));
            let failures = state.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;
            if failures >= 10 {
                state.consecutive_failures.store(0, Ordering::Relaxed);
                logger::warn("Server unreachable after 10 consecutive failures. Waiting 10 minutes before reconnect.");
                tokio::time::sleep(Duration::from_secs(10 * 60)).await;
            }
        }
        Event::Packet(packet) => {
            if let ClientboundGamePacket::SetTime(p) = packet.as_ref() {
                let ticks = p.clock_updates.values().next().map(|c| c.total_ticks).unwrap_or(p.game_time);
                *state.world_time_ticks.write().expect("world_time_ticks lock poisoned") = ticks;

                // Correction step for the !day/!night fallback estimate -- snap to the
                // authoritative clock value and remember its rate for local ticking in
                // between corrections. Picking the first entry (matching world_time_ticks
                // above): resolving the specific "overworld" WorldClock key would require
                // the synced ClientboundRegistryData (ResolvableDataRegistry in azalea-core),
                // real infrastructure this doesn't have access to yet. In practice craftbot
                // never changes dimension, so clock_updates should only ever contain the one
                // relevant entry anyway.
                if let Some(clock) = p.clock_updates.values().next() {
                    logger::debug_cat("daynight", format!(
                        "day/night correction (clock_updates): total_ticks={} rate={}",
                        clock.total_ticks, clock.rate
                    ));
                    *state.day_ticks_accum.lock().expect("day_ticks_accum lock poisoned") =
                        clock.total_ticks as f64;
                    *state.day_clock_rate.lock().expect("day_clock_rate lock poisoned") = clock.rate;
                } else {
                    // Common case on this server: clock_updates is never populated, only
                    // game_time. game_time always increments +1/tick regardless of the
                    // daylight cycle's rate/pause state (confirmed via the decompiled
                    // ClientWorld.tickTime() -- only timeOfDay respects that, not the raw
                    // world-age counter), so absent an explicit rate/pause signal the day
                    // clock tracks it in lockstep. Don't touch day_clock_rate here: if a
                    // genuine non-default rate was ever communicated via a real
                    // clock_updates entry, a plain game_time-only packet shouldn't silently
                    // reset it back to the 1.0 default.
                    logger::debug_cat("daynight", format!(
                        "day/night correction (game_time fallback): game_time={}",
                        p.game_time
                    ));
                    *state.day_ticks_accum.lock().expect("day_ticks_accum lock poisoned") =
                        p.game_time as f64;
                }

                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let mut samples = state.tps_time_samples.lock().expect("tps_time_samples lock poisoned");
                samples.push_back((ticks, now_ms));
                let cutoff = now_ms.saturating_sub(60_000);
                while samples.front().map_or(false, |&(_, t)| t < cutoff) {
                    samples.pop_front();
                }
            } else if matches!(packet.as_ref(), ClientboundGamePacket::StartConfiguration(_)) {
                // Confirmed via reading Azalea's own source (pinned rev) that this does NOT
                // fire a Disconnect event on its own -- it silently flips the connection
                // into the Configuration protocol state internally, with no path back to
                // Play that we can observe. Confirmed live on RefinedVanilla that this is
                // exactly what its queue sends, so treat it the same as a failed probe:
                // give up on this connection and retry later rather than sit in limbo.
                logger::debug_cat("queue", "Received StartConfiguration packet -- likely bounced into the queue, disconnecting to retry later");
                disconnect_for_queue_retry(&bot, &state);
            }
        }

        Event::Tick => {
            flush_outbound_chat(&bot, &state).await;
            // No direct Azalea equivalent exists for Mineflayer's entitySpawn.
            // This scan is separately gated by entitySpawn to preserve Node config behavior.
            handle_entity_spawn_first_sight(&bot, &state);
            handle_player_detection(&bot, &state);

            // Free-run the !day/!night fallback estimate forward -- Event::Tick fires on
            // craftbot's own local ~20Hz wall-clock timer (confirmed independent of server
            // lag), so this keeps the estimate live between SetTime corrections instead of
            // going stale until the next packet arrives.
            let rate = *state.day_clock_rate.lock().expect("day_clock_rate lock poisoned") as f64;
            *state.day_ticks_accum.lock().expect("day_ticks_accum lock poisoned") += rate;
        }
        _ => {}
    }

    Ok(())
}

async fn flush_outbound_chat(bot: &Client, state: &AzaleaState) {
    let message = state
        .outbound_chat
        .lock()
        .expect("outbound chat queue lock poisoned")
        .pop_front();

    if let Some(message) = message {
        let message = match message.strip_prefix(crate::commands::SKIP_CENSOR_MARKER) {
            Some(rest) => rest.to_owned(),
            None => filter_outgoing_message(state, &message).await,
        };
        logger::chat(format!("Sending chat reply: {message}"));
        bot.chat(message);
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

fn mark_background_tasks_started(state: &AzaleaState) -> bool {
    let mut started = state
        .background_tasks_started
        .lock()
        .expect("background task state lock poisoned");
    if *started {
        return false;
    }

    *started = true;
    true
}

const ENTITY_SPAWN_GREETING_TTL_MS: u64 = 500_000;
const ENTITY_SPAWN_SCAN_INTERVAL_MS: i64 = 1_000;
const ENTITY_SPAWN_GREETINGS: &[&str] = &[
    "Hello {username}, Good day!",
    "Hope you're doing well today!",
    "Just wanted to say hi!",
    "Hello, {username}!",
    "Hi there, {username}!",
    "Greetings, {username}!",
    "Hey {username}, welcome!",
    "Hi {username}, nice to see you!",
    "Hello {username}, how's it going?",
    "Hey {username}, hope you're having a great day!",
];

fn event_disabled(state: &AzaleaState, names: &[&str]) -> bool {
    let disabled_events = state
        .runtime
        .read()
        .expect("runtime config lock poisoned")
        .disabled_events
        .clone();
    names.iter().any(|name| {
        disabled_events
            .iter()
            .any(|disabled| disabled.eq_ignore_ascii_case(name))
    })
}

fn handle_entity_spawn_first_sight(bot: &Client, state: &AzaleaState) {
    if event_disabled(state, &["entitySpawn"]) {
        return;
    }

    let now = now_millis_i64();
    {
        let mut next_scan = state
            .next_entity_spawn_scan_at
            .write()
            .expect("entity spawn scan lock poisoned");
        if now < *next_scan {
            return;
        }
        *next_scan = now.saturating_add(ENTITY_SPAWN_SCAN_INTERVAL_MS);
    }

    let bot_username = bot.username();
    let players = state
        .players
        .read()
        .expect("player cache lock poisoned")
        .values()
        .cloned()
        .collect::<Vec<_>>();

    for player in players {
        if player.username.eq_ignore_ascii_case(&bot_username) {
            continue;
        }

        if state
            .seen_entity_spawns
            .read()
            .expect("entity spawn seen lock poisoned")
            .contains(&player.uuid)
        {
            continue;
        }

        let Some(entity) = bot.entity_by_uuid(player.entity_uuid) else {
            continue;
        };

        {
            let mut seen = state
                .seen_entity_spawns
                .write()
                .expect("entity spawn seen lock poisoned");
            if !seen.insert(player.uuid.clone()) {
                continue;
            }
        }

        let position = entity.position();
        let x = round_one_decimal(position.x);
        let y = round_one_decimal(position.y);
        let z = round_one_decimal(position.z);
        logger::world(format!(
            "World: [{}] ({x:.1}, {y:.1}, {z:.1}) Spotted.",
            player.username
        ));

        if !event_disabled(state, &["entitySpawnGreeting"]) {
            let greeting = entity_spawn_greeting(&player.username, now);
            enqueue_outbound_chat(state, format!("/msg {} {}", player.username, greeting));
        }

        let seen = state.seen_entity_spawns.clone();
        let uuid = player.uuid;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(ENTITY_SPAWN_GREETING_TTL_MS)).await;
            seen.write()
                .expect("entity spawn seen lock poisoned")
                .remove(&uuid);
        });
    }
}

fn entity_spawn_greeting(username: &str, now: i64) -> String {
    let index = (now as usize).wrapping_add(username.len()) % ENTITY_SPAWN_GREETINGS.len();
    ENTITY_SPAWN_GREETINGS[index].replace("{username}", username)
}

const PLAYER_DETECTION_COOLDOWN_MS: u64 = 600_000;
const PLAYER_DETECTION_SCAN_INTERVAL_MS: i64 = 1_000;

fn handle_player_detection(bot: &Client, state: &AzaleaState) {
    if event_disabled(state, &["playerDetected"]) {
        return;
    }

    let now = now_millis_i64();
    {
        let mut next_scan = state
            .next_player_detection_scan_at
            .write()
            .expect("player detection scan lock poisoned");
        if now < *next_scan {
            return;
        }
        *next_scan = now.saturating_add(PLAYER_DETECTION_SCAN_INTERVAL_MS);
    }

    let bot_username = bot.username();
    let players = state
        .players
        .read()
        .expect("player cache lock poisoned")
        .values()
        .cloned()
        .collect::<Vec<_>>();

    for player in players {
        if player.username.eq_ignore_ascii_case(&bot_username) {
            continue;
        }

        if state
            .seen_player_detections
            .read()
            .expect("player detection seen lock poisoned")
            .contains(&player.uuid)
        {
            continue;
        }

        if bot.entity_by_uuid(player.entity_uuid).is_none() {
            continue;
        }

        {
            let mut seen = state
                .seen_player_detections
                .write()
                .expect("player detection seen lock poisoned");
            if !seen.insert(player.uuid.clone()) {
                continue;
            }
        }

        enqueue_outbound_chat(state, format!("{}, I can see you!", player.username));

        let seen = state.seen_player_detections.clone();
        let uuid = player.uuid;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(PLAYER_DETECTION_COOLDOWN_MS)).await;
            seen.write()
                .expect("player detection seen lock poisoned")
                .remove(&uuid);
        });
    }
}

fn round_one_decimal(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

async fn filter_outgoing_message(state: &AzaleaState, message: &str) -> String {
    let runtime = state
        .runtime
        .read()
        .expect("runtime config lock poisoned")
        .clone();
    let is_slash_command = message.trim_start().starts_with('/');

    let trie = *state.profanity_trie.read().expect("profanity_trie read");
    let threshold = profanity_filter::censor_threshold_from_config(&runtime.censor_threshold);
    let regular_censored = match trie {
        Some(trie) => profanity_filter::censor_message(trie, message, threshold),
        None => message.to_owned(),
    };

    let censored = if is_slash_command || !runtime.smart_censoring {
        regular_censored
    } else {
        maybe_smart_censor_message(message, &runtime)
            .await
            .map(|smart_censored| match trie {
                Some(trie) => profanity_filter::censor_message(trie, &smart_censored, threshold),
                None => smart_censored,
            })
            .unwrap_or(regular_censored)
    };

    if runtime.use_custom_chat_prefix && !is_slash_command {
        format!("{} {censored}", runtime.custom_chat_prefix)
    } else {
        censored
    }
}

async fn maybe_smart_censor_message(message: &str, runtime: &RuntimeConfig) -> Option<String> {
    let api_key = runtime.together_api_key.trim();
    if api_key.is_empty() {
        if !WARNED_MISSING_TOGETHER_KEY.swap(true, Ordering::Relaxed) {
            logger::warn(
                "Smart censor enabled but together_api_key is blank. Falling back to regular censor.",
            );
        }
        return None;
    }

    let user_text = sanitize_smart_censor_input(message);
    if user_text.is_empty() {
        return None;
    }

    let prompt = [
        "You are a strict Minecraft chat censor.",
        "Censor any text that would violate Mojang/Microsoft Minecraft EULA, Minecraft Usage Guidelines, or Minecraft community standards.",
        "Treat as unsafe: profanity, hate speech, slurs, sexual content, harassment, self-harm encouragement, threats, fraud/scams, extremist content, and suspicious obfuscated variants.",
        "If content is unsafe, censor the unsafe words/phrases so the final message is compliant and non-actionable.",
        "For each unsafe word, replace it with: first character + asterisks for the rest.",
        "Keep safe words as-is and preserve message structure as much as possible.",
        "Return exactly one line in this format: FINAL: <censored text>.",
    ]
    .join(" ");

    let request = reqwest::Client::new()
        .post("https://api.together.xyz/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&json!({
            "model": TOGETHER_MODEL,
            "messages": [
                {
                    "role": "system",
                    "content": "You output only policy-compliant censored Minecraft chat text and must never emit uncensored text that violates the Minecraft EULA, Usage Guidelines, or community standards."
                },
                {
                    "role": "user",
                    "content": format!("{prompt} Input: \"{user_text}\"")
                }
            ],
            "max_tokens": 5000,
            "temperature": 0.1,
            "top_p": 0.9
        }));

    let response = match tokio::time::timeout(
        Duration::from_millis(SMART_CENSOR_TIMEOUT_MS),
        request.send(),
    )
    .await
    {
        Ok(Ok(response)) => response,
        Ok(Err(error)) => {
            warn_smart_censor_failure(format!("Smart censor request failed ({error})."));
            return None;
        }
        Err(_) => {
            warn_smart_censor_failure("Smart censor request timed out.".to_owned());
            return None;
        }
    };

    let value = match response.json::<serde_json::Value>().await {
        Ok(value) => value,
        Err(error) => {
            warn_smart_censor_failure(format!("Smart censor response parse failed ({error})."));
            return None;
        }
    };

    let raw = value
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())?;
    let parsed = extract_smart_censor_final_reply(raw);
    if parsed.is_empty() {
        None
    } else {
        Some(parsed)
    }
}

fn sanitize_smart_censor_input(text: &str) -> String {
    text.chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(SMART_CENSOR_MAX_INPUT_LENGTH)
        .collect()
}

fn extract_smart_censor_final_reply(raw: &str) -> String {
    for line in raw
        .lines()
        .rev()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let value = line
            .strip_prefix("FINAL:")
            .or_else(|| line.strip_prefix("Final answer:"))
            .unwrap_or(line)
            .trim()
            .trim_matches('"')
            .trim_matches('\'');
        if !value.is_empty() {
            return value.chars().take(SMART_CENSOR_MAX_OUTPUT_LENGTH).collect();
        }
    }
    String::new()
}

fn warn_smart_censor_failure(message: String) {
    if !WARNED_SMART_CENSOR_FAILURE.swap(true, Ordering::Relaxed) {
        logger::warn(format!("{message} Falling back to regular censor."));
    }
}

/// Sends `probe_command` and waits up to 5s for vanilla's "unknown command" response.
/// Returns true if that response arrived (we're on the queue proxy), false on timeout
/// or any other response (assume we're on the real server).
async fn run_queue_probe(state: &AzaleaState, probe_command: &str) -> bool {
    let (tx, rx) = tokio::sync::oneshot::channel();
    *state
        .pending_queue_probe
        .lock()
        .expect("pending_queue_probe lock poisoned") = Some(tx);

    crate::commands::enqueue_chat(state, probe_command);

    match tokio::time::timeout(Duration::from_secs(5), rx).await {
        Ok(Ok(true)) => true,
        _ => {
            // Timed out (or the sender was dropped without sending) -- clear the
            // pending slot so a late, unrelated no-sender message can't be misread
            // as a queue-probe response by a future check.
            *state
                .pending_queue_probe
                .lock()
                .expect("pending_queue_probe lock poisoned") = None;
            false
        }
    }
}

/// Flags the next Event::Disconnect to sleep queue_retry_delay_ms (instead of the
/// normal short reconnect_time_ms) before Azalea's own reconnect proceeds, then
/// actually disconnects the bot.
fn disconnect_for_queue_retry(bot: &Client, state: &AzaleaState) {
    state.queue_disconnect_pending.store(true, Ordering::Relaxed);
    bot.disconnect();
}

async fn handle_fallback_message(bot: &Client, state: &AzaleaState, content: &str) {
    let mut full_msg = strip_minecraft_formatting(content);
    let words = split_right_carrot_in_first_word(&full_msg);
    full_msg = words.join(" ");

    let prefix = state
        .runtime
        .read()
        .expect("runtime config lock poisoned")
        .prefix
        .clone();

    let bot_username = bot.username();
    if let Some(whisper) = whisper_parser::parse(&full_msg, &bot_username) {
        let is_for_bot = whisper
            .recipient
            .as_deref()
            .is_none_or(|recipient| recipient.eq_ignore_ascii_case(&bot_username));
        if is_for_bot
            && whisper.message.starts_with(&prefix)
            && sender_allowed_for_command(state, &whisper.sender, &whisper.message)
        {
            command_handler::handle_as_whisper(bot, state, &whisper.sender, &whisper.message).await;
        }
        return;
    }

    if should_ignore_system_line(&full_msg) || full_msg.trim().is_empty() {
        return;
    }

    let raw_first_word = words.first().map(String::as_str).unwrap_or_default();
    let normalized_first_word = normalize_word(raw_first_word);
    let players = state
        .players
        .read()
        .expect("player cache lock poisoned")
        .clone();

    let custom_parsed = {
        let formats = state
            .runtime
            .read()
            .expect("runtime config lock poisoned")
            .custom_chat_formats
            .clone();
        chat_format_parser::parse(&full_msg, &formats)
            .map(|parsed| (parsed.username, parsed.message, true))
    };
    let divider_parsed = custom_parsed
        .clone()
        .or_else(|| parse_divider_message(&full_msg).map(|(p, m)| (p, m, true)));
    let extracted = divider_parsed
        .or_else(|| extract_player_message(&full_msg, &players).map(|(p, m)| (p, m, false)));

    let Some((player, message, is_chat_divider)) = extracted else {
        return;
    };

    if player.eq_ignore_ascii_case(&bot_username) {
        if !is_chat_divider && is_death_or_system_message(&full_msg, raw_first_word, &normalized_first_word) {
            let bot_uuid = players.get(&player).map(|p| p.uuid.clone()).unwrap_or_default();
            let murderer = find_murderer(&words, &players, &player);
            let murderer_uuid = murderer.as_deref().and_then(|name| players.get(name).map(|p| p.uuid.clone()));
            send_player_death(state, &player, &bot_uuid, &full_msg, murderer, murderer_uuid).await;
            logger::death(format!("Death: {full_msg}"));
        }
        return;
    }

    let uuid = players
        .get(&player)
        .map(|player| player.uuid.clone())
        .or_else(|| {
            players
                .values()
                .find(|p| p.username == player)
                .map(|p| p.uuid.clone())
        })
        .or_else(|| {
            players
                .values()
                .find(|p| {
                    p.display_name
                        .as_deref()
                        .is_some_and(|d| d.eq_ignore_ascii_case(&player))
                })
                .map(|p| p.uuid.clone())
        })
        .or_else(|| {
            state
                .nick_cache
                .read()
                .expect("nick cache lock poisoned")
                .get(&player)
                .cloned()
        });
    let uuid = match uuid {
        Some(uuid) => uuid,
        None => match state.api.convert_username_to_uuid(&player).await {
            Some(uuid) => uuid,
            None => return,
        },
    };

    if is_advancement_message(&full_msg) {
        if !full_msg.ends_with(']') || !full_msg.contains('[') {
            return;
        }
        if !full_msg.starts_with(raw_first_word) && !full_msg.starts_with(&normalized_first_word) {
            return;
        }
        send_player_advancement(state, &player, &uuid, &full_msg).await;
        logger::advancement(format!("Advancement: {full_msg}"));
        return;
    }

    if !is_chat_divider
        && is_death_or_system_message(&full_msg, raw_first_word, &normalized_first_word)
    {
        let murderer = find_murderer(&words, &players, &player);
        let murderer_uuid = murderer
            .as_deref()
            .and_then(|name| players.get(name).map(|player| player.uuid.clone()));
        crate::commands::duel::handle_death(state, &player, murderer.as_deref()).await;
        send_player_death(state, &player, &uuid, &full_msg, murderer, murderer_uuid).await;
        logger::death(format!("Death: {full_msg}"));
        return;
    }

    let parsed_command_message = message.trim();
    if parsed_command_message.starts_with(&prefix) {
        command_handler::handle(bot, state, &player, parsed_command_message).await;
        return;
    }

    send_minecraft_chat_message(state, &player, &message, &uuid).await;
    logger::chat(format!("{player}: {message}"));
}

fn handle_fadv_awards(state: &AzaleaState, data: FadvAwardsEvent) {
    if event_disabled(state, &["fadvAnnouncements"]) {
        return;
    }

    let Some(last) = data.awards.last() else { return; };

    let whisper_cmd = state
        .runtime
        .read()
        .expect("runtime config lock poisoned")
        .whisper_command
        .clone();

    enqueue_outbound_chat(
        state,
        format!(
            "📯 {} has made the advancement [{}] — {} 📯",
            data.username, last.name, last.description
        ),
    );

    if data.awards.len() > 1 {
        let others = data.awards[..data.awards.len() - 1]
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        enqueue_outbound_chat(
            state,
            format!(
                "/{} {} You also earned the following Forest Advancements: {}",
                whisper_cmd, data.username, others
            ),
        );
    }
}

fn spawn_websocket_event_task(bot: Client, state: AzaleaState) {
    let Some(websocket) = state.api.websocket.clone() else {
        return;
    };

    let mut events = websocket.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            match event {
                WebsocketEvent::Open => logger::websocket("Websocket connection opened."),
                WebsocketEvent::Close(reason) => {
                    logger::websocket(format!("Websocket connection closed: {reason}"));
                }
                WebsocketEvent::Error(error) => {
                    logger::websocket(format!("Websocket error: {error}"));
                }
                WebsocketEvent::KeyAccepted(_) => {
                    logger::websocket("Websocket API key accepted by hub.");
                }
                WebsocketEvent::NewName(data) => {
                    let should_welcome = {
                        let runtime = state.runtime.read().expect("runtime config lock poisoned");
                        runtime.welcome_messages && data.server == state.mc_server
                    };
                    if should_welcome {
                        enqueue_outbound_chat(
                            &state,
                            format!(
                                "{}, previously known as {} joined the server!",
                                data.new_name, data.old_name
                            ),
                        );
                    }
                }
                WebsocketEvent::NewUser(data) => {
                    let should_welcome = {
                        let runtime = state.runtime.read().expect("runtime config lock poisoned");
                        runtime.welcome_messages && data.server == state.mc_server
                    };
                    if should_welcome {
                        enqueue_outbound_chat(
                            &state,
                            format!("{}, First time here? Welcome!", data.user),
                        );
                    }
                }
                WebsocketEvent::InboundDiscordChat(data) => {
                    handle_inbound_discord_chat(&bot, &state, data).await;
                }
                WebsocketEvent::InboundMinecraftChat(data) => {
                    handle_inbound_minecraft_chat(&state, data);
                }
                WebsocketEvent::ScammerMarked(data) => {
                    if !event_disabled(&state, &["scammerAnnouncements"]) {
                        spawn_announcement(&state, data.user_id, |dn| {
                            format!("📢 {} has been marked as a scammer by trading mods, proceed with caution 📢", dn)
                        });
                    }
                }
                WebsocketEvent::ScammerUnmarked(data) => {
                    if !event_disabled(&state, &["scammerAnnouncements"]) {
                        spawn_announcement(&state, data.user_id, |dn| {
                            format!("📢 {} has had their scammer mark removed by trading mods 📢", dn)
                        });
                    }
                }
                WebsocketEvent::TradesReset(data) => {
                    if !event_disabled(&state, &["tradeResetAnnouncements"]) {
                        let reason = data.reason;
                        spawn_announcement(&state, data.user_id, move |dn| {
                            format!("📢 {}'s trades have been reset by trading mods. Description: {} 📢", dn, reason)
                        });
                    }
                }
                WebsocketEvent::TradesUnreset(data) => {
                    if !event_disabled(&state, &["tradeResetAnnouncements"]) {
                        let reason = data.reason;
                        spawn_announcement(&state, data.user_id, move |dn| {
                            format!("📢 {}'s trades have been restored by trading mods. Description: {} 📢", dn, reason)
                        });
                    }
                }
                WebsocketEvent::FadvAwards(data) => {
                    handle_fadv_awards(&state, data);
                }
                WebsocketEvent::PearlResult(data) => {
                    let runtime = state.runtime.read().expect("runtime lock poisoned");
                    let whisper_cmd = runtime.whisper_command.clone();
                    drop(runtime);
                    let msg = if data.success {
                        format!("🟢 Pearl activated!")
                    } else {
                        format!("🔴 Pearl failed: {}", data.message)
                    };
                    enqueue_outbound_chat(&state, format!("/{whisper_cmd} {} {msg}", data.requester));
                }
                WebsocketEvent::ResolveDiscordUsernameResult(data) => {
                    let result = match data.snowflake {
                        Some(snowflake) => DiscordResolution::Found(snowflake),
                        None => DiscordResolution::NotFound,
                    };
                    fulfill_discord_resolution(&state, &data.request_id, result);
                }
                WebsocketEvent::ResolveDiscordUsernameUnavailable(data) => {
                    fulfill_discord_resolution(&state, &data.request_id, DiscordResolution::Unavailable);
                }
                WebsocketEvent::CasinoDrawResult(data) => {
                    enqueue_outbound_chat(&state, &data.message);
                }
                WebsocketEvent::CasinoWinnerNotify(data) => {
                    enqueue_outbound_chat(&state, format!("/msg {} {}", data.player, data.message));
                }
                WebsocketEvent::UnknownMessage(message) => {
                    logger::websocket(format!("Unknown websocket message: {message}"));
                }
                WebsocketEvent::Ignored
                | WebsocketEvent::MinecraftPlayerDeath(_)
                | WebsocketEvent::MinecraftPlayerKill(_)
                | WebsocketEvent::MinecraftPlayerJoin(_)
                | WebsocketEvent::MinecraftPlayerLeave(_)
                | WebsocketEvent::MinecraftAdvancement(_) => {}
            }
        }
    });
}

fn spawn_player_list_update_task(state: AzaleaState) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(60));
        loop {
            ticker.tick().await;
            send_player_list_update(&state).await;
        }
    });
}

async fn resolve_scammer_display(state: &AzaleaState, user_id: &str) -> String {
    if user_id.contains('-') {
        state.api.tradebot_mc_username(user_id).await
            .unwrap_or_else(|| user_id.to_owned())
    } else if let Some(linked_uuid) = state.api.tradebot_linked_mc_uuid(user_id).await {
        state.api.tradebot_mc_username(&linked_uuid).await
            .unwrap_or(linked_uuid)
    } else {
        user_id.to_owned()
    }
}

fn spawn_announcement<F>(state: &AzaleaState, user_id: String, make_msg: F)
where
    F: FnOnce(String) -> String + Send + 'static,
{
    let state_clone = state.clone();
    tokio::spawn(async move {
        let display_name = resolve_scammer_display(&state_clone, &user_id).await;
        enqueue_outbound_chat(&state_clone, make_msg(display_name));
    });
}

async fn handle_inbound_discord_chat(bot: &Client, state: &AzaleaState, data: DiscordChatMessage) {
    let (allow_chatbridge_input, prefix, use_commands) = {
        let runtime = state.runtime.read().expect("runtime config lock poisoned");
        (
            runtime.allow_chatbridge_input,
            runtime.prefix.clone(),
            runtime.use_commands,
        )
    };

    if !allow_chatbridge_input || data.mc_server != state.mc_server {
        return;
    }

    // If the message is a real, known command, dispatch it through the exact same
    // path real MC chat commands use — sender is tagged "Discord:<username>" so it
    // never collides with (or gets mistaken for) a real MC player identity. Unknown
    // "!word" text and non-command chat both fall through to the plain relay below,
    // matching how real players' unrecognized "!word" chat still just shows as chat.
    if use_commands {
        if let Some(command_line) = data.message.trim().strip_prefix(&prefix) {
            if let Some(command_name) = command_line.split_whitespace().next() {
                if let Some(command) = crate::commands::find(command_name) {
                    // Check every alias of the matched command, not just the one typed —
                    // classification (and the Hub push in mod.rs) treats a command as unsafe
                    // if ANY of its aliases is listed, so a single alias like "!p" for "!pearl"
                    // must inherit "pearl"'s unsafe status even if "p" itself isn't listed.
                    let is_unsafe = {
                        let unsafe_names = state
                            .bridge_unsafe_commands
                            .read()
                            .expect("bridge_unsafe_commands lock poisoned");
                        command
                            .names
                            .iter()
                            .any(|name| unsafe_names.contains(&name.to_lowercase()))
                    };

                    if is_unsafe {
                        // Backstop only — Discord's own gate (messageCreate.ts) should already
                        // have caught this and DMed the user before it ever reached craftbot.
                        // Never post to public MC chat here: craftbot has no Discord user ID to
                        // reply to privately (DiscordChatMessage carries only a username string),
                        // and a public leak is worse than staying silent on this rare fallback path.
                        logger::command(format!(
                            "Bridge-unsafe command blocked (backstop): {command_name} from Discord:{}",
                            data.username
                        ));
                    } else {
                        let sender = format!("Discord:{}", data.username);
                        command_handler::handle(bot, state, &sender, &data.message).await;
                    }
                    return;
                }
            }
        }
    }

    enqueue_outbound_chat(
        state,
        format!("[Discord] {}: {}", data.username, data.message),
    );
}

fn handle_inbound_minecraft_chat(state: &AzaleaState, data: MinecraftChatMessage) {
    let allow_chatbridge_input = state
        .runtime
        .read()
        .expect("runtime config lock poisoned")
        .allow_chatbridge_input;

    let target_server = data.mc_server.to_lowercase();
    let current_server = state.mc_server.to_lowercase();
    let is_for_this_server = target_server == "all" || target_server == current_server;
    let is_own_origin = data
        .origin_server
        .as_deref()
        .is_some_and(|origin| origin.to_lowercase() == current_server);
    let is_shout = data.relay_type.as_deref() == Some("shout");

    if allow_chatbridge_input && is_shout && is_for_this_server && !is_own_origin {
        enqueue_outbound_chat(state, data.message);
    }
}

fn websocket(state: &AzaleaState) -> Option<&WebsocketClient> {
    state.api.websocket.as_ref()
}

fn player_presence_fields(
    state: &AzaleaState,
    username: &str,
    uuid: &str,
) -> (String, String, String, String) {
    (
        username.to_owned(),
        uuid.to_owned(),
        now_millis_string(),
        state.mc_server.clone(),
    )
}

async fn send_minecraft_chat_message(
    state: &AzaleaState,
    username: &str,
    message: &str,
    uuid: &str,
) {
    let Some(websocket) = websocket(state) else {
        return;
    };

    if let Err(error) = websocket
        .send_minecraft_chat_message(MinecraftChatMessage {
            name: username.to_owned(),
            message: message.to_owned(),
            date: now_millis_string(),
            mc_server: state.mc_server.clone(),
            uuid: uuid.to_owned(),
            relay_type: None,
            origin_server: None,
        })
        .await
    {
        logger::websocket(format!("Failed to send websocket chat message: {error}"));
    }
}

async fn send_player_join(state: &AzaleaState, username: &str, uuid: &str) {
    let Some(websocket) = websocket(state) else {
        return;
    };

    if let Err(error) = websocket
        .send_player_join({
            let (username, uuid, timestamp, server) = player_presence_fields(state, username, uuid);
            MinecraftPlayerJoinMessage {
                username,
                uuid,
                timestamp,
                server,
            }
        })
        .await
    {
        logger::websocket(format!("Failed to send websocket player join: {error}"));
    }
}

async fn send_player_leave(state: &AzaleaState, username: &str, uuid: &str) {
    let Some(websocket) = websocket(state) else {
        return;
    };

    if let Err(error) = websocket
        .send_player_leave({
            let (username, uuid, timestamp, server) = player_presence_fields(state, username, uuid);
            MinecraftPlayerLeaveMessage {
                username,
                uuid,
                timestamp,
                server,
            }
        })
        .await
    {
        logger::websocket(format!("Failed to send websocket player leave: {error}"));
    }
}

async fn send_player_list_update(state: &AzaleaState) {
    let Some(websocket) = websocket(state) else {
        return;
    };

    let players = state
        .players
        .read()
        .expect("player cache lock poisoned")
        .values()
        .map(|player| WebsocketPlayer {
            username: player.username.clone(),
            uuid: player.uuid.clone(),
            latency: player.latency,
            server: state.mc_server.clone(),
        })
        .collect::<Vec<_>>();

    if players.is_empty() {
        return;
    }

    if let Err(error) = websocket.send_player_list_update(players).await {
        logger::websocket(format!(
            "Failed to send websocket player list update: {error}"
        ));
    }
}

async fn send_session_flush_leave(state: &AzaleaState) {
    let Some(websocket) = websocket(state) else {
        return;
    };

    if let Err(error) = websocket
        .send_player_leave({
            let (username, uuid, timestamp, server) =
                player_presence_fields(state, "ForestBot", "");
            MinecraftPlayerLeaveMessage {
                username,
                uuid,
                timestamp,
                server,
            }
        })
        .await
    {
        logger::websocket(format!(
            "Failed to send websocket session flush leave: {error}"
        ));
    }
}

fn extract_advancement_key(message: &str) -> Option<&str> {
    let start = message.rfind('[')?;
    let end = message.rfind(']')?;
    if end > start {
        Some(&message[start..=end])
    } else {
        None
    }
}

async fn send_player_advancement(
    state: &AzaleaState,
    username: &str,
    uuid: &str,
    advancement: &str,
) {
    let Some(key) = extract_advancement_key(advancement) else {
        return;
    };

    let needs_fetch = {
        let seen = state
            .seen_advancements
            .lock()
            .expect("seen_advancements lock poisoned");
        !seen.contains_key(uuid)
    };

    if needs_fetch {
        let existing = state
            .api
            .get_advancements(uuid, &state.mc_server, 500, "ASC")
            .await
            .unwrap_or_default();
        let keys: HashSet<String> = existing
            .iter()
            .filter_map(|a| extract_advancement_key(&a.advancement).map(str::to_owned))
            .collect();
        state
            .seen_advancements
            .lock()
            .expect("seen_advancements lock poisoned")
            .insert(uuid.to_owned(), keys);
    }

    let is_duplicate = {
        let mut seen = state
            .seen_advancements
            .lock()
            .expect("seen_advancements lock poisoned");
        let player_seen = seen.entry(uuid.to_owned()).or_default();
        if player_seen.contains(key) {
            true
        } else {
            player_seen.insert(key.to_owned());
            false
        }
    };

    if is_duplicate {
        logger::advancement(format!(
            "Redundant advancement skipped for {username}: {key}"
        ));
        crate::commands::enqueue_chat(
            state,
            format!(
                "/{} {} Your advancement will not be recorded by me, as you already have that advancement on this server.",
                state.runtime.read().expect("runtime lock poisoned").whisper_command,
                username
            ),
        );
        return;
    }

    let Some(websocket) = websocket(state) else {
        return;
    };

    if let Err(error) = websocket
        .send_player_advancement(MinecraftAdvancementMessage {
            username: username.to_owned(),
            advancement: advancement.to_owned(),
            time: now_millis_i64(),
            mc_server: state.mc_server.clone(),
            id: None,
            uuid: uuid.to_owned(),
        })
        .await
    {
        logger::websocket(format!("Failed to send websocket advancement: {error}"));
    }
}

async fn send_player_death(
    state: &AzaleaState,
    victim: &str,
    victim_uuid: &str,
    death_message: &str,
    murderer: Option<String>,
    murderer_uuid: Option<String>,
) {
    let Some(websocket) = websocket(state) else {
        return;
    };

    let death_type = if murderer.is_some() { "pvp" } else { "pve" };
    if let Err(error) = websocket
        .send_player_death(MinecraftPlayerDeathMessage {
            victim: victim.to_owned(),
            death_message: death_message.to_owned(),
            murderer,
            time: now_millis_i64(),
            death_type: death_type.to_owned(),
            mc_server: state.mc_server.clone(),
            id: None,
            victim_uuid: victim_uuid.to_owned(),
            murderer_uuid,
        })
        .await
    {
        logger::websocket(format!("Failed to send websocket death: {error}"));
    }
}

fn spawn_reminder_tick_task(state: AzaleaState, active: Arc<AtomicBool>) {
    tokio::spawn(async move {
        while active.load(Ordering::Relaxed) {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            if !active.load(Ordering::Relaxed) {
                break;
            }
            deliver_timed_reminders(&state).await;
        }
    });
}

async fn deliver_timed_reminders(state: &AzaleaState) {
    let Ok(messages) = load_offline_messages().await else {
        return;
    };
    let now = now_millis_i64() as u64;
    let mut remaining = Vec::new();

    for msg in messages {
        match msg.deliver_at {
            Some(at) if at <= now => {
                let is_online = state
                    .players
                    .read()
                    .expect("players lock poisoned")
                    .contains_key(&msg.recipient);
                if is_online {
                    enqueue_outbound_chat(
                        state,
                        format!("/msg {} Reminder: {}", msg.recipient, msg.message),
                    );
                } else {
                    remaining.push(msg);
                }
            }
            _ => remaining.push(msg),
        }
    }

    if let Err(error) = save_offline_messages(&remaining).await {
        logger::warn(format!("Failed to save offline messages: {error:#}"));
    }
}

async fn deliver_offline_messages(state: &AzaleaState, username: &str) {
    if event_disabled(state, &["offlineMessages"]) {
        return;
    }
    let Ok(messages) = load_offline_messages().await else {
        return;
    };
    let mut pending = Vec::new();
    let mut remaining = Vec::new();

    for message in messages {
        let is_for_user = message.recipient.eq_ignore_ascii_case(username);
        let is_timed = message.deliver_at.is_some();
        if is_for_user && !is_timed {
            pending.push(message);
        } else {
            remaining.push(message);
        }
    }

    let offline_msgs: Vec<_> = pending
        .iter()
        .filter(|m| !m.sender.eq_ignore_ascii_case(username))
        .collect();
    let reminders: Vec<_> = pending
        .iter()
        .filter(|m| m.sender.eq_ignore_ascii_case(username))
        .collect();

    if !offline_msgs.is_empty() {
        enqueue_outbound_chat(
            state,
            format!(
                "/msg {username} You have {} pending offline message(s), I will send them to you now.",
                offline_msgs.len()
            ),
        );
        for message in &offline_msgs {
            enqueue_outbound_chat(
                state,
                format!(
                    "/msg {username} From {}: {} | {}",
                    message.sender,
                    message.message,
                    crate::functions::utils::time::convert_unix_timestamp(message.timestamp / 1000)
                ),
            );
        }
    }

    for message in &reminders {
        enqueue_outbound_chat(
            state,
            format!("/msg {username} Reminder: {}", message.message),
        );
    }

    if let Err(error) = save_offline_messages(&remaining).await {
        logger::warn(format!("Failed to save offline messages: {error:#}"));
    }
}

async fn deliver_casino_notifications(state: &AzaleaState, player_uuid: &str, username: &str) {
    let messages = state.api.casino_claim_notifications(player_uuid).await;
    for message in messages {
        enqueue_outbound_chat(state, format!("/msg {username} {message}"));
    }
}

async fn fire_greeting_if_due(state: &AzaleaState, username: &str) {
    if event_disabled(state, &["greetings"]) {
        return;
    }
    let Some((Some(greeting), last_fired_at)) = state.api.tradebot_get_greeting(username).await else {
        return;
    };
    if let Some(last) = last_fired_at {
        // Parse MariaDB datetime: "YYYY-MM-DD HH:MM:SS"
        let normalized = if last.ends_with('Z') { last.clone() } else { last.replace(' ', "T") + "Z" };
        if let Ok(parsed) = normalized.parse::<chrono::DateTime<Utc>>() {
            let elapsed = Utc::now().signed_duration_since(parsed);
            if elapsed.num_seconds() < 12 * 3600 {
                return;
            }
        }
    }
    enqueue_outbound_chat(state, format!("{greeting}, {username}!"));
    state.api.tradebot_fire_greeting(username).await;
}

fn enqueue_outbound_chat(state: &AzaleaState, message: impl AsRef<str>) {
    state
        .outbound_chat
        .lock()
        .expect("outbound chat queue lock poisoned")
        .push_back(message.as_ref().trim_start().to_owned());
}

fn resolve_sender_uuid(state: &AzaleaState, sender: &str) -> Option<String> {
    state
        .players
        .read()
        .expect("player cache lock poisoned")
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(sender))
        .map(|(_, player)| player.uuid.clone())
}

fn sender_allowed_for_command(state: &AzaleaState, sender: &str, message: &str) -> bool {
    let runtime = state.runtime.read().expect("runtime config lock poisoned");
    let Some(uuid) = resolve_sender_uuid(state, sender) else {
        return true;
    };
    !runtime.user_blacklist.contains(&uuid)
        || whisper_parser::is_self_standing_command(message, &runtime.prefix)
}

// Result of gating a command from a sender that couldn't be resolved to a real, currently
// online MC player -- i.e. a message relayed through an official (non-craftbot) Discord chat
// bridge. There's no other way for craftbot to see a "sender" without a matching UUID.
enum BridgeSenderStatus {
    Allowed,
    Blacklisted,
    NotFound,
    Unavailable,
}

// Asks discordbot (via Hub) to resolve a chat-bridge "server username" to a real Discord
// snowflake, then checks that snowflake directly against mc_blacklist.json (deliberately NOT
// going through the trade-account-linking system -- a separate, unrelated concern). Live
// round trip every time, not cached: usernames can change, and a stale cache would let a
// renamed blacklisted account evade this.
async fn resolve_and_check_bridge_sender(state: &AzaleaState, sender: &str) -> BridgeSenderStatus {
    let Some(ws) = state.api.websocket.as_ref() else {
        return BridgeSenderStatus::Unavailable;
    };

    let request_id = uuid::Uuid::new_v4().to_string();
    let (tx, rx) = tokio::sync::oneshot::channel();
    state
        .pending_discord_resolutions
        .lock()
        .expect("pending_discord_resolutions lock poisoned")
        .insert(request_id.clone(), tx);

    if ws
        .send_message(
            "resolve_discord_username",
            json!({ "request_id": request_id, "username": sender }),
        )
        .await
        .is_err()
    {
        state
            .pending_discord_resolutions
            .lock()
            .expect("pending_discord_resolutions lock poisoned")
            .remove(&request_id);
        return BridgeSenderStatus::Unavailable;
    }

    match tokio::time::timeout(Duration::from_secs(5), rx).await {
        Ok(Ok(DiscordResolution::Found(snowflake))) => {
            let blacklisted = state
                .runtime
                .read()
                .expect("runtime config lock poisoned")
                .user_blacklist
                .contains(&snowflake);
            if blacklisted {
                BridgeSenderStatus::Blacklisted
            } else {
                BridgeSenderStatus::Allowed
            }
        }
        Ok(Ok(DiscordResolution::NotFound)) => BridgeSenderStatus::NotFound,
        Ok(Ok(DiscordResolution::Unavailable)) | Ok(Err(_)) | Err(_) => {
            // Timed out or the sender was dropped -- clean up a stale pending entry if the
            // reply arrives late after we've already given up on it.
            state
                .pending_discord_resolutions
                .lock()
                .expect("pending_discord_resolutions lock poisoned")
                .remove(&request_id);
            BridgeSenderStatus::Unavailable
        }
    }
}

fn fulfill_discord_resolution(state: &AzaleaState, request_id: &str, result: DiscordResolution) {
    let mut pending = state
        .pending_discord_resolutions
        .lock()
        .expect("pending_discord_resolutions lock poisoned");
    if let Some(tx) = pending.remove(request_id) {
        let _ = tx.send(result);
    }
}

fn now_millis_string() -> String {
    now_millis_i64().to_string()
}

fn now_millis_i64() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(i64::MAX)
}

fn strip_minecraft_formatting(message: &str) -> String {
    let mut output = String::with_capacity(message.len());
    let mut skip = false;
    for ch in message.chars() {
        if skip {
            skip = false;
            continue;
        }
        if ch == '§' || ch == '&' {
            skip = true;
            continue;
        }
        output.push(ch);
    }
    output
}

fn split_right_carrot_in_first_word(message: &str) -> Vec<String> {
    let mut words = message
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if let Some(first) = words.first()
        && first.ends_with('>')
        && first.len() > 1
    {
        let first = words.remove(0);
        words.insert(0, ">".to_owned());
        words.insert(0, first.trim_end_matches('>').to_owned());
    }
    words
}

fn should_ignore_system_line(message: &str) -> bool {
    const IGNORE_CONTAINS: &[&str] = &[
        "joined the game",
        "left the game",
        "joined the server",
        "left the server",
        "voted",
        "kicked",
        "banned",
        "muted",
        "tempbanned",
        "temp-banned",
        "has requested to teleport to you.",
        "whisper",
        "[Rcon]",
    ];
    IGNORE_CONTAINS
        .iter()
        .any(|phrase| message.contains(phrase))
        || message.starts_with("From ")
        || message.starts_with("To ")
}

fn parse_divider_message(message: &str) -> Option<(String, String)> {
    for separator in [" » ", " >> ", " > "] {
        if let Some((player, msg)) = message.split_once(separator) {
            return Some((normalize_word(player.trim()), msg.trim().to_owned()));
        }
    }
    None
}

fn extract_player_message(
    message: &str,
    players: &HashMap<String, PlayerSnapshot>,
) -> Option<(String, String)> {
    let words = message
        .split_whitespace()
        .take(2)
        .map(normalize_word)
        .collect::<Vec<_>>();

    for word in words {
        for real_name in players.keys() {
            if word == *real_name {
                return Some((
                    real_name.clone(),
                    remove_player_from_message(message, real_name),
                ));
            }
        }
    }
    None
}

fn remove_player_from_message(message: &str, player: &str) -> String {
    message
        .replace(&format!("<{player}>"), " ")
        .replace(&format!("{player}:"), " ")
        .replace(player, " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_word(word: &str) -> String {
    word.trim_matches(|ch| ch == '<' || ch == '>')
        .trim_end_matches(':')
        .to_owned()
}

fn is_advancement_message(message: &str) -> bool {
    message.contains("has reached the goal")
        || message.contains("has made the advancement")
        || message.contains("has completed the challenge")
}

fn is_death_or_system_message(message: &str, raw_first_word: &str, normalized_first: &str) -> bool {
    if raw_first_word.ends_with(':') {
        return false;
    }
    if message.starts_with(&format!("<{normalized_first}>")) {
        return false;
    }
    if message.contains('<') && message.contains('>') {
        return false;
    }
    if !message.starts_with(normalized_first) {
        return false;
    }
    matches!(
        message.chars().nth(normalized_first.chars().count()),
        None | Some(' ')
    )
}

fn find_murderer(
    words: &[String],
    players: &HashMap<String, PlayerSnapshot>,
    victim: &str,
) -> Option<String> {
    for word in words.iter().skip(1) {
        let token = normalize_word(word);
        if token != victim && players.contains_key(&token) {
            return Some(token);
        }
    }
    None
}

fn is_server_presence_message(content: &str) -> bool {
    let content = content.trim();
    content.ends_with(" joined the server.")
        || content.ends_with(" left the server.")
        || content.ends_with(" joined the game")
        || content.ends_with(" left the game")
}

fn parse_chat_message(message: &ChatPacket, state: &AzaleaState) -> (Option<String>, String) {
    let full_message = message.message().to_string();
    logger::debug_cat("chat_parse", format!("[CHAT_PARSE] full={full_message:?}"));

    let formats = state
        .runtime
        .read()
        .expect("runtime config lock poisoned")
        .custom_chat_formats
        .clone();

    // Custom formats take priority over Azalea's packet-level sender extraction.
    // On servers with chat plugins, Azalea extracts the bracket-wrapped display name
    // as the sender but leaves the redundant "Username » message" as content,
    // breaking command dispatch. Matching the full raw string against a configured
    // format first produces the correct username + message split.
    if !formats.is_empty() {
        logger::debug_cat("chat_parse", format!("[CHAT_PARSE] trying {} custom formats: {:?}", formats.len(), formats));
        if let Some(parsed) = chat_format_parser::parse(&full_message, &formats) {
            logger::debug_cat("chat_parse", format!("[CHAT_PARSE] custom match → sender={:?} content={:?}", parsed.username, parsed.message));
            return (Some(parsed.username), parsed.message);
        }
        logger::debug_cat("chat_parse", "[CHAT_PARSE] no custom format matched → fallback to native".to_string());
    } else {
        logger::debug_cat("chat_parse", "[CHAT_PARSE] formats empty → native parse".to_string());
    }

    let (sender, content) = message.split_sender_and_content();
    if let Some(sender) = sender {
        if sender.eq_ignore_ascii_case("PM") && content.contains(" → ") && content.contains(" » ")
        {
            return (None, full_message);
        }

        return (
            Some(chat_format_parser::normalize_username(&sender)),
            content,
        );
    }

    if let Some(parsed) = chat_format_parser::parse(&full_message, &formats) {
        return (Some(parsed.username), parsed.message);
    }

    (None, full_message)
}
