use crate::commands::{enqueue_chat, CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::market::types::{
    fmt_price, format_remaining, now_unix, parse_duration, Direction, MarketBet,
    PortfolioPosition,
};
use crate::structure::mineflayer::bot::AzaleaState;

use super::casino::chips_str;

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["market", "stock", "stocks", "crypto", "stonk", "stonks"],
    description: "Market data + bets. !market <sym> | history <sym> [1d/7d/30d/1y] | search <q> | long/short <sym> <chips> <dur> | bets | buy/sell <sym> [chips] | portfolio",
    whitelisted: false,
    execute,
};

pub const PORTFOLIO_COMMAND: CommandDefinition = CommandDefinition {
    names: &["portfolio", "port"],
    description: "Show your open portfolio positions with live P&L. Usage: {prefix}portfolio [player]",
    whitelisted: false,
    execute: portfolio_execute,
};

const MIN_BET: i64 = 50;

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let sub = ctx.args.first().copied().unwrap_or("");

        match sub.to_ascii_lowercase().as_str() {
            "" => {
                ctx.whisper("Usage: !market <symbol> | history <sym> [1d/7d] | search <q> | long/short <sym> <chips> <dur> | bets");
            }
            "history" => {
                let sym = match ctx.args.get(1).copied() {
                    Some(s) => s,
                    None => { ctx.whisper("Usage: !market history <symbol> [1d/7d/30d/1y]"); return Ok(()); }
                };
                let period = ctx.args.get(2).copied().unwrap_or("7d");
                let days = period_to_days(period).unwrap_or(7);
                match ctx.state.market_service.history(sym, days).await {
                    Ok(candles) => ctx.whisper_success(format_history(sym, period, &candles)),
                    Err(e) => ctx.whisper_error(format!("No market data for {}: {}", sym, e)),
                }
            }
            "search" => {
                let query = ctx.args[1..].join(" ");
                if query.is_empty() {
                    ctx.whisper("Usage: !market search <query>");
                    return Ok(());
                }
                match ctx.state.market_service.search(&query).await {
                    Ok(results) if results.is_empty() => ctx.whisper("No results."),
                    Ok(results) => {
                        let line = results.iter()
                            .map(|a| format!("{} ({})", a.symbol, a.name))
                            .collect::<Vec<_>>()
                            .join(" | ");
                        ctx.whisper_success(line);
                    }
                    Err(_) => ctx.whisper("Search unavailable."),
                }
            }
            "long" | "short" => {
                place_bet(&ctx, sub == "long" || sub == "l").await?;
            }
            "bets" | "positions" | "pos" => {
                let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
                    ctx.whisper("Could not resolve your UUID.");
                    return Ok(());
                };
                show_bets(&ctx, &player_uuid);
            }
            "cashout" | "close" | "exit" => {
                cashout(&ctx).await?;
            }
            "buy" => {
                portfolio_buy(&ctx).await?;
            }
            "sell" => {
                portfolio_sell(&ctx).await?;
            }
            "portfolio" | "port" => {
                let target = ctx.args.get(1).copied().unwrap_or(ctx.sender);
                let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(target).await else {
                    ctx.whisper_error(format!("Could not resolve UUID for {}.", target));
                    return Ok(());
                };
                show_portfolio(&ctx, &player_uuid).await?;
            }
            // Fallthrough: treat as symbol lookup
            sym => {
                match ctx.state.market_service.quote(sym).await {
                    Ok(q) => {
                        let sign = if q.change_pct >= 0.0 { "+" } else { "" };
                        ctx.whisper_success(format!(
                            "{}: {} ({}{:.2}%) | {}",
                            q.symbol, fmt_price(q.price), sign, q.change_pct, q.name
                        ));
                    }
                    Err(e) => ctx.whisper_error(format!("No market data for {}: {}", sym, e)),
                }
            }
        }

        Ok(())
    })
}

