use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Context;
use azalea::ClientInformation;
use azalea::app::PluginGroup;
use azalea::bot::DefaultBotPlugins;
use azalea::chat_signing::ChatSigningPlugin;
use azalea::client_chat::ChatPacket;
use azalea::prelude::*;
use azalea_viaversion::ViaVersionPlugin;
use uuid::Uuid;

use crate::config::{
    AppState, BotConfig, CommandCooldownConfig, load_offline_messages, save_offline_messages,
};
use crate::structure::{
    endpoints::endpoints::{
        ApiClient, DiscordChatMessage, MinecraftAdvancementMessage, MinecraftChatMessage,
        MinecraftPlayerDeathMessage, MinecraftPlayerJoinMessage, MinecraftPlayerLeaveMessage,
        Player as WebsocketPlayer, WebsocketEvent,
    },
    logger,
    mineflayer::utils::{chat_format_parser, command_handler, whisper_parser},
};

#[derive(Debug, Clone)]
pub struct Command {
    pub names: Vec<String>,
}

#[derive(Debug, Clone)]
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
    pub whitelisted_commands: HashSet<String>,
    pub disabled_events: HashSet<String>,
    pub allow_chatbridge_input: bool,
    pub welcome_messages: bool,
}

#[derive(Debug, Clone)]
pub struct PlayerSnapshot {
    pub username: String,
    pub uuid: String,
    pub entity_uuid: Uuid,
    pub latency: i32,
}

#[derive(Debug, Clone)]
pub struct Bot {
    pub options: BotConfig,
    pub use_whitelist: bool,
    pub welcome_messages: bool,
    pub mc_server: String,
    pub user_whitelist: HashSet<String>,
    pub user_blacklist: HashSet<String>,
    pub whitelisted_commands: HashSet<String>,
    pub commands: HashMap<String, Command>,
    pub command_toggles: HashMap<String, bool>,
    pub disabled_events: HashSet<String>,
    pub use_commands: bool,
    pub prefix: String,
    pub whisper_command: String,
    pub custom_chat_formats: Vec<String>,
    pub allow_chatbridge_input: bool,
    pub anti_spam_global_cooldown_ms: u64,
    pub command_cooldowns: HashMap<String, CommandCooldownConfig>,
    pub reconnect_time_ms: u64,
    pub restart_count: u32,
    pub is_connected: bool,
    pub allow_connection: bool,
    pub api: ApiClient,
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
            whitelisted_commands: state.config.whitelisted_commands.iter().cloned().collect(),
            commands: HashMap::new(),
            command_toggles: state.config.commands.clone(),
            disabled_events: state.config.disabled_events.iter().cloned().collect(),
            use_commands: state.config.use_commands,
            prefix: state.config.prefix.clone(),
            whisper_command: state.config.whisper_command.clone(),
            custom_chat_formats: state.config.custom_chat_formats.clone(),
            allow_chatbridge_input: state.config.allow_chatbridge_input,
            anti_spam_global_cooldown_ms: state.config.anti_spam_global_cooldown,
            command_cooldowns: state.config.command_cooldowns.clone(),
            reconnect_time_ms: state.config.reconnect_time,
            restart_count: 0,
            is_connected: false,
            allow_connection: true,
            api,
        }
    }

    pub async fn start(&mut self) -> anyhow::Result<()> {
        if !self.allow_connection {
            logger::warn("Connection skipped because allow_connection is false.");
            return Ok(());
        }

        self.restart_count += 1;
        logger::info(format!(
            "Starting Azalea bot for {} on {}:{} using configured server version {}",
            self.options.username,
            self.options.host,
            self.options.port,
            self.options.server_version
        ));
        logger::info(format!(
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
                custom_chat_formats: self.custom_chat_formats.clone(),
                command_toggles: self.command_toggles.clone(),
                whitelisted_commands: self.whitelisted_commands.clone(),
                disabled_events: self.disabled_events.clone(),
                allow_chatbridge_input: self.api.options.use_websocket
                    && self.allow_chatbridge_input,
                welcome_messages: self.welcome_messages,
            })),
            players: Arc::new(RwLock::new(HashMap::new())),
            outbound_chat: Arc::new(Mutex::new(VecDeque::new())),
            last_command_at: Arc::new(Mutex::new(None)),
            player_command_cooldowns: Arc::new(Mutex::new(HashMap::new())),
            seen_entity_spawns: Arc::new(RwLock::new(HashSet::new())),
            next_entity_spawn_scan_at: Arc::new(RwLock::new(0)),
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
            logger::info(format!(
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

    pub async fn end_and_restart(&mut self) -> anyhow::Result<()> {
        self.is_connected = false;
        self.start().await
    }

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
    pub last_command_at: Arc<Mutex<Option<Instant>>>,
    pub player_command_cooldowns: Arc<Mutex<HashMap<String, PlayerCommandCooldown>>>,
    pub seen_entity_spawns: Arc<RwLock<HashSet<String>>>,
    pub next_entity_spawn_scan_at: Arc<RwLock<i64>>,
}

#[derive(Debug, Clone, Copy)]
pub struct PlayerCommandCooldown {
    pub last_attempt_at: Instant,
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
                whitelisted_commands: HashSet::new(),
                disabled_events: HashSet::new(),
                allow_chatbridge_input: false,
                welcome_messages: false,
            })),
            players: Arc::new(RwLock::new(HashMap::new())),
            outbound_chat: Arc::new(Mutex::new(VecDeque::new())),
            last_command_at: Arc::new(Mutex::new(None)),
            player_command_cooldowns: Arc::new(Mutex::new(HashMap::new())),
            seen_entity_spawns: Arc::new(RwLock::new(HashSet::new())),
            next_entity_spawn_scan_at: Arc::new(RwLock::new(0)),
        }
    }
}

