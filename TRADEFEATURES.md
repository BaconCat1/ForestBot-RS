# Feature Parity: Craftbot vs Tradebot

Tradebot (Discord) is the reference implementation. This tracks what craftbot (Minecraft) is missing.

## Missing entirely

| Feature | Tradebot | Craftbot |
|---|---|---|
| `!report` command | `/report` — trade or user, reason choices | ✅ done — `!report <player> [reason]`; posts to modlogs |
| Warning display in tradestats | Shows warnings with count, reason, date | ❌ no warnings field in struct or display |
| Scammer details | Shows reason + who marked + when | ✅ done — public 🚨 warning on trade initiation; `!trades`/`!tradestats` return "trade counts not reported" |
| `confirmed_at` in trade list | Shows relative timestamp | ✅ struct fixed — `confirmed_at: Option<i64>` added |

## Struct gaps to fix

`TradebotStatsResponse` missing:
- `warnings: Vec<TradebotWarning>` with `reason: String`, `created_at: i64`

## Minor display gaps

- `!trades` hardcoded to show last 3 — tradebot shows all returned by Hub
- `!trades` truncates description at 30 chars — tradebot shows full description

## Already working (no gap)

- Hub broadcasts `trade_confirmed`/`trade_rejected` WebSocket events on craftbot-side confirms, tradebot's `listenForMcConfirms` catches them and posts to `#verified-trade` — pipeline intact
- Hub broadcasts `report_created` WS event; tradebot `listenForMcReports` posts MC-origin reports to modlogs with action buttons
- Hub broadcasts `scammer_marked`/`scammer_unmarked` WS events; craftbot announces in public chat
- `!link` → `/link` account linking — both sides implemented
