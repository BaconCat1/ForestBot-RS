# Sources for `casino/`

Game logic references and adaptations used in the ForestBot-RS casino module.

## Included

| Module | Reference | Copyright |
|--------|-----------|-----------|
| `baccarat.rs` | _Let's Go Gambling!_ — `BaccaratScreenHandler.java` by BobR0ssiter. Card scoring (`bac_value` / `bac_score`), natural detection, simplified drawing rules (both sides draw on ≤5), and payout ratios (Player 2×, Banker 1.95×, Tie 8×) adapted from source. No session state (instant resolve); chip integration, command flow original. | MIT — © BobR0ssiter. Modrinth: https://modrinth.com/mod/QCz7p8r1 |
| `slots.rs` | _slot-machine-gen_ by Marc S. Brooks. Weighted symbol selection algorithm (`selectRandSymbol` iterate-subtract pattern) adapted from source. Symbol set, weights, and payout table are original design from `CASINOMAP.md`. | MIT — © 2020-2025 Marc S. Brooks |
| `roulette.rs` | _Let's Go Gambling!_ — `RouletteScreenHandler.java` by BobR0ssiter. European wheel (0–36) game model and red-number set adapted from source. Instant single-command port (no GUI). | MIT — © BobR0ssiter. Modrinth: https://modrinth.com/mod/QCz7p8r1 |
| `craps.rs` | _Let's Go Gambling!_ — `CrapsScreenHandler.java` by BobR0ssiter. Pass/don't-pass two-phase dice model and come-out/point-phase logic adapted from source. Field, Any-7, hardway bets not ported (chat UX). | MIT — © BobR0ssiter. Modrinth: https://modrinth.com/mod/QCz7p8r1 |
| `blackjack.rs` | _Let's Go Gambling!_ — `BlackjackScreenHandler.java` by BobR0ssiter. Card scoring, soft-ace logic, dealer-to-17, natural 3:2, double-down adapted from source. Split and side bets (Perfect Pairs / 21+3) not ported (chat UX). | MIT — © BobR0ssiter. Modrinth: https://modrinth.com/mod/QCz7p8r1 |
| `scratch.rs` | Original implementation. Prize tiers and odds modeled after California State Lottery scratcher structure (publicly documented). | Original |
| `mod.rs` (chips / faucet / give / jackpot) | Original implementation. Faucet streak schedule and jackpot mechanics original; economy design from `VirtualCasinoIDL/MAP.md`. | Original |
| `poker/` | _terminal-poker_ — Rust NLHE engine by ashxudev (2025). Extracted: `game/deck.rs`, `game/hand.rs`, `game/state.rs`, `game/actions.rs`, `bot/rule_based.rs`, `bot/draws.rs`, `bot/preflop.rs`. TUI, stats, and main event loop not used. `crate::game::` imports rewritten to relative paths. | MIT — © 2025 ashxudev. Repo: https://github.com/ashxudev/terminal-poker |
| `connect_four/` | _connect-four-ai_ by benjaminrall. Board representation (`Position` bitboard), win detection, and AI (`AIPlayer` with softmax difficulty selection) adapted via path dependency on the `connect-four-ai` crate. Prior `board.rs` / `bot.rs` rewritten from scratch; command flow (`mod.rs`) rewritten from scratch. | MIT — © 2025 benjaminrall. Repo: https://github.com/benjaminrall/connect-four-ai |
| `hilo.rs` | Original implementation. Game rules, probability model (P = favorable/remaining, step mult = 0.99/P), same-rank tie rule, and skip mechanic based on standard HiLo card game mechanics. Reference: _HiLo Card Game: Rules and How to Win More_ — Nice Games Team, 2026 (informational only; no code). | Original |
| `../wordle.rs` | _cl-wordle_ by Conrad Ludgate. `diff()` comparison logic, `Matches`/`Match` types, `WordSet` word validation and solution list used via git dep (`default-features = false`, no TUI/CLI features). Game state (`cl_wordle::game::Game`, `cl_wordle::state::State`) used directly. Command flow, chip integration, payout structure, and hard mode enforcement are original. | MIT — © Conrad Ludgate. Repo: https://github.com/conradludgate/wordle |
| `../battleship.rs` | _battleship-rs_ (`REFERENCE_MATERIAL/battleship-rs/`). Board model (`Vec<Vec<Position>>` with `ship_id` tracking, `take_fire`/`update_status` logic, kill-on-last-cell detection) adapted from reference. Ship types (X/V/H/I shaped) replaced with standard linear ships (Carrier/Battleship/Cruiser/Submarine/Destroyer). Random placement adapted from reference. Hard AI (hunt near previous hits ±2) adapted from reference; proper hunt/target, checkerboard parity, and probability density AI tiers are original. `uuid`/`structopt`/`ratatui` deps not ported. Not used as a crate dep. | MIT — © 2021 Deepu K Sasidharan. Repo: https://github.com/deepu105/battleship-rs |
| `../checkers.rs` | _rusty-checkers_ by dboone. Mandatory jump rule, multi-jump tree exploration, man/king promotion, and man-forward-only / king-omnidirectional movement rules adapted from reference (`REFERENCE_MATERIAL/rusty-checkers/`). **Note:** reference implemented international draughts rules (men capture all-dirs); corrected to American rules (men capture forward-only) to match WCDF English standard. Board representation, move validation, minimax AI, chip integration, and command flow are original. Draw rules (threefold repetition, 40-move rule) from _WCDF Checker-Draughts English Rules_ (`REFERENCE_MATERIAL/rusty-checkers/WCDF Checker - Draughts - English Rules.txt`), rules 1.32.1–1.32.2 — informational only, no code. Not used as a crate dep. | MIT — © dboone. Repo: https://github.com/dboone/rusty-checkers |
| `../poll.rs` | _FlapJack-Cogs / ReactPoll_ by flapjax (Red Discord Bot cog). Poll tally structure (`option → [voter_id]`), one-vote-per-user enforcement (scan-and-replace across all options on vote change), and auto-close timer pattern (background task comparing captured `ends_at`) referenced for design. No code ported; Python source used as design reference only. | Unknown — © flapjax. Repo: https://github.com/flapjax/FlapJack-Cogs |
| `../reversi.rs` | _ReversiRust_ (`REFERENCE_MATERIAL/ReversiRust/`). Board representation (`[u8; 64]`, 0=empty/1=player/2=cpu), 8-direction flip logic, coordinate parsing (`a1`-style), and initial board position (d4/e5=cpu, e4/d5=player) adapted from reference. MCTS AI replaced with minimax + alpha-beta. Positional weight table, greedy difficulty tier, chip integration, and command flow are original. `indexmap`/`regex`/`ansi_term` deps not ported. | MIT OR Apache-2.0 — © Nick Chubb. Repo: https://github.com/NickChubb/ReversiRust |
| `chess.rs` | _chess-tui_ by Thomas Mauran. Board orientation, perspective-flip (player color → rank/file iteration direction), and piece char conventions (uppercase=White, lowercase=Black, `-`=empty) adapted from architecture reference. Move format (`e2-e4`, `O-O`, `e7-e8=Q`), header row (`# a b c d e f g h`), and AI (alpha-beta minimax on `shakmaty`) are original. `ratatui`/TUI code not ported. Used as architecture reference only; `shakmaty` crate added as dep. | MIT — © 2023 Thomas Mauran. Repo: https://github.com/thomas-mauran/chess-tui |

