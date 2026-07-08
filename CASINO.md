

# Casino Commands

**18 games · 13 markets · 40 commands. ** All use the shared chip balance. Starting balance: 1000 chips.
All games feature a 3% house rake deposited into the jackpot. All losings are deposited into the jackpot.

## Wallet & Chips

| Command | Description |
|---|---|
| `!wallet <player?>` | Balance, jackpot tickets, lotto tickets, live portfolio value, and open event bet count |
| `!faucet` / `!daily` | Daily chip claim — base 100 chips + 10 per streak day (streak resets if you miss a day) |
| `!give <player> <chips>` | Send chips to another player |
| `!bets` | List all open event bets (AQI, gas, weather, launch, sports, Kalshi, seismic, train, FAA, flood, space weather) |

## Slots

| Command | Description |
|---|---|
| `!slots <chips>` | Spin the slot machine |

## Scratch Off Tickets

| Command | Description |
|---|---|
| `!scratch` | Free scratch card (once per day) |
| `!scratch <chips>` | Paid scratch card |

## Jackpot (Raffle)

| Command | Description |
|---|---|
| `!jackpot` | Buy a jackpot ticket (100 chips). Pot seeds at 7000 and grows with rake from all games. Weekly draw — odds scale with ticket count. |
| `!jackpot tickets <player?>` | Show ticket count |

## Lotto

| Command | Description |
|---|---|
| `!lotto` | Buy a lotto ticket (pick 5 numbers 1–40, costs 50 chips). Weekly draw (Saturday by default). |
| `!lotto pot` | Show current lotto pot |
| `!lotto tickets <player?>` | Show your tickets for today's draw |
| `!draw lotto` | (Whitelisted) Trigger lotto draw manually |
| `!draw jackpot` | (Whitelisted) Trigger jackpot draw manually |

## Blackjack

| Command | Description |
|---|---|
| `!blackjack <chips>` / `!bj <chips>` | Start a blackjack hand |
| `!blackjack hit` / `!bj h` | Hit |
| `!blackjack stand` / `!bj s` | Stand |
| `!blackjack double` / `!bj d` | Double down (first 2 cards only) |
| `!blackjack quit` / `!bj q` | Forfeit — stake to jackpot |

## Poker (vs bot)

| Command | Description |
|---|---|
| `!poker <chips>` | Start a heads-up poker hand against the bot |
| `!poker check` / `!poker c` | Check |
| `!poker call` / `!poker ca` | Call |
| `!poker fold` / `!poker f` | Fold |
| `!poker raise <amount>` / `!poker r <amount>` | Raise |

## Craps

| Command | Description |
|---|---|
| `!craps pass <chips>` | Come-out roll on the pass line |
| `!craps dontpass <chips>` / `!craps dp <chips>` | Come-out roll on the don't-pass line |
| `!craps roll` / `!craps r` | Roll again once a point is established |
| `!craps quit` / `!craps q` | Forfeit — stake to jackpot |

## Hi-Lo

| Command | Description |
|---|---|
| `!hilo <chips>` | Start a hi-lo game |
| `!hilo hi` / `!hilo h` / `!hilo higher` | Guess the next card is higher |
| `!hilo lo` / `!hilo l` / `!hilo lower` | Guess the next card is lower |
| `!hilo skip` / `!hilo s` | Skip the current card (draw a new one) |
| `!hilo cash` / `!hilo c` / `!hilo cashout` | Cash out current multiplier (available after first guess) |

## Roulette

European wheel (0–36). Format: `!roulette <type> <selection> <chips>`

| Type | Selection | Payout |
|---|---|---|
| `color` | `red` / `black` / `green` | 1:1 (green = 35:1) |
| `parity` | `odd` / `even` | 1:1 |
| `half` | `low` (1–18) / `high` (19–36) | 1:1 |
| `column` | `1` / `2` / `3` | 2:1 |
| `dozen` | `1` (1–12) / `2` (13–24) / `3` (25–36) | 2:1 |
| `number` | `0`–`36` | 35:1 |

Example: `!roulette color red 100` | `!roulette number 17 50`

