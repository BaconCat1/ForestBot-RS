use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::{
    commands::{enqueue_chat, CommandContext, CommandDefinition, CommandFuture},
    structure::mineflayer::bot::PlayerSnapshot,
};

const TRADE_PROPOSE_COOLDOWN: Duration = Duration::from_secs(60);
const TRADE_REJECT_PENALTY_COOLDOWN: Duration = Duration::from_secs(600);

// ===== !trade [confirm | reject | <player> <desc>] =====

pub const TRADE_COMMAND: CommandDefinition = CommandDefinition {
    names: &["trade", "t"],
    whitelisted: false,
    execute: execute_trade,
};

pub fn execute_trade(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        match ctx.args.first().copied() {
            Some("confirm") | Some("c") => confirm_trade(&ctx).await,
            Some("reject") | Some("r") => reject_trade(&ctx).await,
            _ => propose_trade(&ctx).await,
        }
    })
}

async fn propose_trade(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    if ctx.args.len() < 2 {
        ctx.whisper("Usage: !trade <player> <description>  |  !trade confirm  |  !trade reject");
        return Ok(());
    }

    let recipient_name = ctx.args[0];
    let description = ctx.args[1..].join(" ");

    let sender_uuid = {
        let players = ctx.state.players.read().expect("player cache lock poisoned");
        resolve_online_uuid(ctx.sender, &players)
    };
    let sender_uuid = match sender_uuid {
        Some(u) => u,
        None => match ctx.state.api.convert_username_to_uuid(ctx.sender).await {
            Some(u) => u,
            None => {
                ctx.whisper("Could not resolve your UUID.");
                return Ok(());
            }
        },
    };

    {
        let cooldowns = ctx.state.trade_cooldowns.lock().expect("trade cooldown lock poisoned");
        if let Some(&expires) = cooldowns.get(&sender_uuid) {
            if Instant::now() < expires {
                let remaining = expires.duration_since(Instant::now()).as_secs() + 1;
                ctx.whisper(format!("Trade cooldown: {remaining}s remaining."));
                return Ok(());
            }
        }
    }

    let existing = ctx.state.api.tradebot_get_user_trades(&sender_uuid).await;
    if existing.iter().any(|t| t.status == "pending" && t.initiator_id == sender_uuid) {
        ctx.whisper("You already have a pending trade. Cancel it with !trade reject.");
        return Ok(());
    }

    let recipient_uuid = {
        let players = ctx.state.players.read().expect("player cache lock poisoned");
        resolve_online_uuid(recipient_name, &players)
    };
    let recipient_uuid = match recipient_uuid {
        Some(u) => u,
        None => match ctx.state.api.convert_username_to_uuid(recipient_name).await {
            Some(u) => u,
            None => {
                ctx.whisper(format!("Could not find player: {recipient_name}"));
                return Ok(());
            }
        },
    };

    if let Some(_s) = ctx.state.api.tradebot_get_scammer(&sender_uuid).await {
        ctx.chat(format!("🚨 {} is a known scammer, proceed with caution 🚨", ctx.sender));
    }
    if let Some(_s) = ctx.state.api.tradebot_get_scammer(&recipient_uuid).await {
        ctx.chat(format!("🚨 {recipient_name} is a known scammer, proceed with caution 🚨"));
    }

    let server = ctx.state.mc_server.clone();
    let Some(id) = ctx
        .state
        .api
        .tradebot_create_trade(&sender_uuid, &recipient_uuid, &description, &server)
        .await
    else {
        ctx.whisper("Failed to create trade. Try again later.");
        return Ok(());
    };

    {
        let mut cooldowns = ctx.state.trade_cooldowns.lock().expect("trade cooldown lock poisoned");
        cooldowns.insert(sender_uuid, Instant::now() + TRADE_PROPOSE_COOLDOWN);
    }

    ctx.whisper(format!("Trade #{id} proposed to {recipient_name}."));
    enqueue_chat(
        ctx.state,
        format!(
            "/msg {} {} has proposed a trade: \"{}\". Run !trade confirm or !trade reject.",
            recipient_name, ctx.sender, description
        ),
    );

    Ok(())
}

async fn confirm_trade(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let sender_uuid = {
        let players = ctx.state.players.read().expect("player cache lock poisoned");
        resolve_online_uuid(ctx.sender, &players)
    };
    let sender_uuid = match sender_uuid {
        Some(u) => u,
        None => match ctx.state.api.convert_username_to_uuid(ctx.sender).await {
            Some(u) => u,
            None => {
                ctx.whisper("Could not resolve your UUID.");
                return Ok(());
            }
        },
    };

    let trades = ctx.state.api.tradebot_get_user_trades(&sender_uuid).await;
    let Some(trade) = trades
        .iter()
        .find(|t| t.status == "pending" && t.recipient_id == sender_uuid)
    else {
        ctx.whisper("No pending trade addressed to you.");
        return Ok(());
    };
    let trade_id = trade.id;

    match ctx.state.api.tradebot_confirm_trade(trade_id).await {
        Ok(()) => enqueue_chat(ctx.state, format!("Trade #{trade_id} confirmed!")),
        Err(msg) => ctx.whisper(format!("Could not confirm: {msg}")),
    }

    Ok(())
}

