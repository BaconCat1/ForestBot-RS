use crate::config::ApiConfig;

use anyhow::{Result, anyhow};
use futures_util::{SinkExt, StreamExt};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::{broadcast, mpsc};
use tokio::time::{Duration, interval, sleep};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, handshake::client::Request, protocol::Message},
};

#[derive(Debug, Clone)]
pub struct ApiClient {
    pub options: ApiConfig,
    client: reqwest::Client,
    pub websocket: Option<WebsocketClient>,
}

impl ApiClient {
    pub fn new(options: ApiConfig) -> Self {
        let mut headers = HeaderMap::new();
        if !options.api_key.is_empty() {
            if let Ok(value) = HeaderValue::from_str(&options.api_key) {
                headers.insert("x-api-key", value);
            }
            if let Ok(value) = HeaderValue::from_str(&format!("Bearer {}", options.api_key)) {
                headers.insert(AUTHORIZATION, value);
            }
        }

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .expect("request client should build");

        Self {
            options,
            client,
            websocket: None,
        }
    }

    pub async fn init_websocket(&mut self) -> Result<()> {
        if !self.options.use_websocket || self.websocket.is_some() {
            return Ok(());
        }

        let websocket = WebsocketClient::connect(
            self.options.websocket_url.clone(),
            self.options.api_key.clone(),
            self.options.is_bot_client,
            self.options.mc_server.clone(),
        )
        .await?;

        self.websocket = Some(websocket);
        Ok(())
    }

    pub async fn get_stats_by_uuid(&self, uuid: &str, server: &str) -> Option<AllPlayerStats> {
        self.get_json("/user", &[("uuid", uuid), ("server", server)])
            .await
            .and_then(|value| stats_from_value(&value, server))
    }
    #[allow(dead_code)]
    pub async fn get_stats_by_username(
        &self,
        username: &str,
        server: &str,
    ) -> Option<AllPlayerStats> {
        self.get_json("/playername", &[("name", username), ("server", server)])
            .await
            .and_then(|value| stats_from_value(&value, server))
    }

    pub async fn convert_username_to_uuid(&self, username: &str) -> Option<String> {
        self.get_json("/convert-username-to-uuid", &[("username", username)])
            .await
            .and_then(|value| string_field(&value, &["uuid"]))
    }

    pub async fn get_playtime(&self, uuid: &str, server: &str) -> Option<Playtime> {
        self.get_stats_by_uuid(uuid, server)
            .await
            .and_then(|user| user.playtime.map(|playtime| Playtime { playtime }))
    }

    pub async fn get_join_date(&self, uuid: &str, server: &str) -> Option<JoinDate> {
        self.get_stats_by_uuid(uuid, server)
            .await
            .and_then(|user| user.join_date.map(|join_date| JoinDate { join_date }))
    }

    pub async fn get_join_count(&self, uuid: &str, server: &str) -> Option<JoinCount> {
        self.get_stats_by_uuid(uuid, server)
            .await
            .and_then(|user| user.joins.map(|join_count| JoinCount { join_count }))
    }

    pub async fn get_last_seen(&self, uuid: &str, server: &str) -> Option<LastSeen> {
        self.get_stats_by_uuid(uuid, server)
            .await
            .and_then(|user| user.last_seen.map(|last_seen| LastSeen { last_seen }))
    }

    pub async fn get_kd(&self, uuid: &str, server: &str) -> Option<Kd> {
        self.get_stats_by_uuid(uuid, server).await.and_then(|user| {
            Some(Kd {
                kills: user.kills?.try_into().ok()?,
                deaths: user.deaths?.try_into().ok()?,
            })
        })
    }

    pub async fn get_deaths(
        &self,
        uuid: &str,
        server: &str,
        limit: usize,
        order: &str,
        kind: &str,
    ) -> Option<Vec<MinecraftPlayerDeathMessage>> {
        self.get_json(
            "/deaths",
            &[
                ("uuid", uuid),
                ("server", server),
                ("limit", &limit.to_string()),
                ("order", order),
                ("type", kind),
            ],
        )
        .await
        .and_then(|value| self.parse_json("/deaths", value))
    }

    pub async fn get_kills(
        &self,
        uuid: &str,
        server: &str,
        limit: usize,
        order: &str,
    ) -> Option<Vec<MinecraftPlayerDeathMessage>> {
        self.get_json(
            "/kills",
            &[
                ("uuid", uuid),
                ("server", server),
                ("limit", &limit.to_string()),
                ("order", order),
            ],
        )
        .await
        .and_then(|value| {
            value
                .get("data")
                .cloned()
                .unwrap_or(value)
                .as_array()
                .cloned()
        })
        .and_then(|array| {
            array
                .into_iter()
                .map(|item| self.parse_json("/kills item", item))
                .collect::<Option<Vec<_>>>()
        })
    }