## Baccarat

Instant resolution. Bet on Player, Banker, or Tie. Cards drawn by standard mini-baccarat rules (draw on ≤5). Min bet: 10 chips.

| Command | Description |
|---|---|
| `!baccarat player <chips>` / `!bac p <chips>` | Bet on Player hand — 2× |
| `!baccarat banker <chips>` / `!bac b <chips>` | Bet on Banker hand — 1.95× (5% commission) |
| `!baccarat tie <chips>` / `!bac t <chips>` | Bet on Tie — 8× |

## Sic Bo

Instant resolution. Three dice rolled on each bet. Min bet: 10 chips, max 50,000 chips.

| Command | Description |
|---|---|
| `!sicbo small <chips>` | Total 4–10 (no triple) — 1:1 |
| `!sicbo large <chips>` | Total 11–17 (no triple) — 1:1 |
| `!sicbo anytriple <chips>` / `!sic any <chips>` | Any triple — 30:1 |
| `!sicbo total <4-17> <chips>` | Exact total — 4/17: 60:1, 5/16: 30:1, 6/15: 17:1, 7/14: 12:1, 8/13: 8:1, 9–12: 6:1 |
| `!sicbo single <1-6> <chips>` | Die value appears at least once — 1 match: 1:1, 2 match: 2:1, 3 match: 3:1 |
| `!sicbo double <1-6> <chips>` | Value appears on ≥2 dice — 10:1 |
| `!sicbo triple <1-6> <chips>` | All three dice show that value — 180:1 |

## Connect Four (vs bot)

Played in whisper. You are ◕, bot is ▣, ▢ = empty.

| Command | Description |
|---|---|
| `!c4` / `!connectfour <chips>` | Start a connect four game against the bot |
| `!c4 <1-7>` | Drop a piece in that column |
| `!c4 forfeit` | Forfeit the current game |

## Battleship

Played in whisper, per-player. Ships placed randomly. Enemy board shown by default; `!bs own` for your board. Rows `a`–`j`, cols `1`–`9` then `0` (col 10). ◈=ship, ▢=water, ◌=miss, ◕=hit, ▣=sunk (full ship revealed on sink).

| Command | Description |
|---|---|
| `!battleship <chips>` / `!bs <chips>` | Start a game. Randomly matched against one of five opponents at escalating difficulty. |
| `!bs <coord>` | Fire at coordinate (e.g. `!bs a5`, `!bs j0`). |
| `!bs board` | Redisplay enemy board. |
| `!bs own` | Show your board (ships + enemy hits). |
| `!bs forfeit` / `!bs quit` | Forfeit — stake to jackpot. |

Ships: Carrier (5), Battleship (4), Cruiser (3), Submarine (3), Destroyer (2). Opponents: Glass Joe (random), Piston Honda (hunt near hits ±2), Bald Bull (hunt/target), Soda Popinski (target + checkerboard parity), Mike Tyson (probability density). Win = 2× stake. Lose/quit = stake to jackpot.

## Checkers

Played in whisper, per-player. You are red (`r`/`R`), bot is black (`b`/`B`), `-` = empty. American rules: men move and capture forward-only, kings all directions. Jumps mandatory.

| Command | Description |
|---|---|
| `!checkers <chips>` | Start a game. Randomly matched against one of five opponents at escalating difficulty. |
| `!checkers <a1> <b2>` | Move piece from a1 to b2. |
| `!checkers <a1> <c3> <e5>` | Multi-jump: list each landing square in the jump chain. |
| `!checkers board` | Redisplay the current board. |
| `!checkers quit` / `!checkers forfeit` | Forfeit — stake to jackpot. |

Opponents: Glass Joe (random), Piston Honda (easy/depth 2), Bald Bull / Soda Popinski (medium/depth 4), Mike Tyson (hard/depth 6). Win = 2× stake. Lose/quit = stake to jackpot. Draw (threefold repetition or 40-move rule) = stake returned.

## Minesweeper

All in whisper. 10×10 board, 20 mines. Stacking multiplier per click (3% house edge per click). Min bet: 25 chips.