| `sic_bo.rs` | _PySicBo_ by Wing Yung Chan. Bet type criteria, payout multipliers, and small/large definitions (`small()`, `large()`, `payoffs` dict, `bet_types` dict) used as the authoritative odds reference. All Rust code original — no Python ported. Instant resolve, no session state, no DB. | MIT — © 2019 Wing Yung Chan |
| `kalshi.rs` | Original implementation. Uses Kalshi External API v2 (public read endpoints — no auth required for market data). Category→series→market fetch chain, yes/no price model (`yes_ask_dollars`/`no_ask_dollars`), settle task with POLL_INTERVAL retry loop, and chip integration original. | Original |
| `sports.rs` | Original implementation. Uses SharpAPI sports data service (requires `sharpapi_key` in config). Event fetch, odds parse, home/away/draw side model, settle task, and chip integration original. | Original |
| `nasa_space_weather.rs` | Original implementation. Uses NASA DONKI public API (no key required at DEMO_KEY tier). CME/FLR/GST endpoints for daily event settlement, midnight UTC settle window, 1h buffer + poll retry loop, and chip integration original. NASA API data is US government public domain. | Original |
| `faa_airport.rs` | Original implementation. Uses aviationweather.gov METAR API (public, no auth). Fetches `fltCat` (VFR/MVFR/IFR/LIFR) from the METAR endpoint; accepts IATA (3-char, auto-prepends K for US) or ICAO (4-char) codes. Settle task polls METAR at close time; IFR/LIFR = YES wins, VFR/MVFR = NO wins. Chip integration and command flow original. Aviation weather data is US government public domain. | Original |
| `train.rs` | Original implementation. Uses trainstracking.com realtime API (`/api/live/realtime?source=<country>`). Non-commercial use. Multi-word train codes (e.g. "ICE 42") supported via joined-arg parsing. Settle task polls at close_time; delay ≤ 5 min = ontime wins, > 5 min = delayed wins, not found = refund. Chip integration and command flow original. **Attribution required:** data via TrainsTracking (trainstracking.com). |
| `noaa_flooding.rs` | Original implementation. Uses api.weather.gov active alerts endpoint (public, no auth). Fetches alerts for a lat/lon point and checks `properties.event/headline/description` for flood-related keywords ("flood", "storm surge"). YES wins if any flood alert is active at close time, NO wins otherwise. Chip integration and command flow original. NOAA weather alert data is US government public domain. | Original |
| `gas.rs` | Original implementation. Uses GasBuddy GraphQL API (`https://www.gasbuddy.com/graphql`, `LocationBySearchTerm` query). CSRF token extracted from `window.gbcsrf = "..."` in homepage HTML, cached to `gasbuddy_token.json`, refreshed lazily on 4xx. FlareSolver optional fallback if CF blocks homepage. 24h settlement window; refund if GasBuddy unavailable. Probability model (p_up=0.48, p_down=0.52) and chip integration original. GasBuddy is a free public web service; no ToS agreement or API key required. | Original |
| `launch.rs` | Original implementation. Uses Launch Library 2 public API (`ll.thespacedevs.com/2.2.0`). Upcoming launch list, provider history, and settlement outcome (status IDs 3=success, 4=failure, 7=partial failure) fetched from LL2. Provider success/on-time probabilities derived from last 50 launches per LSP. Bet lock at T-2h; settlement polls every 1h up to 7 days. Chip integration and command flow original. LL2 is free public API (rate limited; no auth required for public endpoints). | Original |
| `seismic.rs` | Original implementation. **Quake:** Uses USGS FDSN event API (`earthquake.usgs.gov/fdsnws/event/1/query`). 9 predefined regions; Poisson base rate from 3-year historical catalog (2023–2026); `p = 1 - e^(-λ·7d)`. Settlement queries FDSN for events in region during bet window. **Volcano:** Uses USGS Volcano Hazards Program API (`volcanoes.usgs.gov/vhpstatus.json`). Elevated volcanoes only; probability tiers Advisory=5%/Watch=20%/Warning=70%; resolves YES if `colorCode == "RED"` at 7-day close. USGS data is US government public domain. Chip integration and command flow original. | Original |

