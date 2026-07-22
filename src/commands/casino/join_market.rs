// "Will they join in the next N hours?" -- the join-window half of the server
// event futures item (Casino Phase II item multiple players actually asked
// for). Odds computed Hub-side (hazard-survival estimate over deduped
// `sessions` gaps). Full scoping: REFERENCE_MATERIAL/DOCS/casino_event_futures_scoping.md.
//
// Entry points (`!joins`) stay in `src/commands/joins.rs` per the scoping
// decision -- this file holds the bet-placement/settlement mechanics only.

use crate::commands::CommandContext;
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::market::types::now_unix;

use super::{MIN_BET, chips_str, format_alimony, to_price, fmt_odds, sleep_until, SettleDeps};

/// Mirrors Hub's `WINDOW_HOURS` (getJoinOdds.ts) -- fixed per the scoping decision
/// (item 7: "no variable window options, unnecessary noise"), so it's safe to
/// recompute a bet's `created_at` from `close_time` at settlement/restart-preload
/// time instead of round-tripping a second timestamp through the bet row.
const JOIN_WINDOW_HOURS: u64 = 12;

#[derive(Debug, Clone)]
pub struct JoinWindowBet {
    pub id: Option<i64>,
    pub player: String,       // bettor's uuid
    pub subject_uuid: String, // the player being bet on
    pub subject_name: String,
    pub price: f64,
    pub stake: i64,
    pub close_time: u64,
}

impl super::CasinoBet for JoinWindowBet {
    const TYPE: &'static str = "join_window";

    fn to_insert_json(&self) -> serde_json::Value {
        serde_json::json!({
            "player_uuid":  self.player,
            "subject_uuid": self.subject_uuid,
            "subject_name": self.subject_name,
            "price":        self.price,
            "stake":        self.stake,
            "close_time":   self.close_time,
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
        })
    }
}

/// Appended to a plain `!joins <player>` lookup (no bet arg) so the odds and
/// the bet syntax surface every time someone checks a player's joins, not
/// just when they already know to ask. Silent no-op only on a hard fetch
/// failure -- an ineligible player still gets a short explanatory line so the
/// feature doesn't look broken/missing.
pub async fn whisper_odds_hint(ctx: &CommandContext<'_>, subject_uuid: &str, subject_name: &str) {
    let Some(odds) = ctx.state.api.casino_join_odds(subject_uuid).await else {
        return;
    };

    if !odds.eligible {
        ctx.whisper_success(format!(
            "Not enough join history on {subject_name} to offer odds yet ({} comparable gaps, need 30)."
        , odds.sample_size));
        return;
    }

    let price = to_price(odds.p);
    ctx.whisper_success(format!(
        "Odds {subject_name} logs in within {}h: {} (gone {:.1}h so far) — bet with {}joins {subject_name} <chips>",
        odds.window_hours, fmt_odds(price), odds.elapsed_hours, ctx.runtime.prefix,
    ));
}

