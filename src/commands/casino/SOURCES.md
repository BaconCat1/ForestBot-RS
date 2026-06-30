# Sources for `casino/`

Game logic references and adaptations used in the ForestBot-RS casino module.

## Included

| Module | Reference | Copyright |
|--------|-----------|-----------|
| `slots.rs` | _slot-machine-gen_ by Marc S. Brooks. Weighted symbol selection algorithm (`selectRandSymbol` iterate-subtract pattern) adapted from source. Symbol set, weights, and payout table are original design from `CASINOMAP.md`. | MIT — © 2020-2025 Marc S. Brooks |
| `roulette.rs` | _Let's Go Gambling!_ — `RouletteScreenHandler.java` by BobR0ssiter. European wheel (0–36) game model and red-number set adapted from source. Instant single-command port (no GUI). | MIT — © BobR0ssiter |
| `craps.rs` | _Let's Go Gambling!_ — `CrapsScreenHandler.java` by BobR0ssiter. Pass/don't-pass two-phase dice model and come-out/point-phase logic adapted from source. Field, Any-7, hardway bets not ported (chat UX). | MIT — © BobR0ssiter |
| `blackjack.rs` | _Let's Go Gambling!_ — `BlackjackScreenHandler.java` by BobR0ssiter. Card scoring, soft-ace logic, dealer-to-17, natural 3:2, double-down adapted from source. Split and side bets (Perfect Pairs / 21+3) not ported (chat UX). | MIT — © BobR0ssiter |
| `scratch.rs` | Original implementation. Prize tiers and odds modeled after California State Lottery scratcher structure (publicly documented). | Original |
| `mod.rs` (chips / faucet / give / jackpot) | Original implementation. Faucet streak schedule and jackpot mechanics original; economy design from `VirtualCasinoIDL/MAP.md`. | Original |
| `poker/` | _terminal-poker_ — Rust NLHE engine by ashxudev (2025). Extracted: `game/deck.rs`, `game/hand.rs`, `game/state.rs`, `game/actions.rs`, `bot/rule_based.rs`, `bot/draws.rs`, `bot/preflop.rs`. TUI, stats, and main event loop not used. `crate::game::` imports rewritten to relative paths. | MIT — © 2025 ashxudev. License text: `terminal-poker/LICENSE` |
| `connect_four/` | _connect-four-ai_ by benjaminrall. Board representation (`Position` bitboard), win detection, and AI (`AIPlayer` with softmax difficulty selection) adapted via path dependency on the `connect-four-ai` crate. Prior `board.rs` / `bot.rs` rewritten from scratch; command flow (`mod.rs`) rewritten from scratch. | MIT — © 2025 benjaminrall |
| `hilo.rs` | Original implementation. Game rules, probability model (P = favorable/remaining, step mult = 0.99/P), same-rank tie rule, and skip mechanic based on standard HiLo card game mechanics. Reference: _HiLo Card Game: Rules and How to Win More_ — Nice Games Team, 2026 (informational only; no code). | Original |
| `../wordle.rs` | _cl-wordle_ by Conrad Ludgate. `diff()` comparison logic, `Matches`/`Match` types, `WordSet` word validation and solution list used via git dep (`default-features = false`, no TUI/CLI features). Game state (`cl_wordle::game::Game`, `cl_wordle::state::State`) used directly. Command flow, chip integration, payout structure, and hard mode enforcement are original. | MIT — © Conrad Ludgate. Repo: https://github.com/conradludgate/wordle |
| `../checkers.rs` | _rusty-checkers_ by Sam Bruch. Mandatory jump rule, multi-jump tree exploration, man/king promotion, and man-forward-only / king-omnidirectional movement rules adapted from reference (`REFERENCE_MATERIAL/rusty-checkers/`). **Note:** reference implemented international draughts rules (men capture all-dirs); corrected to American rules (men capture forward-only) to match WCDF English standard. Board representation, move validation, minimax AI, chip integration, and command flow are original. Draw rules (threefold repetition, 40-move rule) from _WCDF Checker-Draughts English Rules_ (`REFERENCE_MATERIAL/rusty-checkers/WCDF Checker - Draughts - English Rules.txt`), rules 1.32.1–1.32.2 — informational only, no code. Not used as a crate dep. | MIT — © Sam Bruch. Repo: https://github.com/sambruch/rusty-checkers |
| `../reversi.rs` | _ReversiRust_ (`REFERENCE_MATERIAL/ReversiRust/`). Board representation (`[u8; 64]`, 0=empty/1=player/2=cpu), 8-direction flip logic, coordinate parsing (`a1`-style), and initial board position (d4/e5=cpu, e4/d5=player) adapted from reference. MCTS AI replaced with minimax + alpha-beta. Positional weight table, greedy difficulty tier, chip integration, and command flow are original. `indexmap`/`regex`/`ansi_term` deps not ported. | MIT OR Apache-2.0 — © Nick Chubb. Repo: https://github.com/NickChubb/ReversiRust |

## Excluded

| Reference | Reason |
|-----------|--------|
| _camelot_ — JS Camelot board game (no author listed). No license declared; issue filed requesting MIT. | Board is 12×16 squares; individual board rows wrap before board content even starts. MC chat width = 240 source px (measured from 2560×1440 GUI scale 3, `chatWidth:1.0` confirmed in options.txt). `[ForestBot -> Player] 04 - - - - - - - - - - - -` measures ~248 source px — already over the limit. Each of 16 rows becomes 2 wrapped lines = 32+ whispers minimum; not fixable by reformatting. Verified via mc-chat-simulator (canonical ascii.png, MC 1.21.4). No Rust impl exists; minimax would also need writing from scratch. |
