# ForestBot Commands

## General

| Command | Description |
|---|---|
| `!help` / `!commands` | Links here |
| `!discord` | Discord server invite link |
| `!ping <player?>` | Show a player's ping |
| `!ap` / `!averageping` | Average server ping |
| `!bp` / `!bestping` | Player with best ping |
| `!wp` / `!worstping` | Player with worst ping |
| `!coords` | Bot's current coordinates |
| `!active` | Recently active players |
| `!profile <player?>` | Link to forestbot.org profile |
| `!standing` / `!status <player?>` | Player's standing/rank |

## Player Stats

| Command | Description |
|---|---|
| `!joindate` / `!jd` / `!firstseen <server?> <player?>` | First seen date |
| `!playtime <server?> <player?>` | Total playtime |
| `!jdpt <server?> <player?>` | Join date and playtime together |
| `!joins <server?> <player?>` | Number of times joined |
| `!lastseen <player?>` | Last time seen online |
| `!survived <player?>` | Time since last death |
| `!kd` / `!kills` / `!deaths <server?> <player?>` | Kill/death stats |
| `!winrate` / `!wr <player?>` | Kill winrate |
| `!advancements` / `!advs <server?> <player?>` | Total advancement count |
| `!lastadvancement` / `!ladv <player?>` | Most recent advancement |
| `!summary` / `!sum <player?>` | Stats overview (kd, playtime, messages, advancements, joindate) |
| `!servers` / `!seenservers <player?>` | Servers a player has been seen on |
| `!users` / `!uniqueusers` | Unique user count |

## Kill/Death

| Command | Description |
|---|---|
| `!firstkill` / `!fk <player?>` | First kill |
| `!lastkill` / `!lk <player?>` | Most recent kill |
| `!firstdeath` / `!fd <player?>` | First death |
| `!lastdeath` / `!ld <player?>` | Most recent death |
| `!victims <player?>` | Number of unique players killed |
| `!vs <player1> <player2>` | Head-to-head comparison |
| `!grudge [killer] <victim>` | How many times killer has killed victim |

## Leaderboards

| Command | Description |
|---|---|
| `!top <stat>` | Top players — stats: `kills`, `deaths`, `joins`, `playtime`, `advancements`, `messages`, `trades`, `rejects` |
| `!oldest` / `!oldheads` | Oldest users on the server |
| `!noobs` / `!newest` | Newest users on the server |
| `!efficiency` / `!eff <player> <stat>` | Rate of a stat per time period |

## Chat and Messaging

| Command | Description |
|---|---|
| `!firstmessage` / `!fm <server?> <player?>` | First chat message |
| `!lastmessage` / `!lm <server?> <player?>` | Most recent chat message |
| `!msgcount <server?> <player?>` | Total message count |
| `!wordcount` / `!count <player> <word>` | How many times a player said a word |
| `!offlinemsg <player> <message>` | Deliver a message when they next join |

## Social / Lookup

| Command | Description |
|---|---|
| `!search` / `!lookup` / `!find <query>` | Find players by name |
| `!oldnames` / `!dox <player?>` | Past usernames |
| `!realname <player?>` | Current username (resolves old names) |
| `!whois <player?>` | Show player description |
| `!iam <description>` | Set your `!whois` description |
| `!quote` / `!q <server?> <player?>` | Random quote from a player |
| `!rq` / `!randomquote` | Random quote from any player |
| `!rqa` / `!randomquoteall` | Random quote across all servers |
| `!lq` / `!listquoteservers` | List servers with quotes |

## FAQ

| Command | Description |
|---|---|
| `!faq <id?>` | Retrieve a FAQ entry |
| `!addfaq` | Add a FAQ entry |
| `!editfaq` | Edit a FAQ entry |
| `!ownsfaq <id>` / `!faqowner` | See who owns a FAQ entry |

## Fun

| Command | Description |
|---|---|
| `!mount` / `!ride` / `!mush <entity?>` | Mount nearest rideable entity |
| `!drop [all]` | Drop held item (or all items) |
| `!sleep` | Put the bot to sleep |
| `!twerk` / `!bootyshake` | Bot crouches for 10 seconds |
| `!setpreset` | Change bot's name chalk preset (RefinedVanilla only) |
| `!shout <message>` | Broadcast to all connected servers (not enabled) |
| `!nickname <name>` | Change bot's in-game nickname |
| `!febzey` | 🤷 |

## Trades

| Command | Description |
|---|---|
| `!trade <player> <description>` / `!t <player> <description>` | Propose a trade; whispers the recipient |
| `!trade confirm` / `!t c` | Confirm the pending trade addressed to you |
| `!trade reject` / `!t r` | Reject a pending trade you're part of |
| `!trades [player?]` | Show last 3 trades (whispered) |
| `!tradestats [player?]` | Show confirmed/rejected count in public chat |
| `!tradestats full [player?]` | Full trade statistics (whispered) |
| `!link` | Generate a one-time code to link your Minecraft account to Discord via `/link` in Discord |
| `!unlink` | Remove your Discord account link (`!unlink UNLINK` to confirm) |
| `!scammers` | List up to 5 known scammers — online players first (`name (online)`), then most recently marked |

Cooldowns: 60s to re-propose after a trade. If someone rejects your proposal, you're locked out for 10 minutes.

## Admin / Whitelist only

| Command | Description |
|---|---|
| `!reload` / `!reloadconfig` | Reload config file |
| `!blacklist <add/remove> <player>` | Manage player blacklist |
| `!whitelist <add/remove> <player>` | Manage player whitelist |
| `!censor <add/remove> <word>` | Manage censored words |
| `!wwl` / `!wordwhitelist <add/remove> <word>` | Censor word exceptions |
| `!execute` / `!exec` / `!run <text>` | Send arbitrary text or commands |
