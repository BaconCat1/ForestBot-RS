use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::commands::{enqueue_chat, CommandContext, CommandDefinition, CommandFuture};
use crate::structure::market::types::now_unix;
use crate::structure::mineflayer::bot::AzaleaState;

use super::{chips_str, deduct_stake, format_alimony};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["duel"],
    description: "Duel a player. !duel <player> <chips> | confirm | reject | odds [player] | bet <player> <chips>",
    whitelisted: false,
    execute,
};

const RAKE: f64 = 0.03;
const MIN_STAKE: i64 = 50;
const MAX_STAKE: i64 = 10_000;
const MIN_KILLS_FOR_ODDS: u64 = 10;

// Only live duels live here; resolved/cancelled are just dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DuelPhase {
    Pending, // challenger escrowed; challenged hasn't confirmed yet
    Active,  // both escrowed; fight is live
}

#[derive(Clone, Debug)]
pub struct Duel {
    pub id: Uuid,
    pub challenger: String,
    // casino_adjust/casino_win are keyed by UUID, not username -- resolved once at
    // escrow time (start_duel/confirm_duel, where ctx is available to bail cleanly on
    // resolution failure) and stored here so the detached payout paths below
    // (resolve_duel/cancel_duel_refund, timeout tasks) never need a ctx to pay out
    // correctly. challenged_uuid is None until confirm_duel resolves it -- only ever
    // read once phase == Active, by which point it's guaranteed Some.
    pub challenger_uuid: String,
    pub challenged: String,
    pub challenged_uuid: Option<String>,
    pub stake: i64,
    pub phase: DuelPhase,
    pub confirm_expires_at: u64,
    pub expires_at: Option<u64>,
    pub side_bets: Vec<SideBet>,
}

#[derive(Clone, Debug)]
pub struct SideBet {
    pub bettor: String,
    pub bettor_uuid: String,
    pub target: String,      // participant name they are betting on
    pub amount: i64,
    pub odds_at_placement: f64, // win probability of `target` when bet was placed
}

// ── Duel state service ────────────────────────────────────────────────────────
//
// Owns the shared duel list behind a small set of named queries/mutations instead
// of exposing the raw `Mutex<Vec<Duel>>` for every call site to lock-and-scan by
// hand. Command handlers, event hooks, and timeout tasks below all go through this
// -- none of them touch the lock directly anymore. Chat messaging, payouts, and
// config reads stay exactly where they were (free functions in this file); this
// only centralizes storage access.
#[derive(Clone, Default)]
pub struct DuelService {
    duels: Arc<Mutex<Vec<Duel>>>,
}

impl DuelService {
    pub fn new() -> Self {
        Self::default()
    }

    /// Any duel (Pending or Active) `username` is a party to.
    pub fn find_participant(&self, username: &str) -> Option<Duel> {
        let duels = self.duels.lock().expect("duels lock");
        duels.iter().find(|d| {
            d.challenger.eq_ignore_ascii_case(username) ||
            d.challenged.eq_ignore_ascii_case(username)
        }).cloned()
    }

    /// A Pending duel where `username` is the challenged party (can !duel confirm).
    pub fn find_pending_for_challenged(&self, username: &str) -> Option<Duel> {
        let duels = self.duels.lock().expect("duels lock");
        duels.iter().find(|d| {
            d.phase == DuelPhase::Pending && d.challenged.eq_ignore_ascii_case(username)
        }).cloned()
    }

    /// A Pending duel where `username` is either party (challenged can reject, challenger can cancel).
    pub fn find_pending_for_either(&self, username: &str) -> Option<Duel> {
        let duels = self.duels.lock().expect("duels lock");
        duels.iter().find(|d| {
            d.phase == DuelPhase::Pending && (
                d.challenged.eq_ignore_ascii_case(username) ||
                d.challenger.eq_ignore_ascii_case(username)
            )
        }).cloned()
    }