async fn place_bet(ctx: &CommandContext<'_>, long: bool) -> anyhow::Result<()> {
    // !market long <symbol> <chips> <duration>
    let sym = match ctx.args.get(1).copied() {
        Some(s) => s.to_uppercase(),
        None => {
            ctx.whisper("Usage: !market long/short <symbol> <chips> <duration: 1m/15m/1h/4h/1d>");
            return Ok(());
        }
    };
    let stake: i64 = match ctx.args.get(2).and_then(|s| s.parse().ok()) {
        Some(n) => n,
        None => {
            ctx.whisper("Chip amount must be a number.");
            return Ok(());
        }
    };
    if stake < MIN_BET {
        ctx.whisper(format!("Min stake is {}.", chips_str(MIN_BET)));
        return Ok(());
    }
    let dur_str = match ctx.args.get(3).copied() {
        Some(s) => s,
        None => {
            ctx.whisper("Specify duration: 1m, 15m, 1h, 4h, 1d");
            return Ok(());
        }
    };
    let dur_secs = match parse_duration(dur_str) {
        Some(d) => d,
        None => {
            ctx.whisper("Invalid duration. Use: 1m, 15m, 1h, 4h, 1d (min 1m, max 1d).");
            return Ok(());
        }
    };

    // Fetch entry price
    let quote = match ctx.state.market_service.quote(&sym).await {
        Ok(q) => q,
        Err(_) => {
            ctx.whisper_error(format!("No market data for {} — bet not placed.", sym));
            return Ok(());
        }
    };

    let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
        ctx.whisper("Could not resolve your UUID.");
        return Ok(());
    };

    // Deduct chips
    match ctx.state.api.casino_adjust(&player_uuid, -stake).await {
        Ok(_) => {}
        Err(CasinoAdjustErr::InsufficientFunds(have)) => {
            ctx.whisper(format!("Need {} but have {}.", chips_str(stake), chips_str(have)));
            return Ok(());
        }
        Err(CasinoAdjustErr::NetworkErr) => {
            ctx.whisper("Casino unavailable.");
            return Ok(());
        }
    }

    let direction = if long { Direction::Long } else { Direction::Short };
    let mut bet = MarketBet {
        id: 0i64,
        player: player_uuid.clone(),
        symbol: sym.clone(),
        market: quote.market,
        direction,
        entry_price: quote.price,
        stake,
        closes_unix: now_unix() + dur_secs,
        duration_label: dur_str.to_owned(),
    };

    match ctx.state.api.casino_market_bet_insert(&bet).await {
        Some(id) => { bet.id = id; }
        None => {
            ctx.whisper("Failed to save bet. Refunding chips.");
            let _ = ctx.state.api.casino_adjust(&player_uuid, stake).await;
            return Ok(());
        }
    }
    {
        let mut bets = ctx.state.market_bets.lock().expect("market bets lock");
        bets.entry(player_uuid.clone()).or_default().push(bet.clone());
    }

    ctx.whisper_success(format!(
        "{} {} @{} | {} chips | settles in {} | !market bets to check",
        direction.label(), sym, fmt_price(quote.price), chips_str(stake), dur_str
    ));

    // Spawn settlement task
    let state = ctx.state.clone();
    let player = player_uuid.clone();
    let whisper_cmd = state.runtime.read().expect("runtime lock").whisper_command.clone();
    tokio::spawn(settle_task(state, player, whisper_cmd, bet, dur_secs));

    Ok(())
}