    pub async fn get_messages(
        &self,
        username: &str,
        server: &str,
        limit: usize,
        order: &str,
        offset: usize,
    ) -> Option<Vec<MinecraftChatMessage>> {
        self.get_json(
            "/messages",
            &[
                ("name", username),
                ("server", server),
                ("limit", &limit.to_string()),
                ("order", order),
                ("offset", &offset.to_string()),
            ],
        )
        .await
        .and_then(|value| self.parse_json("/messages", value))
    }

    pub async fn get_advancements(
        &self,
        uuid: &str,
        server: &str,
        limit: usize,
        order: &str,
    ) -> Option<Vec<MinecraftAdvancementMessage>> {
        self.get_json(
            "/advancements",
            &[
                ("uuid", uuid),
                ("server", server),
                ("limit", &limit.to_string()),
                ("order", order),
            ],
        )
        .await
        .and_then(|value| value.get("advancements").cloned())
        .and_then(|value| self.parse_json("/advancements.advancements", value))
    }

    pub async fn get_total_advancements_count(&self, uuid: &str, server: &str) -> Option<u64> {
        self.get_json("/advancements-count", &[("uuid", uuid), ("server", server)])
            .await
            .and_then(|value| number_field(&value, &["total_advancements"]))
    }

    pub async fn get_message_count(&self, username: &str, server: &str) -> Option<MessageCount> {
        self.get_json(
            "/messagecount",
            &[("username", username), ("server", server)],
        )
        .await
        .and_then(|value| self.parse_json("/messagecount", value))
    }

    pub async fn get_word_occurrence(
        &self,
        username: &str,
        server: &str,
        word: &str,
    ) -> Option<WordOccurrence> {
        self.get_json(
            "/wordcount",
            &[("username", username), ("server", server), ("word", word)],
        )
        .await
        .and_then(|value| self.parse_json("/wordcount", value))
    }

    pub async fn get_name_finder(&self, username: &str, server: &str) -> Option<NameFind> {
        self.get_json("/namesearch", &[("username", username), ("server", server)])
            .await
            .and_then(|value| self.parse_json("/namesearch", value))
    }
    #[allow(dead_code)]
    pub async fn get_online_check(&self, username: &str) -> Option<OnlineCheck> {
        self.get_json("/online", &[("username", username)])
            .await
            .and_then(|value| self.parse_json("/online", value))
    }

    pub async fn get_who_is(&self, username: &str) -> Option<WhoIsData> {
        self.get_json("/whois", &[("username", username)])
            .await
            .and_then(|value| self.parse_json("/whois", value))
    }
    #[allow(dead_code)]
    pub async fn get_users_sorted_by_joindate(
        &self,
        server: &str,
        limit: usize,
        order: &str,
        player_usernames: &[String],
    ) -> Option<Vec<AllPlayerStats>> {
        let usernames = player_usernames.join(",");
        self.get_json(
            "/users-sorted-by-joindate",
            &[
                ("server", server),
                ("limit", &limit.to_string()),
                ("order", order),
                ("usernames", &usernames),
            ],
        )
        .await
        .and_then(|value| self.parse_json("/users-sorted-by-joindate", value))
    }

    pub async fn get_unique_users(&self, server: &str) -> Option<Vec<UniqueUser>> {
        self.get_json("/unique-users", &[("server", server)])
            .await
            .and_then(|value| self.parse_json("/unique-users", value))
    }

    pub async fn get_quote(
        &self,
        username: &str,
        server: &str,
        options: Option<QuoteOptions>,
    ) -> Option<Quote> {
        let mut params: Vec<(&str, String)> = vec![("server", server.to_owned())];
        if let Some(options) = options {
            if options.random {
                params.push(("random", "true".to_owned()));
                if let Some(phrase) = options.phrase {
                    params.push(("phrase", phrase));
                }
            } else {
                params.push(("name", username.to_owned()));
            }
        } else {
            params.push(("name", username.to_owned()));
        }

        self.get_json_string_params("/quote", &params)
            .await
            .and_then(|value| self.parse_json("/quote", value))
    }

    pub async fn get_top_statistic(&self, stat: &str, server: &str, limit: usize) -> Option<Value> {
        self.get_json(
            "/top-statistic",
            &[
                ("statistic", stat),
                ("server", server),
                ("limit", &limit.to_string()),
            ],
        )
        .await
    }

    pub async fn get_leaderboards(&self, server: &str) -> Option<Value> {
        self.get_json("/leaderboards", &[("server", server)]).await
    }
    #[allow(dead_code)]
    pub async fn get_hourly_player_activity(
        &self,
        server: &str,
    ) -> Option<PlayerActivityByHourResponse> {
        self.get_json("/player-activity-by-hour", &[("server", server)])
            .await
            .and_then(|value| self.parse_json("/player-activity-by-hour", value))
    }
    #[allow(dead_code)]
    pub async fn get_total_daily_logins(
        &self,
        server: &str,
    ) -> Option<PlayerActivityByWeekDayResponse> {
        self.get_json("/player-activity-by-week-day", &[("server", server)])
            .await
            .and_then(|value| self.parse_json("/player-activity-by-week-day", value))
    }