    /// An Active duel `username` is a party to (used for death resolution -- a
    /// Pending duel can't be resolved by a kill, only a confirmed/live one).
    pub fn find_active_participant(&self, username: &str) -> Option<Duel> {
        let duels = self.duels.lock().expect("duels lock");
        duels.iter().find(|d| {
            d.phase == DuelPhase::Active && (
                d.challenger.eq_ignore_ascii_case(username) ||
                d.challenged.eq_ignore_ascii_case(username)
            )
        }).cloned()
    }

    /// A specific duel by id, requiring it still be in `phase` -- lets timeout
    /// tasks no-op cleanly if the duel already resolved/expired/was cancelled by
    /// the time the timer fires.
    pub fn find_by_id_in_phase(&self, id: Uuid, phase: DuelPhase) -> Option<Duel> {
        let duels = self.duels.lock().expect("duels lock");
        duels.iter().find(|d| d.id == id && d.phase == phase).cloned()
    }

    pub fn insert(&self, duel: Duel) {
        let mut duels = self.duels.lock().expect("duels lock");
        duels.push(duel);
    }

    /// Upgrades a Pending duel to Active once the challenged party confirms.
    pub fn transition_to_active(&self, id: Uuid, challenged_uuid: String, expires_at: u64) {
        let mut duels = self.duels.lock().expect("duels lock");
        if let Some(d) = duels.iter_mut().find(|d| d.id == id) {
            d.phase = DuelPhase::Active;
            d.expires_at = Some(expires_at);
            d.challenged_uuid = Some(challenged_uuid);
        }
    }

    /// Whether `username` already has a side bet on this duel (one per bettor per duel).
    pub fn has_side_bet_from(&self, id: Uuid, username: &str) -> bool {
        let duels = self.duels.lock().expect("duels lock");
        duels.iter().find(|d| d.id == id)
            .map(|d| d.side_bets.iter().any(|sb| sb.bettor.eq_ignore_ascii_case(username)))
            .unwrap_or(false)
    }

    pub fn add_side_bet(&self, id: Uuid, side_bet: SideBet) {
        let mut duels = self.duels.lock().expect("duels lock");
        if let Some(d) = duels.iter_mut().find(|d| d.id == id) {
            d.side_bets.push(side_bet);
        }
    }

    pub fn remove(&self, id: Uuid) {
        let mut duels = self.duels.lock().expect("duels lock");
        duels.retain(|d| d.id != id);
    }
}

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let sub = ctx.args.first().copied().unwrap_or("");
        match sub.to_ascii_lowercase().as_str() {
            "" => show_usage(&ctx),
            "confirm" => confirm_duel(&ctx).await?,
            "reject" | "cancel" => reject_duel(&ctx).await?,
            "odds" => show_odds(&ctx).await?,
            "bet" => place_side_bet(&ctx).await?,
            target => start_duel(&ctx, target).await?,
        }
        Ok(())
    })
}

// ── Usage ─────────────────────────────────────────────────────────────────────

fn show_usage(ctx: &CommandContext) {
    ctx.whisper_success("!duel <player> <chips> | confirm | reject | odds [player] | bet <player> <chips>");
}

// ── Start duel ────────────────────────────────────────────────────────────────

