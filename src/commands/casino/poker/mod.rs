// © 2025 ashxudev — terminal-poker (MIT)
use rand::Rng;

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::mineflayer::bot::CasinoSession;

use super::{balance_str, chips_str};

pub mod game;
pub mod bot;

use game::actions::{Action, AvailableActions};
use game::deck::Card;
use game::state::{GamePhase, GameState, Player};
use bot::rule_based::RuleBasedBot;

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["poker", "pk"],
    description: "Texas Hold'em vs the bot. !poker <stake> | !poker deal | !poker call/check/fold/raise <n>/allin | !poker quit",
    whitelisted: false,
    execute,
};

const MIN_STAKE: i64 = 25;
const MAX_STAKE: i64 = 5000;

const OPPONENTS: &[(&str, f64, f64)] = &[
    ("Glass Joe",     0.00, 0.19),
    ("Piston Honda",  0.20, 0.39),
    ("Bald Bull",     0.40, 0.59),
    ("Soda Popinski", 0.60, 0.79),
    ("Mike Tyson",    0.80, 1.00),
];

fn pick_opponent() -> (&'static str, f64) {
    let mut rng = rand::thread_rng();
    let (name, lo, hi) = OPPONENTS[rng.gen_range(0..OPPONENTS.len())];
    let aggression = rng.gen_range(lo..=hi);
    (name, aggression)
}

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let subcmd = ctx.args.first().copied().unwrap_or("").to_ascii_lowercase();
        match subcmd.as_str() {
            "" => execute_status(&ctx).await,
            "deal" | "d" => execute_deal(&ctx).await,
            "quit" | "q" => execute_quit(&ctx).await,
            "fold" | "f"
            | "check" | "x"
            | "call" | "c"
            | "bet" | "raise" | "r"
            | "allin" | "all" => execute_action(&ctx, &subcmd).await,
            _ => execute_new_session(&ctx, &subcmd).await,
        }
    })
}

// ── Buy in ────────────────────────────────────────────────────────────────────

async fn execute_new_session(ctx: &CommandContext<'_>, stake_str: &str) -> anyhow::Result<()> {
    {
        let sessions = ctx.state.casino_sessions.lock().expect("casino sessions lock poisoned");
        if let Some(s) = sessions.get(ctx.sender) {
            let which = session_label(s);
            ctx.whisper_success(format!("Already in a {which} game. Finish it first."));
            return Ok(());
        }
    }

    let stake: i64 = match stake_str.parse() {
        Ok(n) => n,
        Err(_) => {
            ctx.whisper_success("Usage: !poker <stake> | deal/call/check/fold/raise/allin/quit");
            return Ok(());
        }
    };

    if stake < MIN_STAKE || stake > MAX_STAKE {
        ctx.whisper_success(format!("Stake must be between {} and {}.", chips_str(MIN_STAKE), chips_str(MAX_STAKE)));
        return Ok(());
    }

    let Some(player_uuid) = ctx.require_player_uuid().await else { return Ok(()); };

    match ctx.state.api.casino_adjust(&player_uuid, -stake).await {
        Ok(_) => {}
        Err(CasinoAdjustErr::InsufficientFunds(have)) => {
            ctx.whisper_success(format!("Need {} but have {}.", chips_str(stake), chips_str(have)));
            return Ok(());
        }
        Err(CasinoAdjustErr::NetworkErr) => {
            ctx.whisper_success("Casino unavailable.");
            return Ok(());
        }
    }

    let (opponent_name, aggression) = pick_opponent();
    let mut game = GameState::new((stake / 2) as u32);
    let bot = RuleBasedBot::new(aggression);

    ctx.whisper_success(format!("Poker vs {}! Stake: {}.", opponent_name, chips_str(stake)));

    let (bot_events, last_pot) = run_bot_turns(&mut game, &bot, opponent_name);

    if matches!(game.phase, GamePhase::HandComplete | GamePhase::Showdown) {
        let pot = hand_end_pot(&game, last_pot);
        handle_hand_end(ctx, &mut game, opponent_name, aggression, stake, &bot_events, pot).await;
        return Ok(());
    }

    show_hand_state(ctx, &game, opponent_name, &bot_events);
    let started = super::try_start_session(ctx.state, ctx.sender, crate::structure::mineflayer::bot::CasinoSession::Poker {
        stake, opponent_name, aggression, game: Box::new(game),
    });
    if !started {
        let bal = ctx.state.api.casino_adjust(&player_uuid, stake).await.unwrap_or(0);
        ctx.whisper_success(format!("Already in another game — this stake refunded. Balance: {}", chips_str(bal)));
    }
    Ok(())
}

