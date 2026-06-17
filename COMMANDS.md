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
| `!delfaq` / `!deletefaq <id>` | Delete a FAQ entry |
| `!ownsfaq <id>` / `!faqowner` | See who owns a FAQ entry |

## Fun

| Command | Description |
|---|---|
| `!crouch` | Single crouch press/release; `!crouch hold` holds for up to 10 min, `!crouch` again releases |
| `!mount` / `!ride` / `!mush <entity?>` | Mount nearest rideable entity |
| `!drop [all]` | Drop held item (or all items) |
| `!sleep` | Put the bot to sleep |
| `!twerk` / `!bootyshake` | Bot crouches for 10 seconds |
| `!setpreset` | Change bot's name chalk preset (RefinedVanilla only) |
| `!shout <message>` | Broadcast to all connected servers (not enabled) |
| `!nickname <name>` | Change bot's in-game nickname |
| `!febzey` | 🤷 |
| `!askgod` / `!agod <god>` | Consult the divine oracle — random corpus if no arg, or specify a god below (67 corpora) |
| `!listgods` / `!gods` | List one god per corpus with its trigger word |
| `!searchgod` / `!godsearch` / `!sgod <words>` | Search sacred texts for a keyword or phrase |
| `!godverse` / `!verse` / `!vgod <reference>` | Look up a verse by reference |
| `!godstats` | Show corpora count, size, compression ratio, verse count, load time, total god aliases |
| `!slurcount <server\|all>(optional) <player>` | Show total slur usage for a player (excludes command messages) |

## !askgod — Available Gods

No arg picks randomly from all corpora. Corpus files live in `godtexts/`. (67 corpora)

