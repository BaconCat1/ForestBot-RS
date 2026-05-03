# ForestBot Rust Port Checklist

This checklist tracks the remaining work to port the original Node `ForestBot` and `forestbot-wrapper-v2-master` into the Rust/Azalea version.

Legend:
- `[x]` Complete and wired into the Rust runtime.
- `[~]` Partially complete, scaffolded, or compile-present but not fully finished.
- `[ ]` Not started or not yet ported.

---

## Current Baseline

### Complete

- [x] Mirror the original Node project layout at a high level in the Rust project structure.
- [x] Load `config.json` using the Rust `Config` schema.
- [x] Load `.env` credentials through `dotenvy`.
- [x] Require `MC_USER`, `MC_PASS`, and `API_KEY` or `apiKey` at startup.
- [x] Pin the nightly Rust toolchain with `rust-toolchain.toml`.
- [x] Wire Azalea client startup.
- [x] Wire ViaVersion support through `azalea-viaversion`.
- [x] Add `disableChatSigning` config support.
- [x] Add basic reconnect delay through Azalea `reconnect_after`.
- [x] Add runtime reload for command-related runtime config.

### In Progress

- [~] Expand runtime reload so it refreshes every `Config` field used by the original Node bot lifecycle.

---

## API Wrapper And Database Compatibility

### Complete

- [x] Port ForestBot API base URL handling.
- [x] Send the API key as `x-api-key`.
- [x] Send the API key as `Authorization: Bearer ...`.
- [x] Wrap `GET /user` as `get_stats_by_uuid`.
- [x] Wrap `GET /playername` as `get_stats_by_username`.
- [x] Wrap `GET /convert-username-to-uuid` as `convert_username_to_uuid`.
- [x] Wrap `GET /deaths`.
- [x] Wrap `GET /kills`.
- [x] Wrap `GET /messages`.
- [x] Wrap `GET /advancements`.
- [x] Wrap `GET /advancements-count`.
- [x] Wrap `GET /messagecount`.
- [x] Wrap `GET /wordcount`.
- [x] Wrap `GET /namesearch`.
- [x] Wrap `GET /online`.
- [x] Wrap `GET /whois`.
- [x] Wrap `GET /users-sorted-by-joindate`.
- [x] Wrap `GET /unique-users`.
- [x] Wrap `GET /quote`.
- [x] Wrap `GET /top-statistic`.
- [x] Wrap `GET /player-activity-by-hour`.
- [x] Wrap `GET /player-activity-by-week-day`.
- [x] Wrap `GET /faq`.
- [x] Wrap `POST /whois-description`.
- [x] Wrap `POST /post-faq`.
- [x] Wrap `POST /edit-faq`.

### In Progress

- [~] Connect the wrapped API endpoints to the Rust command system.
- [~] Runtime-test REST response normalization against every original hub/database response shape.

---

## WebSocket Wrapper Compatibility

### Complete

- [x] Connect WebSocket to `${websocket_url}/websocket/connect`.
- [x] Send `x-api-key`, `client-type`, and `mc_server` headers.
- [x] Use `client-type: minecraft` for bot clients.
- [x] Send keepalive ping every 5 seconds.
- [x] Use `pingdata` as the ping payload.
- [x] Parse and log incoming `key-accepted` messages.
- [x] Parse incoming `new_user` messages.
- [x] Parse incoming `new_name` messages.
- [x] Parse incoming `inbound_discord_chat` messages.
- [x] Parse incoming `inbound_minecraft_chat` messages.
- [x] Parse incoming player death, kill, join, leave, and advancement messages.
- [x] Log unknown websocket messages.
- [x] Reconnect WebSocket after close/error.
- [x] Use 5-second reconnect delay initially and 60-second delay after repeated failures.
- [x] Send outbound Minecraft chat using action `inbound_minecraft_chat`.
- [x] Add outbound Discord chat using action `inbound_discord_chat`.
- [x] Add outbound player list updates using action `send_update_player_list`.
- [x] Add outbound advancements using action `minecraft_advancement`.
- [x] Add outbound player joins using action `minecraft_player_join`.
- [x] Add outbound player leaves using action `minecraft_player_leave`.
- [x] Add outbound player deaths using action `minecraft_player_death`.

### In Progress

- [~] Decide whether outbound websocket messages should queue while disconnected or continue failing fast.
- [~] Live-test WebSocket compatibility against a running hub.

---

## Chat Parsing And Logging

### Complete

- [x] Wire basic Azalea chat parsing through `ChatPacket::split_sender_and_content`.
- [x] Extract vanilla-style sender/content when Azalea provides sender metadata.
- [x] Parse custom chat formats from `customChatFormats`.
- [x] Support `{username}` and `{message}` placeholders in custom chat formats.
- [x] Support `{skip}` placeholders in custom chat formats.
- [x] Strip Minecraft formatting codes in the custom parser path.
- [x] Filter join/leave server presence messages out of regular chat logging.
- [x] Send non-command player chat to the websocket/database as `MinecraftChatMessage`.
- [x] Prevent command messages from being logged as normal chat records.