// ── Deal next hand ─────────────────────────────────────────────────────────────

async fn execute_deal(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let (stake, _, _, mut game) = match get_poker_session(ctx) {
        Some(v) => v,
        None => return Ok(()),
    };

    if !matches!(game.phase, GamePhase::HandComplete | GamePhase::Showdown) {
        ctx.whisper_success("Hand still in progress. !pk fold/call/check/raise/allin to play.");
        return Ok(());
    }

    if game.player_stack == 0 || game.bot_stack == 0 {
        ctx.whisper_success("Session over. Start a new one with !poker <stake>.");
        return Ok(());
    }

    let (opponent_name, aggression) = pick_opponent();
    game.start_new_hand();
    let bot = RuleBasedBot::new(aggression);
    let (bot_events, last_pot) = run_bot_turns(&mut game, &bot, opponent_name);

    if matches!(game.phase, GamePhase::HandComplete | GamePhase::Showdown) {
        let pot = hand_end_pot(&game, last_pot);
        handle_hand_end(ctx, &mut game, opponent_name, aggression, stake, &bot_events, pot).await;
        return Ok(());
    }

    show_hand_state(ctx, &game, opponent_name, &bot_events);
    save_session(ctx, stake, opponent_name, aggression, game);
    Ok(())
}

// ── Player action ─────────────────────────────────────────────────────────────

async fn execute_action(ctx: &CommandContext<'_>, subcmd: &str) -> anyhow::Result<()> {
    let (stake, opponent_name, aggression, mut game) = match get_poker_session(ctx) {
        Some(v) => v,
        None => return Ok(()),
    };

    if !game.is_player_turn() {
        if matches!(game.phase, GamePhase::HandComplete | GamePhase::Showdown) {
            ctx.whisper_success("Hand over. !pk deal for next hand or !pk quit.");
        } else {
            ctx.whisper_success("Not your turn.");
        }
        return Ok(());
    }

    let available = game.available_actions();
    let player_action = match parse_action(ctx, subcmd, &available, &game) {
        Some(a) => a,
        None => return Ok(()),
    };

    let pot_before_player = game.pot;
    let player_desc = action_display(&player_action, "You");
    game.apply_action(Player::Human, player_action);

    // Hand ended from player action (player fold or river complete)
    if matches!(game.phase, GamePhase::HandComplete | GamePhase::Showdown) {
        let pot = hand_end_pot(&game, pot_before_player);
        let events = vec![player_desc];
        handle_hand_end(ctx, &mut game, opponent_name, aggression, stake, &events, pot).await;
        return Ok(());
    }

    // Run bot until player's turn or hand ends
    let bot = RuleBasedBot::new(aggression);
    let (mut bot_events, last_pot) = run_bot_turns(&mut game, &bot, opponent_name);

    if matches!(game.phase, GamePhase::HandComplete | GamePhase::Showdown) {
        let pot = hand_end_pot(&game, last_pot);
        let mut all_events = vec![player_desc];
        all_events.append(&mut bot_events);
        handle_hand_end(ctx, &mut game, opponent_name, aggression, stake, &all_events, pot).await;
        return Ok(());
    }

    // Hand ongoing — show new state
    let mut all_events = vec![player_desc];
    all_events.append(&mut bot_events);
    show_hand_state(ctx, &game, opponent_name, &all_events);
    save_session(ctx, stake, opponent_name, aggression, game);
    Ok(())
}

// ── Status ────────────────────────────────────────────────────────────────────

async fn execute_status(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let sessions = ctx.state.casino_sessions.lock().expect("casino sessions lock poisoned");
    match sessions.get(ctx.sender) {
        Some(CasinoSession::Poker { opponent_name, game, .. }) => {
            let g: &GameState = game;
            let is_player_turn = g.is_player_turn();
            ctx.whisper_success(format!(
                "[H{}] vs {} | You: {} | {}: {} | Pot: {} | Stack: {}",
                g.hand_number, opponent_name,
                cards_str(&g.player_cards),
                phase_label(g.phase), board_str(&g.board),
                g.pot, chips_str(g.player_stack as i64),
            ));
            drop(sessions);
            if is_player_turn {
                ctx.whisper_success("!pk fold/call/check/raise <n>/allin | !pk quit");
            } else {
                ctx.whisper_success("!pk deal for next hand | !pk quit");
            }
        }
        Some(s) => {
            let which = session_label(s);
            drop(sessions);
            ctx.whisper_success(format!("In a {which} game. Usage: !poker <stake>"));
        }
        None => {
            drop(sessions);
            ctx.whisper_success("Usage: !poker <stake> (25–5000) | opponents: Glass Joe → Mike Tyson");
        }
    }
    Ok(())
}

