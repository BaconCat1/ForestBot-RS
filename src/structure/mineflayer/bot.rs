use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Context;
use azalea::ClientInformation;
use azalea::protocol::packets::game::ClientboundGamePacket;
use azalea::app::PluginGroup;
use azalea::bot::DefaultBotPlugins;
use azalea::chat_signing::ChatSigningPlugin;
use azalea::client_chat::ChatPacket;
use azalea::prelude::*;
use azalea_viaversion::ViaVersionPlugin;
use serde_json::json;
use uuid::Uuid;

use crate::commands::stat_history;
use crate::config::{
    AppState, BotConfig, CommandCooldownConfig, load_offline_messages, load_word_list,
    save_offline_messages,
};
use crate::structure::{
    endpoints::endpoints::{
        ApiClient, DiscordChatMessage, MinecraftAdvancementMessage, MinecraftChatMessage,
        MinecraftPlayerDeathMessage, MinecraftPlayerJoinMessage, MinecraftPlayerLeaveMessage,
        Player as WebsocketPlayer, WebsocketClient, WebsocketEvent,
    },
    logger,
    mineflayer::utils::{announce, anti_afk, chat_format_parser, command_handler, profanity_filter, whisper_parser},
};

const BAD_WORDS_PATH: &str = "./json/bad_words.json";
const WORD_WHITELIST_PATH: &str = "./json/word_whitelist.json";
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
    pub welcome_messages: bool,
    pub use_custom_chat_prefix: bool,
    pub custom_chat_prefix: String,
    pub smart_censoring: bool,
    pub together_api_key: String,
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
    pub smart_censoring: bool,
    pub together_api_key: String,
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
            smart_censoring: state.config.smart_censoring,
            together_api_key: state.config.together_api_key.clone(),
            anti_spam_global_cooldown_ms: state.config.anti_spam_global_cooldown,
            command_cooldowns: state.config.command_cooldowns.clone(),
            reconnect_time_ms: state.config.reconnect_time,
            restart_count: 0,
            is_connected: false,
            allow_connection: true,
            api,
            antiafk: state.config.antiafk,
            announce: state.config.announce,
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
                welcome_messages: self.welcome_messages,
                use_custom_chat_prefix: self.use_custom_chat_prefix,
                custom_chat_prefix: self.custom_chat_prefix.clone(),
                smart_censoring: self.smart_censoring,
                together_api_key: self.together_api_key.clone(),
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
            initial_spawn_done: Arc::new(AtomicBool::new(false)),
            antiafk: self.antiafk,
            antiafk_active: Arc::new(AtomicBool::new(false)),
            announce: self.announce,
            announce_active: Arc::new(AtomicBool::new(false)),
            consecutive_failures: Arc::new(AtomicU32::new(0)),
            seen_advancements: Arc::new(Mutex::new(HashMap::new())),
            nick_cache: Arc::new(RwLock::new(HashMap::new())),
            world_time_ticks: Arc::new(RwLock::new(0)),
        };

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
    pub initial_spawn_done: Arc<AtomicBool>,
    pub antiafk: bool,
    pub antiafk_active: Arc<AtomicBool>,
    pub announce: bool,
    pub announce_active: Arc<AtomicBool>,
    pub consecutive_failures: Arc<AtomicU32>,
    pub seen_advancements: Arc<Mutex<HashMap<String, HashSet<String>>>>,
    pub nick_cache: Arc<RwLock<HashMap<String, String>>>,
    pub world_time_ticks: Arc<RwLock<u64>>,
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
                welcome_messages: false,
                use_custom_chat_prefix: false,
                custom_chat_prefix: String::new(),
                smart_censoring: false,
                together_api_key: String::new(),
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
            initial_spawn_done: Arc::new(AtomicBool::new(false)),
            antiafk: false,
            antiafk_active: Arc::new(AtomicBool::new(false)),
            announce: false,
            announce_active: Arc::new(AtomicBool::new(false)),
            consecutive_failures: Arc::new(AtomicU32::new(0)),
            seen_advancements: Arc::new(Mutex::new(HashMap::new())),
            nick_cache: Arc::new(RwLock::new(HashMap::new())),
            world_time_ticks: Arc::new(RwLock::new(0)),
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
                spawn_websocket_event_task(state.clone());
                spawn_player_list_update_task(state.clone());
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

                let prefix = state
                    .runtime
                    .read()
                    .expect("runtime config lock poisoned")
                    .prefix
                    .clone();

                if content.starts_with(&prefix) {
                    if sender_allowed_for_command(&state, &sender, &content) {
                        command_handler::handle(&bot, &state, &sender, &content).await;
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
            deliver_offline_messages(&state, &username).await;
            if state.initial_spawn_done.load(Ordering::Relaxed) {
                send_player_join(&state, &username, &uuid).await;
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
            send_session_flush_leave(&state).await;
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
                if let Some(clock) = p.clock_updates.values().next() {
                    *state.world_time_ticks.write().expect("world_time_ticks lock poisoned") = clock.total_ticks;
                }
            }
        }
        Event::Tick => {
            flush_outbound_chat(&bot, &state).await;
            // No direct Azalea equivalent exists for Mineflayer's entitySpawn.
            // This scan is separately gated by entitySpawn to preserve Node config behavior.
            handle_entity_spawn_first_sight(&bot, &state);
            handle_player_detection(&bot, &state);
        }
        _ => {}
    }

    Ok(())
}

