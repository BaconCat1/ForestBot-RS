use uuid::Uuid;

use crate::commands::{enqueue_chat, CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::market::types::now_unix;
use crate::structure::mineflayer::bot::AzaleaState;

use super::casino::chips_str;

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["duel"],
    description: "Duel a player. !duel <player> <chips> | confirm | reject | odds [player] | bet <player> <chips>",
    whitelisted: false,
    bridge_ok: true,
    execute,
};

const CONFIRM_WINDOW_SECS: u64 = 60;
const DUEL_TIMEOUT_SECS: u64 = 600;
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
    pub challenged: String,
    pub stake: i64,
    pub phase: DuelPhase,
    pub confirm_expires_at: u64,
    pub expires_at: Option<u64>,
    pub side_bets: Vec<SideBet>,
}

#[derive(Clone, Debug)]
pub struct SideBet {
    pub bettor: String,
    pub target: String,      // participant name they are betting on
    pub amount: i64,
    pub odds_at_placement: f64, // win probability of `target` when bet was placed
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
    ctx.whisper("!duel <player> <chips> | confirm | reject | odds [player] | bet <player> <chips>");
}

// ── Start duel ────────────────────────────────────────────────────────────────

async fn start_duel(ctx: &CommandContext<'_>, target: &str) -> anyhow::Result<()> {
    let stake: i64 = match ctx.args.get(1).and_then(|s| s.parse().ok()) {
        Some(n) => n,
        None => { ctx.whisper("Usage: !duel <player> <chips>"); return Ok(()); }
    };

    if stake < MIN_STAKE || stake > MAX_STAKE {
        ctx.whisper(format!("Stake must be {}-{}.", chips_str(MIN_STAKE), chips_str(MAX_STAKE)));
        return Ok(());
    }

    let sender = ctx.sender;

    if target.eq_ignore_ascii_case(sender) {
        ctx.whisper("Can't duel yourself.");
        return Ok(());
    }

    // Target must be online
    {
        let players = ctx.state.players.read().expect("players lock");
        if !players.contains_key(target) {
            ctx.whisper(format!("{} isn't online.", target));
            return Ok(());
        }
    }

    // No existing duel for either party
    if let Some(existing) = find_participant_duel(ctx.state, sender) {
        ctx.whisper(format!(
            "Already in a duel ({} vs {}). Finish it first.",
            existing.challenger, existing.challenged
        ));
        return Ok(());
    }
    if find_participant_duel(ctx.state, target).is_some() {
        ctx.whisper(format!("{} is already in a duel.", target));
        return Ok(());
    }

    // Escrow challenger chips
    match ctx.state.api.casino_adjust(sender, -stake).await {
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

    let confirm_expires_at = now_unix() + CONFIRM_WINDOW_SECS;
    let duel = Duel {
        id: Uuid::new_v4(),
        challenger: sender.to_owned(),
        challenged: target.to_owned(),
        stake,
        phase: DuelPhase::Pending,
        confirm_expires_at,
        expires_at: None,
        side_bets: Vec::new(),
    };

    {
        let mut duels = ctx.state.duels.lock().expect("duels lock");
        duels.push(duel.clone());
    }

    // Announce in public chat so challenged player sees it
    enqueue_chat(ctx.state, format!(
        "{} challenges {} to a duel for {}! Type !duel confirm to accept ({}s to respond).",
        sender, target, chips_str(stake), CONFIRM_WINDOW_SECS
    ));

    // Spawn confirm timeout
    let state = ctx.state.clone();
    tokio::spawn(confirm_timeout_task(state, duel.id));

    Ok(())
}

// ── Confirm ───────────────────────────────────────────────────────────────────

async fn confirm_duel(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let duel = {
        let duels = ctx.state.duels.lock().expect("duels lock");
        duels.iter().find(|d| {
            d.phase == DuelPhase::Pending && d.challenged.eq_ignore_ascii_case(ctx.sender)
        }).cloned()
    };

    let duel = match duel {
        Some(d) => d,
        None => { ctx.whisper("No pending duel request for you."); return Ok(()); }
    };

    if now_unix() >= duel.confirm_expires_at {
        ctx.whisper("Duel request expired.");
        // Cleanup happens in the timeout task
        return Ok(());
    }

    // Escrow challenged chips
    match ctx.state.api.casino_adjust(ctx.sender, -duel.stake).await {
        Ok(_) => {}
        Err(CasinoAdjustErr::InsufficientFunds(have)) => {
            ctx.whisper(format!("Need {} but have {}.", chips_str(duel.stake), chips_str(have)));
            return Ok(());
        }
        Err(CasinoAdjustErr::NetworkErr) => {
            ctx.whisper("Casino unavailable.");
            return Ok(());
        }
    }

    let expires_at = now_unix() + DUEL_TIMEOUT_SECS;

    // Upgrade phase
    {
        let mut duels = ctx.state.duels.lock().expect("duels lock");
        if let Some(d) = duels.iter_mut().find(|d| d.id == duel.id) {
            d.phase = DuelPhase::Active;
            d.expires_at = Some(expires_at);
        }
    }

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
    let duel = {
        let duels = ctx.state.duels.lock().expect("duels lock");
        // Challenged can reject; challenger can cancel their own pending duel
        duels.iter().find(|d| {
            d.phase == DuelPhase::Pending && (
                d.challenged.eq_ignore_ascii_case(ctx.sender) ||
                d.challenger.eq_ignore_ascii_case(ctx.sender)
            )
        }).cloned()
    };

    let duel = match duel {
        Some(d) => d,
        None => { ctx.whisper("No pending duel to reject."); return Ok(()); }
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
    let duel = find_participant_duel(ctx.state, lookup);
    let duel = match duel {
        Some(d) if d.phase == DuelPhase::Active => d,
        Some(_) => { ctx.whisper("Duel hasn't started yet."); return Ok(()); }
        None => { ctx.whisper(format!("No active duel for {}.", lookup)); return Ok(()); }
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
    ctx.whisper(msg);
    Ok(())
}

// ── Side bets ─────────────────────────────────────────────────────────────────

async fn place_side_bet(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    // !duel bet <player> <chips>
    let target = match ctx.args.get(1).copied() {
        Some(t) => t,
        None => { ctx.whisper("Usage: !duel bet <player> <chips>"); return Ok(()); }
    };
    let amount: i64 = match ctx.args.get(2).and_then(|s| s.parse().ok()) {
        Some(n) => n,
        None => { ctx.whisper("Usage: !duel bet <player> <chips>"); return Ok(()); }
    };
    if amount < MIN_STAKE {
        ctx.whisper(format!("Min side bet is {}.", chips_str(MIN_STAKE)));
        return Ok(());
    }

    let duel = find_participant_duel(ctx.state, target);
    let duel = match duel {
        Some(d) if d.phase == DuelPhase::Active => d,
        Some(_) => { ctx.whisper("That duel hasn't started yet."); return Ok(()); }
        None => { ctx.whisper(format!("No active duel involving {}.", target)); return Ok(()); }
    };

    let target_lc = target.to_ascii_lowercase();
    let resolved_target = if duel.challenger.to_ascii_lowercase() == target_lc {
        duel.challenger.clone()
    } else if duel.challenged.to_ascii_lowercase() == target_lc {
        duel.challenged.clone()
    } else {
        ctx.whisper(format!("{} isn't in that duel.", target));
        return Ok(());
    };

    // Participants can't side-bet on their own duel
    if duel.challenger.eq_ignore_ascii_case(ctx.sender) || duel.challenged.eq_ignore_ascii_case(ctx.sender) {
        ctx.whisper("Participants can't place side bets on their own duel.");
        return Ok(());
    }

    // One side bet per bettor per duel
    {
        let duels = ctx.state.duels.lock().expect("duels lock");
        if let Some(d) = duels.iter().find(|d| d.id == duel.id) {
            if d.side_bets.iter().any(|sb| sb.bettor.eq_ignore_ascii_case(ctx.sender)) {
                ctx.whisper("Already placed a side bet on this duel.");
                return Ok(());
            }
        }
    }

    // Fetch odds for the target
    let (c_odds, x_odds) = duel_odds(ctx.state, &duel.challenger, &duel.challenged).await;
    let odds_for_target = if resolved_target == duel.challenger { c_odds } else { x_odds };

    // Deduct chips
    match ctx.state.api.casino_adjust(ctx.sender, -amount).await {
        Ok(_) => {}
        Err(CasinoAdjustErr::InsufficientFunds(have)) => {
            ctx.whisper(format!("Need {} but have {}.", chips_str(amount), chips_str(have)));
            return Ok(());
        }
        Err(CasinoAdjustErr::NetworkErr) => {
            ctx.whisper("Casino unavailable.");
            return Ok(());
        }
    }

    let potential_payout = ((amount as f64 / odds_for_target.max(0.01)) * (1.0 - RAKE)) as i64;

    {
        let mut duels = ctx.state.duels.lock().expect("duels lock");
        if let Some(d) = duels.iter_mut().find(|d| d.id == duel.id) {
            d.side_bets.push(SideBet {
                bettor: ctx.sender.to_owned(),
                target: resolved_target.clone(),
                amount,
                odds_at_placement: odds_for_target,
            });
        }
    }

    ctx.whisper(format!(
        "Side bet placed: {} chips on {} ({:.0}% odds) — potential payout: {}",
        chips_str(amount), resolved_target, odds_for_target * 100.0, chips_str(potential_payout)
    ));
    Ok(())
}

// ── Public event hooks (called from bot.rs) ───────────────────────────────────

pub async fn handle_death(state: &AzaleaState, victim: &str, murderer: Option<&str>) {
    let duel = {
        let duels = state.duels.lock().expect("duels lock");
        duels.iter().find(|d| {
            d.phase == DuelPhase::Active && (
                d.challenger.eq_ignore_ascii_case(victim) ||
                d.challenged.eq_ignore_ascii_case(victim)
            )
        }).cloned()
    };
    let Some(duel) = duel else { return; };

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
    let duel = find_participant_duel(state, username);
    let Some(duel) = duel else { return; };

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

    remove_duel(state, duel.id);

    // Main pot
    let pot = duel.stake * 2;
    let rake = (pot as f64 * RAKE) as i64;
    let payout = pot - rake;
    let _ = state.api.casino_adjust(winner, payout).await;
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
            let _ = state.api.casino_adjust(&sb.bettor, sb_payout).await;
            enqueue_chat(state, format!(
                "/{whisper_cmd} {} Side bet on {} paid: +{} chips",
                sb.bettor, winner, chips_str(sb_payout)
            ));
        } else {
            jackpot_extra += sb.amount;
        }
    }
    if jackpot_extra > 0 {
        state.api.casino_jackpot_rake(jackpot_extra).await;
    }

    let net = chips_str(payout - duel.stake);
    enqueue_chat(state, format!(
        "{winner} defeated {loser} in a duel! +{net} chips"
    ));
}

async fn cancel_duel_refund(state: &AzaleaState, duel: &Duel) {
    remove_duel(state, duel.id);
    let _ = state.api.casino_adjust(&duel.challenger, duel.stake).await;
    if duel.phase == DuelPhase::Active {
        let _ = state.api.casino_adjust(&duel.challenged, duel.stake).await;
    }
    for sb in &duel.side_bets {
        let _ = state.api.casino_adjust(&sb.bettor, sb.amount).await;
    }
}

// ── Timer tasks ───────────────────────────────────────────────────────────────

async fn confirm_timeout_task(state: AzaleaState, duel_id: Uuid) {
    let expires = {
        let duels = state.duels.lock().expect("duels lock");
        duels.iter().find(|d| d.id == duel_id).map(|d| d.confirm_expires_at)
    };
    let Some(expires) = expires else { return; };

    let now = now_unix();
    if expires > now {
        tokio::time::sleep(std::time::Duration::from_secs(expires - now)).await;
    }

    let duel = {
        let duels = state.duels.lock().expect("duels lock");
        duels.iter()
            .find(|d| d.id == duel_id && d.phase == DuelPhase::Pending)
            .cloned()
    };
    let Some(duel) = duel else { return; };

    cancel_duel_refund(&state, &duel).await;
    enqueue_chat(&state, format!(
        "{} didn't respond to the duel request — cancelled, {} refunded to {}.",
        duel.challenged, chips_str(duel.stake), duel.challenger
    ));
}

async fn active_timeout_task(state: AzaleaState, duel_id: Uuid) {
    tokio::time::sleep(std::time::Duration::from_secs(DUEL_TIMEOUT_SECS)).await;

    let duel = {
        let duels = state.duels.lock().expect("duels lock");
        duels.iter()
            .find(|d| d.id == duel_id && d.phase == DuelPhase::Active)
            .cloned()
    };
    let Some(duel) = duel else { return; };

    cancel_duel_refund(&state, &duel).await;
    enqueue_chat(&state, format!(
        "Duel between {} and {} timed out (10 min) — stakes refunded.",
        duel.challenger, duel.challenged
    ));
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn find_participant_duel(state: &AzaleaState, username: &str) -> Option<Duel> {
    let duels = state.duels.lock().expect("duels lock");
    duels.iter().find(|d| {
        d.challenger.eq_ignore_ascii_case(username) ||
        d.challenged.eq_ignore_ascii_case(username)
    }).cloned()
}

fn remove_duel(state: &AzaleaState, id: Uuid) {
    let mut duels = state.duels.lock().expect("duels lock");
    duels.retain(|d| d.id != id);
}

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