### In Progress

- [~] Integrate arrow/divider parsing as a standalone equivalent to Node `chatDividerParser.ts`.
- [~] Complete `parseUsername`.
- [~] Complete `chatSenderResolver`.
- [~] Reuse `stripMinecraftFormatting` consistently outside the custom parser path.

### To Do

- [x] Port the generic Node `message.ts` behavior for whispers, fallback chat formats, advancements, deaths, and PVP murderer detection into the active Azalea chat path.
- [ ] Port the legacy Node `messagestr.ts` parser.
- [x] Implement whisper parsing in `whisper_parser.rs`.
- [x] Wire whisper command handling for commands addressed to the bot.
- [ ] Port outgoing custom chat prefix behavior.
- [ ] Port `useLegacyChat` behavior.
- [ ] Fully honor `useCustomChatFormatParser` gating.

---

## Minecraft Events

### Complete

- [x] Handle Azalea `Init`.
- [x] Handle Azalea `Login`.
- [x] Handle Azalea `Spawn`.
- [x] Handle Azalea `Chat`.
- [x] Update the Rust player cache on Azalea `AddPlayer`.
- [x] Update the Rust player cache on Azalea `UpdatePlayer`.
- [x] Update the Rust player cache on Azalea `RemovePlayer`.
- [x] Log Azalea `Disconnect`.
- [x] Log Azalea `ConnectionFailed`.
- [x] Send `minecraft_player_join` through websocket on player join.
- [x] Send `minecraft_player_leave` through websocket on player leave.
- [x] Send player list updates on spawn.
- [x] Send player list updates on join/leave.
- [x] Send player list updates every 60 seconds.
- [x] Send welcome messages from websocket `new_user` events when `welcome_messages` is enabled.
- [x] Send welcome messages from websocket `new_name` events when `welcome_messages` is enabled.

### In Progress

- [~] Port the Node `entitySpawn` first-sight welcome behavior.
- [~] Replace or remove inactive Rust stub modules under `src/events/mineflayer`.

### To Do

- [ ] Port Node `end.ts` bot-leave websocket behavior.
- [ ] Port Node `entitySpawn.ts`.
- [ ] Port Node `error.ts`.
- [ ] Port Node `kicked.ts`.
- [ ] Port Node `physicsTick.ts`.
- [ ] Port Node `whisperFrom.ts`.
- [ ] Port Node `whisperTo.ts`.
- [x] Detect advancements from chat/system messages.
- [x] Detect deaths from chat/system messages.
- [x] Detect kills and PVP murderers.
- [x] Deliver offline messages when a player joins.
- [ ] Implement disabled-event config behavior.

---

## Chat Bridge

### Complete

- [x] Send inbound Discord chat to Minecraft as `[Discord] username: message` when `allow_chatbridge_input` is enabled and `mc_server` matches.
- [x] Send inbound Minecraft shout relay messages to Minecraft when `allow_chatbridge_input` is enabled.
- [x] Ignore shout relay messages from the current origin server.
- [x] Support shout relay target server `all`.

### In Progress

- [~] Port the `!shout` command.
- [~] Runtime-test full bidirectional bridge behavior against the live hub.

---

## Moderation And Player Standing

- [x] Store whitelist, blacklist, command toggles, and whitelisted command names in runtime config.
- [x] Check `command_toggles` in the command handler.
- [x] Check `whitelisted_commands` in the command handler.
- [x] Block blacklisted players from normal commands.
- [x] Preserve the self-standing exception for blacklisted players.
- [x] Port standing/status behavior.

### To Do

- [ ] Fully port MC whitelist enforcement.
- [ ] Port Node blacklist commands.
- [ ] Port Node whitelist commands.
- [ ] Implement the main profanity filter.
- [ ] Implement the secondary profanity filter.
- [ ] Port smart AI censoring.
- [ ] Port anti-spam cooldown and message-limit behavior.
- [ ] Port word whitelist behavior.
- [ ] Port self-standing command exception behavior.

---

## Bot Behavior Utilities

### In Progress

- [~] Expand the logger beyond basic `info`, `success`, `warn`, and `error` output.
- [~] Finish time utilities for timestamp formatting and time-ago behavior.

### To Do

- [ ] Mirror Node logger categories for chat, advancement, death, join, leave, kick, login, logout, move, spawn, world, command, and websocket.
- [ ] Port anti-AFK behavior.
- [ ] Port pViewer.
- [ ] Port mount/riding behavior.
- [ ] Port sleep behavior.
- [ ] Port drop behavior.
- [ ] Port twerk/dance behavior.
- [ ] Port raw execute command behavior.
- [ ] Port custom chat suffix/robot marker behavior from the Node spawn hook.

---

## Commands

### Complete