    pub async fn get_faq(&self, id: Option<&str>, server: Option<&str>) -> Option<FaqData> {
        match (id, server) {
            (Some(id), Some(server)) => self
                .get_json("/faq", &[("id", id), ("server", server)])
                .await
                .and_then(|value| self.parse_json("/faq", value)),
            (Some(id), None) => self
                .get_json("/faq", &[("id", id)])
                .await
                .and_then(|value| self.parse_json("/faq", value)),
            (None, Some(server)) => self
                .get_json("/faq", &[("server", server)])
                .await
                .and_then(|value| self.parse_json("/faq", value)),
            _ => self
                .get_json("/faq", &[])
                .await
                .and_then(|value| self.parse_json("/faq", value)),
        }
    }

    pub async fn post_who_is_description(
        &self,
        username: &str,
        description: &str,
    ) -> Option<Value> {
        self.post_json(
            "/whois-description",
            json!({ "username": username, "description": description }),
        )
        .await
    }

    pub async fn post_new_faq(
        &self,
        username: &str,
        faq: &str,
        uuid: &str,
        server: &str,
    ) -> Option<PostFaqResult> {
        let response = self
            .post_json(
                "/post-faq",
                json!({ "username": username, "faq": faq, "uuid": uuid, "server": server }),
            )
            .await?;

        if let Ok(result) = serde_json::from_value::<PostFaqResult>(response.clone()) {
            return Some(result);
        }

        Some(PostFaqResult {
            id: response.get("id").and_then(Value::as_i64).unwrap_or(-1),
            error: response
                .get("error")
                .and_then(Value::as_str)
                .map(str::to_owned),
        })
    }

    pub async fn edit_faq(
        &self,
        id: i64,
        username: &str,
        faq: &str,
        uuid: &str,
        server: &str,
    ) -> Option<EditFaqResult> {
        self.post_json(
            "/edit-faq",
            json!({
                "username": username,
                "faq": faq,
                "uuid": uuid,
                "server": server,
                "id": id
            }),
        )
        .await
        .and_then(|value| self.parse_json("/edit-faq", value))
    }
    #[allow(dead_code)]
    pub async fn with_websocket(&mut self) -> Result<Option<WebsocketClient>> {
        self.init_websocket().await?;
        Ok(self.websocket.clone())
    }

    async fn get_json(&self, path: &str, query: &[(&str, &str)]) -> Option<Value> {
        let url = self.make_url(path, query);
        let response = self.client.get(url).send().await;
        match response {
            Ok(response) => match response.error_for_status() {
                Ok(response) => {
                    let status = response.status();
                    let url = response.url().to_string();
                    match response.text().await {
                        Ok(body) => self.parse_response_body("GET", path, &url, status, &body),
                        Err(error) => {
                            self.log_error(format!("GET {path} failed reading body: {error}"));
                            None
                        }
                    }
                }
                Err(error) => {
                    self.log_error(error);
                    None
                }
            },
            Err(error) => {
                self.log_error(error);
                None
            }
        }
    }

    async fn get_json_string_params(&self, path: &str, params: &[(&str, String)]) -> Option<Value> {
        let url = self.make_url_string_params(path, params);
        let response = self.client.get(url).send().await;
        match response {
            Ok(response) => match response.error_for_status() {
                Ok(response) => {
                    let status = response.status();
                    let url = response.url().to_string();
                    match response.text().await {
                        Ok(body) => self.parse_response_body("GET", path, &url, status, &body),
                        Err(error) => {
                            self.log_error(format!("GET {path} failed reading body: {error}"));
                            None
                        }
                    }
                }
                Err(error) => {
                    self.log_error(error);
                    None
                }
            },
            Err(error) => {
                self.log_error(error);
                None
            }
        }
    }

    async fn post_json(&self, path: &str, body: Value) -> Option<Value> {
        let url = format!("{}{}", self.base_url(), path);
        let response = self.client.post(url).json(&body).send().await;
        match response {
            Ok(response) => {
                let status = response.status();
                let url = response.url().to_string();
                let value = match response.text().await {
                    Ok(body) => self.parse_response_body("POST", path, &url, status, &body),
                    Err(error) => {
                        self.log_error(format!("POST {path} failed reading body: {error}"));
                        None
                    }
                };
                if status.is_success() {
                    value
                } else {
                    self.log_error(anyhow!("POST {path} failed with status {status}"));
                    value
                }
            }
            Err(error) => {
                self.log_error(error);
                None
            }
        }
    }