| Command | Description |
|---|---|
| `!mines <chips>` / `!minesweeper <chips>` | Start a game |
| `!mines <coord>` | Reveal a cell (e.g. `!mines a3`, `!mines j0`). Flood-fills from zero cells. |
| `!mines f<coord>` | Flag / unflag a cell (e.g. `!mines fa3`) |
| `!mines cash` | Cash out at current multiplier (requires ≥1 safe reveal) |
| `!mines board` | Redisplay board and current multiplier |
| `!mines quit` / `!mines forfeit` | Forfeit — stake to jackpot |

Rows `𝐚`–`𝐣`, cols `1`–`9` then `0` (col 10). ▢=unrevealed, ◌=0 neighbors, 𝟏–𝟖=neighbor count, ◈=flag, ◕=mine (revealed on game over). First click guaranteed safe (3×3 safe zone). Reveal all 80 safe cells to win at current multiplier. Hit a mine = stake to jackpot.

## Reversi

Played in whisper, per-player. You are ◕, bot is ▣, ◌ = your legal moves, ▢ = empty. Standard Othello starting position, row 1 at top.

| Command | Description |
|---|---|
| `!reversi <chips>` / `!othello <chips>` | Start a game. Randomly matched against an opponent at escalating difficulty. |
| `!reversi <a1>` | Place your piece at that square. |
| `!reversi board` | Redisplay the current board. |
| `!reversi quit` / `!reversi forfeit` | Forfeit — stake to jackpot. |

Opponents: Glass Joe (random), Piston Honda (greedy), Bald Bull (minimax 3), Soda Popinski (minimax 4), Mike Tyson (minimax 5). Win = 2× stake. Lose/quit = stake to jackpot. Draw = stake returned.

## Trivia

Two-phase. Starter sets the wager; others have 30 seconds to join at the same stake. Question posts after join window closes; participants have 45 seconds to answer. Correct = 2× stake. Wrong or no answer = stake to jackpot. Min stake: 50 chips.

