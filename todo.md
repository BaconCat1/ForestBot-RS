







# ForestBot Rust Port Remaining TypeScript Parity (``todo.md``)

Only behavior still missing or partial compared to `ForestBot/src` is listed here.

🔎 = Feature/functionality missing that was present in ts forest

🆕 = New feature/functionality

🐛 = Working, needs a bug fix

✅ = Complete

❌ = Rejected


## Commands

* ✅ ~~`drop`: TypeScript drops the held item or every inventory item via Mineflayer `tossStack`; Rust currently only replies that Azalea inventory-drop parity is not wired.~~
* ✅ ~~`mount` / `ride` / `mush`: TypeScript finds the nearest mountable entity or vehicle, applies cooldowns, and mounts it; Rust currently only replies that mounting is not wired.~~ // `nearest_entities_by` + `EntityRef::interact()` implemented; Azalea has a TODO for full riding state tracking so mount success cannot be confirmed, but the interact packet is sent
* ✅ ~~`sleep`: TypeScript finds and activates a bed; Rust currently only replies that sleeping is not wired.~~ // `find_block` + `ServerboundUseItemOn`; toggle via `BOT_SLEEPING` static; `!sleep`/`!crouch`/`!twerk` all send `StopSleeping` when in bed
* ✅ ~~`twerk` / `bootyshake` / `booty` / `dance`: TypeScript toggles sneak for 10 seconds; Rust command is registered but still needs equivalent Azalea control-state behavior verified.~~ // timing matches TS (100ms interval, 10s duration)
* ✅ ~~`realname`: display_name: Option<String> added to PlayerSnapshot, populated from PlayerInfo.display_name (FormattedText→plain string) on AddPlayer/UpdatePlayer; !realname resolves display name → real username~~
* ✅ `febzey`: ~~Rust has equivalent last-seen-style behavior, but it is not byte-for-byte identical to the TypeScript command text.~~
	* *Working as intended afaik*

## Bot Runtime Behavior

* ✅ ~~Port the TypeScript outgoing message filter except the secondary filter, which is intentionally not planned for the Rust port:~~
  * ~~`useCustomChatPrefix` / `customChatPrefix`~~
  * ~~`json/bad_words.json` profanity filter~~
  * ~~`json/word_whitelist.json`~~
  * ~~`smart_censoring` / Together API censor path~~
  * ~~queued outgoing send behavior~~
* ✅ ~~Port `announce`: TypeScript periodically advertises enabled non-whitelisted command descriptions after spawn.~~
  * ✅ ~~`description: &'static str` added to `CommandDefinition`; all commands have descriptions with `{prefix}` placeholder~~
* ✅ ~~Port `antiafk`: TypeScript starts anti-AFK on spawn when enabled.~~ // tokio::spawn loop on Event::Spawn, cancelled on Event::Disconnect via Arc<AtomicBool>
* ❌ ~~Port `usePViewer` / `pViewerPort`.~~ // prismarine-viewer is Mineflayer-only, no Azalea equivalent
* ✅ ~~Port startup ping/retry behavior from TypeScript `Bot.startBot()`, including the 10-failure long backoff.~~ // consecutive_failures AtomicU32 on AzaleaState; reset on Spawn, increment on ConnectionFailed/Disconnect; 10th failure sleeps 10 min — confirmed working in live test
* ✅ ~~Port TypeScript reconnect lifecycle exactly: `endAndRestart()`, `isConnected`, and explicit bot quit/end handling.~~ // `isConnected` has no consumers; `endAndRestart()` covered by Azalea `.reconnect_after()`; `sendPlayerLeave` WS packet sent on `Disconnect` via `send_session_flush_leave`
* ✅ ~~Port TypeScript logger categories and message wording where runtime parity matters.~~ // Rust has all TS categories; TS `move` category has no callers in either codebase

## Chat Parsing And Message Handling

* ❌ ~~`useLegacyChat` / `messagestr.ts`~~ // Azalea always provides structured chat data; legacy raw-string path is not applicable
* ✅ ~~`useCustomChatFormatParser` — custom format parser now gated on config flag; empty formats vec when disabled~~ // gated in Bot::new(), Bot::start(), and reload.rs
* ✅ ~~Verify Rust whisper parsing covers both TypeScript `whisperFrom.ts` and `whisperTo.ts` cases.~~ // `whisperTo.ts` is a no-op stub; Rust `whisper_parser.rs` covers all 12 TS patterns + extra `PM:` variant

## Events