## Excluded — Games

| Idea | Reason |
|------|--------|
| _camelot_ — JS Camelot board game (no author listed). | Board is 12×16 squares; rows wrap before board content even starts. MC chat width = 240 source px (measured from 2560×1440 GUI scale 3). `[ForestBot -> Player] 04 - - - - - - - - - - - -` measures ~248 source px — already over the limit. 32+ whispers minimum, not fixable by reformatting. No Rust impl exists. |
| Keno | No color in Minecraft chat; draw results become unreadable wall of numbers. |
| Video Poker | Hold mechanic requires two-step chat exchange, loses poker's social element. |
| Crash | Too abstract without a live visual multiplier; tension is entirely visual. |

## Excluded — Economy Mechanics

| Idea | Reason |
|------|--------|
| Chip loans | No enforcement mechanism; transfer command already covers manual lending. |
| Bounties | Alt-account kill abuse; no way to verify target identity. |
| Speedrun markets | No clear implementation path. |
| Tournaments | Too much buy-in/understanding required from playerbase right now. On hold, not dead. |

## Excluded — Real-World Markets

| Idea | Reason |
|------|--------|
| Horse racing (Betfair) | Geo-locked for US users; US parimutuel data hard to get cheaply. On hold. |
| Wikipedia edit stream | Too boring; no real tension or outcome to bet against. |
| Polymarket | Same ground as Kalshi; redundant. |
| Cherry blossom bloom date | Too niche. |
| Monarch butterfly migration | Too niche. |
| Aurora sighting reports | Crowd-sourced, unreliable for settlement. |
| ISS visibility pass | No meaningful variance; pass times are deterministic. |
| Satellite reentry | Window too wide until near-event; thin frequency (few/month). Marginal at best. |
| Asteroid betting (NASA NEO API) | Close-approach dates are deterministic and known years in advance — no uncertainty to bet on. "Within N lunar distances" is always known beforehand; no variance. |
| Blood donation supply (American Red Cross regional shortage data) | No clear odds derivation; shortage classifications are coarse (adequate/limited/critical). |
| Solar/lunar eclipse cloud cover | Too compound; requires both eclipse timing + weather forecast; niche. |
| Meteor shower peak count | Niche; counts vary too much on observer conditions to settle reliably. |
| Beach rip current/surf height | Niche. |
| National park visitor counts | Niche; no real-time data, only historical stats. |
| Power outage tracker | No unified national API; fragmented by utility. |
| School closing/snow day | No unified API; district-level only, dead end. |
| Amusement park wait times (Queue-Times) | No derivable odds without pre-built historical baseline per park. |
| Traffic congestion (TomTom/INRIX) | Same issue — no odds without pre-built baseline. Enterprise-gated data. |
| CDC WONDER | Historical stats only; no forecast, no derivable odds. |
| data.cdc.gov (Socrata hub) | Reporting data, not forecast; no derivable odds. |

## Excluded — Rail/Transit Dead Ends (Geography)

| Region | Reason |
|--------|--------|
| China | Closed data; 12306.cn is domestic only, no public API. |
| Southeast Asia | No national rail APIs; Malaysia GTFS has vehicle position only, no trip updates (check back 2026+). |
| Russia | No official API; reverse-engineered one is stale (2021); sanctions/hosting risk. |
| Middle East (Saudi, UAE, Turkey) | No public developer APIs found; all closed. |
| Central Asia | No public APIs; Soviet-era closed systems. |
| Africa | No major rail operator has a public API. |
| Latin America | No national rail APIs; only scattered city GTFS. |
| Greyhound (bus) | No public API; unofficial reverse-engineered tracker only. |
| Freight rail (BNSF/UP/CSX/NS) | Enterprise-gated (mTLS + waybill auth); no fixed passenger schedule to bet against. |
