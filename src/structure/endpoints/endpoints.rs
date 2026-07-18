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
            .and_then(|value| u64_or_string(&value, &["total_advancements"]))
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
        exclude_commands: bool,
    ) -> Option<WordOccurrence> {
        self.get_json(
            "/wordcount",
            &[
                ("username", username),
                ("server", server),
                ("word", word),
                ("exclude_commands", if exclude_commands { "true" } else { "false" }),
            ],
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

    pub async fn get_top_messages(&self, server: &str, limit: usize) -> Option<Value> {
        self.get_json(
            "/top-messages",
            &[("server", server), ("limit", &limit.to_string())],
        )
        .await
    }

    pub async fn get_top_slurcount(
        &self,
        server: &str,
        words: &[String],
        limit: usize,
    ) -> Option<Value> {
        let words_param = words.join(",");
        self.get_json(
            "/top-slurcount",
            &[
                ("server", server),
                ("words", &words_param),
                ("limit", &limit.to_string()),
            ],
        )
        .await
    }

    pub async fn get_server_summary(&self, server: &str) -> Option<Value> {
        self.get_json("/server-summary", &[("server", server)]).await
    }

    pub async fn get_trade_leaderboard(&self) -> Option<Value> {
        self.get_json("/tradebot/leaderboard", &[]).await
    }

    pub async fn get_scammers(&self) -> Option<Value> {
        self.get_json("/tradebot/scammers", &[]).await
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

    pub async fn get_owned_faq_ids(&self, username: &str) -> Option<Vec<OwnedFaqEntry>> {
        self.get_json("/get-owned-faq-ids", &[("name", username)])
            .await
            .and_then(|value| self.parse_json::<OwnedFaqsResponse>("/get-owned-faq-ids", value))
            .map(|r| r.faqs)
    }

    /// Pushes the full {name, bridge_ok}[] command list to Hub, replacing its map wholesale.
    /// Discord bot reads this back via GET /craftbot/bridge-commands to gate chat-bridge relaying.
    pub async fn push_bridge_commands(&self, commands: &[(String, bool)]) -> Option<Value> {
        let payload: Vec<Value> = commands
            .iter()
            .map(|(name, bridge_ok)| json!({ "name": name, "bridge_ok": bridge_ok }))
            .collect();
        self.post_json("/craftbot/bridge-commands", json!({ "commands": payload }))
            .await
    }

    /// Fire-and-forget from craftbot on player join -- Hub skips the actual fetch if a
    /// fresh (<24h) head is already cached, so this is cheap for returning players.
    pub async fn ensure_head_cached(&self, uuid: &str) -> Option<Value> {
        self.post_json("/craftbot/ensure-head", json!({ "uuid": uuid }))
            .await
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

    pub async fn delete_faq(&self, id: i64, username: &str) -> Option<DeleteFaqResult> {
        self.post_json(
            "/delete-faq",
            json!({ "id": id, "username": username }),
        )
        .await
        .and_then(|value| self.parse_json("/delete-faq", value))
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
    pub async fn tradebot_get_trade(&self, trade_id: i64) -> Option<TradebotTrade> {
        self.get_json(&format!("/tradebot/trade/{trade_id}"), &[])
            .await
            .and_then(|v| self.parse_json("/tradebot/trade", v))
    }

    pub async fn tradebot_create_trade(
        &self,
        initiator_id: &str,
        recipient_id: &str,
        description: &str,
        server: &str,
    ) -> Option<i64> {
        self.post_json(
            "/tradebot/trade",
            json!({
                "initiator_id": initiator_id,
                "recipient_id": recipient_id,
                "description": description,
                "guild_id": server,
                "channel_id": "minecraft",
            }),
        )
        .await
        .and_then(|v| v.get("id").and_then(Value::as_i64))
    }

    pub async fn tradebot_confirm_trade(&self, trade_id: i64) -> Result<(), String> {
        match self
            .post_json(&format!("/tradebot/trade/{trade_id}/confirm"), json!({}))
            .await
        {
            Some(v) => match v.get("error").and_then(Value::as_str) {
                Some(err) => Err(err.to_owned()),
                None => Ok(()),
            },
            None => Err("request failed".to_owned()),
        }
    }

    pub async fn tradebot_reject_trade(&self, trade_id: i64) -> Result<(), String> {
        match self
            .post_json(&format!("/tradebot/trade/{trade_id}/reject"), json!({}))
            .await
        {
            Some(v) => match v.get("error").and_then(Value::as_str) {
                Some(err) => Err(err.to_owned()),
                None => Ok(()),
            },
            None => Err("request failed".to_owned()),
        }
    }

    pub async fn tradebot_get_user_trades(&self, user_id: &str) -> Vec<TradebotTrade> {
        self.get_json(&format!("/tradebot/user/{user_id}/trades"), &[])
            .await
            .and_then(|v| self.parse_json("/tradebot/user/trades", v))
            .unwrap_or_default()
    }

    pub async fn tradebot_get_stats(&self, user_id: &str) -> Option<TradebotStatsResponse> {
        self.get_json(&format!("/tradebot/user/{user_id}/trade-stats"), &[])
            .await
            .and_then(|v| self.parse_json("/tradebot/user/trade-stats", v))
    }

    pub async fn tradebot_get_scammer(&self, user_id: &str) -> Option<TradebotScammer> {
        let v = self.get_json(&format!("/tradebot/user/{user_id}/scammer"), &[]).await?;
        let scammer = v.get("scammer")?.clone();
        if scammer.is_null() { return None; }
        self.parse_json("/tradebot/user/scammer", scammer)
    }

    pub async fn tradebot_report_user(
        &self,
        reporter_id: &str,
        reported_user_id: &str,
        description: &str,
        server: &str,
    ) -> bool {
        self.post_json(
            "/tradebot/report",
            json!({
                "reporter_id": reporter_id,
                "reported_user_id": reported_user_id,
                "reason": "other",
                "description": description,
                "guild_id": server,
            }),
        )
        .await
        .is_some()
    }

    pub async fn tradebot_mc_username(&self, uuid: &str) -> Option<String> {
        self.get_json(&format!("/tradebot/mc-username/{uuid}"), &[])
            .await
            .and_then(|v| v.get("username").and_then(|u| u.as_str()).map(str::to_owned))
    }

    pub async fn tradebot_discord_username(&self, user_id: &str) -> Option<String> {
        self.get_json(&format!("/tradebot/discord-username/{user_id}"), &[])
            .await
            .and_then(|v| v.get("username").and_then(|u| u.as_str()).map(str::to_owned))
    }

    pub async fn tradebot_linked_mc_uuid(&self, discord_id: &str) -> Option<String> {
        self.get_json(&format!("/tradebot/link/{discord_id}"), &[])
            .await
            .and_then(|v| {
                v.get("link")?.get("mc_uuid")?.as_str().map(str::to_owned)
            })
    }

    pub async fn tradebot_unlink(&self, mc_uuid: &str) -> bool {
        self.delete_json(&format!("/tradebot/link/{mc_uuid}"))
            .await
            .and_then(|v| v.get("success")?.as_bool())
            .unwrap_or(false)
    }

    pub async fn tradebot_request_link_code(&self, mc_uuid: &str, code: &str) -> bool {
        match self
            .post_json(
                "/tradebot/link-code",
                json!({ "mc_uuid": mc_uuid, "code": code }),
            )
            .await
        {
            Some(v) => v.get("success").and_then(|v| v.as_bool()).unwrap_or(false),
            None => false,
        }
    }

    pub async fn tradebot_get_greeting(&self, username: &str) -> Option<(Option<String>, Option<String>)> {
        let v = self.get_json(&format!("/tradebot/greeting/{username}"), &[]).await?;
        let greeting = v.get("greeting").and_then(|g| g.as_str()).map(str::to_owned);
        let last_fired_at = v.get("last_fired_at").and_then(|t| t.as_str()).map(str::to_owned);
        Some((greeting, last_fired_at))
    }

    pub async fn tradebot_set_greeting(&self, username: &str, greeting: Option<&str>) -> bool {
        match self
            .post_json(
                &format!("/tradebot/greeting/{username}"),
                json!({ "greeting": greeting }),
            )
            .await
        {
            Some(v) => v.get("ok").and_then(|v| v.as_bool()).unwrap_or(false),
            None => false,
        }
    }

    pub async fn tradebot_fire_greeting(&self, username: &str) -> bool {
        match self
            .post_json(&format!("/tradebot/greeting/{username}/fired"), json!({}))
            .await
        {
            Some(v) => v.get("ok").and_then(|v| v.as_bool()).unwrap_or(false),
            None => false,
        }
    }

    // ── Casino API ────────────────────────────────────────────────────────────

    pub async fn casino_get_balance(&self, player_uuid: &str) -> Option<CasinoBalance> {
        self.get_json(&format!("/casino/balance/{player_uuid}"), &[])
            .await
            .and_then(|v| self.parse_json("/casino/balance", v))
    }

    pub async fn casino_faucet(&self, player_uuid: &str) -> CasinoFaucetResult {
        let Some(v) = self
            .post_json("/casino/faucet", json!({ "player_uuid": player_uuid }))
            .await
        else {
            return CasinoFaucetResult::Err;
        };
        if v.get("error").and_then(|e| e.as_str()) == Some("cooldown") {
            let next_secs = v
                .get("next_claim_secs")
                .and_then(|s| s.as_u64())
                .unwrap_or(0);
            return CasinoFaucetResult::OnCooldown { next_secs };
        }
        CasinoFaucetResult::Awarded {
            chips_awarded: v.get("chips_awarded").and_then(|c| c.as_i64()).unwrap_or(0),
            streak: v.get("streak").and_then(|s| s.as_i64()).unwrap_or(0) as i32,
            chips: v.get("chips").and_then(|c| c.as_i64()).unwrap_or(0),
            lotto_pick: v.get("lotto_pick").and_then(|p| p.as_str()).unwrap_or("").to_owned(),
            draw_date: v.get("draw_date").and_then(|d| d.as_str()).unwrap_or("").to_owned(),
        }
    }

    pub async fn casino_adjust(
        &self,
        player_uuid: &str,
        delta: i64,
    ) -> Result<i64, CasinoAdjustErr> {
        let Some(v) = self
            .post_json("/casino/adjust", json!({ "player_uuid": player_uuid, "delta": delta }))
            .await
        else {
            return Err(CasinoAdjustErr::NetworkErr);
        };
        if v.get("error").and_then(|e| e.as_str()) == Some("insufficient_funds") {
            let chips = v.get("chips").and_then(|c| c.as_i64()).unwrap_or(0);
            return Err(CasinoAdjustErr::InsufficientFunds(chips));
        }
        Ok(v.get("chips").and_then(|c| c.as_i64()).unwrap_or(0))
    }

    pub async fn casino_transfer(
        &self,
        from_uuid: &str,
        to_uuid: &str,
        amount: i64,
    ) -> Result<(), String> {
        let Some(v) = self
            .post_json(
                "/casino/transfer",
                json!({ "from_uuid": from_uuid, "to_uuid": to_uuid, "amount": amount }),
            )
            .await
        else {
            return Err("Network error".to_owned());
        };
        if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
            return Err(match err {
                "insufficient_funds" => {
                    let chips = v.get("chips").and_then(|c| c.as_i64()).unwrap_or(0);
                    format!("Not enough chips (have {chips})")
                }
                other => other.to_owned(),
            });
        }
        Ok(())
    }

    pub async fn casino_free_scratch(&self, player_uuid: &str) -> CasinoScratchResult {
        let Some(v) = self
            .post_json("/casino/scratch/free", json!({ "player_uuid": player_uuid }))
            .await
        else {
            return CasinoScratchResult::Err;
        };
        if v.get("error").and_then(|e| e.as_str()) == Some("cooldown") {
            let next_secs = v
                .get("next_scratch_secs")
                .and_then(|s| s.as_u64())
                .unwrap_or(0);
            return CasinoScratchResult::OnCooldown { next_secs };
        }
        CasinoScratchResult::Ok
    }

    pub async fn casino_jackpot_get(&self, player: Option<&str>) -> Option<CasinoJackpotInfo> {
        let v = match player {
            Some(p) => self.get_json("/casino/jackpot", &[("player_uuid", p)]).await?,
            None => self.get_json("/casino/jackpot", &[]).await?,
        };
        Some(CasinoJackpotInfo {
            pot: v.get("pot").and_then(|p| p.as_i64()).unwrap_or(0),
            tickets: v.get("tickets").and_then(|t| t.as_i64()).unwrap_or(0) as i32,
            next_draw: v.get("next_draw").and_then(|d| d.as_str()).unwrap_or("").to_owned(),
        })
    }

    pub async fn casino_jackpot_buy_ticket(
        &self,
        player_uuid: &str,
        count: u32,
    ) -> Result<CasinoJackpotInfo, String> {
        let Some(v) = self
            .post_json("/casino/jackpot/ticket", json!({ "player_uuid": player_uuid, "count": count }))
            .await
        else {
            return Err("Network error".to_owned());
        };
        if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
            return Err(match err {
                "insufficient_funds" => format!("Not enough chips (25 each, {} total)", 25 * count as i64),
                other => other.to_owned(),
            });
        }
        Ok(CasinoJackpotInfo {
            pot: v.get("pot").and_then(|p| p.as_i64()).unwrap_or(0),
            tickets: v.get("tickets").and_then(|t| t.as_i64()).unwrap_or(0) as i32,
            next_draw: v.get("next_draw").and_then(|d| d.as_str()).unwrap_or("").to_owned(),
        })
    }

    pub async fn casino_jackpot_rake(&self, amount: i64) {
        let _ = self
            .post_json("/casino/jackpot/rake", json!({ "amount": amount }))
            .await;
    }

    // ── Marriage / alimony ───────────────────────────────────────────────────
    // casino_win is the universal payout endpoint -- every real win-credit call
    // site uses this instead of casino_adjust, so alimony garnishment (computed
    // Hub-side against the alimony_debt ledger) applies uniformly. Non-win
    // credits (debits, refunds, pushes, admin transfers) stay on casino_adjust.

    pub async fn casino_win(
        &self,
        player_uuid: &str,
        gross_win: i64,
    ) -> Result<CasinoWinResult, CasinoAdjustErr> {
        let Some(v) = self
            .post_json("/casino/win", json!({ "player_uuid": player_uuid, "gross_win": gross_win }))
            .await
        else {
            return Err(CasinoAdjustErr::NetworkErr);
        };
        if v.get("error").is_some() {
            return Err(CasinoAdjustErr::NetworkErr);
        }
        Ok(CasinoWinResult {
            chips: v.get("chips").and_then(|c| c.as_i64()).unwrap_or(0),
            alimony_paid: v.get("alimony_paid").and_then(|c| c.as_i64()).unwrap_or(0),
            ex_count: v.get("ex_count").and_then(|c| c.as_u64()).unwrap_or(0) as usize,
            net: v.get("net").and_then(|c| c.as_i64()).unwrap_or(gross_win),
        })
    }

    pub async fn marry_propose(&self, proposer_uuid: &str, target_uuid: &str) -> Result<(), String> {
        let Some(v) = self
            .post_json("/casino/marriage/propose", json!({ "proposer_uuid": proposer_uuid, "target_uuid": target_uuid }))
            .await
        else {
            return Err("Network error".to_owned());
        };
        if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
            return Err(err.to_owned());
        }
        Ok(())
    }

    pub async fn marry_dowry(&self, target_uuid: &str, dowry: i64) -> Result<String, String> {
        let Some(v) = self
            .post_json("/casino/marriage/dowry", json!({ "target_uuid": target_uuid, "dowry": dowry }))
            .await
        else {
            return Err("Network error".to_owned());
        };
        if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
            return Err(err.to_owned());
        }
        Ok(v.get("proposer_uuid").and_then(|p| p.as_str()).unwrap_or("").to_owned())
    }

    pub async fn marry_accept(&self, uuid: &str) -> Result<MarryAcceptResult, String> {
        let Some(v) = self
            .post_json("/casino/marriage/accept", json!({ "uuid": uuid }))
            .await
        else {
            return Err("Network error".to_owned());
        };
        if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
            return Err(err.to_owned());
        }
        Ok(MarryAcceptResult {
            proposer_uuid: v.get("proposer_uuid").and_then(|p| p.as_str()).unwrap_or("").to_owned(),
            target_uuid: v.get("target_uuid").and_then(|p| p.as_str()).unwrap_or("").to_owned(),
            dowry_paid: v.get("dowry_paid").and_then(|d| d.as_i64()).unwrap_or(0),
        })
    }

    pub async fn marry_reject(&self, uuid: &str) -> Result<(), String> {
        let Some(v) = self
            .post_json("/casino/marriage/reject", json!({ "uuid": uuid }))
            .await
        else {
            return Err("Network error".to_owned());
        };
        if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
            return Err(err.to_owned());
        }
        Ok(())
    }

    pub async fn marry_get_proposal(&self, uuid: &str) -> Option<MarriageProposal> {
        let v = self.get_json(&format!("/casino/marriage/proposal/{uuid}"), &[]).await?;
        if v.get("error").is_some() {
            return None;
        }
        let role = v.get("role").and_then(|r| r.as_str()).unwrap_or("").to_owned();
        let p = v.get("proposal")?;
        if p.is_null() {
            return None;
        }
        Some(MarriageProposal {
            proposer_uuid: p.get("proposer_uuid").and_then(|s| s.as_str()).unwrap_or("").to_owned(),
            target_uuid: p.get("target_uuid").and_then(|s| s.as_str()).unwrap_or("").to_owned(),
            dowry: p.get("dowry").and_then(|d| d.as_i64()).unwrap_or(0),
            state: p.get("state").and_then(|s| s.as_str()).unwrap_or("pending").to_owned(),
            role,
        })
    }

    pub async fn marry_get_spouses(&self, uuid: &str) -> Vec<MarriageSpouseEntry> {
        let Some(v) = self.get_json(&format!("/casino/marriage/spouses/{uuid}"), &[]).await else {
            return vec![];
        };
        v.get("spouses")
            .and_then(|s| s.as_array())
            .map(|arr| arr.iter().filter_map(|e| {
                Some(MarriageSpouseEntry {
                    spouse_uuid: e.get("spouse_uuid").and_then(|s| s.as_str())?.to_owned(),
                })
            }).collect())
            .unwrap_or_default()
    }

    pub async fn marry_divorce(&self, initiator_uuid: &str, partner_uuid: &str) -> Result<DivorceResult, String> {
        let Some(v) = self
            .post_json(
                "/casino/marriage/divorce",
                json!({ "initiator_uuid": initiator_uuid, "partner_uuid": partner_uuid }),
            )
            .await
        else {
            return Err("Network error".to_owned());
        };
        if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
            return Err(err.to_owned());
        }
        let status = v.get("status").and_then(|s| s.as_str()).unwrap_or("");
        match status {
            "divorced" => Ok(DivorceResult::Divorced),
            _ => Ok(DivorceResult::Pending {
                already_waiting: v.get("already_waiting").and_then(|w| w.as_bool()).unwrap_or(false),
            }),
        }
    }

    pub async fn marry_divorce_force(
        &self,
        initiator_uuid: &str,
        partner_uuid: &str,
    ) -> Result<ForceDivorceResult, String> {
        let Some(v) = self
            .post_json(
                "/casino/marriage/divorce/force",
                json!({ "initiator_uuid": initiator_uuid, "partner_uuid": partner_uuid }),
            )
            .await
        else {
            return Err("Network error".to_owned());
        };
        if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
            return Err(err.to_owned());
        }
        Ok(ForceDivorceResult {
            partner_uuid: v.get("partner_uuid").and_then(|p| p.as_str()).unwrap_or("").to_owned(),
            alimony_days: v.get("alimony_days").and_then(|d| d.as_i64()).unwrap_or(0),
        })
    }

    pub async fn casino_claim_notifications(&self, player_uuid: &str) -> Vec<String> {
        let Some(v) = self
            .post_json("/casino/notifications/claim", json!({ "player_uuid": player_uuid }))
            .await
        else {
            return vec![];
        };
        v.get("messages")
            .and_then(|m| m.as_array())
            .map(|arr| arr.iter().filter_map(|s| s.as_str().map(str::to_owned)).collect())
            .unwrap_or_default()
    }

    pub async fn casino_add_notification(&self, player_uuid: &str, message: &str) {
        let _ = self
            .post_json("/casino/notifications/add", json!({ "player_uuid": player_uuid, "message": message }))
            .await;
    }

    pub async fn casino_lotto_get_tickets(&self, player_uuid: &str) -> Vec<CasinoLottoPlayerTicket> {
        let Some(v) = self.get_json(&format!("/casino/lotto/tickets/{player_uuid}"), &[]).await else {
            return vec![];
        };
        v.get("tickets")
            .and_then(|t| t.as_array())
            .map(|arr| {
                arr.iter().filter_map(|item| {
                    Some(CasinoLottoPlayerTicket {
                        numbers: item.get("numbers")?.as_str()?.to_owned(),
                        draw_date: item.get("draw_date")?.as_str()?.to_owned(),
                    })
                }).collect()
            })
            .unwrap_or_default()
    }

    pub async fn casino_lotto_get_pot(&self) -> Option<CasinoLottoPot> {
        let v = self.get_json("/casino/lotto/pot", &[]).await?;
        Some(CasinoLottoPot {
            pot: v.get("pot").and_then(|p| p.as_i64()).unwrap_or(0),
            draw_date: v.get("draw_date").and_then(|d| d.as_str()).map(|s| s.to_owned()),
        })
    }

    pub async fn casino_lotto_last_draw(&self) -> Option<CasinoLastLottoDraw> {
        let v = self.get_json("/casino/lotto/results/last", &[]).await?;
        Some(CasinoLastLottoDraw {
            draw_date: v.get("draw_date")?.as_str()?.to_owned(),
            numbers: v.get("numbers")?.as_str()?.to_owned(),
        })
    }

    pub async fn casino_fire_lotto_draw(&self) -> bool {
        self.post_json("/casino/draw/lotto", json!({})).await.is_some()
    }

    pub async fn casino_fire_jackpot_draw(&self) -> bool {
        self.post_json("/casino/draw/jackpot", json!({})).await.is_some()
    }

    pub async fn casino_lotto_buy_ticket(
        &self,
        player_uuid: &str,
        numbers: &str,
    ) -> Result<CasinoLottoTicketInfo, String> {
        let Some(v) = self
            .post_json(
                "/casino/lotto/ticket",
                json!({ "player_uuid": player_uuid, "numbers": numbers }),
            )
            .await
        else {
            return Err("Network error".to_owned());
        };
        if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
            return Err(match err {
                "insufficient_funds" => "Not enough chips (costs 50)".to_owned(),
                "invalid_numbers" => "Pick 5 unique numbers between 1-40".to_owned(),
                other => other.to_owned(),
            });
        }
        Ok(CasinoLottoTicketInfo {
            numbers: v.get("numbers").and_then(|n| n.as_str()).unwrap_or("").to_owned(),
            draw_date: v.get("draw_date").and_then(|d| d.as_str()).unwrap_or("").to_owned(),
            pot: v.get("pot").and_then(|p| p.as_i64()).unwrap_or(0),
            chips: v.get("chips").and_then(|c| c.as_i64()).unwrap_or(0),
        })
    }

    pub async fn casino_lotto_buy_quick(
        &self,
        player_uuid: &str,
        count: u32,
    ) -> Result<CasinoLottoBulkInfo, String> {
        let Some(v) = self
            .post_json("/casino/lotto/ticket", json!({ "player_uuid": player_uuid, "count": count }))
            .await
        else {
            return Err("Network error".to_owned());
        };
        if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
            return Err(match err {
                "insufficient_funds" => format!("Not enough chips (50 each, {} total)", 50 * count as i64),
                other => other.to_owned(),
            });
        }
        let tickets = v.get("tickets")
            .and_then(|t| t.as_array())
            .map(|arr| arr.iter().filter_map(|s| s.as_str().map(str::to_owned)).collect())
            .unwrap_or_default();
        Ok(CasinoLottoBulkInfo {
            tickets,
            draw_date: v.get("draw_date").and_then(|d| d.as_str()).unwrap_or("").to_owned(),
            pot: v.get("pot").and_then(|p| p.as_i64()).unwrap_or(0),
            chips: v.get("chips").and_then(|c| c.as_i64()).unwrap_or(0),
        })
    }

    pub async fn casino_market_bet_insert(&self, bet: &crate::structure::market::types::MarketBet) -> Option<i64> {
        use crate::structure::market::types::{Direction, MarketKind};
        let v = self.post_json(
            "/casino/market-bet",
            json!({
                "player_uuid": bet.player,
                "symbol": bet.symbol,
                "market": if bet.market == MarketKind::Crypto { "crypto" } else { "stock" },
                "direction": if bet.direction == Direction::Long { "long" } else { "short" },
                "entry_price": bet.entry_price,
                "stake": bet.stake,
                "closes_unix": bet.closes_unix,
                "duration_label": bet.duration_label,
            }),
        )
        .await?;
        v.get("id").and_then(|id| id.as_i64())
    }

    pub async fn casino_market_bet_list(&self) -> Vec<crate::structure::market::types::MarketBet> {
        use crate::structure::market::types::{Direction, MarketBet, MarketKind};
        let Some(v) = self.get_json("/casino/market-bets", &[]).await else { return vec![]; };
        v.get("bets")
            .and_then(|b| b.as_array())
            .map(|arr| arr.iter().filter_map(|item| {
                let id = item.get("id")?.as_i64()?;
                let player = item.get("player_uuid")?.as_str()?.to_owned();
                let symbol = item.get("symbol")?.as_str()?.to_owned();
                let market = match item.get("market")?.as_str()? {
                    "crypto" => MarketKind::Crypto,
                    _ => MarketKind::Stock,
                };
                let direction = match item.get("direction")?.as_str()? {
                    "long" => Direction::Long,
                    _ => Direction::Short,
                };
                let entry_price = item.get("entry_price")?.as_f64()?;
                let stake = item.get("stake")?.as_i64()?;
                let closes_unix = item.get("closes_unix")?.as_u64()?;
                let duration_label = item.get("duration_label")?.as_str()?.to_owned();
                Some(MarketBet { id, player, symbol, market, direction, entry_price, stake, closes_unix, duration_label })
            }).collect())
            .unwrap_or_default()
    }

    pub async fn casino_market_bet_delete(&self, id: i64) {
        let _ = self.delete_json(&format!("/casino/market-bet/{id}")).await;
    }

    pub async fn casino_weather_bet_insert(&self, bet: &crate::commands::weather::WeatherBet) -> Option<i64> {
        let v = self.post_json(
            "/casino/weather-bet",
            json!({
                "player_uuid": bet.player,
                "bet_type": bet.bet_type,
                "city": bet.city,
                "latitude": bet.latitude,
                "longitude": bet.longitude,
                "direction": bet.direction,
                "threshold": bet.threshold,
                "unit": bet.unit,
                "forecast_prob": bet.forecast_prob,
                "payout_mult": bet.payout_mult,
                "stake": bet.stake,
                "closes_unix": bet.closes_unix,
                "duration_label": bet.duration_label,
            }),
        )
        .await?;
        v.get("id").and_then(|id| id.as_i64())
    }

    pub async fn casino_weather_bet_list(&self) -> Vec<crate::commands::weather::WeatherBet> {
        use crate::commands::weather::WeatherBet;
        let Some(v) = self.get_json("/casino/weather-bets", &[]).await else { return vec![]; };
        v.get("bets")
            .and_then(|b| b.as_array())
            .map(|arr| arr.iter().filter_map(|item| {
                let id = item.get("id")?.as_i64()?;
                let player = item.get("player_uuid")?.as_str()?.to_owned();
                let bet_type = item.get("bet_type")?.as_str().unwrap_or("rain").to_owned();
                let city = item.get("city")?.as_str()?.to_owned();
                let latitude = item.get("latitude")?.as_f64()?;
                let longitude = item.get("longitude")?.as_f64()?;
                let direction = item.get("direction")?.as_str()?.to_owned();
                let threshold = item.get("threshold").and_then(|v| v.as_f64());
                let unit = item.get("unit").and_then(|v| v.as_str()).map(|s| s.to_owned());
                let forecast_prob = item.get("forecast_prob")?.as_u64()? as u8;
                let payout_mult = item.get("payout_mult")?.as_f64()?;
                let stake = item.get("stake")?.as_i64()?;
                let closes_unix = item.get("closes_unix")?.as_u64()?;
                let duration_label = item.get("duration_label")?.as_str()?.to_owned();
                Some(WeatherBet { id, player, bet_type, city, latitude, longitude, direction, threshold, unit, forecast_prob, payout_mult, stake, closes_unix, duration_label })
            }).collect())
            .unwrap_or_default()
    }

    pub async fn casino_weather_bet_delete(&self, id: i64) {
        let _ = self.delete_json(&format!("/casino/weather-bet/{id}")).await;
    }

    /// Generic insert for the 11 `CasinoBet` types backed by Hub's consolidated
    /// `/casino/bet/{type}` routes. Per-type field-name mapping lives in each
    /// type's own `CasinoBet` impl (co-located with its struct), not here.
    pub async fn casino_bet_insert<T: crate::commands::casino::CasinoBet>(&self, bet: &T) -> Option<i64> {
        let v = self.post_json(&format!("/casino/bet/{}", T::TYPE), bet.to_insert_json()).await?;
        v.get("id").and_then(|id| id.as_i64())
    }

    pub async fn casino_bet_list<T: crate::commands::casino::CasinoBet>(&self) -> Vec<T> {
        let Some(v) = self.get_json(&format!("/casino/bets/{}", T::TYPE), &[]).await else { return vec![]; };
        v.get("bets")
            .and_then(|b| b.as_array())
            .map(|arr| arr.iter().filter_map(T::from_json).collect())
            .unwrap_or_default()
    }

    pub async fn casino_bet_delete<T: crate::commands::casino::CasinoBet>(&self, id: i64) {
        let _ = self.delete_json(&format!("/casino/bet/{}/{id}", T::TYPE)).await;
    }

    pub async fn casino_event_bets_list(&self, player_uuid: &str) -> Vec<serde_json::Value> {
        let Some(v) = self.get_json(&format!("/casino/event-bets/{player_uuid}"), &[]).await else { return vec![]; };
        v.get("bets")
            .and_then(|b| b.as_array())
            .cloned()
            .unwrap_or_default()
    }

    pub async fn casino_portfolio_insert(&self, pos: &crate::structure::market::types::PortfolioPosition) -> Option<i64> {
        use crate::structure::market::types::MarketKind;
        let v = self.post_json(
            "/casino/portfolio-position",
            json!({
                "player_uuid": pos.player,
                "symbol": pos.symbol,
                "market": if pos.market == MarketKind::Crypto { "crypto" } else { "stock" },
                "entry_price": pos.entry_price,
                "stake": pos.stake,
                "opened_unix": pos.opened_unix,
            }),
        )
        .await?;
        v.get("id").and_then(|id| id.as_i64())
    }

    pub async fn casino_portfolio_list(&self) -> Vec<crate::structure::market::types::PortfolioPosition> {
        use crate::structure::market::types::{MarketKind, PortfolioPosition};
        let Some(v) = self.get_json("/casino/portfolio-positions", &[]).await else { return vec![]; };
        v.get("positions")
            .and_then(|b| b.as_array())
            .map(|arr| arr.iter().filter_map(|item| {
                let id = item.get("id")?.as_i64()?;
                let player = item.get("player_uuid")?.as_str()?.to_owned();
                let symbol = item.get("symbol")?.as_str()?.to_owned();
                let market = match item.get("market")?.as_str()? {
                    "crypto" => MarketKind::Crypto,
                    _ => MarketKind::Stock,
                };
                let entry_price = item.get("entry_price")?.as_f64()?;
                let stake = item.get("stake")?.as_i64()?;
                let opened_unix = item.get("opened_unix")?.as_u64()?;
                Some(PortfolioPosition { id, player, symbol, market, entry_price, stake, opened_unix })
            }).collect())
            .unwrap_or_default()
    }

    pub async fn casino_portfolio_delete(&self, id: i64) {
        let _ = self.delete_json(&format!("/casino/portfolio-position/{id}")).await;
    }

    pub async fn increment_duel_wins(&self, username: &str) {
        let _ = self.post_json("/users/duel-win", json!({ "username": username })).await;
    }

    pub async fn get_user_fadv_ids(&self, uuid: &str, server: &str) -> Option<Vec<String>> {
        let v = self.get_json(
            &format!("/fadv/user-awards/{uuid}"),
            &[("mc_server", server)],
        )
        .await?;
        let ids = v.get("fadv_ids").cloned()?;
        match serde_json::from_value::<Vec<String>>(ids) {
            Ok(ids) => Some(ids),
            Err(e) => {
                self.log_error(format!("/fadv/user-awards shape mismatch: {e}"));
                None
            }
        }
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

    async fn delete_json(&self, path: &str) -> Option<Value> {
        let url = format!("{}{}", self.base_url(), path);
        let response = self.client.delete(url).send().await;
        match response {
            Ok(response) => {
                let status = response.status();
                let url = response.url().to_string();
                let value = match response.text().await {
                    Ok(body) => self.parse_response_body("DELETE", path, &url, status, &body),
                    Err(error) => {
                        self.log_error(format!("DELETE {path} failed reading body: {error}"));
                        None
                    }
                };
                if status.is_success() { value } else {
                    self.log_error(anyhow!("DELETE {path} failed with status {status}"));
                    value
                }
            }
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

    pub async fn send_content_flagged(&self, data: ContentFlaggedData) -> Result<()> {
        self.send_message("content_flagged", serde_json::to_value(data)?).await
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
    #[serde(
        default,
        alias = "time",
        deserialize_with = "optional_string_or_number"
    )]
    pub timestamp: String,
    pub mc_server: String,
    #[serde(default)]
    pub channel_id: String,
    #[serde(default)]
    pub guild_id: String,
    #[serde(default)]
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
    pub joindate: Option<String>,
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
pub struct DeleteFaqResult {
    #[serde(default = "default_true")]
    pub success: bool,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnedFaqEntry {
    pub id: i64,
    pub faq: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OwnedFaqsResponse {
    pub faqs: Vec<OwnedFaqEntry>,
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
    ScammerMarked(ScammerMarkedData),
    ScammerUnmarked(ScammerMarkedData),
    TradesReset(TradesResetData),
    TradesUnreset(TradesResetData),
    FadvAwards(FadvAwardsEvent),
    PearlResult(PearlResultData),
    ResolveDiscordUsernameResult(ResolveDiscordUsernameResultData),
    ResolveDiscordUsernameUnavailable(ResolveDiscordUsernameUnavailableData),
    CasinoDrawResult(CasinoDrawData),
    CasinoWinnerNotify(CasinoWinnerNotifyData),
    Ignored,
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
pub struct CasinoDrawData {
    #[serde(rename = "type")]
    pub draw_type: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CasinoWinnerNotifyData {
    pub player: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PearlResultData {
    pub slot: u8,
    pub success: bool,
    pub message: String,
    pub requester: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveDiscordUsernameResultData {
    pub request_id: String,
    pub snowflake: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveDiscordUsernameUnavailableData {
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FadvAward {
    pub fadv_id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FadvAwardsEvent {
    pub username: String,
    pub awards: Vec<FadvAward>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScammerMarkedData {
    pub user_id: String,
    pub reason: String,
    pub guild_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradesResetData {
    pub user_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentFlaggedData {
    pub username: String,
    pub mc_server: String,
    pub command: String,
    pub content: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradebotTrade {
    pub id: i64,
    pub initiator_id: String,
    pub recipient_id: String,
    pub description: String,
    pub status: String,
    pub created_at: i64,
    pub confirmed_at: Option<i64>,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradebotStats {
    pub total_trades: i64,
    pub confirmed_trades: i64,
    pub rejected_trades: i64,
    pub initiated_trades: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradebotPartner {
    pub partner_id: String,
    pub trade_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradebotStatsResponse {
    pub stats: TradebotStats,
    #[serde(default)]
    pub partners: Vec<TradebotPartner>,
    #[serde(rename = "scammerStatus")]
    pub scammer_status: Option<TradebotScammer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradebotScammer {
    pub reason: String,
    #[serde(default)]
    pub moderator_id: String,
    #[serde(default)]
    pub created_at: i64,
}

// ── Casino types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CasinoBalance {
    pub player_uuid: String,
    pub chips: i64,
    pub streak: i32,
    pub last_claim: Option<i64>,
    pub last_scratch: Option<i64>,
}

#[derive(Debug)]
pub enum CasinoAdjustErr {
    InsufficientFunds(i64),
    NetworkErr,
}

#[derive(Debug)]
pub enum CasinoFaucetResult {
    Awarded { chips_awarded: i64, streak: i32, chips: i64, lotto_pick: String, draw_date: String },
    OnCooldown { next_secs: u64 },
    Err,
}

#[derive(Debug)]
pub enum CasinoScratchResult {
    Ok,
    OnCooldown { #[allow(dead_code)] next_secs: u64 },
    Err,
}

#[derive(Debug, Clone)]
pub struct CasinoJackpotInfo {
    pub pot: i64,
    pub tickets: i32,
    pub next_draw: String,
}

#[derive(Debug, Clone)]
pub struct CasinoLottoTicketInfo {
    pub numbers: String,
    pub draw_date: String,
    pub pot: i64,
    pub chips: i64,
}

#[derive(Debug, Clone)]
pub struct CasinoLottoBulkInfo {
    pub tickets: Vec<String>,
    pub draw_date: String,
    pub pot: i64,
    pub chips: i64,
}

#[derive(Debug, Clone)]
pub struct CasinoLottoPlayerTicket {
    pub numbers: String,
    pub draw_date: String,
}

#[derive(Debug, Clone)]
pub struct CasinoLastLottoDraw {
    pub draw_date: String,
    pub numbers: String,
}

#[derive(Debug, Clone)]
pub struct CasinoLottoPot {
    pub pot: i64,
    pub draw_date: Option<String>,
}

// ── Marriage types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct CasinoWinResult {
    pub chips: i64,
    pub alimony_paid: i64,
    pub ex_count: usize,
    pub net: i64,
}

#[derive(Debug, Clone)]
pub struct MarriageProposal {
    pub proposer_uuid: String,
    pub target_uuid: String,
    pub dowry: i64,
    pub state: String,
    pub role: String,
}

#[derive(Debug, Clone)]
pub struct MarriageSpouseEntry {
    pub spouse_uuid: String,
}

#[derive(Debug, Clone)]
pub struct MarryAcceptResult {
    pub proposer_uuid: String,
    pub target_uuid: String,
    pub dowry_paid: i64,
}

#[derive(Debug, Clone)]
pub enum DivorceResult {
    Divorced,
    Pending { already_waiting: bool },
}

#[derive(Debug, Clone)]
pub struct ForceDivorceResult {
    pub partner_uuid: String,
    pub alimony_days: i64,
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
        "scammer_marked" => serde_json::from_value(value.data)
            .ok()
            .map(WebsocketEvent::ScammerMarked),
        "scammer_unmarked" => serde_json::from_value(value.data)
            .ok()
            .map(WebsocketEvent::ScammerUnmarked),
        "trades_reset" => serde_json::from_value(value.data)
            .ok()
            .map(WebsocketEvent::TradesReset),
        "trades_unreset" => serde_json::from_value(value.data)
            .ok()
            .map(WebsocketEvent::TradesUnreset),
        "fadv_awards" => serde_json::from_value(value.data)
            .ok()
            .map(WebsocketEvent::FadvAwards),
        "pearl_result" => serde_json::from_value(value.data)
            .ok()
            .map(WebsocketEvent::PearlResult),
        "resolve_discord_username_result" => serde_json::from_value(value.data)
            .ok()
            .map(WebsocketEvent::ResolveDiscordUsernameResult),
        "resolve_discord_username_unavailable" => serde_json::from_value(value.data)
            .ok()
            .map(WebsocketEvent::ResolveDiscordUsernameUnavailable),
        "casino_draw_result" => serde_json::from_value(value.data)
            .ok()
            .map(WebsocketEvent::CasinoDrawResult),
        "casino_winner_notify" => serde_json::from_value(value.data)
            .ok()
            .map(WebsocketEvent::CasinoWinnerNotify),
        "report_created" | "trade_confirmed" | "trade_rejected" | "content_flagged" => Some(WebsocketEvent::Ignored),
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

    #[test]
    fn parses_original_hub_discord_bridge_payload() {
        let event = parse_inbound_message(
            r#"{"action":"inbound_discord_chat","data":{"username":"DiscordUser","message":"hello from discord","mc_server":"refinedvanilla"}}"#,
        )
        .unwrap();

        let WebsocketEvent::InboundDiscordChat(message) = event else {
            panic!("expected inbound discord chat event");
        };

        assert_eq!(message.username, "DiscordUser");
        assert_eq!(message.message, "hello from discord");
        assert_eq!(message.mc_server, "refinedvanilla");
        assert_eq!(message.timestamp, "");
        assert_eq!(message.channel_id, "");
        assert_eq!(message.guild_id, "");
        assert_eq!(message.guild_name, "");
    }

    #[test]
    fn parses_wrapper_discord_bridge_payload_with_time_alias() {
        let message: DiscordChatMessage = serde_json::from_value(json!({
            "username": "DiscordUser",
            "message": "hello from discord",
            "time": 1710000000000_i64,
            "mc_server": "refinedvanilla",
            "channel_id": "channel-1",
            "guild_id": "guild-1",
            "guild_name": "Forest"
        }))
        .unwrap();

        assert_eq!(message.timestamp, "1710000000000");
        assert_eq!(message.channel_id, "channel-1");
        assert_eq!(message.guild_id, "guild-1");
        assert_eq!(message.guild_name, "Forest");
    }
}