    fn make_url(&self, path: &str, query: &[(&str, &str)]) -> String {
        let mut url =
            reqwest::Url::parse(&format!("{}{}", self.base_url(), path)).expect("valid api url");
        {
            let mut pairs = url.query_pairs_mut();
            for (name, value) in query {
                pairs.append_pair(name, value);
            }
        }
        url.to_string()
    }

    fn make_url_string_params(&self, path: &str, query: &[(&str, String)]) -> String {
        let mut url =
            reqwest::Url::parse(&format!("{}{}", self.base_url(), path)).expect("valid api url");
        {
            let mut pairs = url.query_pairs_mut();
            for (name, value) in query {
                pairs.append_pair(name, value);
            }
        }
        url.to_string()
    }

    fn base_url(&self) -> String {
        self.options.api_url.trim_end_matches('/').to_owned()
    }

    fn log_error(&self, error: impl std::fmt::Display) {
        if self.options.log_errors {
            eprintln!("[API] {error}");
        }
    }

    fn parse_response_body(
        &self,
        method: &str,
        path: &str,
        url: &str,
        status: reqwest::StatusCode,
        body: &str,
    ) -> Option<Value> {
        self.log_error(format!("{method} {url} -> {status} {}", preview_body(body)));
        match serde_json::from_str::<Value>(body) {
            Ok(value) => Some(unwrap_data(value)),
            Err(error) => {
                self.log_error(format!(
                    "{method} {path} returned invalid JSON: {error}; body={}",
                    preview_body(body)
                ));
                None
            }
        }
    }

