
# Casino Commands

All casino commands use the shared chip balance. Starting balance: 1000 chips.
All games feature a 3% house rake deposited into the jackpot. All losings are deposited into the jackpot.

## Wallet & Chips

| Command | Description |
|---|---|
| `!wallet <player?>` | Balance, jackpot tickets, lotto tickets, and live portfolio value |
| `!faucet` / `!daily` | Daily chip claim — base 100 chips + 10 per streak day (streak resets if you miss a day) |
| `!give <player> <chips>` | Send chips to another player |

## Slots

| Command | Description |
|---|---|
| `!slots <chips>` | Spin the slot machine |

## Scratch

| Command | Description |
|---|---|
| `!scratch` | Free scratch card (once per day) |
| `!scratch <chips>` | Paid scratch card |

## Jackpot

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
| `!blackjack double` / `!bj d` | Double down |

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
| `!craps <chips>` | Roll the come-out roll (pass line bet) |
| `!craps roll` | Roll again once a point is established |

## Hi-Lo

| Command | Description |
|---|---|
| `!hilo <chips>` | Start a hi-lo game |
| `!hilo higher` / `!hilo h` | Guess the next card is higher |
| `!hilo lower` / `!hilo l` | Guess the next card is lower |
| `!hilo cashout` / `!hilo c` | Cash out current multiplier |

## Roulette

| Command | Description |
|---|---|
| `!roulette <bet> <chips>` | Place a roulette bet. Bet types: `red`, `black`, `odd`, `even`, `1-18`, `19-36`, or a number 0–36 |

## Connect Four (vs bot)

| Command | Description |
|---|---|
| `!c4` / `!connectfour <chips>` | Start a connect four game against the bot |
| `!c4 <1-7>` | Drop a piece in that column |
| `!c4 forfeit` | Forfeit the current game |

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
