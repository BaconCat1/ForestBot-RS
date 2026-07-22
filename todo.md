

# ForestBot Rust Port Remaining TypeScript Parity (``todo.md``)

Only behavior still missing or partial compared to `ForestBot/src` is listed here.

🔎 = Feature/functionality missing that was present in ts forest

🆕 = New feature/functionality

🐛 = Working, needs a bug fix

✅ = Complete

❌ = Rejected

⏸️ = On hold


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
* ✅ ~~Discord chat bridge~~
	* Bot's own messages now forwarded to bridge (sender == bot path relays if chatbridge enabled + not a command)
	* Hub WS reconnect: `websocket_close` fires `authenticate()` after 5s in forestapi.ts
	* Discord→MC rapid message drops fixed: removed 10s per-user cooldown from messageCreate.ts
	* Blacklisted users still relay through bridge (blacklist only blocks commands, not relay)
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
* ✅ ~~need some kind of alert system in discord for bad behavior that requires manual intervention~~ // content_flagged WS event pipeline: craftbot checks !addfaq/!editfaq/!iam/!greeting input against bad_words.json (leet-speak normalized, ASCII-only enforced); sends content_flagged WS event → Hub broadcasts → Discord bot posts to sudo channel
* ✅ ~~!askgod if user gives multi word non god arg, should assume it's a question for the oracle and answer "The Gods have heard you, and they send you their divine wisdom:" followed by a random quote~~ // ctx.args.len() >= 2 early-path before god match; random corpus, 200-char cap
* ✅ ~~!status allows / commands to run~~ // target.starts_with('/') guard added; root cause: enqueue_chat trim_start strips leading space convention
* ✅ ~~announce when players are detected, 10 min cooldown per player.~~ // `handle_player_detection` on Tick; entity_by_uuid check = nametag visible; `seen_player_detections` HashSet + 600s async remove; gated by `playerDetected` disabled_events key
* ✅ ~~custom advancements! — ForestBot announces fake MC-style advancement unlocks triggered by tracked events (deaths, kills, etc.)~~ // Hub `fadv_awards` table + threshold checks in `checkFadv.ts`; WS event `fadvAwards` → craftbot announces public + whispers player; `!fadvs [category]` command shows per-category progress; one-time per player
 * ✅ ~~Change all relevant functionality to be toggleable via config.json~~ // all automatic chat-sending behaviors now gated via `disabled_events` keys; all commands toggleable via `commands` map
 * ✅ ~~extend offlinemsg to do "remindme"~~ // `!remindme`/`!remind` aliases; optional duration `1s2m3h4d`; no duration = next login; timed = background 30s tick fires when online; `!remindme stop` cancels all; `deliver_at: Option<u64>` added to `OfflineMessage`
* ✅ ~~add pearl bot infrastructure~~ // pearlbot binary; Hub WS routing; ForestBot-RS `!pearl`/`!p <slot>` command; UUID whitelist + per-slot chamber config; multi-pearl tracking (HashSet); deployed to prod RefinedVanilla
* ✅ ~~queue detection, data driven. upon detection, disconnect for 5 minutes. Needs to count "reconfiguring" screen as reconnecting to server~~ //  working, can't test it against the actual refinedvanilla queue without a genuine server outage. should work
* ✅ ~~extend `!help` to take commands as args~~ // `!help <command>` whispers description + aliases; unknown falls back to link
* ✅ ~~**behavioural tweak**: if API limits are reached, users should be informed that is specifically why it failed, since a more generic error could waste time hunting a bug that doesn't exist~~ // `FetchErr::RateLimit` + `check_resp` in `casino/mod.rs`; all 9 external-API casino files surface 429 with specific message; settle paths treat rate limit as refund (`.ok()`)
* ✅ ~~Universe: need watchdog and alerts, bypass/restart vpn~~ // used healthcheck.io, far simpler than anything else I was imagining lol
* ✅ ~~**refactor**: `src/commands/stat_history.rs` could be split into files for each command it contains. should be refactored and sent as a pr~~
	* ✅ ~~can only be tested on refinedvanilla: `!shout`, `!setpreset`~~