| Command | Description |
|---|---|
| `!trivia <chips>` | Start a trivia round with a random category |
| `!trivia <category> <chips>` | Start a trivia round in a specific category |
| `!trivia join` | Join the active round within the 30s join window (matches starter's wager) |
| `!trivia categories` | List all available categories |
| `!answer <A/B/C/D or true/false>` | Answer the question once it goes live (participants only) |

Categories: general, books, film, music, musicals, tv, games, board games, science, computers, math, mythology, sports, geography, history, politics, art, celebrities, animals, vehicles, comics, gadgets, anime, cartoons.

## Wordle

All in whisper. Per-player — multiple games can run simultaneously.

| Command | Description |
|---|---|
| `!wordle <chips>` | Start a game. Stake deducted immediately. |
| `!wordle <chips> hard` | Start in hard mode — exact matches must stay fixed in later guesses |
| `!wordle <word>` | Submit a 5-letter guess |
| `!wordle board` | Show current board |
| `!wordle quit` / `!wordle forfeit` | Forfeit — stake sent to jackpot |

Win multipliers (by guess number): 8x / 5x / 3x / 2x / 1.5x / 1.2x. Losing forfeits stake to jackpot. Word list: NYT Wordle list (swappable).

## Chess

All in whisper. Per-player — multiple games can run simultaneously.

| Command | Description |
|---|---|
| `!chess white <chips>` | Start a game as White (you move first) |
| `!chess black <chips>` | Start a game as Black (bot moves first) |
| `!chess <from> <to>` | Make a move. E.g. `!chess e2 e4` |
| `!chess <from> <to> <promo>` | Promote a pawn. E.g. `!chess e7 e8 q` (q/r/b/n) |
| `!chess` | Show current board and status |
| `!chess quit` / `!chess q` | Forfeit — stake sent to jackpot |

Board renders in whisper with Unicode pieces (♔♕♖♗♘♙ / ♚♛♜♝♞♟), empty=▢ (U+25A2). Displayed from your color's perspective (rank 1 at bottom for White, rank 8 at bottom for Black).

Win multiplier: 2×. Loss or forfeit: stake sent to jackpot. Draw (50-move rule, stalemate, insufficient material): stake returned.

**Opponents (random at game start):**
| Opponent | Strength |
|---|---|
| Glass Joe | Random moves |
| Piston Honda | Greedy (depth 1) |
| Bald Bull | Alpha-beta depth 2 |
| Soda Popinski | Alpha-beta depth 3 |
| Mike Tyson | Alpha-beta depth 4 |

## Duels

| Command | Description |
|---|---|
| `!duel <player> <chips>` | Challenge a player to a duel. Chips escrowed immediately. Challenged has 60s to respond. |
| `!duel confirm` | Accept the pending duel challenge directed at you |
| `!duel reject` / `!duel cancel` | Decline or cancel a pending duel. Challenger gets refund. |
| `!duel odds [player]` | Show win probabilities for an active duel (K/D based, 50/50 if under 10 kills) |
| `!duel bet <player> <chips>` | Side bet on a participant in an active duel. Pays at implied odds. One bet per duel. |

**Duel rules:**
- Both players escrow equal chips
- Winner takes pot minus 3% rake (goes to jackpot)
- Duel auto-cancels (full refund) on: 10-minute timeout, either player disconnecting, third-party kill
- Side bet winners paid at implied odds from bet placement time; losers go to jackpot
- Participants cannot place side bets on their own duel

# Betting Markets

## Market (paper trading)

| Command | Description |
|---|---|
| `!market <symbol>` | Live quote for a stock or crypto symbol |
| `!market history <symbol> [1d/7d/30d/1y]` | Price history |
| `!market search <query>` | Search for a symbol by name |
| `!market long <symbol> <chips> <duration>` | Bet the price goes up. Duration: `1m`, `15m`, `1h`, `4h`, `1d`. Min 50 chips, max 10,000. |
| `!market short <symbol> <chips> <duration>` | Bet the price goes down |
| `!market bets` / `!market pos` | Show your open timed bets |
| `!market cashout [index]` | Exit a timed bet early at current price. Index required if you have multiple open bets. |
| `!market buy <symbol> <chips>` | Open a portfolio position (no expiry). One position per symbol. |
| `!market sell <symbol>` | Close a portfolio position at current price |
| `!market sell all` | Close all portfolio positions at once |
| `!portfolio` / `!port` | Live P&L breakdown of all open portfolio positions (whispered) |

Payout: `ceil(stake × exit_price / entry_price)` for longs, inverse for shorts. Min stake: 50 chips. One portfolio position per symbol per player.

## Weather Futures

Bet on whether it will rain in a city on a specific future date. Odds are derived from the Open-Meteo forecast probability — betting against the forecast pays more. Min bet: 50 chips.

| Command | Description |
|---|---|
| `!weather <city>` | Current weather (public) + tomorrow's rain odds (whispered) |
| `!weather odds <city>` | Rain odds for all durations 1d/3d/7d/14d (whispered) |
| `!weather bet <city> rain yes <chips> <duration>` | Bet it will rain |
| `!weather bet <city> rain no <chips> <duration>` | Bet it won't rain |
| `!weather bets` | Show your open weather bets |

Durations: `1d`, `3d`, `7d`, `14d`. Payout = `stake × odds` (correct) or stake to jackpot (wrong). Odds shown at bet time based on forecast probability for the target date. API failure = full refund.

Example: `!weather bet London rain yes 100 3d` — if forecast says 80% rain, you get 1.25× for yes or 5× for no.

## Sports Betting

Bet on live sports events from the SharpAPI feed. Stake debited immediately; settled when the event resolves. Requires `sharpapi_key` in config. Min bet: 50 chips.

| Command | Description |
|---|---|
| `!sports` / `!sb` | List available sports categories with event counts |
| `!sports <sport>` | List upcoming bettable events — shows odds + date label |
| `!sports bet <#> home\|away\|draw` | Preview odds for that event (no chips = no bet placed) |
| `!sports bet <#> home\|away\|draw <chips>` | Place a bet. Aliases: `h`, `a`, `d` |
| `!sports bets` | Show your open sports bets |

Events shown are pre-game only (SharpAPI drops them once they start). Payout = `stake × decimal_odds`. Correct = winnings credited. Wrong = stake to jackpot. API unavailable at settlement = full refund.

## Kalshi Prediction Markets

Bet on real-money prediction markets via the Kalshi public API. Stake debited immediately; settled when the market resolves. Min bet: 25 chips.

| Command | Description |
|---|---|
| `!kalshi` / `!k` | List available categories |
| `!kalshi <category>` | List open markets (up to 5) |
| `!kalshi <#> yes\|no <chips>` | Place a bet on a market |
| `!kalshi bets` | Show your open Kalshi bets |

Categories: sports, crypto, politics, economics, entertainment, tech, climate, finance, elections, health. Payout = `stake / price`. Correct = payout credited. Wrong = stake to jackpot. Market unavailable = full refund.

## Space Weather

Bet on NASA-tracked space weather events. Bets settle at midnight UTC + 1h buffer, then the bot polls the DONKI API. Odds are live (fetched from DONKI + SWPC) — run `!sw` to see current multipliers. Min bet: 25 chips.

| Command | Description |
|---|---|
| `!spaceweather` / `!sw` | List bet types and odds |
| `!sw cme <chips>` | Coronal Mass Ejection recorded today |
| `!sw xflare <chips>` | X-class solar flare recorded today |
| `!sw gstorm <chips>` | Geomagnetic storm recorded today |
| `!sw bets` | Show your open space weather bets |

Events sourced from NASA DONKI (api.nasa.gov). Occurred = payout credited. Not occurred = stake to jackpot. API unavailable = full refund.

## Airport Conditions

Bet on whether a US airport will be in IFR or LIFR flight conditions in 2 hours. Conditions sourced from aviationweather.gov METAR. Min bet: 25 chips.

| Command | Description |
|---|---|
| `!faa` / `!airport` | Show usage and supported airport code examples |
| `!faa <IATA/ICAO>` | Current flight category + odds (e.g. `!faa JFK` or `!faa KJFK`) |
| `!faa <code> yes <chips>` | Bet airport will be IFR or LIFR in 2h |
| `!faa <code> no <chips>` | Bet airport will be VFR or MVFR in 2h |
| `!faa bets` | Show your open airport condition bets |

Flight categories: VFR (clear) → MVFR (marginal) → IFR (instrument) → LIFR (low instrument). IFR/LIFR = low visibility/ceiling = delays likely. Correct = payout credited. Wrong = stake to jackpot. METAR unavailable = full refund.

## NOAA Flood Alerts

Bet on whether a NOAA flood warning is active at a location within 2 hours. Alerts from the NOAA Weather.gov API. Min bet: 25 chips.

| Command | Description |
|---|---|
| `!flood list` / `!flood ls` | List active flood warnings with numbered locations |
| `!flood bet <#> yes\|no` | Preview odds for that location (no bet placed) |
| `!flood bet <#> yes\|no <chips>` | Bet flood alert is (yes) or isn't (no) active |
| `!flood bets` | Show your open flood bets |

Run `!flood list` first to see numbered locations, then `!flood bet <#>`. Raw lat/lon also works: `!flood <lat> <lon> yes|no <chips>`. Payout = `stake / price`. Correct = payout credited. Wrong = stake to jackpot. API unavailable = full refund.

## Seismic Events

Bet on whether an earthquake or volcanic eruption occurs within 7 days. Data from USGS. Min bet: 25 chips.

| Command | Description |
|---|---|
| `!quake list` / `!eq list` | List available earthquake regions |
| `!quake <region> yes\|no` | Preview odds (no chips = no bet placed) |
| `!quake <region> yes\|no <chips>` | Bet on M5+ quake in region within 7 days. Optional magnitude override: `m<mag>` (e.g. `!quake california m6 yes 100`) |
| `!quake bets` | Show your open quake bets |
| `!volcano list` / `!vol list` | List active/monitored volcanoes |
| `!volcano <name> yes\|no` | Preview odds (no chips = no bet placed) |
| `!volcano <name> yes\|no <chips>` | Bet on volcanic eruption within 7 days |
| `!volcano bets` | Show your open volcano bets |

Regions: `california`, `alaska`, `pacific-nw`, `japan`, `indonesia`, `chile`, `italy`, `turkey`, `new-zealand`. Payout = `stake / price`. Correct = payout credited. Wrong = stake to jackpot. API unavailable = full refund.

## Train Delays

Bet on whether a running intercity train will be on time (≤5 min delay) or delayed (>5 min delay) at settlement, 2 hours from bet placement. Data from trainstracking.com realtime feed. Train not found at settlement (arrived/cancelled) = full refund. Min bet: 25 chips.

Supported countries: `us` (Amtrak), `de` (Germany), `fr`, `be`, `ch`, `fi`, `nl`, `no`, `at`, `se`, `it`, `es`, `pl`, `cz`, `my`. Multi-word train codes (e.g. `ICE 42`) are supported — type them space-separated after the country.

| Command | Description |
|---|---|
| `!train` / `!trains` | Show usage |
| `!train list <country>` | List up to 8 running trains with current delays |
| `!train <country> <code> ontime\|delayed` | Preview odds (no chips = no bet placed) |
| `!train <country> <code> ontime\|delayed <chips>` | Bet train is on time or delayed at 2h settlement |
| `!train bets` | Show your open train bets |

Payout = `stake / price`. Currently delayed → ontime is ~2.94×, delayed is ~1.45×. Currently on time → ontime is ~1.45×, delayed is ~2.94×. Correct = payout credited. Wrong = stake to jackpot.

## AQI (Air Quality)

Bet on whether tomorrow's AQI will be Good or Unhealthy for any US zip code. Uses AirNow forecast data. Min bet: 25 chips. Settles 24h after placement using live observation data.

| Command | Description |
|---|---|
| `!aqi <zip>` / `!airquality <zip>` | Show current + tomorrow's forecast and odds for that zip |
| `!aqi <zip> good <chips>` | Bet that tomorrow's AQI will be Good (≤50) |
| `!aqi <zip> unhealthy <chips>` | Bet that tomorrow's AQI will be Unhealthy (>100) |
| `!aqi bets` | List your open AQI bets |

Odds derived from AirNow forecast category: Cat1(Good)→GOOD favored, Cat4+(Unhealthy)→UNHEALTHY favored. If AirNow API unavailable at settlement, stake is refunded. Requires `airnow` API key in config.

## Rocket Launch

Bet on upcoming rocket launches using real LL2 (Launch Library 2) data. Two bet types per launch: **success** (launch succeeds) and **ontime** (no scrub or delay >24h). Min bet: 25 chips. Bets lock 2h before launch window. Settles when LL2 reports a final status; polls hourly up to 7 days after window end. Refunds if no final status after 7 days.

| Command | Description |
|---|---|
| `!rocket` / `!launch` | List next 5 Go/TBC launches with short IDs |
| `!rocket <id>` | Show odds for a specific launch (success + ontime) |
| `!rocket <id> success <chips>` | Bet that the launch succeeds |
| `!rocket <id> ontime <chips>` | Bet the launch isn't delayed more than 24h |
| `!rocket bets` | List your open launch bets |

Odds derived from the provider's recent launch history (last 50 launches). Success floor: 0.70×, ontime floor: 0.50×. No API key required (uses public LL2 API).

## Gas Price

Bet on whether the average regular gas price in your area will be higher or lower tomorrow. Uses GasBuddy data. Min bet: 25 chips. 24h window. Refunds if GasBuddy is unavailable at settlement.

| Command | Description |
|---|---|
| `!gas <zip>` | Show current avg price and odds for up/down |
| `!gas <zip> up <chips>` | Bet the price will be higher tomorrow |
| `!gas <zip> down <chips>` | Bet the price will be lower tomorrow |
| `!gas bets` | List your open gas bets |

Odds: ~50/50 baseline.
