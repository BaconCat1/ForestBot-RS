# ForestBot Rust Port TODO

## Core
- [x] Create Rust project structure matching the Node bot layout
- [x] Load full `config.json` schema
- [x] Load `.env` credentials
- [x] Pin nightly Rust toolchain
- [x] Start Azalea client
- [x] Add ViaVersion support
- [x] Add `disableChatSigning` support
- [x] Add basic runtime reload state
- [x] Replace scaffold warnings with wired modules 

## Chat
- [x] Parse vanilla/Azalea chat
- [x] Parse `username: message`
- [x] Parse clan/rank prefixes like `RSP DaddyPayMe: message`
- [x] Parse arrow formats like `username » message`
- [x] Parse configured `customChatFormats`
- [x] Filter join/leave system messages from chat logs
- [ ] Port whisper parser
- [ ] Port outgoing custom chat prefix behavior
- [ ] Port legacy/custom chat parser options fully

## Commands
- [x] Add command registry/framework
- [x] Port `ping`
- [x] Port `help`
- [x] Port `discord`
- [x] Port `reload`
- [ ] Port `coords`
- [ ] Port `whitelist`
- [ ] Port `wordwhitelist`
- [x] Port API read commands: `lastseen`, `msgcount`, `playtime`, `joins`
- [x] Port quote commands
- [ ] Port profanity/censor commands
- [ ] Port complex stats/history commands

## API And Bridge
- [x] Port `forestbot-api-wrapper-v2` equivalent
- [x] Port REST endpoint client scaffold for copied command behavior
- [x] Port API helpers for `convertUsernameToUuid`, `getStatsByUuid`, `getStatsByUsername`, `getLastSeen`, `getMessageCount`, `getPlaytime`, `getJoinCount`, `getQuote`, and related wrapper methods
- [x] Port WebSocket bridge
- [ ] Port chatbridge input/output behavior
- [ ] Port offline messages

## Moderation
- [ ] Port profanity filter
- [ ] Port secondary profanity filter
- [ ] Port smart AI censoring
- [ ] Port anti-spam cooldown/message limit
- [ ] Enforce whitelist/blacklist behavior

## Events And Behavior
- [x] Track tab-list players for ping lookup
- [x] Handle spawn/login/disconnect basics
- [ ] Port welcome messages
- [ ] Port anti-AFK
- [ ] Port disabled event config behavior
- [ ] Port player join/left behavior beyond tab cache
- [ ] Port pViewer equivalent or decide to drop it

## Tests
- [x] Add chat parser tests
- [ ] Add command handler tests
- [ ] Add config loading tests
- [ ] Add API client tests after API port
