# Feature Parity: Craftbot vs Tradebot

Tradebot (Discord) is the reference implementation. This tracks what craftbot (Minecraft) is missing.

## Missing entirely

| Feature | Tradebot | Craftbot |
|---|---|---|
| `!report` command | `/report` — trade or user, reason choices | ❌ absent |
| Warning display in tradestats | Shows warnings with count, reason, date | ❌ no warnings field in struct or display |
| Scammer details | Shows reason + who marked + when | ⚠️ only `[SCAMMER]` tag |
| `confirmed_at` in trade list | Shows relative timestamp | ❌ struct missing field |

## Struct gaps to fix

`TradebotTrade` in `endpoints.rs` is missing:
- `confirmed_at: Option<i64>` (BIGINT unix ms)
- `expires_at: Option<i64>`

`TradebotStatsResponse` missing:
- `warnings: Vec<TradebotWarning>` with `reason: String`, `created_at: i64`

Scammer check currently returns `bool` — needs `Option<TradebotScammer>` with `reason`, `moderator_id`, `created_at` to show details.

## Minor display gaps

- `!trades` hardcoded to show last 3 — tradebot shows all returned by Hub
- `!trades` truncates description at 30 chars — tradebot shows full description

## Already working (no gap)

- Hub broadcasts `trade_confirmed`/`trade_rejected` WebSocket events on craftbot-side confirms, tradebot's `listenForMcConfirms` catches them and posts to `#verified-trade` — pipeline intact
- `!link` → `/link` account linking — both sides implemented