* ✅ ~~`end.ts`: TypeScript marks the bot disconnected, quits, restarts, logs end args, and sends the bot leave websocket packet.~~ // `isConnected` has no consumers (N/A); quit/restart via Azalea `.reconnect_after()`; reason logged via `logger::kick`/`logger::logout`; leave WS packet via `send_session_flush_leave` on `Event::Disconnect`
* ✅ ~~`kicked.ts` / `error.ts`: Both register as `name: 'kicked'` in TS — log kick reason, call bot.end(). Not handled in Rust.~~ // Rust `Event::Disconnect` logs readable reason + restart message; reconnect handled natively by Azalea `.reconnect_after()`
* ✅ ~~`spawn.ts` extras~~ // anti-AFK + announce wired; robot marker handled via `use_custom_chat_prefix` config; `isConnected` has no consumers in Rust — N/A; `restartCount` covered by `consecutive_failures` reset on spawn
* ✅ ~~`physicsTick.ts`: TypeScript writes `tick_end` packets; confirm whether Azalea truly makes this unnecessary, then either document or port.~~ // TS handler is a no-op stub (only commented-out look-at-entity code)

## Moderation

* ✅ ~~Fully port MC whitelist enforcement beyond command/admin gating~~ // `use_whitelist` toggle matches TS parity; `whitelisted_commands` config field was dead code in TS, dropped from Rust
* ❌ ~~Port TypeScript `anti_spam_cooldown` and `anti_spam_msg_limit`~~ // dead code in TS, never wired; per-command cooldowns with incremental penalties cover the abuse case

---

# `todo2.md` (jolly is bad at lists)

## General
* ✅ ~~Movement commands, !sleep, and !drop are unimplemented. !mount~~
	* ✅ ~~!twerk does run but it doesn't really match the ts behavior. The bot does dismount things it's riding so it is crouching, probably too fast to be visible when observed. Maybe replace with !crouch where it just does it once?~~
* ✅ ~~**bug** !setpreset doesn't work in /msg~~
	* ~~*Should be working, needs to be tested in prod to confirm, pending hw migration*~~
* ✅ ~~**bug** !oldest and !newest show incorrect dates. !oldest also shows the oldest users ever, while it should only compare the join dates of who's online.~~
* ✅ ~~**bug**(?) don't record redundant advancements (from the queue or ever)~~ // on `send_player_advancement`, lazy-fetch existing advancements for uuid on first encounter, extract `[bracket-key]` (name-change-proof), skip + whisper if duplicate
* 🐛 ~~Discord chat bridge~~
	* The chat bridge is functional. Previously, you would see your own messages as a server message on discord. This wouldn't matter, except the bridge is pretty iffy and doesn't deliver messages reliably, which is probably the discord side being buggy, it's always been like that. Just showing the bot's messages is good enough unless you really want to deep dive this, which I wouldn't blame you for not wanting to.
	* Bot doesn't show it's own messages still as of commit 6dd5efe58c92b116acebe6d938e0f86af6fcf1bf.
	* *Will have to wait till post hw migration*
* ✅ ~~!servers \<username> (not sure on the name), lists servers forest has data of the player on~~
	* ~~slightly bugged, just needs to break up lists that are too long~~
* ✅ ~~Add cross server functionality to stats commands (this is mostly done, !lk, !ld, !vicitims, !fm, !lm, !ladv, and !top are missing, if this is intentional I can mark this one complete)~~
	*  // !top <stat> all would need hub support.
* ✅ ~~Make faqs backfillable // NEEDS HUB CHANGES~~
* ✅ ~~!delfaq aka !deletefaq, deletes the faq, freeing up the number. Should be done after faqs are backfillable. Should confirm in whisper. // NEEDS HUB CHANGES~~
* ✅ ~~!advancementcount \<advancement>, shows the number of times an advancement has been reached~~
* ✅ ~~!averageping, !ap, shows the average ping of the server as well as best and worst.~~
* ✅ ~~Cooldowns should be cumulative. For example, the initial 10 second cooldown for !q is fine, but if someone quotes again within cooldown * 2 (20 seconds initially) the cooldown should then increase. I'm thinking just 1 extra second (making it 11 seconds until you can run it again, and 22 until the cooldown resets). This punishes over use and repeated use, since even a small cooldown doesn't seem to be enough to dissuade people to chill on the command spam. This concept should also be implemented for !lm, only waaay more aggressive. There should be a 300 second cooldown for last message on an individual user basis with the same "punishment" style increases. People use forest to bypass ignores and this is meant to dissuade that.~~
* ✅ ~~Self censorship~~
* ✅ ~~Whisper that a command is disabled to the player who ran said command~~
* ✅ ~~**bug**: fix discord bug(?) where blacklisted people's messages don't get sent to discord~~ // uuid blacklist check + case-insensitive player lookup before send_minecraft_chat_message in bot.rs
* ✅ ~~**bug**: fix discord bug where it fails to show /playtimegraph for a user without a join date~~ // Hub returns non-array on missing join date; `buildPlaytimeEmbed` now checks `res.ok` + `Array.isArray(graphData)` before processing
* ✅ Nick resolution for nicked players (EssentialsX `/nick`): `nick_cache` (display_name → uuid) populated from PlayerInfo AddPlayer/UpdatePlayer; checked before Mojang API fallback in chat/advancement UUID resolution and all trade commands. Requires server to send PlayerInfo display_name — EssentialsX needs `change-playerlist: true`.
* ✅ ~~pivot from ashcon api to crafty api for username history lookups~~ // `GET https://api.crafty.gg/api/v2/players/{username}` → `data.usernames[].username`; replaced `AshconProfile`/`AshconUsernameHistory` structs with `CraftyPlayerResponse`/`CraftyPlayerData`/`CraftyUsername`
* 🆕 need some kind of alert system in discord for bad behavior that requires manual intervention
* ✅ ~~!askgod if user gives multi word non god arg, should assume it's a question for the oracle and answer "The Gods have heard you, and they send you their divine wisdom:" followed by a random quote~~ // ctx.args.len() >= 2 early-path before god match; random corpus, 200-char cap
* ✅ ~~!status allows / commands to run~~ // target.starts_with('/') guard added; root cause: enqueue_chat trim_start strips leading space convention


