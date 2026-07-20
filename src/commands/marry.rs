use std::time::{Duration, Instant};
use crate::commands::{enqueue_chat, CommandContext, CommandDefinition, CommandFuture};
use crate::commands::casino::{chips_str, deliver};
use crate::structure::endpoints::endpoints::DivorceResult;

pub const MARRY_COMMAND: CommandDefinition = CommandDefinition {
    names: &["marry"],
    description: "Marriage: !marry <player> | !marry preview | !marry dowry <amount> | !marry accept | !marry reject",
    whitelisted: false,
    execute: marry_execute,
};

pub const DIVORCE_COMMAND: CommandDefinition = CommandDefinition {
    names: &["divorce"],
    description: "Divorce: !divorce <player> (mutual) | !divorce force <player> | !divorce force confirm",
    whitelisted: false,
    execute: divorce_execute,
};

pub const SPOUSE_COMMAND: CommandDefinition = CommandDefinition {
    names: &["spouse", "spouses"],
    description: "List your current spouses",
    whitelisted: false,
    execute: spouse_execute,
};

fn marry_execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(sender_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
            ctx.whisper_success("Could not resolve your UUID.");
            return Ok(());
        };

        let sub = ctx.args.first().copied().unwrap_or("");

        match sub {
            "preview" => {
                let proposal = ctx.state.api.marry_get_proposal(&sender_uuid).await;
                match proposal {
                    None => ctx.whisper_success("No pending marriage proposal."),
                    Some(p) => {
                        if p.role == "proposer" {
                            let target_name = ctx.state.api.tradebot_mc_username(&p.target_uuid)
                                .await.unwrap_or_else(|| p.target_uuid.clone());
                            if p.state == "counter" {
                                ctx.whisper_success(format!(
                                    "Proposed to {target_name}. They demanded a dowry of {} chips. !marry accept to pay, !marry reject to decline.",
                                    chips_str(p.dowry)
                                ));
                            } else {
                                ctx.whisper_success(format!("Proposed to {target_name}. Waiting for response."));
                            }
                        } else {
                            let proposer_name = ctx.state.api.tradebot_mc_username(&p.proposer_uuid)
                                .await.unwrap_or_else(|| p.proposer_uuid.clone());
                            if p.state == "counter" {
                                ctx.whisper_success(format!(
                                    "{proposer_name} proposed to you. You demanded {} chips dowry. Waiting for them to accept.",
                                    chips_str(p.dowry)
                                ));
                            } else {
                                ctx.whisper_success(format!(
                                    "{proposer_name} has proposed to you! !marry accept or demand a dowry with !marry dowry <amount>."
                                ));
                            }
                        }
                    }
                }
            }
            "dowry" => {
                let Some(amount_str) = ctx.args.get(1) else {
                    ctx.whisper_success("Usage: !marry dowry <amount>");
                    return Ok(());
                };
                let Ok(amount) = amount_str.parse::<i64>() else {
                    ctx.whisper_success("Amount must be a number.");
                    return Ok(());
                };
                if amount < 1 {
                    ctx.whisper_success("Dowry must be at least 1 chip.");
                    return Ok(());
                }
                match ctx.state.api.marry_dowry(&sender_uuid, amount).await {
                    Ok(proposer_uuid) => {
                        let name = ctx.state.api.tradebot_mc_username(&proposer_uuid)
                            .await.unwrap_or_else(|| proposer_uuid.clone());
                        ctx.whisper_success(format!(
                            "Demanded {} chips from {name}. They can !marry accept to pay or !marry reject to walk away.",
                            chips_str(amount)
                        ));
                    }
                    Err(e) if e == "no_proposal" => ctx.whisper_success("No one has proposed to you."),
                    Err(e) if e == "Dowry must be at least 1" => ctx.whisper_success("Dowry must be at least 1 chip."),
                    Err(_) => ctx.whisper_success("Could not set dowry. Try again."),
                }
            }
            "accept" => {
                match ctx.state.api.marry_accept(&sender_uuid).await {
                    Ok(result) => {
                        let p1 = ctx.state.api.tradebot_mc_username(&result.proposer_uuid)
                            .await.unwrap_or_else(|| result.proposer_uuid.clone());
                        let p2 = ctx.state.api.tradebot_mc_username(&result.target_uuid)
                            .await.unwrap_or_else(|| result.target_uuid.clone());
                        enqueue_chat(ctx.state, format!("\u{1F48D} {p1} and {p2} are now married! Congratulations!"));
                        if result.dowry_paid > 0 {
                            ctx.whisper_success(format!("Paid a dowry of {}.", chips_str(result.dowry_paid)));
                        }
                    }
                    Err(e) if e == "no_proposal" => ctx.whisper_success("No pending proposal to accept."),
                    Err(e) if e == "insufficient_funds" => ctx.whisper_success("Not enough chips to pay the dowry."),
                    Err(_) => ctx.whisper_success("Could not accept proposal. Try again."),
                }
            }
            "reject" => {
                match ctx.state.api.marry_reject(&sender_uuid).await {
                    Ok(()) => ctx.whisper_success("Proposal declined."),
                    Err(e) if e == "no_proposal" => ctx.whisper_success("No pending proposal to reject."),
                    Err(_) => ctx.whisper_success("Could not reject proposal. Try again."),
                }
            }
            "" => {
                ctx.whisper_success("Usage: !marry <player> | !marry preview | !marry dowry <amount> | !marry accept | !marry reject");
            }
            target_name => {
                let Some(target_uuid) = ctx.state.api.convert_username_to_uuid(target_name).await else {
                    ctx.whisper_error(format!("Player {target_name} not found."));
                    return Ok(());
                };
                match ctx.state.api.marry_propose(&sender_uuid, &target_uuid).await {
                    Ok(()) => {
                        ctx.whisper_success(format!(
                            "Proposed to {target_name}! They can !marry accept or demand a dowry with !marry dowry <amount>."
                        ));
                        deliver(
                            ctx.state,
                            &ctx.runtime.whisper_command,
                            &target_uuid,
                            format!(
                                "{} has proposed marriage to you! !marry accept to accept, !marry dowry <amount> to demand a dowry first, or !marry reject to decline.",
                                ctx.sender
                            ),
                        ).await;
                    }
                    Err(e) if e == "already_married" => {
                        ctx.whisper_success(format!("You are already married to {target_name}."));
                    }
                    Err(e) if e == "already_proposed" => {
                        ctx.whisper_success("You already have a pending proposal. Use !marry preview.");
                    }
                    Err(e) if e.contains("target_already_proposed_to_you") => {
                        ctx.whisper_success(format!("{target_name} has already proposed to you! Use !marry accept."));
                    }
                    Err(e) if e == "Cannot propose to yourself" => {
                        ctx.whisper_success("You cannot propose to yourself.");
                    }
                    Err(_) => ctx.whisper_success("Could not send proposal. Try again."),
                }
            }
        }

        Ok(())
    })
}