async fn reject_trade(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let sender_uuid = {
        let players = ctx.state.players.read().expect("player cache lock poisoned");
        resolve_online_uuid(ctx.sender, &players)
    };
    let sender_uuid = match sender_uuid {
        Some(u) => u,
        None => match ctx.state.api.convert_username_to_uuid(ctx.sender).await {
            Some(u) => u,
            None => {
                ctx.whisper("Could not resolve your UUID.");
                return Ok(());
            }
        },
    };

    let trades = ctx.state.api.tradebot_get_user_trades(&sender_uuid).await;
    let Some(trade) = trades.iter().find(|t| {
        t.status == "pending"
            && (t.recipient_id == sender_uuid || t.initiator_id == sender_uuid)
    }) else {
        ctx.whisper("No pending trade to reject.");
        return Ok(());
    };

    let trade_id = trade.id;
    let is_recipient = trade.recipient_id == sender_uuid;
    let initiator_id = trade.initiator_id.clone();

    match ctx.state.api.tradebot_reject_trade(trade_id).await {
        Ok(()) => {
            enqueue_chat(ctx.state, format!("Trade #{trade_id} rejected."));
            if is_recipient {
                let mut cooldowns =
                    ctx.state.trade_cooldowns.lock().expect("trade cooldown lock poisoned");
                cooldowns.insert(initiator_id, Instant::now() + TRADE_REJECT_PENALTY_COOLDOWN);
            }
        }
        Err(msg) => ctx.whisper(format!("Could not reject: {msg}")),
    }

    Ok(())
}

// ===== !trades [player] =====

pub const TRADES_COMMAND: CommandDefinition = CommandDefinition {
    names: &["trades"],
    whitelisted: false,
    execute: execute_trades,
};

pub fn execute_trades(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let target = ctx.args.first().copied().unwrap_or(ctx.sender);

        let target_uuid = {
            let players = ctx.state.players.read().expect("player cache lock poisoned");
            resolve_online_uuid(target, &players)
        };
        let target_uuid = match target_uuid {
            Some(u) => u,
            None => match ctx.state.api.convert_username_to_uuid(target).await {
                Some(u) => u,
                None => {
                    ctx.whisper(format!("Could not find player: {target}"));
                    return Ok(());
                }
            },
        };

        if let Some(_s) = ctx.state.api.tradebot_get_scammer(&target_uuid).await {
            ctx.chat(format!("🚨 {target} is a known scammer, trade counts not reported 🚨"));
            return Ok(());
        }

        let trades = ctx.state.api.tradebot_get_user_trades(&target_uuid).await;

        if trades.is_empty() {
            ctx.whisper(format!("{target} has no trades."));
            return Ok(());
        }

        let lines: Vec<String> = {
            let players = ctx.state.players.read().expect("player cache lock poisoned");
            trades
                .iter()
                .rev()
                .take(3)
                .map(|t| {
                    let init_name = if t.initiator_id == target_uuid {
                        target.to_owned()
                    } else {
                        uuid_to_name(&t.initiator_id, &players)
                            .map(|s| s.to_owned())
                            .unwrap_or_else(|| t.initiator_id.chars().take(8).collect())
                    };
                    let recv_name = if t.recipient_id == target_uuid {
                        target.to_owned()
                    } else {
                        uuid_to_name(&t.recipient_id, &players)
                            .map(|s| s.to_owned())
                            .unwrap_or_else(|| t.recipient_id.chars().take(8).collect())
                    };
                    format!(
                        "#{} [{}] {} -> {} | {}",
                        t.id,
                        t.status,
                        init_name,
                        recv_name,
                        truncate(&t.description, 190)
                    )
                })
                .collect()
        };

        ctx.whisper(format!("Trades for {} ({} shown):", target, lines.len().min(3)));
        for line in &lines {
            ctx.whisper(line);
        }

        Ok(())
    })
}

// ===== !tradestats [player] =====

pub const TRADESTATS_COMMAND: CommandDefinition = CommandDefinition {
    names: &["tradestats"],
    whitelisted: false,
    execute: execute_tradestats,
};