## !quote
* ✅ ~~Add support for !q <username> <keyword>~~
* ✅ ~~!q <server>, without username specified, shows random quote from specified server~~
* ✅ ~~Missing the basic 10 second cooldown from pre rewrite. (also needs some extra stuff noted in General)~~

## !faq
* ✅ ~~Should pull a random faq if run without an id number, would match pre rewrite.~~

## new commands
* ✅ ~~!hardware - shows os and hardware info, aliased to !hw~~
* ✅ ~~!alias - lists aliases of a command~~
* ✅ ~~!crouch — single press/release; !crouch hold crouches for up to 10min, whispers instructions, !crouch releases~~
* ✅ ~~!slurcount, checks a user's message history for slurs and presents a total. Should not count messages that were commands (ie, a player checking another's word count for a slur shouldn't be counted against them)~~ // Hub `/wordcount` updated: whole-word REGEXP, `exclude_commands=true` adds `AND message NOT LIKE '!%'`; slur list in `json/slurcount_list.json` (bare array, populate manually); parallel requests per slur, counts summed
* ✅ ~~!health, display bot's health, hunger, armor stats~~ // whispers `Health: X/20 | Hunger: X/20 (sat: X) | Armor: X | Effects: ...`; mount health skipped (azalea `set_passengers` is a no-op stub)
* ✅ ~~!askgod, logically translate TempleOS's talk to god stuff lol~~ // KJV loaded from godtexts/kjv.txt.gz via flate2; timestamp entropy (subsec_nanos >> 4) picks a random verse; displays [Book chapter:verse] text in public chat, truncated to 240 chars; multi-god support pending more corpus files
	* God's messages are based on source texts, you can ask different gods (ie: `!askgod buddha`) by using different source texts!
* ✅ ~~!equip, so the bot can actually wear armor to show for the !health command~~ // open_inventory, scan slots 9-44 for Equippable (Head/Chest/Legs/Feet), PickupClick pick+place into armor slots 5-8

---

# Feature Parity: Craftbot vs Tradebot (``TRADEFEATURES.md``)

Tradebot (Discord) is the reference implementation. This tracks what craftbot (Minecraft) is missing.

## Missing entirely

| Feature | Tradebot | Craftbot |
|---|---|---|
| `!report` command | `/report` — trade or user, reason choices | ✅ done — `!report <player> [reason]`; posts to modlogs |
| Warning display in tradestats | Shows warnings with count, reason, date | ~~out of scope~~ |
| Scammer details | Shows reason + who marked + when | ✅ done — public 🚨 warning on trade initiation; `!trades`/`!tradestats` return "trade counts not reported" |
| `confirmed_at` in trade list | Shows relative timestamp | ✅ struct fixed — `confirmed_at: Option<i64>` added |

## Minor display gaps

* ✅ ~~`!trades` hardcoded to show last 3 — tradebot shows all returned by Hub~~
* ✅ ~~`!trades` truncates description at 30 chars~~ — increased to 190 (256 chat limit minus line overhead)

## Already working (no gap)

* ✅ ~~Hub broadcasts `trade_confirmed`/`trade_rejected` WebSocket events on craftbot-side confirms, tradebot's `listenForMcConfirms` catches them and posts to `#verified-trade` — pipeline intact~~
* ✅ ~~Hub broadcasts `report_created` WS event; tradebot `listenForMcReports` posts MC-origin reports to modlogs with action buttons~~
* ✅ ~~Hub broadcasts `scammer_marked`/`scammer_unmarked` WS events; craftbot announces in public chat~~
* ✅ ~~`!link` → `/link` account linking — both sides implemented~~

* ✅ ~~!unlink~~
* ✅ ~~!top trades~~
* ✅ ~~!top rejects~~
* ✅ ~~!scammers~~
* ✅ ~~/scammers~~
* ✅ ~~## Bug "Showing 3 of 5 warnings. Use /warnings for full list."~~
* ✅ ~~## bug: double posting scam mark announcments~~
	fake bug, evil ghost node instance did this
* ✅ ~~## bug: discord is removing scammer mark, but saying they aren't marked as a scammer.~~
	fake bug, evil ghost node instance did this
* ✅ ~~## bug: /link shouldn't work if the craft user is offline~~
* ✅ ~~## revisit bacon's "out of scope, needs Hub changes"~~