/// `!joins <player> <bet>` -- places a bet that `player` (the subject, never
/// the bettor themself) logs in within the odds' fixed window.
pub async fn place_bet(ctx: &CommandContext<'_>, subject_name: &str, stake: i64) -> anyhow::Result<()> {
    let Some(subject_uuid) = ctx.state.api.convert_username_to_uuid(subject_name).await else {
        ctx.whisper_error(format!("Player {subject_name} not found."));
        return Ok(());
    };

    let Some(bettor_uuid) = ctx.require_player_uuid().await else { return Ok(()); };

    // Anti-manufacture guard (scoping doc): a bettor could otherwise force the
    // outcome by just logging in or staying off on their own bet.
    if subject_uuid == bettor_uuid {
        ctx.whisper_success("You can't bet on your own joins.");
        return Ok(());
    }

    let limit = ctx.bet_limit("join_window", MIN_BET, Some(10_000));
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

    let Some(odds) = ctx.state.api.casino_join_odds(&subject_uuid).await else {
        ctx.whisper_success("Could not fetch odds. Try again.");
        return Ok(());
    };
    if !odds.eligible {
        ctx.whisper_success(format!(
            "Not enough join history on {subject_name} to offer odds yet ({} comparable gaps, need 30).",
            odds.sample_size
        ));
        return Ok(());
    }

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

    let close_time = now_unix() + odds.window_hours as u64 * 3600;
    let mut bet = JoinWindowBet {
        id: None,
        player: bettor_uuid.clone(),
        subject_uuid: subject_uuid.clone(),
        subject_name: subject_name.to_owned(),
        price,
        stake,
        close_time,
    };

    match ctx.state.api.casino_bet_insert(&bet).await {
        Some(i) => bet.id = Some(i),
        None => {
            if let Err(e) = ctx.state.api.casino_adjust(&bettor_uuid, stake).await {
                eprintln!("[JoinWindow] refund failed for {bettor_uuid}: {e:?}");
                ctx.whisper_success("Failed to record bet. Refund also failed — contact an admin.");
            } else {
                ctx.whisper_success("Failed to record bet. Chips refunded.");
            }
            return Ok(());
        }
    }

    let payout = super::calc_payout(stake, price);
    ctx.whisper_success(format!(
        "Bet {} that {subject_name} logs in within {}h — pays {} at {}. ({} comparable gaps)",
        chips_str(stake), odds.window_hours, chips_str(payout), fmt_odds(price), odds.sample_size,
    ));

    ctx.state.join_window_bets.lock().unwrap()
        .entry(bettor_uuid)
        .or_default()
        .push(bet.clone());

    let deps = SettleDeps::from(ctx.state);
    let bets_map = ctx.state.join_window_bets.clone();
    let whisper_cmd = ctx.runtime.whisper_command.clone();
    tokio::spawn(join_window_settle_task(deps, bets_map, whisper_cmd, bet));

    Ok(())
}

// ── Settlement ────────────────────────────────────────────────────────────────

pub async fn join_window_settle_task(
    deps: SettleDeps,
    bets_map: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<JoinWindowBet>>>>,
    whisper_cmd: String,
    bet: JoinWindowBet,
) {
    sleep_until(bet.close_time).await;

    {
        let mut bets = bets_map.lock().unwrap();
        if let Some(v) = bets.get_mut(&bet.player) {
            v.retain(|b| b.id != bet.id);
        }
    }

    let created_at = bet.close_time.saturating_sub(JOIN_WINDOW_HOURS * 3600);
    let joined = deps.api.casino_joined_since(&bet.subject_uuid, created_at).await;

    deps.api.casino_bet_delete::<JoinWindowBet>(bet.id.unwrap()).await;

    let msg = match joined {
        Some(true) => {
            let payout = super::calc_payout(bet.stake, bet.price);
            match deps.api.casino_win(&bet.player, payout).await {
                Ok(win) => {
                    let alimony_note = format_alimony(win.alimony_paid);
                    format!(
                        "[JOINS] {} logged in — WIN +{}{alimony_note} ({} @ {}).",
                        bet.subject_name, chips_str(payout - bet.stake), chips_str(bet.stake), fmt_odds(bet.price),
                    )
                }
                Err(e) => {
                    eprintln!("[JoinWindow settle] casino_win failed for {}: {e:?}", bet.player);
                    format!("[JOINS] {} logged in, bet wins but payout failed. Contact an admin.", bet.subject_name)
                }
            }
        }
        Some(false) => {
            deps.api.casino_jackpot_rake(bet.stake).await;
            format!(
                "[JOINS] {} did not log in — LOSS -{} (to jackpot).",
                bet.subject_name, chips_str(bet.stake),
            )
        }
        None => {
            match deps.api.casino_adjust(&bet.player, bet.stake).await {
                Ok(_) => format!(
                    "[JOINS] {} — could not check login status at settlement. {} refunded.",
                    bet.subject_name, chips_str(bet.stake),
                ),
                Err(e) => {
                    eprintln!("[JoinWindow settle] refund failed for {}: {e:?}", bet.player);
                    format!("[JOINS] {} — could not check login status. Refund failed — contact an admin.", bet.subject_name)
                }
            }
        }
    };

    deps.deliver(&whisper_cmd, &bet.player, msg).await;
}