async fn cashout(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
        ctx.whisper("Could not resolve your UUID.");
        return Ok(());
    };

    // !market cashout [index]   — index is 1-based, matches !market bets output
    let bet = {
        let bets = ctx.state.market_bets.lock().expect("market bets lock");
        let player_bets = match bets.get(&player_uuid) {
            Some(v) if !v.is_empty() => v,
            _ => { ctx.whisper("No open market bets."); return Ok(()); }
        };
        if player_bets.len() == 1 {
            player_bets[0].clone()
        } else {
            let idx: usize = match ctx.args.get(1).and_then(|s| s.parse::<usize>().ok()) {
                Some(n) if n >= 1 && n <= player_bets.len() => n - 1,
                _ => {
                    ctx.whisper(format!(
                        "Specify index 1-{}. Use !market bets to list.",
                        player_bets.len()
                    ));
                    return Ok(());
                }
            };
            player_bets[idx].clone()
        }
    };

    let exit_price = match ctx.state.market_service.quote(&bet.symbol).await {
        Ok(q) => q.price,
        Err(_) => {
            ctx.whisper_error(format!("Market data unavailable for {} — try again.", bet.symbol));
            return Ok(());
        }
    };

    let ratio = match bet.direction {
        Direction::Long  => exit_price / bet.entry_price,
        Direction::Short => bet.entry_price / exit_price,
    };
    let payout = (bet.stake as f64 * ratio).ceil() as i64;
    let payout = payout.max(0);
    let net = payout - bet.stake;

    // Remove from state first so the settle_task finds nothing and exits cleanly.
    remove_bet(&ctx.state, &player_uuid, bet.id);
    ctx.state.api.casino_market_bet_delete(bet.id).await;

    let mut alimony_note = String::new();
    if payout > bet.stake {
        let win = ctx.state.api.casino_win(&player_uuid, payout).await.unwrap_or_default();
        if win.alimony_paid > 0 {
            alimony_note = format!(" (-{} alimony)", chips_str(win.alimony_paid));
        }
    } else if payout > 0 {
        let _ = ctx.state.api.casino_adjust(&player_uuid, payout).await;
    }

    let pct = (exit_price - bet.entry_price) / bet.entry_price * 100.0;
    let sign = if pct >= 0.0 { "+" } else { "" };
    let (result_str, net_str) = if net > 0 {
        ("WIN", format!("+{}", chips_str(net)))
    } else if net == 0 {
        ("BREAK EVEN", "net 0".to_owned())
    } else {
        ("LOSS", format!("-{}", chips_str(net.abs())))
    };

    ctx.whisper_success(format!(
        "Cashed out {} {} | {} | {}→{} ({}{:.2}%) | {}{alimony_note}",
        bet.direction.label(), bet.symbol, result_str,
        fmt_price(bet.entry_price), fmt_price(exit_price), sign, pct, net_str
    ));

    Ok(())
}

