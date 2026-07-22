// "Will they die within the next N hours of PLAYED time?" -- the death-timing
// half of the server event futures item, deferred out of the join-window build
// per explicit scope decision (playtime-denominated window, not join-window's
// simpler wall-clock shape). Odds computed Hub-side via a hazard-survival
// estimate over historical death-to-death gaps, reconstructed in playtime-ms
// from `sessions` (deaths carry no playtime snapshot of their own -- see
// getDeathOdds.ts). Full scoping: REFERENCE_MATERIAL/DOCS/casino_event_futures_scoping.md.
//
// Unlike join-window (fixed wall-clock `close_time`, single sleep-until),
// this resolves on an ACCUMULATING stat (subject's live playtime), so
// settlement is a repeating poll, not a one-shot sleep: each tick checks (1)
// did the subject die since bet placement -- WIN, resolves immediately even
// mid-window; (2) has the subject's playtime advanced a full window's worth
// with no death -- LOSS/timeout. Nothing else in the codebase resolves this
// way (everything else settles against a fixed timestamp).

use crate::commands::CommandContext;
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::market::types::now_unix;

use super::{MIN_BET, chips_str, format_alimony, to_price, fmt_odds, SettleDeps};

/// Mirrors Hub's `WINDOW_HOURS` (getDeathOdds.ts) -- used to compute the
/// playtime threshold a bet loses at, since that's not round-tripped through
/// the bet row (only the placement snapshot is).
const DEATH_WINDOW_HOURS: u64 = 12;

/// How often the settle task checks in. Coarser than join-window's exact
/// sleep-until since there's no fixed resolve time to sleep to -- an hourly
/// tick means a WIN can lag up to this long behind the actual death, which
/// is an accepted tradeoff for a playtime-driven poll, not a bug to tighten
/// without being asked.
const POLL_INTERVAL_SECS: u64 = 3600;

#[derive(Debug, Clone)]
pub struct DeathWindowBet {
    pub id: Option<i64>,
    pub player: String,       // bettor's uuid
    pub subject_uuid: String, // the player being bet on
    pub subject_name: String,
    pub price: f64,
    pub stake: i64,
    pub close_time: u64,             // informational estimate only, not used to settle
    pub placement_time: u64,         // wall-clock placement, unix SECONDS (Hub created_at)
    pub placement_playtime_ms: u64,  // subject's users.playtime at placement, ms
}

impl super::CasinoBet for DeathWindowBet {
    const TYPE: &'static str = "death_window";

    fn to_insert_json(&self) -> serde_json::Value {
        serde_json::json!({
            "player_uuid":            self.player,
            "subject_uuid":           self.subject_uuid,
            "subject_name":           self.subject_name,
            "price":                  self.price,
            "stake":                  self.stake,
            "close_time":             self.close_time,
            "placement_playtime_ms":  self.placement_playtime_ms,
        })
    }

    fn from_json(item: &serde_json::Value) -> Option<Self> {
        Some(Self {
            id:           Some(item.get("id")?.as_i64()?),
            player:       item.get("player_uuid")?.as_str()?.to_owned(),
            subject_uuid: item.get("subject_uuid")?.as_str()?.to_owned(),
            subject_name: item.get("subject_name")?.as_str()?.to_owned(),
            price:        item.get("price")?.as_f64()?,
            stake:        item.get("stake")?.as_i64()?,
            close_time:   item.get("close_time")?.as_u64()?,
            placement_time: item.get("placement_time")?.as_u64()?,
            // Sourced from a DOUBLE column (latitude) -- comes back as a JSON
            // float, not an integer, so as_u64() would silently fail here.
            placement_playtime_ms: item.get("placement_playtime_ms")?.as_f64()? as u64,
        })
    }
}