async fn start_duel(ctx: &CommandContext<'_>, target: &str) -> anyhow::Result<()> {
    let stake: i64 = match ctx.args.get(1).and_then(|s| s.parse().ok()) {
        Some(n) => n,
        None => { ctx.whisper_success("Usage: !duel <player> <chips>"); return Ok(()); }
    };

    let limit = ctx.bet_limit("duel", MIN_STAKE, Some(MAX_STAKE));
    let max = limit.max.unwrap_or(MAX_STAKE);
    if stake < limit.min || stake > max {
        ctx.whisper_success(format!("Stake must be {}-{}.", chips_str(limit.min), chips_str(max)));
        return Ok(());
    }

    let sender = ctx.sender;

    if target.eq_ignore_ascii_case(sender) {
        ctx.whisper_success("Can't duel yourself.");
        return Ok(());
    }

    // Target must be online
    {
        let players = ctx.state.players.read().expect("players lock");
        if !players.contains_key(target) {
            ctx.whisper_error(format!("{} isn't online.", target));
            return Ok(());
        }
    }

    // No existing duel for either party -- target is confirmed online (checked above), safe to echo.
    if let Some(existing) = ctx.state.duels.find_participant(sender) {
        ctx.whisper_success(format!(
            "Already in a duel ({} vs {}). Finish it first.",
            existing.challenger, existing.challenged
        ));
        return Ok(());
    }
    if ctx.state.duels.find_participant(target).is_some() {
        ctx.whisper_success(format!("{} is already in a duel.", target));
        return Ok(());
    }

    let Some(challenger_uuid) = ctx.require_player_uuid().await else { return Ok(()); };

    // Escrow challenger chips
    let Some(_) = deduct_stake(ctx, &challenger_uuid, stake).await else { return Ok(()); };

    let confirm_window_ms = ctx.runtime.player_economy.duel_confirm_window_ms;
    let confirm_expires_at = now_unix() + confirm_window_ms / 1000;
    let duel = Duel {
        id: Uuid::new_v4(),
        challenger: sender.to_owned(),
        challenger_uuid,
        challenged: target.to_owned(),
        challenged_uuid: None,
        stake,
        phase: DuelPhase::Pending,
        confirm_expires_at,
        expires_at: None,
        side_bets: Vec::new(),
    };

    ctx.state.duels.insert(duel.clone());

    // Announce in public chat so challenged player sees it
    enqueue_chat(ctx.state, format!(
        "{} challenges {} to a duel for {}! Type !duel confirm to accept ({}s to respond).",
        sender, target, chips_str(stake), confirm_window_ms / 1000
    ));

    // Spawn confirm timeout
    let state = ctx.state.clone();
    tokio::spawn(confirm_timeout_task(state, duel.id));

    Ok(())
}

// ── Confirm ───────────────────────────────────────────────────────────────────

async fn confirm_duel(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let duel = ctx.state.duels.find_pending_for_challenged(ctx.sender);

    let duel = match duel {
        Some(d) => d,
        None => { ctx.whisper_success("No pending duel request for you."); return Ok(()); }
    };

    if now_unix() >= duel.confirm_expires_at {
        ctx.whisper_success("Duel request expired.");
        // Cleanup happens in the timeout task
        return Ok(());
    }

    let Some(challenged_uuid) = ctx.require_player_uuid().await else { return Ok(()); };

    // Escrow challenged chips
    let Some(_) = deduct_stake(ctx, &challenged_uuid, duel.stake).await else { return Ok(()); };

    let expires_at = now_unix() + ctx.runtime.player_economy.duel_timeout_ms / 1000;

    ctx.state.duels.transition_to_active(duel.id, challenged_uuid, expires_at);

    // Fetch odds for announcement
    let (c_pct, x_pct) = duel_odds(ctx.state, &duel.challenger, &duel.challenged).await;
    enqueue_chat(ctx.state, format!(
        "DUEL: {} ({:.0}%) vs {} ({:.0}%) — {} chips each. Fight! (!duel bet <player> <chips> to side-bet)",
        duel.challenger, c_pct * 100.0, duel.challenged, x_pct * 100.0, chips_str(duel.stake)
    ));

    // Spawn expiry timer
    let state = ctx.state.clone();
    tokio::spawn(active_timeout_task(state, duel.id));

    Ok(())
}

// ── Reject / cancel ───────────────────────────────────────────────────────────

async fn reject_duel(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    // Challenged can reject; challenger can cancel their own pending duel
    let duel = ctx.state.duels.find_pending_for_either(ctx.sender);

    let duel = match duel {
        Some(d) => d,
        None => { ctx.whisper_success("No pending duel to reject."); return Ok(()); }
    };

    cancel_duel_refund(ctx.state, &duel).await;
    enqueue_chat(ctx.state, format!(
        "Duel between {} and {} cancelled — {} refunded.",
        duel.challenger, duel.challenged, chips_str(duel.stake)
    ));
    Ok(())
}