pub async fn settle_task(
    state: AzaleaState,
    player: String,
    whisper_cmd: String,
    bet: MarketBet,
    dur_secs: u64,
) {
    tokio::time::sleep(std::time::Duration::from_secs(dur_secs)).await;

    // Bail if bet was already cashed out manually while we slept.
    {
        let bets = state.market_bets.lock().expect("market bets lock");
        let still_open = bets.get(&player)
            .map(|v| v.iter().any(|b| b.id == bet.id))
            .unwrap_or(false);
        if !still_open {
            return;
        }
    }

    // If the bet closed while the bot was down, use historical price at the close time.
    let age_secs = now_unix().saturating_sub(bet.closes_unix);
    let exit_price_result = if age_secs > 60 {
        match state.market_service.price_at(&bet.symbol, bet.closes_unix).await {
            Ok(p) => Ok(p),
            Err(_) => state.market_service.quote(&bet.symbol).await.map(|q| q.price),
        }
    } else {
        state.market_service.quote(&bet.symbol).await.map(|q| q.price)
    };

    let exit_price = match exit_price_result {
        Ok(p) => p,
        Err(_) => {
            let _ = state.api.casino_adjust(&player, bet.stake).await;
            remove_bet(&state, &player, bet.id);
            state.api.casino_market_bet_delete(bet.id).await;
            let username_for_msg = state.players.read().ok()
                .and_then(|pl| pl.values().find(|s| s.uuid == player).map(|s| s.username.clone()))
                .unwrap_or_else(|| player.clone());
            enqueue_chat(&state, format!(
                "/{whisper_cmd} {username_for_msg} Market data unavailable — {} {} bet refunded. +{}",
                bet.direction.label(), bet.symbol, chips_str(bet.stake)
            ));
            return;
        }
    };

    let pct = (exit_price - bet.entry_price) / bet.entry_price * 100.0;

    // Proportional paper-trading payout: ceil(stake * exit/entry) for longs,
    // ceil(stake * entry/exit) for shorts. Ceiling rounds in player's favour.
    let ratio = match bet.direction {
        Direction::Long  => exit_price / bet.entry_price,
        Direction::Short => bet.entry_price / exit_price,
    };
    let payout = (bet.stake as f64 * ratio).ceil() as i64;
    let payout = payout.max(0);
    let net = payout - bet.stake;

    remove_bet(&state, &player, bet.id);
    state.api.casino_market_bet_delete(bet.id).await;

    let mut alimony_note = String::new();
    if payout > bet.stake {
        let win = state.api.casino_win(&player, payout).await.unwrap_or_default();
        if win.alimony_paid > 0 {
            alimony_note = format!(" (-{} alimony)", chips_str(win.alimony_paid));
        }
    } else if payout > 0 {
        let _ = state.api.casino_adjust(&player, payout).await;
    }

    let (result_str, net_str) = if net > 0 {
        ("WIN", format!("+{}", chips_str(net)))
    } else if net == 0 {
        ("BREAK EVEN", "net 0".to_owned())
    } else {
        ("LOSS", format!("-{}", chips_str(net.abs())))
    };

    let sign = if pct >= 0.0 { "+" } else { "" };
    let username_for_msg = state.players.read().ok()
        .and_then(|pl| pl.values().find(|s| s.uuid == player).map(|s| s.username.clone()))
        .unwrap_or_else(|| player.clone());
    enqueue_chat(&state, format!(
        "/{whisper_cmd} {username_for_msg} {} {} settled: {} | {}→{} ({}{:.2}%) | {}{alimony_note}",
        bet.direction.label(), bet.symbol, result_str,
        fmt_price(bet.entry_price), fmt_price(exit_price), sign, pct, net_str
    ));
}

fn portfolio_execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let target = ctx.args.first().copied().unwrap_or(ctx.sender);
        let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(target).await else {
            ctx.whisper_error(format!("Could not resolve UUID for {}.", target));
            return Ok(());
        };
        show_portfolio(&ctx, &player_uuid).await
    })
}