// ── Quit ──────────────────────────────────────────────────────────────────────

async fn execute_quit(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let (stake, opponent_name, _, game) = match get_poker_session(ctx) {
        Some(v) => v,
        None => return Ok(()),
    };

    ctx.state.casino_sessions.lock().expect("casino sessions lock poisoned").remove(ctx.sender);

    let Some(player_uuid) = ctx.require_player_uuid().await else { return Ok(()); };
    let credit = game.player_stack as i64;
    let net = credit - stake;
    let sign = if net >= 0 { "+" } else { "" };
    if credit > stake {
        match ctx.state.api.casino_win(&player_uuid, credit).await {
            Ok(win) => {
                let note = if win.alimony_paid > 0 { format!(" (-{} alimony)", chips_str(win.alimony_paid)) } else { String::new() };
                ctx.whisper_success(format!(
                    "Quit poker vs {}. Returned {}. Net: {sign}{}{note} | Balance: {}",
                    opponent_name, chips_str(credit), chips_str(net), chips_str(win.chips)
                ));
            }
            Err(e) => {
                eprintln!("[Poker] payout failed for {player_uuid}: {e:?}");
                ctx.whisper_error(format!(
                    "Quit poker vs {}. Returned {}, but payout failed. Contact an admin.",
                    opponent_name, chips_str(credit)
                ));
            }
        }
    } else if credit > 0 {
        let bal = ctx.state.api.casino_adjust(&player_uuid, credit).await.unwrap_or(0);
        ctx.whisper_success(format!(
            "Quit poker vs {}. Returned {}. Net: {sign}{} | Balance: {}",
            opponent_name, chips_str(credit), chips_str(net), chips_str(bal)
        ));
    } else {
        let bal = ctx.state.api.casino_get_balance(&player_uuid).await.map(|b| b.chips);
        ctx.whisper_success(format!(
            "Quit poker vs {}. Returned {}. Net: {sign}{} | Balance: {}",
            opponent_name, chips_str(credit), chips_str(net), balance_str(bal)
        ));
    }
    Ok(())
}

// ── Hand end ──────────────────────────────────────────────────────────────────

async fn handle_hand_end(
    ctx: &CommandContext<'_>,
    game: &mut GameState,
    opponent_name: &'static str,
    aggression: f64,
    stake: i64,
    events: &[String],
    _pot: u32,
) {
    let bot_won = if let Some(result) = &game.showdown_result {
        result.winner == Some(Player::Bot)
    } else {
        // HandComplete = fold; player folded → bot wins
        game.last_action.as_ref()
            .map(|(actor, _)| *actor == Player::Human)
            .unwrap_or(false)
    };

    if bot_won {
        ctx.state.api.casino_jackpot_rake(stake).await;
    }

    let result_line = format_hand_result(game, opponent_name);
    if events.is_empty() {
        ctx.whisper_success(result_line);
    } else {
        ctx.whisper_success(format!("{} | {}", events.join(" → "), result_line));
    }

    if game.player_stack == 0 || game.bot_stack == 0 {
        ctx.state.casino_sessions.lock().expect("casino sessions lock poisoned").remove(ctx.sender);
        let credit = game.player_stack as i64;
        let net = credit - stake;
        let Some(player_uuid) = ctx.require_player_uuid().await else { return };
        let sign = if net >= 0 { "+" } else { "" };
        let who = if game.player_stack == 0 { "You're bust" } else { "Bot is bust" };
        if credit > stake {
            match ctx.state.api.casino_win(&player_uuid, credit).await {
                Ok(win) => {
                    let note = if win.alimony_paid > 0 { format!(" (-{} alimony)", chips_str(win.alimony_paid)) } else { String::new() };
                    ctx.whisper_success(format!("{who}! Net: {sign}{}{note} | Balance: {}", chips_str(net), chips_str(win.chips)));
                }
                Err(e) => {
                    eprintln!("[Poker] payout failed for {player_uuid}: {e:?}");
                    ctx.whisper_error(format!("{who}! Net: {sign}{}, but payout failed. Contact an admin.", chips_str(net)));
                }
            }
        } else if credit > 0 {
            let bal = ctx.state.api.casino_adjust(&player_uuid, credit).await.unwrap_or(0);
            ctx.whisper_success(format!("{who}! Net: {sign}{} | Balance: {}", chips_str(net), chips_str(bal)));
        } else {
            let bal = ctx.state.api.casino_get_balance(&player_uuid).await.map(|b| b.chips);
            ctx.whisper_success(format!("{who}! Net: {sign}{} | Balance: {}", chips_str(net), balance_str(bal)));
        }
    } else {
        save_session(ctx, stake, opponent_name, aggression, game.clone());
        ctx.whisper_success(format!(
            "Your stack: {} | Bot: {} | !pk deal or !pk quit",
            chips_str(game.player_stack as i64), chips_str(game.bot_stack as i64)
        ));
    }
}