* ✅ ~~survey all commands with cross server data to confirm they can all pull cross server data, verify syntax is unified~~
	* ✅ ~~also need to survey command descriptions to verify they all have usage instructions~~
* ✅ ~~**bug**: [blacklisted people can dodge the blacklist if they have access to a discord bridge](https://discord.com/channels/1427088370676400241/1428539816764506294/1527024487223263374). Idea: when a command is run from a user without a uuid, assume it's a discord user. Parse out their username, and pass it to Discord via Hub to resolve the snowflake id, then check that against the blacklist. If found, command silently doesn't run, in line with existing blacklist behaviour. If Discord is offline, give an error to all non uuid commands.~~
* ✅ ~~**bug**: Forest spammed chat with command announces (the reminders that are supposed to fire every 45 min on how to use a command). Need to investigate why and should gate the "announce" in `config.json`~~ // root-caused + fixed 2026-07-19: `spawn_announce_loop`/`antiafk`/`reminder` were spawned on every `Event::Spawn` (every reconnect), not just the first connection, and never cancelled — confirmed via live log analysis, 159 concurrent announce loops had accumulated over a month, each firing its own randomized 15-45min interval independently. Moved all three inside the existing `mark_background_tasks_started()` first-connection-only guard. Also added `announce_min_interval_ms`/`announce_max_interval_ms` config fields (defaults match the old hardcoded 15-45min range) so the interval itself is now tunable. Not yet compiled/tested.
* ✅ ~~**check**: `drop()` doesn't reliably end a `!Send` value's (MutexGuard, ThreadRng, etc.) liveness for the async `Send` check — has caused 2 compile bugs the same way (`!day`/`!night` fix, board-whisper-delay feature). Codebase needs a scan for other `drop()`-before-`.await` sites that happen to still compile, to block-scope them before an unrelated future edit trips the same error.~~ // scanned 2026-07-19, all 8 real `drop(` sites checked individually, none followed by an `.await` in the same open scope — clean, nothing to fix
* ✅ ~~scan through code base for any additional candidates for data driven gating in `config.json`~~ // 28 hardcoded timing/threshold values moved to config.json (bot.rs core timers, per-game confirm windows/animation delays, market cache TTLs, websocket keepalive, url blocklist timeout).


## !quote
* ✅ ~~Add support for !q <username> <keyword>~~
* ✅ ~~!q <server>, without username specified, shows random quote from specified server~~
* ✅ ~~Missing the basic 10 second cooldown from pre rewrite. (also needs some extra stuff noted in General)~~

## !faq
* ✅ ~~Should pull a random faq if run without an id number, would match pre rewrite.~~

## !top
* ✅ ~~"we need !top slurcount"~~ // `!top slurcount`/`!top slurs`; sums `get_word_occurrence` across all slurs in `slurcount_list.json` per player; cached same as other top stats
* ✅ ~~optimize db calls for efficiency~~ // `top messages`: was N Hub calls → new Hub `GET /top-messages` (single SQL GROUP BY); `top slurcount`: was N×M calls → new Hub `GET /top-slurcount` (single SQL SUM of REGEXP per word); kills/deaths/joins/playtime/trades/rejects already single-call; advancements already uses leaderboard endpoint

## !trade
* ✅ ~~!trade preview, let you see the proposed trade that's preventing you from making a new one, prompt people when they hit that snag~~ // `!trade preview` whispers pending trade details + next steps; propose error now hints about preview
* ✅ ~~**bug**: users can propose trades to themselves, should throw an error instead.~~

## casino
* ✅ casino style games, create ethereal "chips" currency to go along side them 
* ✅ ~~!duel, let's people bet ethereal points then they fight, winner gets the pot. People should be able to place side bets as well, maybe odds can be calculated using k/d stats?~~
* ✅ ~~add wagering to `trivia` command~~ // !answer <guess> <chips>; correct=2×, wrong=jackpot; 45s window; category support + !trivia categories
* ✅ ~~battleship~~
* ✅ ~~chess~~
* ✅ ~~reversi~~
* ❌ ~~uno~~ // hand too complicated to represent over text, fast reactions too difficult to do by text, rejected
* ✅ ~~wordle~~
* ✅ ~~baccarat~~ // Player 2×, Banker 1.95×, Tie 8×; simplified drawing rules from letsgogambling reference; instant resolve, no session state
* ✅ ~~sic bo~~
* ✅ ~~mines~~
* ✅ ~~stock market portfolios and future, mapped out but not written~~
	* ✅ ~~**bug**: doesn't take users as an arg to show other people's portfolio~~ // `market.rs` `portfolio_execute` + `!market portfolio` branch now resolve `ctx.args` before falling back to sender
* ✅ ~~add kalshi (prediction market) to extend stock market system~~ // compiles, pending testing
* ❌ ~~add Betfair (horse race betting) to extend stock market system~~ // geo-locked and low priority, won't pursue horse betting api without demand
* ✅ ~~add SharpAPI (sports betting) to extend stock market system~~
* ✅ ~~weather futures, bet on changes in the weather.~~ // rain yes/no bets; odds from forecast precipitation_probability_max; open-meteo forecast endpoint (past_days=92 for resolution); settle_task pattern matches market bets
* ✅ ~~train betting phase I~~
* ✅ ~~train betting phase II (https://mobilitydatabase.org/)~~
* ✅ ~~earthquake betting (earthquake-volcano-gambling-feature.md)~~
* ✅ ~~volcano betting (earthquake-volcano-gambling-feature.md)~~
* ✅ ~~Air Quality Index betting (EPA AirNow, free)~~
* ✅ ~~Rocket launches (RocketLaunch.live, free)~~
* ✅ ~~gasbuddy betting, also just treat national gas price as a stock to let people invest/buy "real gasoline" (https://github.com/firstof9/py-gasbuddy)~~
	* ⏸️ extend gasbuddy feature to support diesel // casino phase II

### Casino Phase II
* ✅️ ~~migrate to a betting api over the current approach (requires Hub changes, deslopify-2026-07-07.md line 111)~~
* ✅ ~~board building whisper delay should be data driven in config.json~~ // done 2026-07-19: `board_whisper_delay_ms` config field + `CommandContext::whisper_board()`, applied to battleship/checkers/chess/connect_four/mines/reversi/wordle. Not yet compiled/tested.
* ⏸️ eSports betting (OddsPapi — oddspapi.io (or panda api... wtv), free key, 250 req/month) — CS2, LoL, Dota 2, Valorant, CoD, Rocket League, Overwatch, R6, StarCraft; match winner moneyline; reuses existing odds-market UI // casino phase II
* ⏸️ Hurricane betting (NHC — nhc.noaa.gov, no auth, flat file/GeoJSON) — "will storm X make landfall in region Y?"; seasonal; no REST API, flat file parse required // casino phase II
* ⏸️ River/streamflow betting (NOAA NWPS — water.noaa.gov, no auth) — HEFS ensemble forecast, "will river X exceed flood stage?"; distinct from existing alert-based noaa_flooding.rs // casino phase II
* ⏸️ Tide betting (NOAA CO-OPS — tidesandcurrents.noaa.gov, no auth) — over/under tide height at a station; deterministic, low-variance market // casino phase II
* ⏸️ Ocean buoy betting (NOAA NDBC or Open-Meteo marine endpoint) — wave height over/under; Open-Meteo marine = zero new integration cost // casino phase II
* ⏸️ Wildfire betting (NIFC — nifc.gov, no auth) — "will fire X grow today?" based on perimeter data // casino phase II
* ⏸️ Migratory bird betting (eBird — free key) — "first sighting of species X in region Y by date Z?"; citizen-science, settleable against reported sightings // casino phase II
* ⏸️ Pollen betting (Ambee Pollen API — free key) — daily pollen severity over/under; seasonal variance // casino phase II
* ⏸️ Public health forecast betting (CDC FluSight + RSV + COVID Forecast Hub, free, no key) — weekly regional ensemble; same odds derivation as Open-Meteo; no Kalshi overlap // casino phase II
* ⏸️ Boat/vessel arrival betting (Norway Coastal Administration AIS, free) — arrival-time over/under for tracked vessels; same shape as train delay bets // casino phase II
* 🔧 server event futures, same idea, just about stuff that happens on the server // join-window market built 2026-07-21 (`!joins <player> <bet>`), odds SQL not yet verified against real data; death-timing market scoped, not built; death cause-distribution dropped from scope
* ⏸️ parlays across all betting types (needs mapping) // on hold for casino phase II
* ⏸️ side betting on any game, not just dueling // on hold for casino phase II
* ⏸️ add multiplayer where applicable to "casino games" // on hold for casino phase II
* ✅ ~~!marry, as well as !divorce and !spouse, let's you marry a player, check their spouse. Append marital status to whois, alimony system based on winning casino games?~~ // built for real 2026-07-18, compiled + live-tested + pushed (Hub schema + endpoints, marry.rs, universal alimony hook across every casino/market/weather win-credit site). The old "betting api" blocker is resolved by the `casino_win`/`SettleDeps` payout choke point built the same session — alimony now applies automatically to any game, no per-game wiring needed.
	* ✅ ~~**extension**: should take users as args to show other people's spouses~~ // `!spouse <player>` optional arg, defaults to sender; error on unknown player

* ✅ ~~**bug**: `increment_ms` cooldown not working~~ // spam path (blocked attempt) now also increments cooldown; success path already had it
* ✅ ~~**bug**: !ud upvotes/downvotes always 0~~ // improved type parsing (i64 fallback); votes hidden when both 0 rather than showing (+0/-0)
* ✅ ~~portfolio shows all bets, not just stocks~~ // !bets command lists all open event bets; !wallet + !portfolio show event bet count inline

## new commands
* ✅ ~~!hardware - shows os and hardware info, aliased to !hw~~
* ✅ ~~!alias - lists aliases of a command~~
* ✅ ~~!crouch — single press/release; !crouch hold crouches for up to 10min, whispers instructions, !crouch releases~~
* ✅ ~~!slurcount, checks a user's message history for slurs and presents a total. Should not count messages that were commands (ie, a player checking another's word count for a slur shouldn't be counted against them)~~ // Hub `/wordcount` updated: whole-word REGEXP, `exclude_commands=true` adds `AND message NOT LIKE '!%'`; slur list in `json/slurcount_list.json` (bare array, populate manually); parallel requests per slur, counts summed
* ✅ ~~!health, display bot's health, hunger, armor stats~~ // whispers `Health: X/20 | Hunger: X/20 (sat: X) | Armor: X | Effects: ...`; mount health skipped (azalea `set_passengers` is a no-op stub)
* ✅ ~~!askgod, logically translate TempleOS's talk to god stuff lol~~ // KJV loaded from godtexts/kjv.txt.gz via flate2; timestamp entropy (subsec_nanos >> 4) picks a random verse; displays [Book chapter:verse] text in public chat, truncated to 240 chars; multi-god support pending more corpus files
	* God's messages are based on source texts, you can ask different gods (ie: `!askgod buddha`) by using different source texts!
* ✅ ~~!equip, so the bot can actually wear armor to show for the !health command~~ // open_inventory, scan slots 9-44 for Equippable (Head/Chest/Legs/Feet), PickupClick pick+place into armor slots 5-8
* ✅ ~~!wiki let's you search wikipedia based on <arg>.~~ // MediaWiki search API → extract API (2-step); posts `[Title] first line...` truncated 200 chars to public chat; 1-min cooldown per player; aliases `!wiki`/`!wikipedia`
* ✅ ~~!news, let's you find headlines from rss feeds.~~ // BBC RSS via `rss` crate; `!news` whispers categories + top stories; `!news <cat>` whispers 5 numbered headlines; `!news [cat] <N>` posts article description + link (tracking params stripped) to public chat
* ✅ ~~!day/!night, reports irl time until it's either day or night in game~~ // listens for `ClientboundSetTime`, stores `total_ticks` in AzaleaState; day=23460–13188, night=13188–23460; converts ticks to m/s real time
	* ✅ ~~**bug**: seems to drift out of sync over time. needs investigation~~
* ✅ ~~!urbandictionary, api seems to be at https://api.urbandictionary.com/v0/define?term={TERM}~~ // `list[0]` → strips `[bracket]` links, collapses newlines, truncates 180 chars, appends `(+N/-N)`; public chat; aliases `!urbandictionary`/`!ud`
* ✅ ~~!greeting, users can give themselves a welcome back message that has a 12 hour cooldown~~ // `greeting` + `greeting_last_fired_at` columns on `users` table; fires on join as `"<message>, Username!"`; 12h cooldown via DB timestamp; preview/clear subcommands
* ✅ ~~!minewiki, same behaviour as !wiki, only for the minecraft wiki~~ // same 2-step flow against minecraft.wiki (`/api.php`); public chat; 1-min cooldown per player; aliases `!minewiki`/`!mcwiki`
* ❌ ~~!weather — predict next weather change using Java LCG seed calibration~~ // not feasible: Azalea does not expose server-internal game time; the tick value available via `SetTime` is client-side and drifts from the server's `ServerLevel.random` draw counter, making LCG calibration impossible
* ✅ ~~!calc, alias !wolframalpha, !wa, sends requests to the wolframalpha public api~~ // LLM API endpoint; `wolfram_app_id` in bot config; parses all labeled sections with priority order (Result→Solution→Derivative→Definite integral→Indefinite integral→Infinite sum→Sum→Limit→Decimal approximation→Property→…), posts `query = answer` truncated to 220 chars; aliases `!calc`/`!wa`/`!wolframalpha`
* ✅ ~~!translate, add support for azure api for translation~~ // Azure AI Translator; `azure_translator_key` + `azure_translator_region` in config; lang optional (default `en`); single-word input checks online players → translates last message; FROM-English blocked (whatlang local detection, 4+ words); aliases `!translate`/`!tr`/`!tl`
* ✅ ~~!trivia / !answer — server trivia round via Open Trivia DB (no key); boolean and MCQ; 15s answer window open to all players; whispers "Answer received!" on submit; public summary at close shows ✓/✗ lists + answer; latecomers whispered answer for 60s after close~~
* ✅ ~~!roast, leverage together api to roast a player, takes user name as arg~~
* ✅ ~~!ai, leverage free tier llm providers to respond to querys from chat. idea is to use highest quality to lowest quality, as usage gets consumed. known "truly free" providers: gemini, groq, cerebras, mistral, openrouter, cloudflare workers ai.~~
* ✅ ~~!afk, let you set a response if people say your name at the beginning of a message or whisper to you, resets if you talk in chat or disconnect.~~
	* ✅ ~~**bug**: set and instantly cleared without any input on refinedvanilla, needs investigation~~ // root cause: server echoes bot's own outgoing `/msg` whispers back into chat; generic `"{username}: {message}"` custom format matched the echo as the target speaking. `enqueue_chat` now records recent outgoing whispers, chat handler suppresses matching echoes before AFK/command logic runs. Confirmed working on prod.
* ✅ ~~!poll, popular enough in other bots to warrant inclusion, might end up disabled like `fadvs`. Needs high cooldown, 5 min minimum~~
	* ✅ ~~**bug**: needs to treat answering polls as different from creating them~~ // fix was in `command_handler.rs`, not `poll.rs` — cooldown gate runs pre-dispatch, now peeks args and skips the gate for single-numeric-arg (vote) invocations
* ✅ ~~!tps, if azalea/minecraft or wtv lets you see server performance, report it via a command~~
* ✅ ~~!url, don't webpages have some seo text built in by default? if so, leverage that for a text only preview of a url, so you can see what it is without having to leave the game.~~ // ~~working, needs some fall back and further testing~~
* ⏸️ !poke, add a "poke ai" personality users can talk to (requested by Bacon) // ON HOLD, infra required to support feels like it overly complicates ForestBot, unfinished features stored in `poke-unfinished` branch

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