async fn portfolio_buy(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    // !market buy <symbol> <chips>
    let sym = match ctx.args.get(1).copied() {
        Some(s) => s.to_uppercase(),
        None => { ctx.whisper("Usage: !market buy <symbol> <chips>"); return Ok(()); }
    };
    let stake: i64 = match ctx.args.get(2).and_then(|s| s.parse().ok()) {
        Some(n) => n,
        None => { ctx.whisper("Usage: !market buy <symbol> <chips>"); return Ok(()); }
    };
    if stake < MIN_BET {
        ctx.whisper(format!("Min stake is {}.", chips_str(MIN_BET)));
        return Ok(());
    }

    let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
        ctx.whisper("Could not resolve your UUID.");
        return Ok(());
    };

    // One position per symbol per player
    {
        let positions = ctx.state.portfolio_positions.lock().expect("portfolio lock");
        if let Some(v) = positions.get(&player_uuid) {
            if v.iter().any(|p| p.symbol == sym) {
                ctx.whisper_error(format!("Already have a {} position. Use !market sell {} to close it first.", sym, sym));
                return Ok(());
            }
        }
    }

    let quote = match ctx.state.market_service.quote(&sym).await {
        Ok(q) => q,
        Err(_) => { ctx.whisper_error(format!("No market data for {} — position not opened.", sym)); return Ok(()); }
    };

    match ctx.state.api.casino_adjust(&player_uuid, -stake).await {
        Ok(_) => {}
        Err(CasinoAdjustErr::InsufficientFunds(have)) => {
            ctx.whisper(format!("Need {} but have {}.", chips_str(stake), chips_str(have)));
            return Ok(());
        }
        Err(CasinoAdjustErr::NetworkErr) => { ctx.whisper("Casino unavailable."); return Ok(()); }
    }

    let mut pos = PortfolioPosition {
        id: 0i64,
        player: player_uuid.clone(),
        symbol: sym.clone(),
        market: quote.market,
        entry_price: quote.price,
        stake,
        opened_unix: now_unix(),
    };

    match ctx.state.api.casino_portfolio_insert(&pos).await {
        Some(id) => { pos.id = id; }
        None => {
            ctx.whisper("Failed to save position. Refunding chips.");
            let _ = ctx.state.api.casino_adjust(&player_uuid, stake).await;
            return Ok(());
        }
    }
    {
        let mut positions = ctx.state.portfolio_positions.lock().expect("portfolio lock");
        positions.entry(player_uuid.clone()).or_default().push(pos.clone());
    }

    ctx.whisper_success(format!(
        "Opened {} position @{} | {} chips invested | use !market sell {} to close",
        sym, fmt_price(quote.price), chips_str(stake), sym
    ));
    Ok(())
}

async fn portfolio_sell(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let arg = match ctx.args.get(1).copied() {
        Some(s) => s,
        None => { ctx.whisper("Usage: !market sell <symbol> | sell all"); return Ok(()); }
    };

    if arg.eq_ignore_ascii_case("all") {
        return portfolio_sell_all(ctx).await;
    }

    let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
        ctx.whisper("Could not resolve your UUID.");
        return Ok(());
    };

    // !market sell <symbol>
    let sym = arg.to_uppercase();

    let pos = {
        let positions = ctx.state.portfolio_positions.lock().expect("portfolio lock");
        positions.get(&player_uuid)
            .and_then(|v| v.iter().find(|p| p.symbol == sym).cloned())
    };

    let pos = match pos {
        Some(p) => p,
        None => { ctx.whisper_error(format!("No {} position in portfolio.", sym)); return Ok(()); }
    };

    let exit_price = match ctx.state.market_service.quote(&sym).await {
        Ok(q) => q.price,
        Err(_) => { ctx.whisper_error(format!("Market data unavailable for {} — try again.", sym)); return Ok(()); }
    };

    let payout = (pos.stake as f64 * exit_price / pos.entry_price).ceil() as i64;
    let payout = payout.max(0);
    let net = payout - pos.stake;

    // Remove from state first, then DB, then pay out
    {
        let mut positions = ctx.state.portfolio_positions.lock().expect("portfolio lock");
        if let Some(v) = positions.get_mut(&player_uuid) {
            v.retain(|p| p.id != pos.id);
        }
    }
    ctx.state.api.casino_portfolio_delete(pos.id).await;
    let mut alimony_note = String::new();
    if payout > pos.stake {
        let win = ctx.state.api.casino_win(&player_uuid, payout).await.unwrap_or_default();
        if win.alimony_paid > 0 {
            alimony_note = format!(" (-{} alimony)", chips_str(win.alimony_paid));
        }
    } else if payout > 0 {
        let _ = ctx.state.api.casino_adjust(&player_uuid, payout).await;
    }

    let pct = (exit_price - pos.entry_price) / pos.entry_price * 100.0;
    let sign = if pct >= 0.0 { "+" } else { "" };
    let (result_str, net_str) = if net > 0 {
        ("WIN", format!("+{}", chips_str(net)))
    } else if net == 0 {
        ("BREAK EVEN", "net 0".to_owned())
    } else {
        ("LOSS", format!("-{}", chips_str(net.abs())))
    };

    ctx.whisper_success(format!(
        "Closed {} | {} | {}→{} ({}{:.2}%) | {}{alimony_note}",
        sym, result_str,
        fmt_price(pos.entry_price), fmt_price(exit_price), sign, pct, net_str
    ));
    Ok(())
}