fn format_hand_result(game: &GameState, opponent_name: &'static str) -> String {
    if let Some(result) = &game.showdown_result {
        let winner_label = match result.winner {
            Some(Player::Human) => "You win!".to_string(),
            Some(Player::Bot) => format!("{opponent_name} wins (→ jackpot)!"),
            None => "Split pot!".to_string(),
        };
        format!(
            "[H{}] Showdown: You {} ({}) vs {} {} ({}) — {} Pot: {}",
            game.hand_number,
            cards_str(&game.player_cards), result.player_hand.description,
            opponent_name,
            cards_str(&game.bot_cards), result.bot_hand.description,
            winner_label, chips_str(result.pot_won as i64),
        )
    } else {
        let (folder_name, winner_label) = match &game.last_action {
            Some((Player::Human, Action::Fold)) => ("You", format!("{opponent_name} wins (→ jackpot)!")),
            _ => (opponent_name, "You win!".to_string()),
        };
        format!("[H{}] {} folds — {}", game.hand_number, folder_name, winner_label)
    }
}

// ── Bot turn runner ────────────────────────────────────────────────────────────

fn run_bot_turns(
    game: &mut GameState,
    bot: &RuleBasedBot,
    opponent_name: &'static str,
) -> (Vec<String>, u32) {
    let mut events = Vec::new();
    let mut last_pot = game.pot;

    while !matches!(game.phase, GamePhase::HandComplete | GamePhase::Showdown)
        && !game.is_player_turn()
    {
        let old_phase = game.phase;
        last_pot = game.pot;
        let action = bot.decide(game);
        events.push(action_display(&action, opponent_name));
        game.apply_action(Player::Bot, action);

        let new_phase = game.phase;
        if old_phase != new_phase
            && matches!(new_phase, GamePhase::Flop | GamePhase::Turn | GamePhase::River)
        {
            events.push(format!("{}: {}", phase_label(new_phase), board_str(&game.board)));
        }
    }

    (events, last_pot)
}

// ── Display ───────────────────────────────────────────────────────────────────

fn show_hand_state(
    ctx: &CommandContext<'_>,
    game: &GameState,
    opponent_name: &'static str,
    events: &[String],
) {
    ctx.whisper_success(format!(
        "[H{}] vs {} | You: {} | {}: {} | Pot: {} | Stack: {}",
        game.hand_number, opponent_name,
        cards_str(&game.player_cards),
        phase_label(game.phase), board_str(&game.board),
        game.pot, chips_str(game.player_stack as i64),
    ));

    let available = game.available_actions();
    let prompt = action_prompt_str(&available, game.player_stack);
    if events.is_empty() {
        ctx.whisper_success(prompt);
    } else {
        ctx.whisper_success(format!("{} | {}", events.join(" → "), prompt));
    }
}

fn cards_str(cards: &[Card]) -> String {
    cards.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" ")
}

fn board_str(board: &[Card]) -> String {
    if board.is_empty() { "-".to_string() } else { cards_str(board) }
}

fn phase_label(phase: GamePhase) -> &'static str {
    match phase {
        GamePhase::Preflop => "Preflop",
        GamePhase::Flop => "Flop",
        GamePhase::Turn => "Turn",
        GamePhase::River => "River",
        _ => "Done",
    }
}

fn action_display(action: &Action, actor: &str) -> String {
    match action {
        Action::Fold => format!("{actor} folds"),
        Action::Check => format!("{actor} checks"),
        Action::Call(n) => format!("{actor} calls {n}"),
        Action::Bet(n) => format!("{actor} bets {n}"),
        Action::Raise(n) => format!("{actor} raises to {n}"),
        Action::AllIn(n) => format!("{actor} all-in {n}"),
    }
}

fn action_prompt_str(av: &AvailableActions, player_stack: u32) -> String {
    let mut parts: Vec<String> = Vec::new();
    if av.can_fold { parts.push("!pk fold".into()); }
    if av.can_check { parts.push("!pk check".into()); }
    if let Some(n) = av.can_call { parts.push(format!("!pk call {n}")); }
    if let Some(n) = av.min_bet { parts.push(format!("!pk bet {n}+")); }
    if let Some(n) = av.min_raise { parts.push(format!("!pk raise {n}+")); }
    if player_stack > 0 { parts.push(format!("!pk allin {player_stack}")); }
    parts.push("!pk quit".into());
    parts.join(" | ")
}

