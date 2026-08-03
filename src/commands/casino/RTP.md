# Casino RTP Audit

Tracks the actual, verified return-to-player for every game/market that takes a real chip stake.
**97% (3% house rake) is the DEFAULT target for invented/synthetic odds** (`HOUSE_EDGE = 0.03` in
`casino/mod.rs`, see `[[feedback_casino_house_edge]]`), **not a hard requirement**. When a game's
odds are sourced from real, documented, external math (a real casino game's actual rules, a real
published PAR sheet, etc.), the real number is correct as-is even if it lands above or below 97%.
That's not an oversight to fix, it's the whole point of using real data instead of made-up numbers
(2026-08-03, explicit user clarification during the post-slots audit). Only synthetic/invented odds
are held to the 97% default.

**Why this doc exists**: `!slots` was live with a 344% RTP (paying out 3.44x every chip wagered)
for an unknown period before catching it 2026-08-03, a real exploited bug, not a rounding issue.
The multiplier table was invented, never verified against the game's actual 5-payline structure,
and nothing tracked what RTP any game was supposed to have vs. what it actually pays. This doc is
the fix for that: one place recording verified vs. unverified status for every game, so a future
session (or a future feature added to an existing game) has something to check against instead of
assuming someone must have checked this already.

**Verification method matters**, note how a number was confirmed, not just what it is:
- **Brute-force**: exhaustively enumerated every possible outcome (feasible for games with a small
  finite outcome space, like slots' 21 cubed reel-stop combinations). The strongest verification,
  zero estimation error.
- **Formula-check**: confirmed the code's probability-to-payout formula algebraically reduces to
  exactly `0.97` (e.g. event markets using `p / (1 - HOUSE_EDGE)`, the rake is baked into the odds
  conversion itself, provable without simulation).
- **Monte Carlo**: large-N simulation, used only when brute-force is computationally infeasible
  (e.g. games with too many card/board states to enumerate). Has real (small) estimation error,
  weakest tier, note the N used.
- **N/A**: no fixed RTP concept applies (PvP wagers, skill-based board games vs. an AI whose real
  win rate isn't a designed constant, pass-through external market prices).

## Games (fixed math odds)

| Game | File | Mechanism | Target RTP | Verified RTP | Method | Status |
|------|------|-----------|------------|---------------|--------|--------|
| Slots | `slots.rs` | 3-reel/5-line strip, per-symbol triple-match multiplier | 97% | **97.006%** | Brute-force (21 cubed combos) | Fixed 2026-08-03 (was 344%) |
| Roulette | `roulette.rs` | European wheel (0-36), standard bet types | Real (European single-zero) | **97.297%** (36/37, all bet types identical) | Formula-check (algebraic) | Verified. Real, unmodified European roulette math, correct as-is |
| Craps | `craps.rs` | Pass/don't-pass, come-out/point phases | Real (casino craps) | **98.586%** (pass), **98.636%** (don't pass) | Formula-check (renewal probability, cross-checked against documented real values) | Verified. Byte-accurate real casino craps rules including the correct "bar 12" push, correct as-is |
| Blackjack | `blackjack.rs` | Shared 6-deck shoe, S17, blackjack 3:2, double any 2, no splits/insurance | Real (casino blackjack, minus splits) | **99.067%** | Monte Carlo (3M hands, basic strategy) | Verified. Real S17 6-deck rules, matches published no-split house-edge range, correct as-is |
| Baccarat | `baccarat.rs` | Shared multi-deck shoe, Player 2x / Banker 1.95x / Tie 8x | Real (casino baccarat) | Player **98.772%**, Banker **98.936%**, Tie **76.341%** | Brute-force (exact, idealized independent draws, same method as roulette/craps) | Fixed 2026-08-03. Banker's 3rd-card draw was simplified to ignore the player's 3rd card, making Player/Banker win probability exactly symmetric and zeroing the Player bet's house edge (was 100.000% RTP, a real exploit). Replaced with the real published banker-draw table; all 3 bets now match real casino baccarat within rounding. |
| Hi-Lo | `hilo.rs` | Live P=favorable/remaining (ties count as wins), step mult = HOUSE_EDGE/P | 97% per step | **97% per step, exact** | Formula-check (algebraic: E[next value] = P * (0.97/P) * current = 0.97 * current, holds for any P) | Verified. Each step is exactly 97% by construction. Overall session RTP is player-dependent (chaining compounds the edge, 0.97^n after n correct guesses), same as any progressive-multiplier game genre, not a bug |
| Scratch | `scratch.rs` | Prize-tier ticket, real CA Lottery table (Golden State Riches #1735) scaled to 3 chip tiers | Real (CA Lottery Scratchers) | Copper **79.191%**, Gold **78.624%**, Diamond **78.767%** | Brute-force (exact, 5-bucket EV-preserving merge of real 11-tier table) | Fixed 2026-08-03. Previous prize tables were invented and badly miscalibrated (11-21% RTP, opposite direction from slots). Replaced with a real, official prize table pulled from calottery.com; all 3 tiers now land at the real game's own ~79% RTP |
| Mines | `mines.rs` | 10x10 grid, 20 mines, step mult = HOUSE_EDGE/p per click (p = safe_remaining/unrevealed) | 97% per click | **97% per click, exact** | Formula-check (same proof as hilo.rs) | Verified. Flood-fill (one click can auto-reveal a cluster) only costs one multiplier step, not one per cell, which is correct since the flooded cells are a deterministic side effect of the one real gamble, not separate risky events. Overall session RTP is player-dependent, same as hilo.rs |
| Sic Bo | `sic_bo.rs` | 3-dice, small/large/total/single/double/triple bets (PySicBo reference payouts) | Real (casino Sic Bo) | Small/Large **97.222%**, Any Triple **86.111%**, Triple **83.796%**, Double **81.481%**, Single **92.130%**, Total(4-17) **81.0-90.3%** | Brute-force (exact, all 216 dice combos) | Verified. Every bet type matches well-documented real Sic Bo house edges, correct as-is |
| Poker | `poker/` | Multiplayer NLHE vs. rule-based bot | N/A | not yet audited | N/A | Rake structure, not a fixed-RTP game. Needs its own framing, not a single RTP number |

## Board games (stake vs. AI opponent, no fixed math RTP)

Win/lose/draw multiplier is fixed (win = 2x stake, lose = jackpot rake, draw = stake back per
`[[project_casino_state]]`), but the real RTP for a human player depends on how good the AI
actually is at each difficulty tier. Getting a real number here would mean either a code-level
strength assessment (still a qualitative guess) or building an actual self-play simulation
harness (a genuinely new, separate piece of work, not just checking existing math). Neither
attempted 2026-08-03 given session length.

**Real, current exposure found and fixed instead (2026-08-03)**: checkers/reversi/battleship/
wordle had **no configured max stake at all** in `bet_limits.json` (only chess/connect_four had a
real cap, 5,000 stake / 10,000 max payout). A player could stake their entire balance, whatever
size, against an AI whose real win rate was never verified. Rather than derive the odds, capped
max payout at 10,000 to match chess/connect_four's existing convention:

| Game | File | Payout structure | Max stake (fixed 2026-08-03) | Max payout |
|------|------|-------------------|-------------------------------|------------|
| Checkers | `../checkers.rs` | Win 2x, lose rake, draw back | 5,000 (was unbounded) | 10,000 |
| Chess | `chess.rs` | Win 2x, lose rake, draw back | 5,000 (already capped) | 10,000 |
| Reversi | `reversi.rs` | Win 2x, lose rake, draw back | 5,000 (was unbounded) | 10,000 |
| Battleship | `../battleship.rs` | Win 2x, lose rake, no draw | 5,000 (was unbounded) | 10,000 |
| Connect Four | `connect_four/` | Win 2x, lose rake | 5,000 (already capped) | 10,000 |
| Wordle | `../wordle.rs` | Win up to 8x (1-guess), scaling down to 1.2x (6-guess) | 1,250 (was unbounded) | 10,000 |

AI win-rate per tier is still not measured for any of these — the stake cap limits financial
exposure regardless of true odds, it doesn't make the odds real. Revisit if a self-play harness
is ever built.

## Event/price markets (probability-derived odds)

Per `[[feedback_casino_house_edge]]`, these should already bake in the rake via formula
(`p / (1 - HOUSE_EDGE)` or equivalent), which is provable by inspection rather than simulation,
but "should" isn't "verified." Same mistake that let slots slip through with a comment nobody
checked against the real code path.

| Market | File | Formula (claimed) | Verified? | Status |
|--------|------|---------------------|-----------|--------|
| Weather | `../weather.rs` | `1/p * RAKE(0.97)`, p = live ensemble forecast probability | Real (live forecast data) | 97% (formula exact) | Formula-check only -- confirmed p comes from a real live ensemble forecast API each call, not a guess. Exact real-world calibration of the forecast API itself not independently re-verified | Verified 2026-08-03 at formula level, no fix needed |
| Gas price | `gas.rs` | Hardcoded p_up=0.48/p_down=0.52, payout=floor(stake/price) | no | **Pinned 2026-08-03, viable path exists.** No free bulk historical source exists (EIA weekly not daily, AAA no export, GasBuddy historical is commercial-only), but the live source is a single API (GasBuddy GraphQL, per-zip, `gas_cache_ttl_ms` cache) -- one real distribution to build by recording live price moves at that cadence over time. Not fixed yet, but tractable, unlike train below |
| FAA flight category | `faa_airport.rs` | Real persistence/onset rates from METAR historical data (was invented 67/33 symmetric guess) | Real (METAR-derived) | 97% (formula exact, real input probabilities: 67.70% persistence / 2.22% onset, 2h window) | Formula-check (exact) + real historical data (104,781 real hourly obs, 6 airports, 2022-2023) | Fixed 2026-08-03. Real persistence (67.70%) was remarkably close to the original guess; real onset (2.22%) was ~15x lower than the guessed 33%. Same asymmetry as noaa_flooding.rs. See `REFERENCE_MATERIAL/DOCS/faa-flight-category-persistence/` |
| Rocket launch | `launch.rs` | `calc_provider_probs` counts real successes/on-time launches out of last 50 real LL2 results per launch provider, clamped to 70-98% (success) / 50-98% (on-time), fed into shared `to_price` | Real (LL2 provider historical data), formula confirmed | 97% (formula exact) | Full read (510 lines): fetch/cache/settle chain all real and standard, same pattern as every other correctly-implemented market | Verified 2026-08-03, no fix needed. Older memory note calling this "house edge pass deferred" was stale -- formula and real data are both present. Only caveat: the 70-98%/50-98% clamps mean a genuinely worse-than-70% or better-than-98% real provider track record gets silently overridden by the floor/ceiling instead of the true rate, same style of guard-rail clamp used elsewhere (aqi.rs/seismic.rs cap `to_price`'s output range for the same reason) -- not treated as a bug |
| Kalshi | `kalshi.rs` | Pass-through external market price | N/A | Market spread is the implicit rake, per convention |
| Sports | `sports.rs` | Pass-through external odds (SharpAPI) | N/A | Same as Kalshi |
| AQI | `aqi.rs` | Real forecast-verification data (TCEQ real forecast category vs. real measured AQI, was invented round-number guess) | Real (TCEQ-derived) | 97% (formula exact, real input probabilities: see breakdown below) | Formula-check (exact) + real historical data (1,565 real paired days, 11 TX cities, 2012 season) | Fixed 2026-08-03. Cat 1-3 (Good/Moderate/Sensitive) real, substantial samples (52-1,157 days each): P(good)=81.9%/13.8%/0%, P(unhealthy)=0.3%/28.2%/84.6%. Cat 4-5 had almost no real samples (n=1, n=0) and are extrapolated from the real trend, not measured. See `REFERENCE_MATERIAL/DOCS/aqi-forecast-accuracy/` |
| NOAA flooding | `noaa_flooding.rs` | Real persistence/onset rates from NWS VTEC historical data (was invented 67/33 symmetric guess) | Real (VTEC-derived) | 97% (formula exact, real input probabilities: 54.92% persistence / 16.02% onset, 24h window) | Formula-check (exact) + real historical data (2,486 unique warnings, 6 WFOs, 2020-2025) | Fixed 2026-08-03. Real data showed persistence (currently-flooding stays flooding) and onset (new flooding starts) are NOT mirror images at any window size. Also bumped the settlement window 2h->24h: real 2h numbers (95.26%/1.44%) made it a near-guaranteed bet either way; 24h lands near a real coin flip and matches every sibling market's cadence. See `REFERENCE_MATERIAL/DOCS/flood-warning-persistence/` |
| NASA space weather | `nasa_space_weather.rs` | `HOUSE_EDGE / p`, p = live DONKI 27-day rolling + SWPC Kp data | Real (live data) | 97% (formula exact) | Formula-check only -- confirmed p comes from real live DONKI/SWPC APIs, not a guess. Exact calibration of those feeds not independently re-verified | Verified 2026-08-03 at formula level, no fix needed |
| Seismic (quake) | `seismic.rs` | Poisson base rate from real 3-year historical USGS catalog, `p = 1 - e^(-lambda * 7d)` | Real (USGS historical data) | 97% (formula exact) | Formula-check only -- confirmed lambda comes from a real historical catalog per region, not a guess | Verified 2026-08-03 at formula level, no fix needed |
| Seismic (volcano) | `seismic.rs` | Tiered probability (Advisory/Watch/Warning) from real USGS Volcano Hazards status | Real (USGS status tiers) | 97% (formula exact) | Formula-check only | Verified 2026-08-03 at formula level, no fix needed |
| Train | `train.rs` | One hardcoded 67/33 `compute_odds` (train.rs:100) applied uniformly to every train on every source | no | **Pinned 2026-08-03, scope larger than first framed.** Real source found (Amtrak ASMAD) and real data manually pulled (train 5, 1 month, California Zephyr) -- only 32 real trips, too thin to use, site behind Cloudflare JS challenge blocking bigger automated pulls. But the deeper problem: this single guess is shared across ~15 legacy countries (`trainstracking.com`) *and* all 10 GTFS-RT agencies (MBTA + 7 MTA feeds + LIRR + Metro-North), each covering many distinct routes/trips -- a real fix isn't one data pull, it's a real per-network delay distribution for ~25 separate rail systems. Not tractable the way gas.rs's single-source live-collection plan is. Not fixed, flagged as a reasonable heuristic, revisit only if someone wants to build real per-network tracking |
| GTFS-RT (train) | `gtfs_rt.rs` | N/A -- pure protobuf fetch/decode + trip-snapshot lookup, no odds/probability logic of any kind | N/A | N/A | Read in full 2026-08-03: proto2 types, feed fetch, rail-route filter, trip snapshot helpers only. All real odds logic for train bets lives in `train.rs` (already pinned above), this file just supplies it real-time trip data. No RTP concept applies here |
| Join-window futures | `join_market.rs` | Real per-player historical join-gap statistics via Hub `casino_join_odds` endpoint (has its own eligibility gate, "need 30 comparable gaps") | Real (per-player historical data) | 97% (formula exact) | Formula-check only -- confirmed the endpoint computes from real historical data with a sample-size gate, not a guess. Hub-side computation itself not independently re-derived/re-verified | Verified 2026-08-03 at formula level, no fix needed |
| Death-window futures | `death_market.rs` | Same pattern as join-window, real per-player historical death-gap statistics (reconstructed in playtime-ms) | Real (per-player historical data) | 97% (formula exact) | Formula-check only, same caveat as join-window | Verified 2026-08-03 at formula level, no fix needed |
| Market/portfolio | `../market.rs` | Real stock-price simulation, not a fixed-odds bet | N/A | Fee/spread structure, not an RTP question |

## Economy mechanics (not games, no RTP concept)

| Item | File | Notes |
|------|------|-------|
| Faucet | `mod.rs` | Free chips, streak-scheduled. No stake, no RTP |
| Jackpot | `mod.rs` | 100% payout, reseeds at 7000 (per memory). Funded by every other game's rake, not its own odds |
| Lotto | Hub `draws.ts` | 5-from-40, custom prize ladder (match4=5000, match3=250, match2=50, match1=1, ticket cost 50, plus a rollover jackpot pot growing 10/ticket) | Real (verified against actual Powerball structure) | Fixed-tier RTP (excl. jackpot) **17.9%**, verified real: real Powerball's own fixed-tier RTP (excl. jackpot) is **15.99%** (computed from the real official prize chart), nearly identical | Exact combinatorics (hypergeometric, C(40,5)=658,008 total combos) + direct comparison to real Powerball prize chart | Verified 2026-08-03, no fix needed. Multi-tier lotteries are *designed* this way -- tiny fixed prizes, nearly all real value sitting in the jackpot alone. Our jackpot (1-in-658,008, pot grows 10/ticket) will realistically never grow large enough or get hit at this server's real scale to provide the value-boost Powerball's jackpot does -- an honest structural fact about running a real-lottery-shaped game on a small player base, not a math error |
| Duel | `duel.rs` | Player vs. player wager | N/A, no house edge, it's a peer wager |

## How to audit an unchecked row

1. Find the actual payout table / multiplier formula in the file.
2. If the outcome space is small and finite (a strip, a fixed deck+rules combo), brute-force it.
   See `slots.rs`'s git history (2026-08-03 fix) for the pattern: enumerate every equally-likely
   outcome, sum `probability x payout`, compare to 0.97.
3. If it's a probability-to-payout formula (event markets), confirm algebraically it reduces to
   exactly `0.97`, don't assume the comment/variable name is accurate. `gas.rs`'s hardcoded
   0.48/0.52 split is exactly the kind of thing that looks like it should be a formula but isn't.
4. Update this table with the real number, the method used, and today's date in a short note if
   a bug was found and fixed (mirroring the slots row above).
