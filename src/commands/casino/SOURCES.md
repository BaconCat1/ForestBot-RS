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

## Excluded

| Reference | Reason |
|-----------|--------|
| _camelot_ — JS Camelot board game (no author listed). No license declared; issue filed requesting MIT. | Board is 12×16 squares; individual board rows wrap before board content even starts. MC chat width = 240 source px (measured from 2560×1440 GUI scale 3, `chatWidth:1.0` confirmed in options.txt). `[ForestBot -> Player] 04 - - - - - - - - - - - -` measures ~248 source px — already over the limit. Each of 16 rows becomes 2 wrapped lines = 32+ whispers minimum; not fixable by reformatting. Verified via mc-chat-simulator (canonical ascii.png, MC 1.21.4). No Rust impl exists; minimax would also need writing from scratch. |
