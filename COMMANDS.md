# ForestBot Commands

## General

| Command | Description |
|---|---|
| `!help` / `!commands` | Links here |
| `!alias <command>` | Show aliases for a command |
| `!hardware` / `!hw` | Show OS and hardware info |
| `!health` | Show bot health, hunger, armor, and active effects |
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
| `!fadvs` / `!fadv <player?>` | Forest Advancements overview — shows earned/total per category (whispered) |
| `!fadvs <category> <player?>` | Forest Advancements for a specific category — categories: `kills`, `deaths`, `playtime`, `messages`, `joins`, `trades`, `kd`, `killmethods`, `deathmethods` |
| `!advancement` / `!advancementcount <advancement>` | Times a specific advancement was reached |
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
| `!grudge <killer?> <victim>` | How many times killer has killed victim |

## Leaderboards

| Command | Description |
|---|---|
| `!top <stat>` | Top players — stats: `kills`, `deaths`, `joins`, `playtime`, `advancements`, `messages`, `trades`, `rejects`, `slurcount` |
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
| `!remindme` / `!remind [1s2m3h4d] <message>` | Set a self-reminder. No duration = fires on next login. With duration (e.g. `1h30m`) = fires after that time if online, otherwise on next login after time expires. `!remindme stop` cancels all pending reminders. |

## Social / Lookup

| Command | Description |
|---|---|
| `!search` / `!lookup` / `!find <query>` | Find players by name |
| `!oldnames` / `!dox <player?>` | Past usernames |
| `!realname <player?>` | Current username (resolves old names) |
| `!whois <player?>` | Show player description |
| `!iam <description>` | Set your `!whois` description (ASCII only) |
| `!quote` / `!q <server?> <player?>` | Random quote from a player |
| `!rq` / `!randomquote` | Random quote from any player |
| `!rqa` / `!randomquoteall` | Random quote across all servers |
| `!lq` / `!listquoteservers` | List servers with quotes |

## FAQ

| Command | Description |
|---|---|
| `!faq <id?>` | Retrieve a FAQ entry |
| `!addfaq` | Add a FAQ entry (ASCII only) |
| `!editfaq` | Edit a FAQ entry (ASCII only) |
| `!delfaq` / `!deletefaq <id>` | Delete a FAQ entry |
| `!ownsfaq <id>` / `!faqowner` | Public: who owns a FAQ entry |
| `!ownsfaq <username>` | Whispered: list all FAQs owned by that player |
| `!ownsfaq` | Whispered: list your own FAQs |

## Fun