// ── Action parser ─────────────────────────────────────────────────────────────

fn parse_action(
    ctx: &CommandContext<'_>,
    subcmd: &str,
    available: &AvailableActions,
    game: &GameState,
) -> Option<Action> {
    match subcmd {
        "fold" | "f" => {
            if !available.can_fold {
                ctx.whisper_success("Nothing to fold against. Try !pk check.");
                return None;
            }
            Some(Action::Fold)
        }
        "check" | "x" => {
            if !available.can_check {
                if let Some(n) = available.can_call {
                    ctx.whisper_success(format!("Bet to call. !pk call {n} or !pk fold."));
                } else {
                    ctx.whisper_success("Can't check here. !pk allin or !pk fold.");
                }
                return None;
            }
            Some(Action::Check)
        }
        "call" | "c" => match available.can_call {
            Some(n) => Some(Action::Call(n)),
            None => {
                if available.can_check {
                    ctx.whisper_success("No bet to call. Try !pk check.");
                } else {
                    ctx.whisper_success("Nothing to call. Use !pk allin.");
                }
                None
            }
        },
        "bet" | "raise" | "r" => {
            let amt_str = ctx.args.get(1).copied().unwrap_or("");
            let Ok(amt) = amt_str.parse::<u32>() else {
                ctx.whisper_success("Usage: !pk raise <amount>");
                return None;
            };
            if let Some(min_bet) = available.min_bet {
                if amt < min_bet {
                    ctx.whisper_success(format!("Min bet: {min_bet}."));
                    return None;
                }
                if amt >= available.max_raise {
                    Some(Action::AllIn(game.player_bet + game.player_stack))
                } else {
                    Some(Action::Bet(amt))
                }
            } else if let Some(min_raise) = available.min_raise {
                if amt < min_raise {
                    ctx.whisper_success(format!("Min raise to: {min_raise}."));
                    return None;
                }
                if amt >= available.max_raise {
                    Some(Action::AllIn(game.player_bet + game.player_stack))
                } else {
                    Some(Action::Raise(amt))
                }
            } else {
                ctx.whisper_success("Can't bet/raise now.");
                None
            }
        }
        "allin" | "all" => Some(Action::AllIn(game.player_bet + game.player_stack)),
        _ => {
            ctx.whisper_success("Unknown action. !pk call/check/fold/raise <n>/allin");
            None
        }
    }
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn hand_end_pot(game: &GameState, fallback: u32) -> u32 {
    game.showdown_result.as_ref().map(|r| r.pot_won).unwrap_or(fallback)
}

fn get_poker_session(ctx: &CommandContext<'_>) -> Option<(i64, &'static str, f64, GameState)> {
    let sessions = ctx.state.casino_sessions.lock().expect("casino sessions lock poisoned");
    match sessions.get(ctx.sender) {
        Some(CasinoSession::Poker { stake, opponent_name, aggression, game }) => {
            Some((*stake, *opponent_name, *aggression, (**game).clone()))
        }
        Some(s) => {
            let which = session_label(s);
            drop(sessions);
            ctx.whisper_success(format!("In a {which} game, not poker."));
            None
        }
        None => {
            drop(sessions);
            ctx.whisper_success("No active poker session. Start: !poker <stake>");
            None
        }
    }
}

fn save_session(
    ctx: &CommandContext<'_>,
    stake: i64,
    opponent_name: &'static str,
    aggression: f64,
    game: GameState,
) {
    ctx.state.casino_sessions.lock().expect("casino sessions lock poisoned")
        .insert(ctx.sender.to_owned(), CasinoSession::Poker {
            stake,
            opponent_name,
            aggression,
            game: Box::new(game),
        });
}

fn session_label(s: &CasinoSession) -> &'static str {
    match s {
        CasinoSession::Craps { .. }       => "craps (!craps roll)",
        CasinoSession::Hilo { .. }        => "hilo (!hilo higher/lower/cashout)",
        CasinoSession::Blackjack { .. }   => "blackjack (!bj hit/stand)",
        CasinoSession::Poker { .. }       => "poker (!pk call/check/fold/raise/allin/quit)",
        CasinoSession::ConnectFour { .. } => "Connect Four (!c4 <1-7>)",
        CasinoSession::Chess { .. }       => "chess (!chess <from> <to>)",
    }
}