pub fn execute_tradestats(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let full = ctx.args.iter().any(|a| a.eq_ignore_ascii_case("full"));
        let target = ctx.args.iter()
            .copied()
            .find(|a| !a.eq_ignore_ascii_case("full"))
            .unwrap_or(ctx.sender);

        let target_uuid = {
            let players = ctx.state.players.read().expect("player cache lock poisoned");
            resolve_online_uuid(target, &players)
        };
        let target_uuid = match target_uuid {
            Some(u) => u,
            None => match ctx.state.api.convert_username_to_uuid(target).await {
                Some(u) => u,
                None => {
                    ctx.whisper(format!("Could not find player: {target}"));
                    return Ok(());
                }
            },
        };

        let Some(data) = ctx.state.api.tradebot_get_stats(&target_uuid).await else {
            ctx.whisper(format!("No stats for {target}."));
            return Ok(());
        };

        let s = &data.stats;
        let scammer = data.scammer_status.as_ref().map_or(false, |v| !v.is_null());

        if scammer {
            ctx.chat(format!("🚨 {target} is a known scammer, trade counts not reported 🚨"));
            return Ok(());
        }

        if !full {
            ctx.chat(format!(
                "{target} | {} confirmed, {} rejected trades",
                s.confirmed_trades, s.rejected_trades
            ));
            return Ok(());
        }

        ctx.whisper(format!(
            "{target} | {} total, {} confirmed, {} rejected",
            s.total_trades, s.confirmed_trades, s.rejected_trades
        ));

        if !data.partners.is_empty() {
            let mut names = Vec::new();
            for p in &data.partners {
                let name = if p.partner_id.chars().all(|c| c.is_ascii_digit()) {
                    // Discord ID — try to resolve via linked MC account
                    if let Some(mc_uuid) = ctx.state.api.tradebot_linked_mc_uuid(&p.partner_id).await {
                        let online = {
                            let players = ctx.state.players.read().expect("player cache lock poisoned");
                            uuid_to_name(&mc_uuid, &players).map(str::to_owned)
                        };
                        if let Some(n) = online {
                            n
                        } else {
                            ctx.state.api.tradebot_mc_username(&mc_uuid).await
                                .unwrap_or_else(|| format!("@{}", p.partner_id))
                        }
                    } else {
                        ctx.state.api.tradebot_discord_username(&p.partner_id).await
                            .map(|name| format!("@{name}"))
                            .unwrap_or_else(|| format!("@{}", p.partner_id))
                    }
                } else {
                    let online = {
                        let players = ctx.state.players.read().expect("player cache lock poisoned");
                        uuid_to_name(&p.partner_id, &players).map(str::to_owned)
                    };
                    if let Some(n) = online {
                        n
                    } else {
                        ctx.state.api.tradebot_mc_username(&p.partner_id).await
                            .unwrap_or_else(|| p.partner_id.chars().take(8).collect())
                    }
                };
                names.push(name);
            }
            ctx.whisper(format!("Top partners: {}", names.join(", ")));
        }

        Ok(())
    })
}

// ===== Helpers =====

fn resolve_online_uuid(username: &str, players: &HashMap<String, PlayerSnapshot>) -> Option<String> {
    players.get(username).map(|p| p.uuid.clone())
}

fn uuid_to_name<'a>(uuid: &str, players: &'a HashMap<String, PlayerSnapshot>) -> Option<&'a str> {
    players
        .values()
        .find(|p| p.uuid == uuid)
        .map(|p| p.username.as_str())
}

// ===== !scammers =====

pub const SCAMMERS_COMMAND: CommandDefinition = CommandDefinition {
    names: &["scammers"],
    whitelisted: false,
    execute: execute_scammers,
};

pub fn execute_scammers(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(data) = ctx.state.api.get_scammers().await else {
            ctx.whisper("Api error");
            return Ok(());
        };

        let scammers = match data.get("scammers").and_then(|v| v.as_array()) {
            Some(s) if !s.is_empty() => s.clone(),
            _ => {
                ctx.whisper("No scammers on record.");
                return Ok(());
            }
        };

        let mut online: Vec<String> = Vec::new();
        let mut offline: Vec<String> = Vec::new();

        for scammer in &scammers {
            let Some(user_id) = scammer.get("user_id").and_then(|v| v.as_str()) else {
                continue;
            };
            let player_name = scammer
                .get("player_name")
                .and_then(|v| v.as_str())
                .unwrap_or(user_id)
                .to_owned();

            let is_online = if user_id.chars().all(|c| c.is_ascii_digit()) {
                match ctx.state.api.tradebot_linked_mc_uuid(user_id).await {
                    Some(mc_uuid) => {
                        let players = ctx.state.players.read().expect("player cache lock poisoned");
                        uuid_to_name(&mc_uuid, &players).is_some()
                    }
                    None => false,
                }
            } else {
                let players = ctx.state.players.read().expect("player cache lock poisoned");
                uuid_to_name(user_id, &players).is_some()
            };

            if is_online {
                online.push(format!("{player_name} (online)"));
            } else {
                offline.push(player_name);
            }
        }

        let mut result = online;
        let remaining = 5usize.saturating_sub(result.len());
        result.extend(offline.into_iter().take(remaining));

        ctx.chat(format!(" [SCAMMERS]: {}", result.join(", ")));
        Ok(())
    })
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        format!("{}...", s.chars().take(max.saturating_sub(3)).collect::<String>())
    }
}
