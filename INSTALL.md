# ForestBot-RS — Installation

## Prerequisites

- Rust nightly toolchain (managed via `rust-toolchain.toml` — `rustup` handles this automatically)
- Hub running at `http://localhost:8001`
- A Minecraft account

## Setup

Copy `.env.example` to `.env` and fill in:

```env
MC_USER=your_minecraft_username
MC_PASS=your_minecraft_password
API_KEY=your_hub_api_key
```

`API_KEY` must match `APIKEY` in Hub's `.env`.

Copy `example.config.json` to `config.json` and configure:

| Key | Description |
|-----|-------------|
| `mc_server` | Unique identifier for this bot instance (used in Hub) |
| `host` / `port` | Minecraft server address |
| `version` | Minecraft protocol version |
| `api_url` / `websocket_url` | Hub base URL (default `http://localhost:8001`) |
| `prefix` | In-game command prefix |
| `use_mc_whitelist` | If true, only whitelisted players can use commands — edit `json/mc_whitelist.json` |
| `reconnect_time` | Milliseconds to wait before reconnect on disconnect |
| `smart_censoring` | If true, flagged outbound messages are rephrased via Together AI instead of star-masked (needs `together_api_key` set — falls back to regular censoring if blank) |
| `censor_threshold` | Minimum rustrict severity that gets censored in outbound chat: `"mild"`, `"moderate"` (default), or `"severe"`. Live-editable via `!reload`, no recompile needed |

## Build & Run

```bash
RUSTFLAGS="-C target-cpu=native" cargo build --release
./target/release/ForestBot-RS
```

Or for development:

```bash
RUSTFLAGS="-C target-cpu=native" cargo run
```

First build downloads azalea from git and may take several minutes.

## JSON files

The `json/` directory holds runtime data files:

| File | Purpose |
|------|---------|
| `mc_blacklist.json` | Blacklisted player UUIDs |
| `mc_whitelist.json` | Whitelisted player UUIDs (if `use_mc_whitelist` is true) |
| `bad_words.json` | Profanity filter custom additions (layered on rustrict's built-in dictionary) |
| `word_whitelist.json` | Profanity filter false-positive exceptions (layered as safe overrides) |
| `colors.json` | Color config |
| `offline_messages.json` | Pending offline messages — **instance-specific, do not share** |

To share a list across multiple bot instances (e.g. a single blacklist for all servers), use hardlinks. The files will appear as separate files on disk but write to the same inode — any edit is instantly visible to all instances without coordination.