async fn flush_outbound_chat(bot: &Client, state: &AzaleaState) {
    let messages = {
        let mut queue = state
            .outbound_chat
            .lock()
            .expect("outbound chat queue lock poisoned");
        let mut messages = Vec::new();
        for _ in 0..3 {
            let Some(message) = queue.pop_front() else {
                break;
            };
            messages.push(message);
        }
        messages
    };

    for message in messages {
        let message = filter_outgoing_message(state, &message).await;
        logger::chat(format!("Sending chat reply: {message}"));
        bot.chat(message);
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

        let greeting = entity_spawn_greeting(&player.username, now);
        enqueue_outbound_chat(state, format!("/msg {} {}", player.username, greeting));

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

    let bad_words = load_word_list(BAD_WORDS_PATH).await.unwrap_or_default();
    let word_whitelist = load_word_list(WORD_WHITELIST_PATH)
        .await
        .unwrap_or_default();
    let regular_censored = profanity_filter::censor_bad_words(message, &bad_words, &word_whitelist);

    let censored = if is_slash_command || !runtime.smart_censoring {
        regular_censored
    } else {
        maybe_smart_censor_message(message, &runtime)
            .await
            .map(|smart_censored| {
                profanity_filter::censor_bad_words(&smart_censored, &bad_words, &word_whitelist)
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

fn spawn_websocket_event_task(state: AzaleaState) {
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
                    handle_inbound_discord_chat(&state, data);
                }
                WebsocketEvent::InboundMinecraftChat(data) => {
                    handle_inbound_minecraft_chat(&state, data);
                }
                WebsocketEvent::ScammerMarked(data) => {
                    let state_clone = state.clone();
                    tokio::spawn(async move {
                        let display_name = resolve_scammer_display(&state_clone, &data.user_id).await;
                        enqueue_outbound_chat(
                            &state_clone,
                            format!("📢 {} has been marked as a scammer by trading mods, proceed with caution 📢", display_name),
                        );
                    });
                }
                WebsocketEvent::ScammerUnmarked(data) => {
                    let state_clone = state.clone();
                    tokio::spawn(async move {
                        let display_name = resolve_scammer_display(&state_clone, &data.user_id).await;
                        enqueue_outbound_chat(
                            &state_clone,
                            format!("📢 {} has had their scammer mark removed by trading mods 📢", display_name),
                        );
                    });
                }
                WebsocketEvent::TradesReset(data) => {
                    let state_clone = state.clone();
                    tokio::spawn(async move {
                        let display_name = resolve_scammer_display(&state_clone, &data.user_id).await;
                        enqueue_outbound_chat(
                            &state_clone,
                            format!("📢 {}'s trades have been reset by trading mods. Description: {} 📢", display_name, data.reason),
                        );
                    });
                }
                WebsocketEvent::TradesUnreset(data) => {
                    let state_clone = state.clone();
                    tokio::spawn(async move {
                        let display_name = resolve_scammer_display(&state_clone, &data.user_id).await;
                        enqueue_outbound_chat(
                            &state_clone,
                            format!("📢 {}'s trades have been restored by trading mods. Description: {} 📢", display_name, data.reason),
                        );
                    });
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

fn handle_inbound_discord_chat(state: &AzaleaState, data: DiscordChatMessage) {
    let allow_chatbridge_input = state
        .runtime
        .read()
        .expect("runtime config lock poisoned")
        .allow_chatbridge_input;

    if allow_chatbridge_input && data.mc_server == state.mc_server {
        enqueue_outbound_chat(
            state,
            format!("[Discord] {}: {}", data.username, data.message),
        );
    }
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

async fn deliver_offline_messages(state: &AzaleaState, username: &str) {
    let Ok(messages) = load_offline_messages().await else {
        return;
    };
    let mut pending = Vec::new();
    let mut remaining = Vec::new();

    for message in messages {
        if message.recipient.eq_ignore_ascii_case(username) {
            pending.push(message);
        } else {
            remaining.push(message);
        }
    }

    if pending.is_empty() {
        return;
    }

    enqueue_outbound_chat(
        state,
        format!(
            "/msg {username} You have {} pending offline messages, I will send them to you now.",
            pending.len()
        ),
    );

    for message in pending {
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

    if let Err(error) = save_offline_messages(&remaining).await {
        logger::warn(format!("Failed to save offline messages: {error:#}"));
    }
}

fn enqueue_outbound_chat(state: &AzaleaState, message: impl AsRef<str>) {
    state
        .outbound_chat
        .lock()
        .expect("outbound chat queue lock poisoned")
        .push_back(message.as_ref().trim_start().to_owned());
}

fn sender_allowed_for_command(state: &AzaleaState, sender: &str, message: &str) -> bool {
    let runtime = state.runtime.read().expect("runtime config lock poisoned");
    let uuid = state
        .players
        .read()
        .expect("player cache lock poisoned")
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(sender))
        .map(|(_, player)| player.uuid.clone());
    let Some(uuid) = uuid else {
        return true;
    };
    !runtime.user_blacklist.contains(&uuid)
        || whisper_parser::is_self_standing_command(message, &runtime.prefix)
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
        if let Some(parsed) = chat_format_parser::parse(&full_message, &formats) {
            return (Some(parsed.username), parsed.message);
        }
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