| Arg(s) | Corpus |
|---|---|
| `god` `bible` `jesus` `christ` `kjv` `christian` | King James Bible |
| `allah` `quran` `koran` `islam` `muslim` `muhammad` | Quran |
| `moroni` `nephi` `mormon` `joseph` `lds` `bom` | Book of Mormon |
| `bahai` `baha` `bahaullah` `aqdas` | Bahá'í texts |
| `piby` `rastafari` `rasta` `athlyi` `rogers` `jah` | Holy Piby |
| `hayyi` `hiia` `mandaean` `mandaeanism` `ginza` `manda` `nasoraean` `nasorean` | Mandaean texts |
| `mani` `manichean` `manichaean` `manichaeism` `manicheanism` | Manichaean writings |
| `moon` `unification` `moonies` `divine` `principle` | Divine Principle (Unification Church) |
| `noi` `nation` `blackman` `yakub` `yakoub` | Nation of Islam |
| `gnostic` `gnosticism` `nag` `hammadi` `sophia` `pleroma` `demiurge` | Nag Hammadi library |
| `eddy` `christianscience` `scienceandhealth` | Science and Health (Christian Science) |
| `brahma` `vishnu` `shiva` `krishna` `indra` `veda` `gita` `mahabharata` | Hindu sacred texts |
| `buddha` `buddhism` `pali` `dharma` `nirvana` `tipitaka` `gautama` `theravada` `sangha` | Pali Canon |
| `waheguru` `nanak` `sikh` `sikhism` `granth` `ggs` `guru` | Guru Granth Sahib |
| `tao` `taoist` `taoism` `laozi` `laotzu` `zhuangzi` `chuangtzu` `ttc` | Tao Te Ching + Zhuangzi |
| `confucius` `confucianism` `analects` `kongzi` `lunyu` `zhongyong` | Confucian Analects |
| `shinto` `kami` `amaterasu` `izanagi` `izanami` `kojiki` `norito` `nihongi` | Shinto texts |
| `caodai` `cao_dai` `jade` `jadeemperor` | Cao Dai teachings |
| `zoroaster` `zoroastrian` `zoroastrianism` `ahura` `mazda` `zarathustra` `avesta` `parsi` `zend` | Zend-Avesta |
| `egypt` `egyptian` `ra` `osiris` `isis` `horus` `anubis` `thoth` `amon` `amun` `aten` | Book of the Dead + Pyramid Texts |
| `norse` `odin` `thor` `loki` `freyr` `freyja` `edda` `valhalla` `asgard` `yggdrasil` `ragnarok` | Poetic Edda (Voluspo, Hovamol, Lokasenna, etc.) |
| `greek` `olympian` `zeus` `athena` `apollo` `poseidon` `hera` `ares` `hermes` `artemis` `iliad` `odyssey` `homer` | Iliad + Odyssey (Homer, Butler trans.) |
| `mayan` `maya` `hurakan` `xibalba` `kukulkan` `quetzalcoatl` `kiche` `popolvuh` `itzamna` | Popol Vuh (Kiché Maya, Spence retelling) |
| `babylon` `babylonian` `hammurabi` `marduk` `shamash` `ishtar` `akkad` `akkadian` `mesopotamia` | Code of Hammurabi (King trans., 1910) |
| `sumerian` `sumer` `gilgamesh` `enkidu` `enumaelish` `tiamat` `apsu` `anunnaki` `enlil` `enki` `inanna` `nanna` `utu` | Sumerian sacred texts — Enuma Elish / Seven Tablets of Creation (King trans., 1902) + texts TBD |
| `aztec` `azteca` `mexica` `nahua` `nahuatl` `huitzilopochtli` `tlaloc` `tezcatlipoca` `xipe` `coatlicue` `tonatiuh` `chalchiuhtlicue` | Sacred Songs of the Ancient Mexicans (Brinton, 1890) |
| `hermetic` `hermeticism` `trismegistus` `poemandres` `corpus` `emerald` `kybalion` | Corpus Hermeticum (Mead trans., 1906) |
| `thelema` `crowley` `aleister` `liber` `beast` `nuit` `hadit` `hoor` `aiwass` `therion` | The Book of the Law / Liber AL vel Legis (Aleister Crowley, 1904) |
| `eris` `discordia` `discordian` `discordianism` `principia` `fnord` `kallisti` `malaclypse` `chaos` | Principia Discordia (Malaclypse the Younger / Gregory Hill, 1965) |
| `spiritism` `spiritist` `kardec` `allankardec` `medium` `spirits` `spiritsbook` | The Spirits' Book (Allan Kardec, 1857) |
| `tenrikyo` `ofudesaki` `oyasama` `tsukihi` `miki` `nakayama` `jiba` | Ofudesaki (Miki Nakayama / Oyasama, 1869–1882) |
| `falun` `falundafa` `falungong` `zhuanfalun` `dafa` `lihongzhi` `shifu` | Zhuan Falun (Li Hongzhi, 1994) |
| `rael` `raelian` `raelism` `elohim` `vorilhon` `clonaid` | The Raelian Message (Claude Vorilhon / Rael, 1973–1975) |
| `elderscrolls` `vivec` `tamriel` `daedra` `aedra` `aurbis` `nirn` `nerevarine` `monomyth` `veloth` `dunmer` `morrowind` `khajiit` `alduin` `talos` `shor` `lorkhan` | Elder Scrolls lore (Vivec's 36 Lessons, Monomyth, Anticipations, Battle of Red Mountain, etc.) |
| `subgenius` `dobbs` `slack` `xist` `bulldada` `jhvh` `stang` | The Book of the SubGenius (Rev. Ivan Stang et al., 1983) |
| `bokonon` `bokononism` `foma` `karass` `granfalloon` `wampeter` `duprass` `vonnegut` `catscradle` `calypso` | Books of Bokonon (Kurt Vonnegut, Cat's Cradle, 1963) |
| `tolkien` `silmarillion` `ainulindale` `valaquenta` `akallabeth` `numenor` `valar` `maiar` `eru` `iluvatar` `morgoth` `melkor` `arda` `valinor` `feanor` `eldar` `ainur` `middleearth` | The Silmarillion (J.R.R. Tolkien, 1977) — Ainulindalë, Valaquenta, Quenta Silmarillion, Akallabêth, Of the Rings of Power |
| `shaker` `shakers` `annlee` `secondappearing` `millennial` `youngs` | The Testimony of Christ's Second Appearing (Benjamin Seth Youngs, 1808) — foundational Shaker doctrinal text |
| `swedenborg` `newchurch` `newjerusalem` `arcana` `coelestia` `conjugial` `influx` `correspondences` `spiritualworld` | Emanuel Swedenborg's complete theological works — Heaven and Hell, Arcana Coelestia, Divine Love and Wisdom, Divine Providence, True Christian Religion, Conjugial Love, Apocalypse Explained/Revealed, and more |
| `canaan` `canaanite` `ugarit` `ugaritic` `baal` `anat` `asherah` `astarte` `aqhat` `kirta` `rephaim` `mot` `yamm` `kothar` | Stories from Ancient Canaan (Coogan & Smith, 2nd ed.) — Aqhat, The Rephaim, Kirta, the Baal Cycle, The Lovely Gods, El's Drinking Party (Ugaritic myths, ~1200 BCE) |
| `moorish` `moorishscience` `drewali` `circle7` `noblepath` `moor` `asiatic` | The Holy Koran of the Moorish Science Temple of America / Circle 7 Koran (Noble Prophet Drew Ali, 1927) |
| `setian` `templeofset` `xeper` `harwer` `aquino` `bookofthenight` `setianblackflame` | The Book of Coming Forth by Night (Michael A. Aquino / Temple of Set, 1975) |
| `urantia` `urantiabook` `urantian` `orvonton` `nebadon` `havona` `thoughtadjuster` `finaliter` `uversa` `salvington` | The Urantia Book (Urantia Foundation, 1955) — 196 papers on cosmology, theology, and the life of Jesus |
| `heavensgate` `telah` `tido` `applewhite` `nettles` `nextlevel` `hale-bopp` `halebopp` | Heaven's Gate writings — statements, Beyond Human transcripts, student testimonials (Ti and Do, 1975–1997) |
| `process` `processchurch` `processian` `jehovah` `lucifer` `satan` `robertdevegrimston` `maryannmaclean` | The Process Church of the Final Judgment — _Humanity Is the Devil_ / _People_ (Robert de Grimston, 1967) |
| `andraste` `andrastianism` `maker` `chantoflight` `thedas` `ferelden` `orlais` `dragonage` `chantry` `divine` | The Chant of Light — scripture of Andrastianism (Dragon Age / BioWare) |
| `orphic` `orpheus` `orphism` `dionysus` `persephone` `hecate` `protogonus` `phanes` `mysteries` `bacchic` | Orphic Hymns (Thomas Taylor trans., 1792) — 87 hymns to Greek deities including Night, Pan, Hecate, Bacchus, Apollo, Persephone |
| `neoplatonism` `neoplatonist` `plotinus` `plotinos` `enneads` `theone` `emanation` `nous` `proclus` `porphyry` `iamblichus` | Plotinus — Complete Works / Enneads (Kenneth Guthrie trans., 1918) — The One, emanation, the soul, intellect, beauty, providence |
| `kabbalah` `zohar` `kabbalist` `kabbalistic` `sefirot` `sephirot` `simeonbaryochai` `rashbi` `einsof` `soncino` `jewishmysticism` | The Zohar (Soncino Press trans., 1933) — mystical commentary on the Torah, Bereshit through Ha'Azinu, Idra Rabba, Idra Zuta |
| `lavey` `laveyanism` `satanic` `churchofsatan` `blackpope` `satanbible` `nineantsatanicstatements` | The Satanic Bible (Anton LaVey) |
| `cathar` `catharism` `cathari` `albigensian` `albigenses` `parfait` `consolamentum` `bogomil` `bogomilism` `secretsupper` `interrogatio` | Catharism texts |
| `caine` `noddism` `kindred` `vampire` `masquerade` `gehenna` `jyhad` `sabbat` `camarilla` `antediluvian` `bookofnod` `vtm` `worldofdarkness` | Book of Nod (Vampire: The Masquerade) |
| `earthseed` `olamina` `godischange` `godseed` | Earthseed verses (Octavia Butler, Parable of the Sower/Talents) |
| `jain` `jainism` `mahavira` `gaina` `tirthankara` `akaranga` `vardhamana` | Jain Agamas |
| `incan` `inca` `huarochiri` `pariacaca` `paria` `quechua` `andean` `huallallo` `viracocha` | Huarochiri Manuscript |
| `iching` `yijing` `yiching` `yi` `zhouyi` `hexagram` `legge` `khien` `bagua` `trigram` | I Ching (Legge trans.) |
| `jedi` `jedipath` `theforce` `force` `yoda` `skywalker` `anakin` `luke` `obi` `kenobi` `mace` `windu` `sith` `midichlorian` | Jedi Path / Force teachings (Star Wars) |
| `dss` `deadseascrolls` `qumran` `essene` `essenes` `communityrule` `damascusdocument` `warscroll` `thanksgivinghymns` `templeoscroll` `vermes` | Dead Sea Scrolls (Vermes 7th ed.) |
| `enoch` `jubilees` `deuterocanon` | 1 Enoch + Jubilees (R.H. Charles trans.) |
| `acim` `miracles` `workbook` | A Course in Miracles |
| `faithism` `oahspe` `jehovih` `kosmon` | Oahspe (John Ballou Newbrough, 1882) |
| `aquarian` `dowling` `akashic` | Aquarian Gospel of Jesus the Christ (Levi H. Dowling, 1908) |
| `lawofone` `ramaterial` `confederation` `densities` | The Law of One / Ra Material (L/L Research) |
| `iammovement` `iam` `saintgermain` `godfreking` `ballard` `lotusray` `ascendedmaster` `mightyiampresence` | "I AM" Movement / Saint Germain Series (Guy W. Ballard) |
| `acadfuturesci` `hurtak` `affs` `academyforfuturescience` `brotherhoodoflight` `ophanimenoch` | The Keys of Enoch (J.J. Hurtak, 1996) |
| `unarius` `ernestnorman` `shamballa` `voiceoferos` `voiceofhermes` `voiceoforion` `voiceofvenus` | Unarius (Ernest L. Norman) — Voice of Eros/Hermes/Orion/Venus + The Elysium |
| `aetherius` `georgeking` `ninefreedoms` `twelveblessings` `saintgooling` `marssector6` | The Aetherius Society (George King) — The Twelve Blessings + The Nine Freedoms |
| `anthroposophy` `steiner` `rudolfsteiner` | Rudolf Steiner — An Outline of Occult Science |
| `mahikari` `sukyomahikari` `okada` `sukuinushisama` | Sukyo Mahikari — A Publication for All Mankind |

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

## Moderation

| Command | Description |
|---|---|
| `!report <player> [reason]` | Report a player as a scammer |

## Admin / Whitelist only

| Command | Description |
|---|---|
| `!reload` / `!reloadconfig` | Reload config file |
| `!blacklist <add/remove> <player>` | Manage player blacklist |
| `!whitelist <add/remove> <player>` | Manage player whitelist |
| `!censor <add/remove> <word>` | Manage censored words |
| `!wwl` / `!wordwhitelist <add/remove> <word>` | Censor word exceptions |
| `!execute` / `!exec` / `!run <text>` | Send arbitrary text or commands |