// ── Odds ──────────────────────────────────────────────────────────────────────

async fn show_odds(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let lookup = ctx.args.get(1).copied().unwrap_or(ctx.sender);
    let duel = ctx.state.duels.find_participant(lookup);
    let duel = match duel {
        Some(d) if d.phase == DuelPhase::Active => d,
        Some(_) => { ctx.whisper_success("Duel hasn't started yet."); return Ok(()); }
        None => { ctx.whisper_error(format!("No active duel for {}.", lookup)); return Ok(()); }
    };

    let (c_pct, x_pct) = duel_odds(ctx.state, &duel.challenger, &duel.challenged).await;
    let side_bet_count = duel.side_bets.len();
    let mut msg = format!(
        "{} ({:.0}%) vs {} ({:.0}%)",
        duel.challenger, c_pct * 100.0, duel.challenged, x_pct * 100.0
    );
    if side_bet_count > 0 {
        msg.push_str(&format!(" | {} side bet(s) placed", side_bet_count));
    }
    ctx.whisper_success(msg);
    Ok(())
}

// ── Side bets ─────────────────────────────────────────────────────────────────

async fn place_side_bet(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    // !duel bet <player> <chips>
    let target = match ctx.args.get(1).copied() {
        Some(t) => t,
        None => { ctx.whisper_success("Usage: !duel bet <player> <chips>"); return Ok(()); }
    };
    let amount: i64 = match ctx.args.get(2).and_then(|s| s.parse().ok()) {
        Some(n) => n,
        None => { ctx.whisper_success("Usage: !duel bet <player> <chips>"); return Ok(()); }
    };
    let limit = ctx.bet_limit("duel", MIN_STAKE, Some(MAX_STAKE));
    if amount < limit.min {
        ctx.whisper_success(format!("Min side bet is {}.", chips_str(limit.min)));
        return Ok(());
    }

    let duel = ctx.state.duels.find_participant(target);
    let duel = match duel {
        Some(d) if d.phase == DuelPhase::Active => d,
        Some(_) => { ctx.whisper_success("That duel hasn't started yet."); return Ok(()); }
        None => { ctx.whisper_error(format!("No active duel involving {}.", target)); return Ok(()); }
    };

    let target_lc = target.to_ascii_lowercase();
    let resolved_target = if duel.challenger.to_ascii_lowercase() == target_lc {
        duel.challenger.clone()
    } else if duel.challenged.to_ascii_lowercase() == target_lc {
        duel.challenged.clone()
    } else {
        ctx.whisper_error(format!("{} isn't in that duel.", target));
        return Ok(());
    };

    // Participants can't side-bet on their own duel
    if duel.challenger.eq_ignore_ascii_case(ctx.sender) || duel.challenged.eq_ignore_ascii_case(ctx.sender) {
        ctx.whisper_success("Participants can't place side bets on their own duel.");
        return Ok(());
    }

    // One side bet per bettor per duel
    if ctx.state.duels.has_side_bet_from(duel.id, ctx.sender) {
        ctx.whisper_success("Already placed a side bet on this duel.");
        return Ok(());
    }

    // Fetch odds for the target
    let (c_odds, x_odds) = duel_odds(ctx.state, &duel.challenger, &duel.challenged).await;
    let odds_for_target = if resolved_target == duel.challenger { c_odds } else { x_odds };

    let Some(bettor_uuid) = ctx.require_player_uuid().await else { return Ok(()); };

    // Deduct chips
    let Some(_) = deduct_stake(ctx, &bettor_uuid, amount).await else { return Ok(()); };

    let potential_payout = ((amount as f64 / odds_for_target.max(0.01)) * (1.0 - RAKE)) as i64;

    ctx.state.duels.add_side_bet(duel.id, SideBet {
        bettor: ctx.sender.to_owned(),
        bettor_uuid,
        target: resolved_target.clone(),
        amount,
        odds_at_placement: odds_for_target,
    });

    ctx.whisper_success(format!(
        "Side bet placed: {} chips on {} ({:.0}% odds) — potential payout: {}",
        chips_str(amount), resolved_target, odds_for_target * 100.0, chips_str(potential_payout)
    ));
    Ok(())
}

