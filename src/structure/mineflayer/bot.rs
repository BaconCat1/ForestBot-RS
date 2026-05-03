use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use anyhow::Context;
use azalea::app::PluginGroup;
use azalea::bot::DefaultBotPlugins;
use azalea::chat_signing::ChatSigningPlugin;
use azalea::client_chat::ChatPacket;
use azalea::prelude::*;
use azalea_viaversion::ViaVersionPlugin;

use crate::config::{AppState, BotConfig};
use crate::structure::{
    endpoints::endpoints::ApiClient,
    logger,
    mineflayer::utils::{chat_format_parser, command_handler},
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
    pub use_whitelist: bool,
    pub user_whitelist: HashSet<String>,
    pub custom_chat_formats: Vec<String>,
    pub command_toggles: HashMap<String, bool>,
    pub whitelisted_commands: HashSet<String>,
}

#[derive(Debug, Clone)]
pub struct PlayerSnapshot {
    pub username: String,
    pub uuid: String,
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
    pub prefix: String,
    pub custom_chat_formats: Vec<String>,
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
            prefix: state.config.prefix.clone(),
            custom_chat_formats: state.config.custom_chat_formats.clone(),
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
                whisper_command: "msg".to_owned(),
                use_whitelist: self.use_whitelist,
                user_whitelist: self.user_whitelist.clone(),
                custom_chat_formats: self.custom_chat_formats.clone(),
                command_toggles: self.command_toggles.clone(),
                whitelisted_commands: self.whitelisted_commands.clone(),
            })),
            players: Arc::new(RwLock::new(HashMap::new())),
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
                use_whitelist: false,
                user_whitelist: HashSet::new(),
                custom_chat_formats: Vec::new(),
                command_toggles: HashMap::new(),
                whitelisted_commands: HashSet::new(),
            })),
            players: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

async fn handle_azalea_event(bot: Client, event: Event, state: AzaleaState) -> anyhow::Result<()> {
    match event {
        Event::Init => {
            logger::info("Azalea client initialized.");
        }
        Event::Login => {
            logger::info("Logged into Minecraft server.");
        }
        Event::Spawn => {
            logger::success(format!("Spawned on {}.", state.mc_server));
        }
        Event::Chat(message) => {
            let (sender, content) = parse_chat_message(&message, &state);
            if sender.is_none() && is_server_presence_message(&content) {
                return Ok(());
            }

            logger::info(match &sender {
                Some(sender) => format!("{sender}: {content}"),
                None => format!("Chat: {content}"),
            });

            if let Some(sender) = sender
                && sender != bot.username()
            {
                command_handler::handle(&bot, &state, &sender, &content).await;
            }
        }
        Event::AddPlayer(player) => {
            logger::info(format!("Player joined: {}", player.profile.name));
            state
                .players
                .write()
                .expect("player cache lock poisoned")
                .insert(
                    player.profile.name.clone(),
                    PlayerSnapshot {
                        username: player.profile.name,
                        uuid: player.profile.uuid.to_string(),
                        latency: player.latency,
                    },
                );
        }
        Event::UpdatePlayer(player) => {
            state
                .players
                .write()
                .expect("player cache lock poisoned")
                .insert(
                    player.profile.name.clone(),
                    PlayerSnapshot {
                        username: player.profile.name,
                        uuid: player.profile.uuid.to_string(),
                        latency: player.latency,
                    },
                );
        }
        Event::RemovePlayer(player) => {
            logger::info(format!("Player left: {}", player.profile.name));
            state
                .players
                .write()
                .expect("player cache lock poisoned")
                .remove(&player.profile.name);
        }
        Event::Disconnect(reason) => {
            logger::warn(format!("Disconnected: {reason:?}"));
        }
        Event::ConnectionFailed(error) => {
            logger::warn(format!("Connection failed: {error}"));
        }
        Event::Tick => {}
        _ => {}
    }

    Ok(())
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