    fn parse_json<T>(&self, path: &str, value: Value) -> Option<T>
    where
        T: DeserializeOwned,
    {
        match serde_json::from_value::<T>(value.clone()) {
            Ok(value) => Some(value),
            Err(error) => {
                self.log_error(format!(
                    "{path} JSON shape mismatch: {error}; body={}",
                    preview_value(&value)
                ));
                None
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct WebsocketClient {
    sender: mpsc::UnboundedSender<Message>,
    events: broadcast::Sender<WebsocketEvent>,
    connected: Arc<AtomicBool>,
    #[allow(dead_code)]
    pub websocket_url: String,
}

impl WebsocketClient {
    pub async fn connect(
        websocket_url: String,
        api_key: String,
        is_bot_client: bool,
        mc_server: String,
    ) -> Result<Self> {
        let (sender, mut receiver) = mpsc::unbounded_channel::<Message>();
        let (events, _) = broadcast::channel(64);
        let connected = Arc::new(AtomicBool::new(true));
        connected.store(false, Ordering::Relaxed);

        let connected_for_task = connected.clone();
        let events_for_task = events.clone();
        let task_websocket_url = websocket_url.clone();
        tokio::spawn(async move {
            let client_type = if is_bot_client {
                "minecraft"
            } else {
                "discord"
            };
            let request_url = format!(
                "{}/websocket/connect",
                task_websocket_url.trim_end_matches('/')
            );
            let mut reconnect_count = 0_u32;

            loop {
                let request = match build_websocket_request(
                    &request_url,
                    &api_key,
                    client_type,
                    &mc_server,
                ) {
                    Ok(request) => request,
                    Err(error) => {
                        events_for_task
                            .send(WebsocketEvent::Error(format!(
                                "failed to build websocket request: {error}"
                            )))
                            .ok();
                        sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                };

                match connect_async(request).await {
                    Ok((socket, _)) => {
                        connected_for_task.store(true, Ordering::Relaxed);
                        events_for_task.send(WebsocketEvent::Open).ok();
                        reconnect_count = 0;

                        let (mut write, mut read) = socket.split();
                        let mut keepalive = interval(Duration::from_secs(5));

                        loop {
                            tokio::select! {
                                maybe_message = receiver.recv() => {
                                    match maybe_message {
                                        Some(message) => {
                                            if let Err(error) = write.send(message).await {
                                                events_for_task
                                                    .send(WebsocketEvent::Error(error.to_string()))
                                                    .ok();
                                                break;
                                            }
                                        }
                                        None => return,
                                    }
                                }
                                _ = keepalive.tick() => {
                                    if let Err(error) = write.send(Message::Ping("pingdata".as_bytes().to_vec().into())).await {
                                        events_for_task
                                            .send(WebsocketEvent::Error(error.to_string()))
                                            .ok();
                                        break;
                                    }
                                }
                                maybe_message = read.next() => {
                                    match maybe_message {
                                        Some(Ok(Message::Text(text))) => {
                                            if let Some(event) = parse_inbound_message(&text) {
                                                events_for_task.send(event).ok();
                                            } else {
                                                events_for_task
                                                    .send(WebsocketEvent::UnknownMessage(text.to_string()))
                                                    .ok();
                                            }
                                        }
                                        Some(Ok(Message::Close(frame))) => {
                                            events_for_task
                                                .send(WebsocketEvent::Close(
                                                    frame
                                                        .map(|frame| frame.reason.to_string())
                                                        .unwrap_or_else(|| "closed".to_owned()),
                                                ))
                                                .ok();
                                            break;
                                        }
                                        Some(Ok(_)) => {}
                                        Some(Err(error)) => {
                                            events_for_task
                                                .send(WebsocketEvent::Error(error.to_string()))
                                                .ok();
                                            break;
                                        }
                                        None => {
                                            events_for_task
                                                .send(WebsocketEvent::Close("closed".to_owned()))
                                                .ok();
                                            break;
                                        }
                                    }
                                }
                            }
                        }

                        connected_for_task.store(false, Ordering::Relaxed);
                    }
                    Err(error) => {
                        connected_for_task.store(false, Ordering::Relaxed);
                        events_for_task
                            .send(WebsocketEvent::Error(error.to_string()))
                            .ok();
                    }
                }

                reconnect_count = reconnect_count.saturating_add(1);
                let delay = if reconnect_count >= 5 { 60 } else { 5 };
                events_for_task
                    .send(WebsocketEvent::Close(format!(
                        "reconnecting in {delay}s after {reconnect_count} failed/closed attempt(s)"
                    )))
                    .ok();
                sleep(Duration::from_secs(delay)).await;
            }
        });

        Ok(Self {
            sender,
            events,
            connected,
            websocket_url,
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WebsocketEvent> {
        self.events.subscribe()
    }

    pub fn is_client_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    pub async fn send_message(&self, action: &str, data: Value) -> Result<()> {
        if !self.is_client_connected() {
            return Err(anyhow!("websocket is not connected"));
        }

        let message = Message::Text(serialize_websocket_payload(action, data)?.into());
        self.sender
            .send(message)
            .map_err(|error| anyhow!("failed to send websocket message: {error}"))?;
        Ok(())
    }

    pub async fn send_minecraft_chat_message(&self, msg_data: MinecraftChatMessage) -> Result<()> {
        self.send_message("inbound_minecraft_chat", serde_json::to_value(msg_data)?)
            .await
    }
    #[allow(dead_code)]
    pub async fn send_discord_chat_message(&self, msg_data: DiscordChatMessage) -> Result<()> {
        self.send_message("inbound_discord_chat", serde_json::to_value(msg_data)?)
            .await
    }

    pub async fn send_player_list_update(&self, msg_data: Vec<Player>) -> Result<()> {
        self.send_message("send_update_player_list", json!({ "players": msg_data }))
            .await
    }

    pub async fn send_player_advancement(
        &self,
        msg_data: MinecraftAdvancementMessage,
    ) -> Result<()> {
        self.send_message("minecraft_advancement", serde_json::to_value(msg_data)?)
            .await
    }

    pub async fn send_player_join(&self, msg_data: MinecraftPlayerJoinMessage) -> Result<()> {
        self.send_message("minecraft_player_join", serde_json::to_value(msg_data)?)
            .await
    }

    pub async fn send_player_leave(&self, msg_data: MinecraftPlayerLeaveMessage) -> Result<()> {
        self.send_message("minecraft_player_leave", serde_json::to_value(msg_data)?)
            .await
    }

    pub async fn send_player_death(&self, msg_data: MinecraftPlayerDeathMessage) -> Result<()> {
        self.send_message("minecraft_player_death", serde_json::to_value(msg_data)?)
            .await
    }
}

fn build_websocket_request(
    request_url: &str,
    api_key: &str,
    client_type: &str,
    mc_server: &str,
) -> Result<Request> {
    let mut request = request_url
        .to_owned()
        .into_client_request()
        .map_err(|error| anyhow!("failed to build websocket request: {error}"))?;
    {
        let headers = request.headers_mut();
        headers.insert("x-api-key", HeaderValue::from_str(api_key)?);
        headers.insert("client-type", HeaderValue::from_str(client_type)?);
        headers.insert("mc_server", HeaderValue::from_str(mc_server)?);
    }
    Ok(request)
}

fn serialize_websocket_payload(action: &str, data: Value) -> Result<String> {
    let payload = OutboundWebsocketMessage {
        action: action.to_owned(),
        data,
    };
    Ok(serde_json::to_string(&payload)?)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub username: String,
    pub uuid: String,
    pub latency: i32,
    pub server: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftChatMessage {
    #[serde(alias = "username")]
    pub name: String,
    pub message: String,
    #[serde(alias = "timestamp")]
    #[serde(deserialize_with = "string_or_number")]
    pub date: String,
    #[serde(default)]
    pub mc_server: String,
    #[serde(default, deserialize_with = "optional_string_or_number")]
    pub uuid: String,
    #[serde(default)]
    pub relay_type: Option<String>,
    #[serde(default)]
    pub origin_server: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordChatMessage {
    pub message: String,
    pub username: String,
    pub timestamp: String,
    pub mc_server: String,
    pub channel_id: String,
    pub guild_id: String,
    pub guild_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftAdvancementMessage {
    pub username: String,
    pub advancement: String,
    pub time: i64,
    pub mc_server: String,
    pub id: Option<i64>,
    pub uuid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftPlayerJoinMessage {
    pub username: String,
    pub uuid: String,
    pub timestamp: String,
    pub server: String,
}

pub type MinecraftPlayerLeaveMessage = MinecraftPlayerJoinMessage;
pub type MinecraftPlayerKillMessage = MinecraftPlayerJoinMessage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftPlayerDeathMessage {
    pub victim: String,
    pub death_message: String,
    pub murderer: Option<String>,
    pub time: i64,
    #[serde(rename = "type")]
    pub death_type: String,
    pub mc_server: String,
    pub id: Option<i64>,
    #[serde(rename = "victimUUID")]
    pub victim_uuid: String,
    #[serde(rename = "murdererUUID")]
    pub murderer_uuid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllPlayerStats {
    pub username: Option<String>,
    pub kills: Option<i64>,
    pub deaths: Option<i64>,
    #[serde(rename = "joindate")]
    pub join_date: Option<String>,
    #[serde(rename = "lastseen")]
    pub last_seen: Option<String>,
    #[serde(rename = "UUID", alias = "uuid")]
    pub uuid: Option<String>,
    pub playtime: Option<u64>,
    pub joins: Option<u64>,
    pub leaves: Option<u64>,
    #[serde(rename = "lastdeathTime")]
    pub last_death_time: Option<u64>,
    #[serde(rename = "lastdeathString")]
    pub last_death_string: Option<String>,
    pub mc_server: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playtime {
    pub playtime: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinDate {
    pub join_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinCount {
    pub join_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Kd {
    pub kills: u64,
    pub deaths: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastSeen {
    pub last_seen: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageCount {
    #[serde(default)]
    pub name: String,
    #[serde(rename = "count")]
    pub message_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordOccurrence {
    pub name: String,
    pub count: u64,
    pub word: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NameFind {
    pub usernames: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineCheck {
    pub online: bool,
    pub server: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhoIsData {
    #[serde(deserialize_with = "string_or_vec_string")]
    pub description: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniqueUser {
    pub username: String,
    pub joindate: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub name: String,
    pub message: String,
    pub date: Option<String>,
    #[serde(default)]
    pub mc_server: String,
    #[serde(default)]
    pub uuid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerActivityByHourResponse {
    pub player_activity_by_hour: Vec<PlayerActivityHourlyResults>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerActivityHourlyResults {
    pub weekday: u64,
    pub activity: Vec<HourlyActivity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HourlyActivity {
    pub hour: u64,
    pub logins: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerActivityByWeekDayResponse {
    pub player_activity_by_week_day: PlayerActivityByWeekDay,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerActivityByWeekDay {
    #[serde(rename = "Monday")]
    pub monday: u64,
    #[serde(rename = "Tuesday")]
    pub tuesday: u64,
    #[serde(rename = "Wednesday")]
    pub wednesday: u64,
    #[serde(rename = "Thursday")]
    pub thursday: u64,
    #[serde(rename = "Friday")]
    pub friday: u64,
    #[serde(rename = "Saturday")]
    pub saturday: u64,
    #[serde(rename = "Sunday")]
    pub sunday: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaqData {
    pub username: String,
    pub uuid: String,
    pub server: String,
    pub id: i64,
    pub faq: String,
    #[serde(deserialize_with = "string_or_number")]
    pub timestamp: String,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostFaqResult {
    pub id: i64,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditFaqResult {
    #[serde(default = "default_true")]
    pub success: bool,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteOptions {
    pub random: bool,
    pub phrase: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundWebsocketMessage {
    pub action: String,
    pub data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundWebsocketMessage {
    pub action: String,
    pub data: Value,
}

#[derive(Debug, Clone)]
pub enum WebsocketEvent {
    Open,
    Close(String),
    Error(String),
    UnknownMessage(String),
    #[allow(dead_code)]
    KeyAccepted(Value),
    NewUser(NewUserData),
    NewName(NewUserNameData),
    InboundDiscordChat(DiscordChatMessage),
    InboundMinecraftChat(MinecraftChatMessage),
    #[allow(dead_code)]
    MinecraftPlayerDeath(MinecraftPlayerDeathMessage),
    #[allow(dead_code)]
    MinecraftPlayerKill(MinecraftPlayerKillMessage),
    #[allow(dead_code)]
    MinecraftPlayerJoin(MinecraftPlayerJoinMessage),
    #[allow(dead_code)]
    MinecraftPlayerLeave(MinecraftPlayerLeaveMessage),
    #[allow(dead_code)]
    MinecraftAdvancement(MinecraftAdvancementMessage),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewUserData {
    pub user: String,
    pub server: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewUserNameData {
    pub old_name: String,
    pub new_name: String,
    pub server: String,
}

fn unwrap_data(value: Value) -> Value {
    value
        .get("data")
        .cloned()
        .or_else(|| value.get("result").cloned())
        .unwrap_or(value)
}

fn stats_from_value(value: &Value, server: &str) -> Option<AllPlayerStats> {
    Some(AllPlayerStats {
        username: string_or_null(value, &["username"]),
        kills: int_or_string(value, &["kills"]),
        deaths: int_or_string(value, &["deaths"]),
        join_date: string_or_null(value, &["joindate"]),
        last_seen: string_or_null(value, &["lastseen"]),
        uuid: string_or_null(value, &["UUID", "uuid"]),
        playtime: u64_or_string(value, &["playtime"]),
        joins: u64_or_string(value, &["joins"]),
        leaves: u64_or_string(value, &["leaves"]),
        last_death_time: u64_or_string(value, &["lastdeathTime", "lastdeath_time"]),
        last_death_string: string_or_null(value, &["lastdeathString", "lastdeath_string"]),
        mc_server: Some(string_or_null(value, &["mc_server"]).unwrap_or_else(|| server.to_owned())),
    })
}

fn parse_inbound_message(text: &str) -> Option<WebsocketEvent> {
    let value: InboundWebsocketMessage = serde_json::from_str(text).ok()?;
    match value.action.as_str() {
        "new_user" => serde_json::from_value(value.data)
            .ok()
            .map(WebsocketEvent::NewUser),
        "new_name" => serde_json::from_value(value.data)
            .ok()
            .map(WebsocketEvent::NewName),
        "key-accepted" => Some(WebsocketEvent::KeyAccepted(value.data)),
        "inbound_discord_chat" => serde_json::from_value(value.data)
            .ok()
            .map(WebsocketEvent::InboundDiscordChat),
        "inbound_minecraft_chat" => serde_json::from_value(value.data)
            .ok()
            .map(WebsocketEvent::InboundMinecraftChat),
        "minecraft_player_death" => serde_json::from_value(value.data)
            .ok()
            .map(WebsocketEvent::MinecraftPlayerDeath),
        "minecraft_player_kill" => serde_json::from_value(value.data)
            .ok()
            .map(WebsocketEvent::MinecraftPlayerKill),
        "minecraft_player_join" => serde_json::from_value(value.data)
            .ok()
            .map(WebsocketEvent::MinecraftPlayerJoin),
        "minecraft_player_leave" => serde_json::from_value(value.data)
            .ok()
            .map(WebsocketEvent::MinecraftPlayerLeave),
        "minecraft_advancement" => serde_json::from_value(value.data)
            .ok()
            .map(WebsocketEvent::MinecraftAdvancement),
        "error" => Some(WebsocketEvent::Error(value.data.to_string())),
        _ => Some(WebsocketEvent::UnknownMessage(text.to_owned())),
    }
}

fn string_field(value: &Value, fields: &[&str]) -> Option<String> {
    fields.iter().find_map(|field| {
        value
            .get(*field)
            .and_then(|value| value.as_str().map(str::to_owned))
    })
}

fn string_or_null(value: &Value, fields: &[&str]) -> Option<String> {
    fields.iter().find_map(|field| {
        let value = value.get(*field)?;
        value
            .as_str()
            .map(str::to_owned)
            .or_else(|| value.as_u64().map(|value| value.to_string()))
            .or_else(|| value.as_i64().map(|value| value.to_string()))
    })
}

fn int_or_string(value: &Value, fields: &[&str]) -> Option<i64> {
    fields.iter().find_map(|field| {
        let value = value.get(*field)?;
        value
            .as_i64()
            .or_else(|| value.as_u64().map(|value| value as i64))
            .or_else(|| value.as_str().and_then(|value| value.parse::<i64>().ok()))
    })
}

fn u64_or_string(value: &Value, fields: &[&str]) -> Option<u64> {
    fields.iter().find_map(|field| {
        let value = value.get(*field)?;
        value
            .as_u64()
            .or_else(|| value.as_i64().map(|value| value as u64))
            .or_else(|| value.as_str().and_then(|value| value.parse::<u64>().ok()))
    })
}

fn number_field(value: &Value, fields: &[&str]) -> Option<u64> {
    u64_or_string(value, fields)
}

fn default_true() -> bool {
    true
}

fn string_or_number<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    value
        .as_str()
        .map(str::to_owned)
        .or_else(|| value.as_u64().map(|value| value.to_string()))
        .or_else(|| value.as_i64().map(|value| value.to_string()))
        .ok_or_else(|| serde::de::Error::custom("expected string or number"))
}

fn optional_string_or_number<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    let Some(value) = value else {
        return Ok(String::new());
    };
    if value.is_null() {
        return Ok(String::new());
    }
    value
        .as_str()
        .map(str::to_owned)
        .or_else(|| value.as_u64().map(|value| value.to_string()))
        .or_else(|| value.as_i64().map(|value| value.to_string()))
        .ok_or_else(|| serde::de::Error::custom("expected string, number, or null"))
}

fn string_or_vec_string<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    if let Some(value) = value.as_str() {
        return Ok(vec![value.to_owned()]);
    }
    if let Some(values) = value.as_array() {
        return values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| serde::de::Error::custom("expected string description item"))
            })
            .collect();
    }
    Err(serde::de::Error::custom(
        "expected string or string array description",
    ))
}

fn preview_body(body: &str) -> String {
    const MAX_PREVIEW: usize = 600;
    let compact = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() > MAX_PREVIEW {
        format!(
            "{}...<truncated>",
            compact.chars().take(MAX_PREVIEW).collect::<String>()
        )
    } else {
        compact
    }
}

fn preview_value(value: &Value) -> String {
    preview_body(&value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn websocket_uses_original_hub_headers_and_payload_shape() {
        let request = build_websocket_request(
            "ws://localhost:8001/websocket/connect",
            "test-key",
            "minecraft",
            "RefinedVanilla",
        )
        .unwrap();
        assert_eq!(request.uri().path(), "/websocket/connect");
        assert_eq!(request.headers().get("x-api-key").unwrap(), "test-key");
        assert_eq!(request.headers().get("client-type").unwrap(), "minecraft");
        assert_eq!(
            request.headers().get("mc_server").unwrap(),
            "RefinedVanilla"
        );

        let payload = serialize_websocket_payload(
            "minecraft_player_join",
            serde_json::to_value(MinecraftPlayerJoinMessage {
                username: "Steve".to_owned(),
                uuid: "uuid-1".to_owned(),
                timestamp: "123".to_owned(),
                server: "RefinedVanilla".to_owned(),
            })
            .unwrap(),
        )
        .unwrap();

        let value: Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(value["action"], "minecraft_player_join");
        assert_eq!(value["data"]["username"], "Steve");
        assert_eq!(value["data"]["uuid"], "uuid-1");
        assert_eq!(value["data"]["timestamp"], "123");
        assert_eq!(value["data"]["server"], "RefinedVanilla");
    }

    #[test]
    fn parses_original_hub_quote_payload_without_extra_fields() {
        let quote: Quote = serde_json::from_value(json!({
            "name": "Netherwhal",
            "message": "this is a long enough quote from the database",
            "date": "1710000000000"
        }))
        .unwrap();

        assert_eq!(quote.name, "Netherwhal");
        assert_eq!(
            quote.message,
            "this is a long enough quote from the database"
        );
        assert_eq!(quote.date.as_deref(), Some("1710000000000"));
        assert_eq!(quote.mc_server, "");
        assert_eq!(quote.uuid, "");
    }

    #[test]
    fn parses_original_hub_whois_and_message_count_payloads() {
        let whois: WhoIsData = serde_json::from_value(json!({
            "username": "JollyCurve_",
            "description": "known refinedvanilla player"
        }))
        .unwrap();
        assert_eq!(whois.description, vec!["known refinedvanilla player"]);

        let message_count: MessageCount = serde_json::from_value(json!({
            "count": 42
        }))
        .unwrap();
        assert_eq!(message_count.name, "");
        assert_eq!(message_count.message_count, 42);
    }

    #[test]
    fn parses_original_hub_message_faq_and_edit_faq_payloads() {
        let message: MinecraftChatMessage = serde_json::from_value(json!({
            "name": "CanadaBinny",
            "message": "hello from the old hub",
            "date": 1710000000000_i64,
            "mc_server": "refinedvanilla",
            "uuid": null
        }))
        .unwrap();
        assert_eq!(message.date, "1710000000000");
        assert_eq!(message.uuid, "");

        let faq: FaqData = serde_json::from_value(json!({
            "username": "CanadaBinny",
            "uuid": "uuid-1",
            "server": "refinedvanilla",
            "id": 7,
            "faq": "old hub faq text",
            "timestamp": 1710000000000_i64,
            "total": 9
        }))
        .unwrap();
        assert_eq!(faq.timestamp, "1710000000000");

        let edit: EditFaqResult = serde_json::from_value(json!({
            "message": "FAQ updated successfully.",
            "id": 7
        }))
        .unwrap();
        assert!(edit.success);
        assert_eq!(edit.error, None);
    }
}