// ── Public event hooks (called from bot.rs) ───────────────────────────────────

pub async fn handle_death(state: &AzaleaState, victim: &str, murderer: Option<&str>) {
    let Some(duel) = state.duels.find_active_participant(victim) else { return; };

    let opponent = if duel.challenger.eq_ignore_ascii_case(victim) {
        &duel.challenged
    } else {
        &duel.challenger
    };

    let killer_is_opponent = murderer
        .map(|m| m.eq_ignore_ascii_case(opponent))
        .unwrap_or(false);

    if killer_is_opponent {
        let whisper_cmd = state.runtime.read().expect("runtime lock").whisper_command.clone();
        resolve_duel(state, &duel, opponent, &whisper_cmd).await;
    } else {
        cancel_duel_refund(state, &duel).await;
        enqueue_chat(state, format!(
            "Duel between {} and {} voided (third-party kill) — stakes refunded.",
            duel.challenger, duel.challenged
        ));
    }
}

pub async fn handle_disconnect(state: &AzaleaState, username: &str) {
    let Some(duel) = state.duels.find_participant(username) else { return; };

    cancel_duel_refund(state, &duel).await;
    enqueue_chat(state, format!(
        "{} disconnected — duel cancelled, stakes refunded.",
        username
    ));
}

// ── Resolution ────────────────────────────────────────────────────────────────

async fn resolve_duel(state: &AzaleaState, duel: &Duel, winner: &str, whisper_cmd: &str) {
    let loser = if duel.challenger.eq_ignore_ascii_case(winner) {
        &duel.challenged
    } else {
        &duel.challenger
    };

    let winner_uuid = if duel.challenger.eq_ignore_ascii_case(winner) {
        duel.challenger_uuid.clone()
    } else {
        match &duel.challenged_uuid {
            Some(uuid) => uuid.clone(),
            None => {
                // Should be unreachable -- resolve_duel only fires once phase == Active,
                // which confirm_duel guarantees set challenged_uuid. Refuse the payout
                // rather than risk crediting a bogus username-keyed row.
                eprintln!("[duel] resolve_duel: challenged_uuid missing on an Active duel (id {:?}) -- refusing payout", duel.id);
                state.duels.remove(duel.id);
                return;
            }
        }
    };

    state.duels.remove(duel.id);

    // Main pot
    let pot = duel.stake * 2;
    let rake = (pot as f64 * RAKE) as i64;
    let payout = pot - rake;
    let win_result = state.api.casino_win(&winner_uuid, payout).await;
    state.api.casino_jackpot_rake(rake).await;

    // Duel win stat
    state.api.increment_duel_wins(winner).await;

    // Side bets: winners paid at implied odds, losers to jackpot
    let mut jackpot_extra: i64 = 0;
    for sb in &duel.side_bets {
        if sb.target.eq_ignore_ascii_case(winner) {
            let odds = sb.odds_at_placement.max(0.01);
            let raw = (sb.amount as f64 / odds) as i64;
            let sb_rake = (raw as f64 * RAKE) as i64;
            let sb_payout = (raw - sb_rake).max(0);
            jackpot_extra += sb_rake;
            match state.api.casino_win(&sb.bettor_uuid, sb_payout).await {
                Ok(sb_win) => {
                    let sb_alimony_note = format_alimony(sb_win.alimony_paid);
                    enqueue_chat(state, format!(
                        "/{whisper_cmd} {} Side bet on {} paid: +{} chips{sb_alimony_note}",
                        sb.bettor, winner, chips_str(sb_payout)
                    ));
                }
                Err(e) => {
                    eprintln!("[duel] side bet payout failed for {}: {e:?}", sb.bettor_uuid);
                    enqueue_chat(state, format!(
                        "/{whisper_cmd} {} Side bet on {} won, but payout failed. Contact an admin.",
                        sb.bettor, winner
                    ));
                }
            }
        } else {
            jackpot_extra += sb.amount;
        }
    }
    if jackpot_extra > 0 {
        state.api.casino_jackpot_rake(jackpot_extra).await;
    }

    match win_result {
        Ok(win) => {
            let net = chips_str(payout - duel.stake);
            let alimony_note = format_alimony(win.alimony_paid);
            enqueue_chat(state, format!(
                "{winner} defeated {loser} in a duel! +{net} chips{alimony_note}"
            ));
        }
        Err(e) => {
            eprintln!("[duel] main pot payout failed for {winner_uuid}: {e:?}");
            enqueue_chat(state, format!(
                "{winner} defeated {loser} in a duel, but payout failed. Contact an admin."
            ));
        }
    }
}