async fn handle_azalea_event(bot: Client, event: Event, state: AzaleaState) -> anyhow::Result<()> {
    match event {
        Event::Init => {
            if event_disabled(&state, &["init"]) {
                return Ok(());
            }
            logger::info("Azalea client initialized.");

            bot.set_client_information(ClientInformation {
                view_distance: 2,
                ..Default::default()
            });
        }
        Event::Login => {
            if event_disabled(&state, &["login"]) {
                return Ok(());
            }
            logger::info("Logged into Minecraft server.");
        }
        Event::Spawn => {
            if event_disabled(&state, &["spawn"]) {
                return Ok(());
            }
            logger::success(format!("Spawned on {}.", state.mc_server));
            spawn_websocket_event_task(bot.clone(), state.clone());
            send_player_list_update(&state).await;
            spawn_player_list_update_task(state.clone());
        }
        Event::Chat(message) => {
            if event_disabled(&state, &["message", "messagestr", "chat"]) {
                return Ok(());
            }
            let (sender, content) = parse_chat_message(&message, &state);
            if sender.is_none() && is_server_presence_message(&content) {
                return Ok(());
            }

            logger::info(match &sender {
                Some(sender) => format!("{sender}: {content}"),
                None => format!("Chat: {content}"),
            });

            if sender.is_none() {
                handle_fallback_message(&bot, &state, &content).await;
                return Ok(());
            }

            if let Some(sender) = sender {
                if sender == bot.username() {
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
                    .get(&sender)
                    .map(|player| player.uuid.clone());

                if let Some(uuid) = uuid {
                    send_minecraft_chat_message(&state, &sender, &content, &uuid).await;
                }
            }
        }
        Event::AddPlayer(player) => {
            if event_disabled(&state, &["playerJoined", "playerJoin", "addPlayer"]) {
                return Ok(());
            }
            logger::info(format!("Player joined: {}", player.profile.name));
            let username = player.profile.name.clone();
            let uuid = player.profile.uuid.to_string();
            let latency = player.latency;
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
                    },
                );
            send_player_join(&state, &username, &uuid).await;
            send_player_list_update(&state).await;
            deliver_offline_messages(&state, &username).await;
        }
        Event::UpdatePlayer(player) => {
            if event_disabled(&state, &["updatePlayer"]) {
                return Ok(());
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
                    },
                );
        }
        Event::RemovePlayer(player) => {
            if event_disabled(&state, &["playerLeft", "playerLeave", "removePlayer"]) {
                return Ok(());
            }
            logger::info(format!("Player left: {}", player.profile.name));
            let username = player.profile.name.clone();
            let uuid = player.profile.uuid.to_string();
            state
                .players
                .write()
                .expect("player cache lock poisoned")
                .remove(&username);
            send_player_leave(&state, &username, &uuid).await;
            send_player_list_update(&state).await;
        }
        Event::Disconnect(reason) => {
            if event_disabled(&state, &["end", "disconnect"]) {
                return Ok(());
            }
            logger::warn(format!("Disconnected: {reason:?}"));
            send_session_flush_leave(&state).await;
        }
        Event::ConnectionFailed(error) => {
            if event_disabled(&state, &["error", "kicked", "connectionFailed"]) {
                return Ok(());
            }
            logger::warn(format!("Connection failed: {error}"));
        }
        Event::Tick => {
            flush_outbound_chat(&bot, &state);
            // No direct Azalea equivalent exists for Mineflayer's entitySpawn.
            // This scan is separately gated by entitySpawn to preserve Node config behavior.
            handle_entity_spawn_first_sight(&bot, &state);
        }
        _ => {}
    }

    Ok(())
}