async fn portfolio_sell_all(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let Some(player_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
        ctx.whisper("Could not resolve your UUID.");
        return Ok(());
    };

    let positions = {
        let map = ctx.state.portfolio_positions.lock().expect("portfolio lock");
        match map.get(&player_uuid) {
            Some(v) if !v.is_empty() => v.clone(),
            _ => { ctx.whisper("No open portfolio positions."); return Ok(()); }
        }
    };

    let quote_futures: Vec<_> = positions.iter()
        .map(|p| ctx.state.market_service.quote(&p.symbol))
        .collect();
    let quotes = futures_util::future::join_all(quote_futures).await;

    let mut total_invested = 0i64;
    let mut total_payout = 0i64;
    let mut total_alimony = 0i64;
    let mut quote_failures = 0usize;

    for (pos, quote_result) in positions.iter().zip(quotes.iter()) {
        let exit_price = match quote_result {
            Ok(q) => q.price,
            Err(_) => { quote_failures += 1; pos.entry_price }
        };
        let payout = ((pos.stake as f64 * exit_price / pos.entry_price).ceil() as i64).max(0);
        total_invested += pos.stake;
        total_payout += payout;

        {
            let mut positions_map = ctx.state.portfolio_positions.lock().expect("portfolio lock");
            if let Some(v) = positions_map.get_mut(&player_uuid) {
                v.retain(|p| p.id != pos.id);
            }
        }
        ctx.state.api.casino_portfolio_delete(pos.id).await;
        if payout > pos.stake {
            let win = ctx.state.api.casino_win(&player_uuid, payout).await.unwrap_or_default();
            total_alimony += win.alimony_paid;
        } else if payout > 0 {
            let _ = ctx.state.api.casino_adjust(&player_uuid, payout).await;
        }
    }

    let net = total_payout - total_invested;
    let net_str = if net >= 0 {
        format!("+{}", chips_str(net))
    } else {
        format!("-{}", chips_str(net.abs()))
    };
    let caveat = if quote_failures > 0 {
        format!(" ({} price unavailable, refunded at cost)", quote_failures)
    } else {
        String::new()
    };
    let alimony_note = if total_alimony > 0 {
        format!(" (-{} alimony)", chips_str(total_alimony))
    } else {
        String::new()
    };

    ctx.whisper_success(format!(
        "Closed {} positions | Returned: {} | Net: {}{}{alimony_note}",
        positions.len(), chips_str(total_payout), net_str, caveat
    ));
    Ok(())
}

