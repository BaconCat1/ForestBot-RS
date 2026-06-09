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