fn flush_outbound_chat(bot: &Client, state: &AzaleaState) {
    let mut queue = state
        .outbound_chat
        .lock()
        .expect("outbound chat queue lock poisoned");

    for _ in 0..3 {
        let Some(message) = queue.pop_front() else {
            break;
        };
        logger::info(format!("Sending chat reply: {message}"));
        bot.chat(message);
    }
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
        logger::info(format!(
            "World: [{}] ({x:.1}, {y:.1}, {z:.1}) Spotted.",
            player.username
        ));

        let greeting = entity_spawn_greeting(&player.username, now);
        bot.chat(format!("/msg {} {}", player.username, greeting));

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

fn round_one_decimal(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
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

    let uuid = players
        .get(&player)
        .map(|player| player.uuid.clone())
        .or_else(|| {
            players
                .values()
                .find(|p| p.username == player)
                .map(|p| p.uuid.clone())
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
        logger::info(format!("Advancement: {full_msg}"));
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
        logger::info(format!("Death: {full_msg}"));
        return;
    }

    let parsed_command_message = message.trim();
    if parsed_command_message.starts_with(&prefix) {
        command_handler::handle(bot, state, &player, parsed_command_message).await;
        return;
    }

    send_minecraft_chat_message(state, &player, &message, &uuid).await;
    logger::info(format!("{player}: {message}"));
}

fn spawn_websocket_event_task(bot: Client, state: AzaleaState) {
    let Some(websocket) = state.api.websocket.clone() else {
        return;
    };

    let mut events = websocket.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            match event {
                WebsocketEvent::Open => logger::success("Websocket connection opened."),
                WebsocketEvent::Close(reason) => {
                    logger::warn(format!("Websocket connection closed: {reason}"));
                }
                WebsocketEvent::Error(error) => {
                    logger::warn(format!("Websocket error: {error}"));
                }
                WebsocketEvent::KeyAccepted(_) => {
                    logger::success("Websocket API key accepted by hub.");
                }
                WebsocketEvent::NewName(data) => {
                    let should_welcome = {
                        let runtime = state.runtime.read().expect("runtime config lock poisoned");
                        runtime.welcome_messages && data.server == state.mc_server
                    };
                    if should_welcome {
                        bot.chat(format!(
                            "{}, previously known as {} joined the server!",
                            data.new_name, data.old_name
                        ));
                    }
                }
                WebsocketEvent::NewUser(data) => {
                    let should_welcome = {
                        let runtime = state.runtime.read().expect("runtime config lock poisoned");
                        runtime.welcome_messages && data.server == state.mc_server
                    };
                    if should_welcome {
                        bot.chat(format!("{}, First time here? Welcome!", data.user));
                    }
                }
                WebsocketEvent::InboundDiscordChat(data) => {
                    handle_inbound_discord_chat(&bot, &state, data);
                }
                WebsocketEvent::InboundMinecraftChat(data) => {
                    handle_inbound_minecraft_chat(&bot, &state, data);
                }
                WebsocketEvent::UnknownMessage(message) => {
                    logger::warn(format!("Unknown websocket message: {message}"));
                }
                WebsocketEvent::MinecraftPlayerDeath(_)
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

fn handle_inbound_discord_chat(bot: &Client, state: &AzaleaState, data: DiscordChatMessage) {
    let allow_chatbridge_input = state
        .runtime
        .read()
        .expect("runtime config lock poisoned")
        .allow_chatbridge_input;

    if allow_chatbridge_input && data.mc_server == state.mc_server {
        bot.chat(format!("[Discord] {}: {}", data.username, data.message));
    }
}

fn handle_inbound_minecraft_chat(bot: &Client, state: &AzaleaState, data: MinecraftChatMessage) {
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
        bot.chat(data.message);
    }
}

async fn send_minecraft_chat_message(
    state: &AzaleaState,
    username: &str,
    message: &str,
    uuid: &str,
) {
    let Some(websocket) = state.api.websocket.as_ref() else {
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
        logger::warn(format!("Failed to send websocket chat message: {error}"));
    }
}

async fn send_player_join(state: &AzaleaState, username: &str, uuid: &str) {
    let Some(websocket) = state.api.websocket.as_ref() else {
        return;
    };

    if let Err(error) = websocket
        .send_player_join(MinecraftPlayerJoinMessage {
            username: username.to_owned(),
            uuid: uuid.to_owned(),
            timestamp: now_millis_string(),
            server: state.mc_server.clone(),
        })
        .await
    {
        logger::warn(format!("Failed to send websocket player join: {error}"));
    }
}

async fn send_player_leave(state: &AzaleaState, username: &str, uuid: &str) {
    let Some(websocket) = state.api.websocket.as_ref() else {
        return;
    };

    if let Err(error) = websocket
        .send_player_leave(MinecraftPlayerLeaveMessage {
            username: username.to_owned(),
            uuid: uuid.to_owned(),
            timestamp: now_millis_string(),
            server: state.mc_server.clone(),
        })
        .await
    {
        logger::warn(format!("Failed to send websocket player leave: {error}"));
    }
}

async fn send_player_list_update(state: &AzaleaState) {
    let Some(websocket) = state.api.websocket.as_ref() else {
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
        logger::warn(format!(
            "Failed to send websocket player list update: {error}"
        ));
    }
}

async fn send_session_flush_leave(state: &AzaleaState) {
    let Some(websocket) = state.api.websocket.as_ref() else {
        return;
    };

    if let Err(error) = websocket
        .send_player_leave(MinecraftPlayerLeaveMessage {
            username: "ForestBot".to_owned(),
            uuid: String::new(),
            timestamp: now_millis_string(),
            server: state.mc_server.clone(),
        })
        .await
    {
        logger::warn(format!(
            "Failed to send websocket session flush leave: {error}"
        ));
    }
}

async fn send_player_advancement(
    state: &AzaleaState,
    username: &str,
    uuid: &str,
    advancement: &str,
) {
    let Some(websocket) = state.api.websocket.as_ref() else {
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
        logger::warn(format!("Failed to send websocket advancement: {error}"));
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
    let Some(websocket) = state.api.websocket.as_ref() else {
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
        logger::warn(format!("Failed to send websocket death: {error}"));
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
        .get(sender)
        .map(|player| player.uuid.clone());
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
    let (sender, content) = message.split_sender_and_content();
    if let Some(sender) = sender {
        return (
            Some(chat_format_parser::normalize_username(&sender)),
            content,
        );
    }

    let full_message = message.message().to_string();
    let formats = state
        .runtime
        .read()
        .expect("runtime config lock poisoned")
        .custom_chat_formats
        .clone();

    if let Some(parsed) = chat_format_parser::parse(&full_message, &formats) {
        return (Some(parsed.username), parsed.message);
    }

    (None, full_message)
}