/// Appended to a plain `!deaths <player>` lookup, same placement as join-window's
/// `whisper_odds_hint`. Silent no-op only on a hard fetch failure.
pub async fn whisper_odds_hint(ctx: &CommandContext<'_>, subject_uuid: &str, subject_name: &str) {
    let Some(odds) = ctx.state.api.casino_death_odds(subject_uuid).await else {
        return;
    };

    if !odds.eligible {
        ctx.whisper_success(format!(
            "Not enough death history on {subject_name} to offer odds yet ({} comparable playtime-gaps, need 30).",
            odds.sample_size
        ));
        return;
    }

    let price = to_price(odds.p);
    ctx.whisper_success(format!(
        "Odds {subject_name} dies within next {}h played: {} (played {:.1}h since last death) — bet with {}deaths {subject_name} <chips>",
        odds.window_hours, fmt_odds(price), odds.elapsed_hours, ctx.runtime.prefix,
    ));
}

/// `!deaths <player> <bet>` -- places a bet that `player` (the subject, never
/// the bettor themself) dies within the odds' fixed playtime window.
pub async fn place_bet(ctx: &CommandContext<'_>, subject_name: &str, stake: i64) -> anyhow::Result<()> {
    let Some(subject_uuid) = ctx.state.api.convert_username_to_uuid(subject_name).await else {
        ctx.whisper_error(format!("Player {subject_name} not found."));
        return Ok(());
    };

    let Some(bettor_uuid) = ctx.require_player_uuid().await else { return Ok(()); };

    // Anti-manufacture guard (same rationale as join-window): a bettor could
    // otherwise force the outcome by intentionally dying or staying alive.
    if subject_uuid == bettor_uuid {
        ctx.whisper_success("You can't bet on your own deaths.");
        return Ok(());
    }

    let limit = ctx.bet_limit("death_window", MIN_BET, Some(10_000));
    if stake < limit.min {
        ctx.whisper_success(format!("Min bet: {} chips.", limit.min));
        return Ok(());
    }
    if let Some(max) = limit.max {
        if stake > max {
            ctx.whisper_success(format!("Max bet: {} chips.", max));
            return Ok(());
        }
    }

    let Some(odds) = ctx.state.api.casino_death_odds(&subject_uuid).await else {
        ctx.whisper_success("Could not fetch odds. Try again.");
        return Ok(());
    };
    if !odds.eligible {
        ctx.whisper_success(format!(
            "Not enough death history on {subject_name} to offer odds yet ({} comparable playtime-gaps, need 30).",
            odds.sample_size
        ));
        return Ok(());
    }

    let Some(playtime) = ctx.state.api.get_playtime(&subject_uuid, &ctx.state.mc_server).await else {
        ctx.whisper_success("Could not fetch subject's playtime. Try again.");
        return Ok(());
    };

    let price = to_price(odds.p);

    match ctx.state.api.casino_adjust(&bettor_uuid, -stake).await {
        Err(CasinoAdjustErr::InsufficientFunds(have)) => {
            ctx.whisper_success(format!("Not enough chips (have {}).", chips_str(have)));
            return Ok(());
        }
        Err(e) => {
            ctx.whisper_success(format!("Error: {e:?}"));
            return Ok(());
        }
        Ok(_) => {}
    }

    let now = now_unix();
    let close_time = now + odds.window_hours as u64 * 3600; // informational only
    let mut bet = DeathWindowBet {
        id: None,
        player: bettor_uuid.clone(),
        subject_uuid: subject_uuid.clone(),
        subject_name: subject_name.to_owned(),
        price,
        stake,
        close_time,
        placement_time: now,
        placement_playtime_ms: playtime.playtime,
    };

    match ctx.state.api.casino_bet_insert(&bet).await {
        Some(i) => bet.id = Some(i),
        None => {
            if let Err(e) = ctx.state.api.casino_adjust(&bettor_uuid, stake).await {
                eprintln!("[DeathWindow] refund failed for {bettor_uuid}: {e:?}");
                ctx.whisper_success("Failed to record bet. Refund also failed — contact an admin.");
            } else {
                ctx.whisper_success("Failed to record bet. Chips refunded.");
            }
            return Ok(());
        }
    }

    let payout = super::calc_payout(stake, price);
    ctx.whisper_success(format!(
        "Bet {} that {subject_name} dies within {}h played — pays {} at {}. ({} comparable playtime-gaps)",
        chips_str(stake), odds.window_hours, chips_str(payout), fmt_odds(price), odds.sample_size,
    ));

    ctx.state.death_window_bets.lock().unwrap()
        .entry(bettor_uuid)
        .or_default()
        .push(bet.clone());

    let deps = SettleDeps::from(ctx.state);
    let bets_map = ctx.state.death_window_bets.clone();
    let whisper_cmd = ctx.runtime.whisper_command.clone();
    let mc_server = ctx.state.mc_server.clone();
    tokio::spawn(death_window_settle_task(deps, bets_map, whisper_cmd, mc_server, bet));

    Ok(())
}