async fn cancel_duel_refund(state: &AzaleaState, duel: &Duel) {
    state.duels.remove(duel.id);
    let _ = state.api.casino_adjust(&duel.challenger_uuid, duel.stake).await;
    if duel.phase == DuelPhase::Active {
        if let Some(uuid) = &duel.challenged_uuid {
            let _ = state.api.casino_adjust(uuid, duel.stake).await;
        }
    }
    for sb in &duel.side_bets {
        let _ = state.api.casino_adjust(&sb.bettor_uuid, sb.amount).await;
    }
}

// ── Timer tasks ───────────────────────────────────────────────────────────────

async fn confirm_timeout_task(state: AzaleaState, duel_id: Uuid) {
    let expires = state.duels.find_by_id_in_phase(duel_id, DuelPhase::Pending)
        .map(|d| d.confirm_expires_at);
    let Some(expires) = expires else { return; };

    let now = now_unix();
    if expires > now {
        tokio::time::sleep(std::time::Duration::from_secs(expires - now)).await;
    }

    let Some(duel) = state.duels.find_by_id_in_phase(duel_id, DuelPhase::Pending) else { return; };

    cancel_duel_refund(&state, &duel).await;
    enqueue_chat(&state, format!(
        "{} didn't respond to the duel request — cancelled, {} refunded to {}.",
        duel.challenged, chips_str(duel.stake), duel.challenger
    ));
}

async fn active_timeout_task(state: AzaleaState, duel_id: Uuid) {
    let timeout_ms = state
        .runtime
        .read()
        .expect("runtime config lock poisoned")
        .player_economy.duel_timeout_ms;
    tokio::time::sleep(std::time::Duration::from_millis(timeout_ms)).await;

    let Some(duel) = state.duels.find_by_id_in_phase(duel_id, DuelPhase::Active) else { return; };

    cancel_duel_refund(&state, &duel).await;
    enqueue_chat(&state, format!(
        "Duel between {} and {} timed out (10 min) — stakes refunded.",
        duel.challenger, duel.challenged
    ));
}

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn player_kd(state: &AzaleaState, username: &str) -> Option<f64> {
    let uuid = {
        let players = state.players.read().expect("players lock");
        players.get(username).map(|p| p.uuid.clone())?
    };
    let kd = state.api.get_kd(&uuid, &state.mc_server).await?;
    if kd.kills < MIN_KILLS_FOR_ODDS {
        return None;
    }
    Some(kd.kills as f64 / kd.deaths.max(1) as f64)
}

async fn duel_odds(state: &AzaleaState, challenger: &str, challenged: &str) -> (f64, f64) {
    let (ckd, xkd) = tokio::join!(
        player_kd(state, challenger),
        player_kd(state, challenged)
    );
    match (ckd, xkd) {
        (Some(c), Some(x)) => {
            let total = c + x;
            if total <= 0.0 { return (0.5, 0.5); }
            let p = c / total;
            (p, 1.0 - p)
        }
        _ => (0.5, 0.5),
    }
}