fn divorce_execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(sender_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
            ctx.whisper_success("Could not resolve your UUID.");
            return Ok(());
        };

        let sub = ctx.args.first().copied().unwrap_or("");

        if sub == "force" {
            let arg2 = ctx.args.get(1).copied().unwrap_or("");

            if arg2 == "confirm" {
                // Second step: execute force divorce
                let pending = {
                    let map = ctx.state.pending_force_divorces.lock().expect("pending_force_divorces lock poisoned");
                    map.get(&sender_uuid).cloned()
                };
                match pending {
                    None => {
                        ctx.whisper_success("No pending force-divorce. Use !divorce force <player> first.");
                    }
                    Some((partner_uuid, started_at)) => {
                        if started_at.elapsed() > Duration::from_secs(ctx.runtime.marry_confirm_window_secs) {
                            ctx.state.pending_force_divorces.lock().expect("pending_force_divorces lock poisoned").remove(&sender_uuid);
                            ctx.whisper_success("Confirmation window expired. Use !divorce force <player> again.");
                            return Ok(());
                        }
                        ctx.state.pending_force_divorces.lock().expect("pending_force_divorces lock poisoned").remove(&sender_uuid);
                        match ctx.state.api.marry_divorce_force(&sender_uuid, &partner_uuid).await {
                            Ok(result) => {
                                let your_name = ctx.sender.to_owned();
                                let their_name = ctx.state.api.tradebot_mc_username(&result.partner_uuid)
                                    .await.unwrap_or_else(|| result.partner_uuid.clone());
                                enqueue_chat(ctx.state, format!(
                                    "\u{1F494} {your_name} has force-divorced {their_name}. Alimony owed for {} days.",
                                    result.alimony_days
                                ));
                                ctx.whisper_success(format!(
                                    "Divorced {their_name}. You will pay alimony for {} days.",
                                    result.alimony_days
                                ));
                            }
                            Err(e) if e == "not_married" => {
                                ctx.whisper_success("You are not married to that player.");
                            }
                            Err(_) => ctx.whisper_success("Divorce failed. Try again."),
                        }
                    }
                }
            } else if arg2.is_empty() {
                ctx.whisper_success("Usage: !divorce force <player>");
            } else {
                // First step: set pending
                let target_name = arg2;
                let Some(target_uuid) = ctx.state.api.convert_username_to_uuid(target_name).await else {
                    ctx.whisper_error(format!("Player {target_name} not found."));
                    return Ok(());
                };
                ctx.state.pending_force_divorces.lock().expect("pending_force_divorces lock poisoned")
                    .insert(sender_uuid.clone(), (target_uuid, Instant::now()));
                ctx.whisper_success(format!(
                    "\u{26A0} Force-divorcing {target_name} means you pay alimony for the full marriage duration. Type !divorce force confirm within 60s to proceed."
                ));
            }
        } else if sub.is_empty() {
            ctx.whisper_success("Usage: !divorce <player> (mutual) | !divorce force <player>");
        } else {
            // Mutual divorce
            let partner_name = sub;
            let Some(partner_uuid) = ctx.state.api.convert_username_to_uuid(partner_name).await else {
                ctx.whisper_error(format!("Player {partner_name} not found."));
                return Ok(());
            };
            match ctx.state.api.marry_divorce(&sender_uuid, &partner_uuid).await {
                Ok(DivorceResult::Divorced) => {
                    enqueue_chat(ctx.state, format!("\u{1F494} {} and {partner_name} have mutually divorced.", ctx.sender));
                    ctx.whisper_success(format!("Divorce from {partner_name} complete. No alimony."));
                }
                Ok(DivorceResult::Pending { already_waiting: true }) => {
                    ctx.whisper_success("Already waiting on their confirmation.");
                }
                Ok(DivorceResult::Pending { already_waiting: false }) => {
                    ctx.whisper_success(format!(
                        "{partner_name} must also type !divorce {} within 10 minutes to finalize.",
                        ctx.sender
                    ));
                }
                Err(e) if e == "not_married" => {
                    ctx.whisper_success(format!("You are not married to {partner_name}."));
                }
                Err(_) => ctx.whisper_success("Divorce request failed. Try again."),
            }
        }

        Ok(())
    })
}

fn spouse_execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let Some(sender_uuid) = ctx.state.api.convert_username_to_uuid(ctx.sender).await else {
            ctx.whisper_success("Could not resolve your UUID.");
            return Ok(());
        };

        let spouses = ctx.state.api.marry_get_spouses(&sender_uuid).await;
        if spouses.is_empty() {
            ctx.whisper_success("You are not married to anyone.");
            return Ok(());
        }

        let mut names: Vec<String> = Vec::with_capacity(spouses.len());
        for s in &spouses {
            let name = ctx.state.api.tradebot_mc_username(&s.spouse_uuid)
                .await.unwrap_or_else(|| s.spouse_uuid[..8.min(s.spouse_uuid.len())].to_owned());
            names.push(name);
        }
        ctx.whisper_success(format!("Married to: {}", names.join(", ")));

        Ok(())
    })
}
