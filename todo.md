



# ForestBot Rust Port Remaining TypeScript Parity (``todo.md``)

Only behavior still missing or partial compared to `ForestBot/src` is listed here.

🔎 = Feature/functionality missing that was present in ts forest

🆕 = New feature/functionality

🐛 = Working, needs a bug fix

✅ = Complete

❌ = Rejected


## Commands

* ✅ ~~`drop`: TypeScript drops the held item or every inventory item via Mineflayer `tossStack`; Rust currently only replies that Azalea inventory-drop parity is not wired.~~
* 🔎 `mount` / `ride` / `mush`: TypeScript finds the nearest mountable entity or vehicle, applies cooldowns, and mounts it; Rust currently only replies that mounting is not wired.
  * 🆕 Add `logger::move_log` category to `logger.rs` once movement commands are implemented
* 🔎 `sleep`: TypeScript finds and activates a bed; Rust currently only replies that sleeping is not wired.
* ✅ ~~`twerk` / `bootyshake` / `booty` / `dance`: TypeScript toggles sneak for 10 seconds; Rust command is registered but still needs equivalent Azalea control-state behavior verified.~~ // timing matches TS (100ms interval, 10s duration)
* 🔎 `realname`: TypeScript resolves visible display/nickname data from Mineflayer player state; Rust needs equivalent display-name data in the player cache for exact parity.
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
* 🔎 Port TypeScript reconnect lifecycle exactly: `endAndRestart()`, `isConnected`, and explicit bot quit/end handling.
* 🔎 Port TypeScript logger categories and message wording where runtime parity matters.

## Chat Parsing And Message Handling

* ❌ ~~`useLegacyChat` / `messagestr.ts`~~ // Azalea always provides structured chat data; legacy raw-string path is not applicable
* ✅ ~~`useCustomChatFormatParser` — custom format parser now gated on config flag; empty formats vec when disabled~~ // gated in Bot::new(), Bot::start(), and reload.rs
* 🔎 Verify Rust whisper parsing covers both TypeScript `whisperFrom.ts` and `whisperTo.ts` cases.

## Events

* 🔎 `end.ts`: TypeScript marks the bot disconnected, quits, restarts, logs end args, and sends the bot leave websocket packet.
* ✅ ~~`kicked.ts` / `error.ts`: Both register as `name: 'kicked'` in TS — log kick reason, call bot.end(). Not handled in Rust.~~ // Rust `Event::Disconnect` logs readable reason + restart message; reconnect handled natively by Azalea `.reconnect_after()`
* 🔎 `spawn.ts`: Rust sends player-list updates and starts websocket listeners, but still lacks TypeScript spawn extras: outgoing robot marker hook, and `restartCount` / `isConnected` state updates. // anti-AFK and announce loop now wired
* ✅ ~~`physicsTick.ts`: TypeScript writes `tick_end` packets; confirm whether Azalea truly makes this unnecessary, then either document or port.~~ // TS handler is a no-op stub (only commented-out look-at-entity code)

## Moderation

* 🔎 Fully port MC whitelist enforcement beyond command/admin gating
* 🔎 Port TypeScript `anti_spam_cooldown` and `anti_spam_msg_limit`; Rust currently only has command cooldown handling.

---

# `todo2.md` (jolly is bad at lists)

## General
* 🔎 Movement commands, !mount, !sleep, and !drop are unimplemented.
	* ✅ ~~!twerk does run but it doesn't really match the ts behavior. The bot does dismount things it's riding so it is crouching, probably too fast to be visible when observed. Maybe replace with !crouch where it just does it once?~~
* 🐛 ~~**bug** !setpreset doesn't work in /msg~~
	* *Should be working, needs to be tested in prod to confirm, pending hw migration*
* ✅ ~~**bug** !oldest and !newest show incorrect dates. !oldest also shows the oldest users ever, while it should only compare the join dates of who's online.~~
* 🐛 **bug**(?) don't record redundant advancements (from the queue or ever) // THIS IS HARD
	* If the bot can detect the queue and both record no data and also take no commands, I think that would be good enough. //Detecting this is the issue and why i said this is hard
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
* ✅ ~~**bug**: fix discord bug(?) where blacklisted people's messages don't get sent to discord~~
* ❌ ~~**bug**: fix discord bug where it fails to show /playtimegraph for a user without a join date~~ // OUT OF SCOPE
* 🆕 pivot from ashcon api to crafty api for username history lookups


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

* ✅ Hub broadcasts `trade_confirmed`/`trade_rejected` WebSocket events on craftbot-side confirms, tradebot's `listenForMcConfirms` catches them and posts to `#verified-trade` — pipeline intact
* ✅ Hub broadcasts `report_created` WS event; tradebot `listenForMcReports` posts MC-origin reports to modlogs with action buttons
* ✅ Hub broadcasts `scammer_marked`/`scammer_unmarked` WS events; craftbot announces in public chat
* ✅ `!link` → `/link` account linking — both sides implemented

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
