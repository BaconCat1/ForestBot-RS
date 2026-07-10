
# ForestBot

<p align="center"><img src="animatedBot.gif" alt="ForestBot Animated Image" width="200"/></p>

**ForestBot** is a Minecraft bot that tracks player stats — kills, deaths, playtime, advancements, join dates, chat history, and more. It joins a Minecraft server and collects data through [Hub](https://github.com/jollycurv-e/Hub), a shared REST API and database backend.

Beyond stats, ForestBot includes a scriptural oracle (`askgod`), live weather, interactive trivia, news lookup, Wikipedia/Minecraft Wiki lookup, Wolfram|Alpha queries, Urban Dictionary, text translation, username history lookup, and more.

See [COMMANDS.md](COMMANDS.md) for the full command list.

Discord chatbridge runs through [Discord](https://github.com/jollycurv-e/Discord).

ForestBot has a fully integrated trading system with trade proposals, confirmations, and scammer reporting on both the Minecraft and Discord sides. Discord trade integration is through [tradebot](https://github.com/jollycurv-e/tradebot).

ForestBot can be used to initiate stasis chamber pulls via [pearlbot](https://github.com/jollycurv-e/pearlbot). This functionality is configured server side, and is therefore not a feature for the general public.

ForestBot includes a casino module (`!chips`, `!faucet`, `!slots`, `!hilo`, `!c4`, `!roulette`, `!craps`, `!bj`, `!poker`, `!scratch`, `!jackpot`, `!lotto`, and more). See [src/commands/casino/SOURCES.md](src/commands/casino/SOURCES.md) for full source attributions.

## Attributions

### Casino

Game logic adapted or referenced from the following sources:

**terminal-poker** — Rust NLHE poker engine by ashxudev (2025).
Extracted: game engine (`deck`, `hand`, `state`, `actions`) and rule-based bot (`rule_based`, `draws`, `preflop`). Used in `casino/poker/`.
License: MIT — © 2025 ashxudev. Repo: https://github.com/ashxudev/terminal-poker

**Let's Go Gambling!** — Fabric mod by BobR0ssiter providing game models for roulette, craps, blackjack, and baccarat (`RouletteScreenHandler`, `CrapsScreenHandler`, `BlackjackScreenHandler`, `BaccaratScreenHandler`). Adapted for MC chat: GUI/inventory/split/side-bet logic dropped; instant-command and session-based ports written in Rust.
License: MIT — © BobR0ssiter. Modrinth: https://modrinth.com/mod/QCz7p8r1

**connect-four-ai** — Rust Connect Four library by benjaminrall (2025). Board representation (`Position` bitboard), win detection, and AI player (`AIPlayer`) used via path dependency. Command flow and opponent roster written from scratch.
License: MIT — © 2025 benjaminrall. Repo: https://github.com/benjaminrall/connect-four-ai

**slot-machine-gen** — JavaScript slot machine library by Marc S. Brooks (2020–2025). Strip-model symbol selection approach referenced for weighted probability design. No code ported; JS source used as conceptual reference only.
License: MIT — © 2020-2025 Marc S. Brooks.

**rusty-checkers** — Rust checkers library by dboone. Mandatory jump rule, multi-jump logic, man/king promotion rules adapted from source. Corrected from international draughts to American rules. Board representation, minimax AI, and command flow are original.
License: MIT — © dboone. Repo: https://github.com/dboone/rusty-checkers

**battleship-rs** — Rust Battleship game by Deepu K Sasidharan (2021). Board model, ship placement, hit/sunk tracking, and AI hunt logic adapted from source. Ship types replaced with standard linear fleet; AI tiers above hunt/target are original.
License: MIT — © 2021 Deepu K Sasidharan. Repo: https://github.com/deepu105/battleship-rs

**chess-tui** — Rust chess TUI by Thomas Mauran. Board orientation, perspective-flip rendering, and piece char conventions adapted as architecture reference. Move format, header row, and alpha-beta minimax AI on `shakmaty` are original. TUI code not ported.
License: MIT — © 2023 Thomas Mauran. Repo: https://github.com/thomas-mauran/chess-tui

**cl-wordle** — Rust Wordle library by Conrad Ludgate. `diff()` comparison logic, `Matches`/`Match` types, `WordSet` word validation, and solution word list used via git dependency (`default-features = false`, no TUI/CLI). Game state (`cl_wordle::game::Game`, `cl_wordle::state::State`) used directly. Command flow, chip integration, payout structure, and hard mode enforcement are original.
License: MIT — © Conrad Ludgate. Repo: https://github.com/conradludgate/wordle

**ReversiRust** — Rust Reversi game by Nick Chubb. Board representation (`[u8; 64]`), 8-direction flip logic, coordinate parsing (`a1`-style), and initial board position adapted from source. MCTS AI replaced with minimax + alpha-beta. Positional weight table, greedy difficulty tier, chip integration, and command flow are original.
License: MIT OR Apache-2.0 — © Nick Chubb. Repo: https://github.com/NickChubb/ReversiRust

**PySicBo** — Python Sic Bo implementation by Wing Yung Chan. Bet type criteria, payout multipliers, and small/large definitions used as authoritative odds reference. All Rust code original — no Python ported.
License: MIT — © 2019 Wing Yung Chan.

### Poll

**FlapJack-Cogs / ReactPoll** — Red Discord Bot cog by flapjax. Poll tally structure (`option → [voter_id]`), one-vote-per-user enforcement (scan-and-replace across all options), and auto-close timer pattern referenced for `!poll`. No code ported; JS/Python source used as design reference only.
License: Unknown — © flapjax. Repo: https://github.com/flapjax/FlapJack-Cogs