# ForestBot Rust Port Remaining Work

This list only tracks work still needed after the Rust/Azalea port work already completed.

Legend:
- `[ ]` Not done.
- `[~]` Registered or partially implemented, but not functionally identical to Node yet.

---

## Runtime And Config

- [ ] Expand `reload` / `reloadconfig` so they refresh every live runtime field from `config.json`, not only command-facing config.
- [ ] Decide whether outbound websocket messages should queue while disconnected or keep failing fast.

---

## Chat Parsing

- [ ] Port legacy Node `messagestr.ts` behavior if `useLegacyChat` is enabled.
- [ ] Fully honor `useCustomChatFormatParser` instead of always trying custom/fallback parsing.
- [ ] Port outgoing custom chat prefix behavior.
- [~] Collapse duplicated parser helpers into the standalone utility modules: `chat_divider_parser`, `parse_username`, `chat_sender_resolver`, and `strip_minecraft_formatting`.

---

## Events

- [ ] Port Node `end.ts` bot-leave websocket behavior.
- [ ] Port Node `error.ts`.
- [ ] Port Node `kicked.ts`.
- [ ] Port Node `physicsTick.ts`. - This not needed because azalea doesnt have the issues of mineflayer
- [ ] Port Node `whisperFrom.ts` / `whisperTo.ts` if the direct Azalea chat/whisper path still misses any Node cases.
- [ ] Remove or finish inactive stubs under `src/events/mineflayer`.

---

## Moderation

- [ ] Fully port MC whitelist enforcement beyond command/admin gating.
- [ ] Implement main profanity filtering using `json/bad_words.json`.
- [ ] Implement secondary profanity filtering.
- [ ] Wire `json/word_whitelist.json` into the actual profanity filter, not only the list edit command.
- [ ] Port smart AI censoring.
- [ ] Port anti-spam cooldown and message-limit behavior.

---

## Commands Still Partial

- [~] `drop`: registered, but Mineflayer `tossStack` parity still needs Azalea inventory packet work.
- [~] `mount`, `ride`, `mush`: registered, but entity mounting/riding still needs Azalea interaction work.
- [~] `sleep`: registered, but bed finding/activation still needs Azalea block interaction work.
- [~] `realname`: registered, but exact nickname/display-name matching needs display-name data in the Rust player cache.
- [~] `febzey`: registered with equivalent last-seen behavior, but not byte-for-byte identical to the obfuscated Node text.
- [ ] Update `help` / `commands` so command output is generated dynamically from the Rust registry.
- [ ] Audit original `ForestBot/src/commands` once more for any command files or aliases not represented in `commands::registry()`.

---

## Bot Behavior Utilities

- [ ] Port anti-AFK behavior.
- [ ] Port pViewer.
- [ ] Mirror Node logger categories for chat, advancement, death, join, leave, kick, login, logout, move, spawn, world, command, and websocket.
- [ ] Port custom chat suffix / robot marker behavior from the Node spawn hook.
- [~] Finish time utilities where output still differs from Node formatting.

---

## Tests

- [ ] Add command handler tests for whitelist/blacklist gating and command enable toggles.
- [ ] Add tests for the newly ported command behavior where mocking is practical.
- [ ] Add config loading tests for Node-compatible aliases and JSON list files.
- [ ] Add API client normalization tests for original wrapper/database response shapes.
- [ ] Add a local Minecraft/websocket harness for command-to-hub payload regression testing.

---

## Runtime Proof

- [ ] Live-test websocket compatibility against the original ForestBot hub.
- [ ] Live-test REST command behavior against the original ForestBot database/API.
- [ ] Runtime-test against a real Minecraft server and save proof logs.
