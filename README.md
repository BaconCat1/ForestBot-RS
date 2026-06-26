
# ForestBot

<p align="center"><img src="animatedBot.gif" alt="ForestBot Animated Image" width="200"/></p>

**ForestBot** is a Minecraft bot that tracks player stats — kills, deaths, playtime, advancements, join dates, chat history, and more. It joins a Minecraft server and collects data through [Hub](https://github.com/jollycurv-e/Hub), a shared REST API and database backend.

Beyond stats, ForestBot includes a scriptural oracle (`askgod`), live weather, interactive trivia, news lookup, Wikipedia/Minecraft Wiki lookup, Wolfram|Alpha queries, Urban Dictionary, text translation, username history lookup, and more.

See [COMMANDS.md](COMMANDS.md) for the full command list.

Discord chatbridge runs through [Discord](https://github.com/jollycurv-e/Discord).

ForestBot has a fully integrated trading system with trade proposals, confirmations, and scammer reporting on both the Minecraft and Discord sides. Discord trade integration is through [tradebot](https://github.com/jollycurv-e/tradebot).

ForestBot can be used to initiate stasis chamber pulls via [pearlbot](https://github.com/jollycurv-e/pearlbot). This functionality is configured server side, and is therefore not a feature for the general public.

ForestBot includes a casino module (`!chips`, `!roulette`, `!craps`, `!bj`, `!poker`, `!scratch`, `!jackpot`, `!lotto`, and more). See [src/commands/casino/SOURCES.md](src/commands/casino/SOURCES.md) for full source attributions.

## Attributions

### Casino

Game logic adapted or referenced from the following sources:

**terminal-poker** — Rust NLHE poker engine by ashxudev (2025).
Extracted: game engine (`deck`, `hand`, `state`, `actions`) and rule-based bot (`rule_based`, `draws`, `preflop`). Used in `casino/poker/`.
License: MIT — © 2025 ashxudev.

**Let's Go Gambling!** — Fabric mod by BobR0ssiter providing game models for roulette, craps, and blackjack (`RouletteScreenHandler`, `CrapsScreenHandler`, `BlackjackScreenHandler`). Adapted for MC chat: GUI/inventory/split/side-bet logic dropped; instant-command and session-based ports written in Rust.
License: MIT — © BobR0ssiter.

**connect-four-rust** (Antonio Scotti, 2019) and **VirtualCasinoIDL** — previously referenced; no license declared. Implementations quarantined to `_quarantine/`. Pending rewrite from clean licensed sources.