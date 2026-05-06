# ForestBot Rust Port Remaining TypeScript Parity

Only behavior still missing or partial compared to `ForestBot/src` is listed here.

## Commands

- [ ] `drop`: TypeScript drops the held item or every inventory item via Mineflayer `tossStack`; Rust currently only replies that Azalea inventory-drop parity is not wired.
- [ ] `mount` / `ride` / `mush`: TypeScript finds the nearest mountable entity or vehicle, applies cooldowns, and mounts it; Rust currently only replies that mounting is not wired.
- [ ] `sleep`: TypeScript finds and activates a bed; Rust currently only replies that sleeping is not wired.
- [ ] `twerk` / `bootyshake` / `booty` / `dance`: TypeScript toggles sneak for 10 seconds; Rust command is registered but still needs equivalent Azalea control-state behavior verified.
- [ ] `realname`: TypeScript resolves visible display/nickname data from Mineflayer player state; Rust needs equivalent display-name data in the player cache for exact parity.
- [ ] `febzey`: Rust has equivalent last-seen-style behavior, but it is not byte-for-byte identical to the TypeScript command text.

## Bot Runtime Behavior

- [x] Port the TypeScript outgoing message filter except the secondary filter, which is intentionally not planned for the Rust port:
  - `useCustomChatPrefix` / `customChatPrefix`
  - `json/bad_words.json` profanity filter
  - `json/word_whitelist.json`
  - `smart_censoring` / Together API censor path
  - queued outgoing send behavior
- [ ] Port `announce`: TypeScript periodically advertises enabled non-whitelisted command descriptions after spawn.
- [ ] Port `antiafk`: TypeScript starts anti-AFK on spawn when enabled.
- [ ] Port `usePViewer` / `pViewerPort`.
- [ ] Port startup ping/retry behavior from TypeScript `Bot.startBot()`, including the 10-failure long backoff.
- [ ] Port TypeScript reconnect lifecycle exactly: `endAndRestart()`, `isConnected`, and explicit bot quit/end handling.
- [ ] Port TypeScript logger categories and message wording where runtime parity matters.

## Chat Parsing And Message Handling

- [ ] Honor `useLegacyChat` with the TypeScript `messagestr.ts` behavior.
- [ ] Fully honor `useCustomChatFormatParser`; Rust still attempts custom/fallback parsing rather than matching the config switch exactly.
- [ ] Verify Rust whisper parsing covers both TypeScript `whisperFrom.ts` and `whisperTo.ts` cases.

## Events

- [ ] `end.ts`: TypeScript marks the bot disconnected, quits, restarts, logs end args, and sends the bot leave websocket packet.
- [ ] `error.ts`: TypeScript event behavior is not separately represented in Rust.
- [ ] `kicked.ts`: TypeScript logs the full kick payload and ends the bot.
- [ ] `spawn.ts`: Rust sends player-list updates and starts websocket listeners, but still lacks TypeScript spawn extras: pViewer, anti-AFK, announce interval, outgoing robot marker hook, and `restartCount` / `isConnected` state updates.
- [ ] `physicsTick.ts`: TypeScript writes `tick_end` packets; confirm whether Azalea truly makes this unnecessary, then either document or port.

## Moderation

- [ ] Fully port MC whitelist enforcement beyond command/admin gating
- [ ] Port TypeScript `anti_spam_cooldown` and `anti_spam_msg_limit`; Rust currently only has command cooldown handling.