// ── Settlement (repeating poll, not sleep-until) ───────────────────────────────

type BetsMap = std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<DeathWindowBet>>>>;

pub async fn death_window_settle_task(
    deps: SettleDeps,
    bets_map: BetsMap,
    whisper_cmd: String,
    mc_server: String,
    bet: DeathWindowBet,
) {
    // Hub's created_at is unix SECONDS; deaths.time / died-since's `since` are
    // both epoch-ms (see getDiedSince.ts) -- convert once, up front.
    let placement_since_ms = bet.placement_time * 1000;
    let loss_threshold_ms = bet.placement_playtime_ms + DEATH_WINDOW_HOURS * 3_600_000;

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;

        match deps.api.casino_died_since(&bet.subject_uuid, placement_since_ms).await {
            Some(true) => {
                if let Ok(mut bets) = bets_map.lock() {
                    if let Some(v) = bets.get_mut(&bet.player) { v.retain(|b| b.id != bet.id); }
                }
                deps.api.casino_bet_delete::<DeathWindowBet>(bet.id.unwrap()).await;

                let payout = super::calc_payout(bet.stake, bet.price);
                let msg = match deps.api.casino_win(&bet.player, payout).await {
                    Ok(win) => {
                        let alimony_note = format_alimony(win.alimony_paid);
                        format!(
                            "[DEATHS] {} died — WIN +{}{alimony_note} ({} @ {}).",
                            bet.subject_name, chips_str(payout - bet.stake), chips_str(bet.stake), fmt_odds(bet.price),
                        )
                    }
                    Err(e) => {
                        eprintln!("[DeathWindow settle] casino_win failed for {}: {e:?}", bet.player);
                        format!("[DEATHS] {} died, bet wins but payout failed. Contact an admin.", bet.subject_name)
                    }
                };
                deps.deliver(&whisper_cmd, &bet.player, msg).await;
                return;
            }
            Some(false) => {
                let Some(playtime) = deps.api.get_playtime(&bet.subject_uuid, &mc_server).await else {
                    continue; // fetch failed, retry next tick rather than guess
                };
                if playtime.playtime < loss_threshold_ms {
                    continue; // still within window, keep polling
                }

                if let Ok(mut bets) = bets_map.lock() {
                    if let Some(v) = bets.get_mut(&bet.player) { v.retain(|b| b.id != bet.id); }
                }
                deps.api.casino_bet_delete::<DeathWindowBet>(bet.id.unwrap()).await;
                deps.api.casino_jackpot_rake(bet.stake).await;
                let msg = format!(
                    "[DEATHS] {} played {}h without dying — LOSS -{} (to jackpot).",
                    bet.subject_name, DEATH_WINDOW_HOURS, chips_str(bet.stake),
                );
                deps.deliver(&whisper_cmd, &bet.player, msg).await;
                return;
            }
            None => {
                // Fetch failed -- retry next tick, same fail-safe posture as
                // every other settle task's fetch failure.
            }
        }
    }
}