| Command | Description |
|---|---|
| `!crouch` | Single crouch press/release; `!crouch hold` holds for up to 10 min, `!crouch` again releases |
| `!mount` / `!ride` / `!mush <entity?>` | Mount nearest rideable entity |
| `!drop <all?>` | Drop held item (or all items) |
| `!equip` | Equip any armor pieces found in inventory |
| `!unequip` | Remove all equipped armor back to inventory |
| `!sleep` | Put the bot to sleep |
| `!twerk` / `!bootyshake` | Bot crouches for 10 seconds |
| `!setpreset` | Change bot's name chalk preset (RefinedVanilla only) |
| `!shout <message>` | Broadcast to all connected servers (not enabled) |
| `!nickname <name>` | Change bot's in-game nickname |
| `!febzey` | 🤷 |
| `!askgod` / `!agod <god>` | Consult the divine oracle — random corpus if no arg, specify a god below (75 corpora), or ask a multi-word question for an oracle response |
| `!listgods` / `!gods` | List one god per corpus with its trigger word |
| `!searchgod` / `!godsearch` / `!sgod <words>` | Search sacred texts for a keyword or phrase |
| `!godverse` / `!verse` / `!vgod <reference>` | Look up a verse by reference |
| `!godstats` | Show corpora count, size, compression ratio, verse count, load time, total god aliases |
| `!godfight <god1> <god2> <keyword?>` | Two gods, one verse each — keyword narrows both draws to matching verses |
| `!weather` / `!w <city>` | Current weather for any location — temp, feels-like, conditions, wind, humidity (Open-Meteo, no API key needed) |
| `!trivia` | Start a trivia round — question posted publicly, all players have 15 seconds to `!answer`. Summary posted at close; late answers whispered the correct answer. |
| `!answer <choice>` | Answer the active trivia round. Use A/B/C/D for multiple choice, true/false for T/F questions. |
| `!slurcount <server\|all>(optional) <player>` | Show total slur usage for a player (excludes command messages) |
| `!greeting` | Whispers format info + your current greeting |
| `!greeting <message>` | Set your join greeting — fires as `"<message>, YourName!"` (12h cooldown, ASCII only) |
| `!greeting preview` | Whispers how your greeting will look |
| `!greeting clear` | Remove your greeting |
| `!day` | Posts time until next dawn (tick 23460), or "currently daytime" |
| `!night` | Posts time until next nightfall (tick 13188), or "currently nighttime" |
| `!news` | Whispers BBC categories + top 5 top stories (numbered) |
| `!news <category>` | Whispers top 5 headlines from that category |
| `!news <category?> <N>` | Posts article N's description + link to public chat |
| `!calc` / `!wa` / `!wolframalpha <query>` | Query Wolfram\|Alpha — posts `query = result` to public chat |
| `!serversummary` / `!ssummary <server>` | Posts aggregate stats for a server: players, messages, kills, deaths, playtime, top chatter, tracking start date |
| `!compare <serverA> <serverB>` | Compares two servers side by side in a single message |
| `!wiki` / `!wikipedia <query\|random>` | Search Wikipedia — posts `[Title] summary url` to public chat. `!wiki random` for a random article. Disambiguation pages prompt for a more specific term. (60s cooldown) |
| `!minewiki` / `!mcwiki <query>` | Search the Minecraft Wiki — same format as `!wiki` with article URL included (60s cooldown) |
| `!urbandictionary` / `!ud <query>` | Search Urban Dictionary — posts `[Word] definition (+N/-N)` to public chat (60s cooldown) |
| `!translate` / `!tr` / `!tl [lang] <text\|player>` | Translate non-English text to English (default) or a target lang — posts `[from→to] result` to public chat. Single-word input checks online players and translates their last message. Requires Azure AI Translator key in config. |

## !askgod — Available Gods

See [GODS.md](GODS.md) for the full list of 75 corpora and their trigger words.

## Trades

| Command | Description |
|---|---|
| `!trade <player> <description>` / `!t <player> <description>` | Propose a trade; whispers the recipient |
| `!trade confirm` / `!t c` | Confirm the pending trade addressed to you |
| `!trade reject` / `!t r` | Reject a pending trade you're part of |
| `!trades <player?>` | Show last 3 trades (whispered) |
| `!tradestats <player?>` | Show confirmed/rejected count in public chat |
| `!tradestats full <player?>` | Full trade statistics (whispered) |
| `!link` | Generate a one-time code to link your Minecraft account to Discord via `/link` in Discord |
| `!unlink` | Remove your Discord account link (`!unlink UNLINK` to confirm) |
| `!scammers` | List up to 5 known scammers — online players first (`name (online)`), then most recently marked |
| `!report <player> <reason?>` | Report a player as a scammer |

Cooldowns: 60s to re-propose after a trade. If someone rejects your proposal, you're locked out for 10 minutes.

## Stasis Pearl

| Command | Description |
|---|---|
| `!pearl <slot>` / `!p <slot>` | Activate your stasis pearl in the given slot — bot logs in briefly, opens the trapdoor, disconnects on pearl despawn |

Whitelist per slot in `pearlbot.toml`. Result whispered on success or failure.

## Admin / Whitelist only

| Command | Description |
|---|---|
| `!reload` / `!reloadconfig` | Reload config file |
| `!blacklist <add/remove> <player>` | Manage player blacklist |
| `!whitelist <add/remove> <player>` | Manage player whitelist |
| `!censor <add/remove> <word>` | Manage censored words |
| `!wwl` / `!wordwhitelist <add/remove> <word>` | Censor word exceptions |
| `!execute` / `!exec` / `!run <text>` | Send arbitrary text or commands |