async fn show_portfolio(ctx: &CommandContext<'_>, player_uuid: &str) -> anyhow::Result<()> {
    let positions = {
        let map = ctx.state.portfolio_positions.lock().expect("portfolio lock");
        match map.get(player_uuid) {
            Some(v) if !v.is_empty() => v.clone(),
            _ => { ctx.whisper("No open portfolio positions. Use !market buy <sym> <chips>."); return Ok(()); }
        }
    };

    // Fetch all quotes in parallel
    let quote_futures: Vec<_> = positions.iter()
        .map(|p| ctx.state.market_service.quote(&p.symbol))
        .collect();
    let quotes = futures_util::future::join_all(quote_futures).await;

    let mut total_invested = 0i64;
    let mut total_value = 0i64;

    for (i, (pos, quote_result)) in positions.iter().zip(quotes.iter()).enumerate() {
        total_invested += pos.stake;
        match quote_result {
            Ok(q) => {
                let value = (pos.stake as f64 * q.price / pos.entry_price).ceil() as i64;
                total_value += value;
                let pct = (q.price - pos.entry_price) / pos.entry_price * 100.0;
                let sign = if pct >= 0.0 { "+" } else { "" };
                let net = value - pos.stake;
                let net_str = if net >= 0 { format!("+{}", chips_str(net)) } else { format!("-{}", chips_str(net.abs())) };
                ctx.whisper_success(format!(
                    "{}. {} @{} → {} ({}{:.2}%) | {} → {} chips | {}",
                    i + 1, pos.symbol,
                    fmt_price(pos.entry_price), fmt_price(q.price), sign, pct,
                    chips_str(pos.stake), chips_str(value), net_str
                ));
            }
            Err(_) => {
                total_value += pos.stake;
                ctx.whisper_success(format!(
                    "{}. {} @{} | {} chips | (price unavailable)",
                    i + 1, pos.symbol, fmt_price(pos.entry_price), chips_str(pos.stake)
                ));
            }
        }
    }

    let net_total = total_value - total_invested;
    let net_str = if net_total >= 0 {
        format!("+{}", chips_str(net_total))
    } else {
        format!("-{}", chips_str(net_total.abs()))
    };
    let event_bet_count = crate::commands::casino::count_event_bets(ctx.state, player_uuid);
    let bets_suffix = if event_bet_count > 0 {
        format!(" | {} event bet{} (!bets)", event_bet_count, if event_bet_count == 1 { "" } else { "s" })
    } else {
        String::new()
    };
    ctx.whisper_success(format!(
        "Total: {} positions | Invested: {} | Value: {} | Net: {}{}",
        positions.len(), chips_str(total_invested), chips_str(total_value), net_str, bets_suffix
    ));
    Ok(())
}

fn remove_bet(state: &AzaleaState, player: &str, id: i64) {
    if let Ok(mut bets) = state.market_bets.lock() {
        if let Some(v) = bets.get_mut(player) {
            v.retain(|b| b.id != id);
        }
    }
}

fn show_bets(ctx: &CommandContext, player_uuid: &str) {
    let bets = ctx.state.market_bets.lock().expect("market bets lock");
    let player_bets = match bets.get(player_uuid) {
        Some(v) if !v.is_empty() => v,
        _ => { ctx.whisper("No open market bets."); return; }
    };
    let now = now_unix();
    for (i, b) in player_bets.iter().enumerate() {
        let remaining = b.closes_unix.saturating_sub(now);
        ctx.whisper_success(format!(
            "{}. {} {} @{} | {} | closes in {}",
            i + 1, b.direction.label(), b.symbol,
            fmt_price(b.entry_price), chips_str(b.stake),
            format_remaining(remaining)
        ));
    }
}

fn format_history(sym: &str, period: &str, candles: &[crate::structure::market::types::Candle]) -> String {
    let first_open  = candles.first().map(|c| c.open).unwrap_or(0.0);
    let last_close  = candles.last().map(|c| c.close).unwrap_or(0.0);
    let period_high = candles.iter().map(|c| c.high).fold(f64::NEG_INFINITY, f64::max);
    let period_low  = candles.iter().map(|c| c.low).fold(f64::INFINITY, f64::min);
    let pct = if first_open != 0.0 { (last_close - first_open) / first_open * 100.0 } else { 0.0 };
    let sign = if pct >= 0.0 { "+" } else { "" };
    format!(
        "{} ({}): {} → {} ({}{:.2}%) | H: {} L: {}",
        sym.to_uppercase(), period,
        fmt_price(first_open), fmt_price(last_close), sign, pct,
        fmt_price(period_high), fmt_price(period_low)
    )
}

fn period_to_days(period: &str) -> Option<u32> {
    match period.to_ascii_lowercase().as_str() {
        "1d" => Some(1),
        "7d" => Some(7),
        "30d" | "1mo" => Some(30),
        "1y" => Some(365),
        _ => None,
    }
}