- [x] Port and register `ping`.
- [x] Port and register `help`, `commands`.
- [x] Port and register `discord`.
- [x] Port and register `reload`, `reloadconfig`.
- [x] Port and register `lastseen`, `seen`, `ls`.
- [x] Port and register `msgcount`, `messages`.
- [x] Port and register `playtime`, `pt`.
- [x] Port and register `joins`.
- [x] Port and register `quote`, `q`.
- [x] Port and register `jdpt`, `ptjd`, `joindateplaytime`, `playtimejoindate`.
- [x] Port and register `joindate`, `jd`, `firstseen`.
- [x] Port and register `kd`, `kills`, `deaths`.
- [x] Port and register `lastkill`, `lk`.
- [x] Port and register `lastadvancement`, `ladv`.
- [x] Port and register `lastdeath`, `ld`.
- [x] Port and register `lastmessage`, `lm`.
- [x] Port and register `lq`, `listquoteservers`.
- [x] Port and register `search`, `lookup`, `find`.
- [x] Port and register `noobs`, `noob`, `newest`, `newusers`, `newbs`, `newb`.
- [x] Port and register `offlinemsg`.
- [x] Port and register `oldest`, `oldheads`, `oldusers`, `oldestusers`, `oldfags`.
- [x] Port and register `rq`, `randomquote`.
- [x] Port and register `standing`, `status`.
- [x] Port and register `summary`, `sum`.
- [x] Port and register `top`.
- [x] Port and register `advancements`, `totaladvancements`, `advs`, `adv`.
- [x] Port and register `users`, `uniqueusers`.
- [x] Port and register `whois`.
- [x] Port and register `winrate`, `wr`.
- [x] Port and register `wordcount`, `words`, `count`.

### In Progress

- [~] Update `help` and `commands` so the command list is generated dynamically instead of using static text.
- [~] Finish quote-related command coverage beyond normal lookup and `all` search.
- [~] Expand `reload` and `reloadconfig` so they rebuild the whole bot/API lifecycle when needed.

### To Do

- [ ] Port `active`.
- [ ] Port `addfaq`.
- [ ] Port `blacklist`.
- [ ] Port `bp`, `bestping`.
- [ ] Port `censor`.
- [ ] Port `coords`.
- [ ] Port `drop`.
- [ ] Port `editfaq`.
- [ ] Port `efficiency`, `eff`.
- [ ] Port `execute`, `exec`, `run`.
- [ ] Port `febzey`.
- [ ] Port `firstdeath`, `fd`.
- [ ] Port `firstkill`, `fk`.
- [ ] Port `firstmessage`, `fm`.
- [ ] Port `faq`, `getfaq`.
- [ ] Port `grudge`.
- [ ] Port `iam`.
- [ ] Port `mount`, `ride`, `mush`.
- [ ] Port `nickname`.
- [ ] Port `oldnames`, `dox`, `doxx`.
- [ ] Port `ownsfaq`, `ownfaq`, `faqowner`.
- [ ] Port `profile`.
- [ ] Port `rqa`, `randomquoteall`.
- [ ] Port `realname`.
- [ ] Port `setpreset`.
- [ ] Port `shout`.
- [ ] Port `sleep`.
- [ ] Port `survived`.
- [ ] Port `twerk`, `bootyshake`, `booty`, `dance`.
- [ ] Port `victims`, `murders`, `bested`.
- [ ] Port `vs`.
- [ ] Port `whitelist`.
- [ ] Port `wordwhitelist`, `wwl`.
- [ ] Port `wp`, `worstping`.

---

## Data Files

### Complete

- [x] Load `json/colors.json`.
- [x] Load `json/mc_whitelist.json`.
- [x] Load `json/mc_blacklist.json`.

### In Progress

- [~] Fully use `json/patterns.json` in Rust moderation/chat parsing.
- [~] Connect `json/bad_words.json` to a real Rust profanity filter.
- [~] Connect `json/word_whitelist.json` to a real Rust word-whitelist filter.
- [x] Connect `json/offline_messages.json` to Rust offline messaging.

---

## Tests And Verification

### Complete

- [x] Add unit tests for the custom-format chat parser.
- [x] Add unit tests for whisper parsing.
- [x] Add a non-network websocket protocol harness for exact hub headers and payload field names.
- [x] Verify the current Rust code with `cargo check`.
- [x] Verify the current Rust code with `cargo test`.

### In Progress

- [~] Live-verify WebSocket compatibility against the original hub/database.

### To Do

- [ ] Add command handler tests.
- [ ] Add config loading tests.
- [ ] Add API client tests.
- [x] Add WebSocket protocol tests.
- [ ] Add runtime integration tests against a real Minecraft server.

---

## Highest-Impact Remaining Work

- [x] Port full Node `message.ts` parsing for advancements, deaths, kills, whispers, and fallback chat formats.
- [x] Port all stats/history commands that already have Rust API wrappers available.
- [x] Port moderation and standing behavior before exposing administrative commands.
- [x] Port offline message storage and delivery.
- [x] Add a websocket integration test or local harness that verifies exact hub actions and payload field names.
- [ ] Runtime-test against the original ForestBot hub/database and save proof logs.
