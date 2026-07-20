use connect_four_ai::{AIPlayer, Difficulty, Position};
use rand::Rng;
use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::mineflayer::bot::CasinoSession;
use super::{balance_str, chips_str, format_alimony};

const MIN_STAKE: i64 = 25;
const MAX_STAKE: i64 = 5000;

const OPPONENTS: &[(&str, Difficulty)] = &[
    ("Glass Joe",     Difficulty::Easy),
    ("Piston Honda",  Difficulty::Medium),
    ("Bald Bull",     Difficulty::Hard),
    ("Soda Popinski", Difficulty::Hard),
    ("Mike Tyson",    Difficulty::Impossible),
];

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["c4", "connect4"],
    description: "Connect Four vs NPC. !c4 <stake> | !c4 <1-7> | !c4 quit",
    whitelisted: false,
    execute,
};

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let subcmd = ctx.args.first().copied().unwrap_or("").to_ascii_lowercase();
        match subcmd.as_str() {
            "" => {
                let session = {
                    let sessions = ctx.state.casino_sessions.lock().expect("casino sessions lock poisoned");
                    sessions.get(ctx.sender).and_then(|s| match s {
                        CasinoSession::ConnectFour { stake, opponent_name, position, .. } => {
                            Some((*stake, *opponent_name, *position))
                        }
                        _ => None,
                    })
                };
                match session {
                    Some((stake, opponent_name, position)) => {
                        show_board(&ctx, &position).await;
                        ctx.whisper_success(format!(
                            "Your turn (\u{25D5}) vs {} | Stake: {} | !c4 <1-7> or !c4 quit",
                            opponent_name,
                            chips_str(stake)
                        ));
                    }
                    None => ctx.whisper_success("No active C4 game. Start: !c4 <stake>"),
                }
                Ok(())
            }
            "quit" | "q" => execute_quit(&ctx).await,
            _ => {
                if let Ok(col) = subcmd.parse::<u8>() {
                    let in_session = {
                        let sessions = ctx.state.casino_sessions.lock().expect("casino sessions lock poisoned");
                        sessions.contains_key(ctx.sender)
                    };
                    if in_session {
                        if (1..=7).contains(&col) {
                            execute_drop(&ctx, col).await
                        } else {
                            ctx.whisper_success("Column must be 1-7.");
                            Ok(())
                        }
                    } else {
                        execute_new_game(&ctx, &subcmd).await
                    }
                } else {
                    execute_new_game(&ctx, &subcmd).await
                }
            }
        }
    })
}

async fn execute_new_game(ctx: &CommandContext<'_>, stake_str: &str) -> anyhow::Result<()> {
    {
        let sessions = ctx.state.casino_sessions.lock().expect("casino sessions lock poisoned");
        if let Some(s) = sessions.get(ctx.sender) {
            ctx.whisper_success(format!("Already in a {} game. Finish it first.", session_label(s)));
            return Ok(());
        }
    }

    let stake: i64 = match stake_str.parse() {
        Ok(n) => n,
        Err(_) => {
            ctx.whisper_success("Usage: !c4 <stake>");
            return Ok(());
        }
    };

    if stake < MIN_STAKE || stake > MAX_STAKE {
        ctx.whisper_success(format!("Stake must be {}-{}.", chips_str(MIN_STAKE), chips_str(MAX_STAKE)));
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

    let idx = rand::thread_rng().gen_range(0..OPPONENTS.len());
    let (opponent_name, difficulty) = OPPONENTS[idx];
    let position = Position::new();

    let started = super::try_start_session(ctx.state, ctx.sender, CasinoSession::ConnectFour {
        stake,
        opponent_name,
        difficulty,
        position,
    });
    if !started {
        let bal = ctx.state.api.casino_adjust(&player_uuid, stake).await.unwrap_or(0);
        ctx.whisper_success(format!("Already in another game — this stake refunded. Balance: {}", chips_str(bal)));
        return Ok(());
    }

    ctx.whisper_success(format!(
        "C4: You (\u{25D5}) vs {} | Stake: {} | You go first!",
        opponent_name,
        chips_str(stake)
    ));
    show_board(ctx, &position).await;
    ctx.whisper_success("!c4 <1-7> to drop | !c4 quit to forfeit");

    Ok(())
}

async fn execute_drop(ctx: &CommandContext<'_>, col: u8) -> anyhow::Result<()> {
    let (stake, opponent_name, difficulty, mut position) = {
        let sessions = ctx.state.casino_sessions.lock().expect("casino sessions lock poisoned");
        match sessions.get(ctx.sender) {
            Some(CasinoSession::ConnectFour { stake, opponent_name, difficulty, position }) => {
                (*stake, *opponent_name, *difficulty, *position)
            }
            Some(s) => {
                ctx.whisper_success(format!("In a {} game, not Connect Four.", session_label(s)));
                return Ok(());
            }
            None => {
                ctx.whisper_success("No active C4 game. Start: !c4 <stake>");
                return Ok(());
            }
        }
    };

    let Some(player_uuid) = ctx.require_player_uuid().await else { return Ok(()); };

    let col_idx = (col - 1) as usize;

    if !position.is_playable(col_idx) {
        ctx.whisper_success(format!("Column {col} is full. Pick another (1-7)."));
        return Ok(());
    }

    // Player plays
    let player_wins = position.is_winning_move(col_idx);
    position.play(col_idx);

    if player_wins {
        ctx.state.casino_sessions.lock().expect("casino sessions lock poisoned").remove(ctx.sender);
        show_board(ctx, &position).await;
        match ctx.state.api.casino_win(&player_uuid, stake * 2).await {
            Ok(win) => {
                let alimony_note = format_alimony(win.alimony_paid);
                ctx.whisper_success(format!(
                    "You WIN vs {}! +{}{alimony_note} | Balance: {}",
                    opponent_name,
                    chips_str(stake),
                    chips_str(win.chips)
                ));
            }
            Err(e) => {
                eprintln!("[Connect Four] payout failed for {player_uuid}: {e:?}");
                ctx.whisper_error(format!("You WIN vs {}, but payout failed. Contact an admin.", opponent_name));
            }
        }
        return Ok(());
    }

    if position.get_moves() == Position::BOARD_SIZE {
        ctx.state.casino_sessions.lock().expect("casino sessions lock poisoned").remove(ctx.sender);
        show_board(ctx, &position).await;
        let bal = ctx.state.api.casino_adjust(&player_uuid, stake).await.unwrap_or(0);
        ctx.whisper_success(format!("Draw! Stake returned. Balance: {}", chips_str(bal)));
        return Ok(());
    }

    // Bot plays
    let bot_col = match AIPlayer::new(difficulty).get_move(&position) {
        Some(c) => c,
        None => {
            ctx.state.casino_sessions.lock().expect("casino sessions lock poisoned").remove(ctx.sender);
            let bal = ctx.state.api.casino_adjust(&player_uuid, stake).await.unwrap_or(0);
            ctx.whisper_success(format!("Draw! Stake returned. Balance: {}", chips_str(bal)));
            return Ok(());
        }
    };

    let bot_wins = position.is_winning_move(bot_col);
    position.play(bot_col);

    if bot_wins {
        ctx.state.casino_sessions.lock().expect("casino sessions lock poisoned").remove(ctx.sender);
        show_board(ctx, &position).await;
        ctx.state.api.casino_jackpot_rake(stake).await;
        let bal = ctx.state.api.casino_get_balance(&player_uuid).await.map(|b| b.chips);
        ctx.whisper_success(format!(
            "{} wins! -{} | Balance: {}",
            opponent_name,
            chips_str(stake),
            balance_str(bal)
        ));
        return Ok(());
    }

    if position.get_moves() == Position::BOARD_SIZE {
        ctx.state.casino_sessions.lock().expect("casino sessions lock poisoned").remove(ctx.sender);
        show_board(ctx, &position).await;
        let bal = ctx.state.api.casino_adjust(&player_uuid, stake).await.unwrap_or(0);
        ctx.whisper_success(format!("Draw! Stake returned. Balance: {}", chips_str(bal)));
        return Ok(());
    }

    ctx.state.casino_sessions.lock().expect("casino sessions lock poisoned")
        .insert(ctx.sender.to_owned(), CasinoSession::ConnectFour {
            stake,
            opponent_name,
            difficulty,
            position,
        });

    show_board(ctx, &position).await;
    ctx.whisper_success(format!("Your turn (\u{25D5}) vs {} | !c4 <1-7>", opponent_name));

    Ok(())
}

async fn execute_quit(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let (stake, opponent_name) = {
        let sessions = ctx.state.casino_sessions.lock().expect("casino sessions lock poisoned");
        match sessions.get(ctx.sender) {
            Some(CasinoSession::ConnectFour { stake, opponent_name, .. }) => (*stake, *opponent_name),
            Some(s) => {
                ctx.whisper_success(format!("In a {} game, not Connect Four.", session_label(s)));
                return Ok(());
            }
            None => {
                ctx.whisper_success("No active C4 game.");
                return Ok(());
            }
        }
    };
    ctx.state.casino_sessions.lock().expect("casino sessions lock poisoned").remove(ctx.sender);
    ctx.state.api.casino_jackpot_rake(stake).await;
    let Some(player_uuid) = ctx.require_player_uuid().await else { return Ok(()); };
    let bal = ctx.state.api.casino_get_balance(&player_uuid).await.map(|b| b.chips);
    ctx.whisper_success(format!(
        "Forfeited vs {}. -{} | Balance: {}",
        opponent_name,
        chips_str(stake),
        balance_str(bal)
    ));
    Ok(())
}

async fn show_board(ctx: &CommandContext<'_>, pos: &Position) {
    // Even move count → player is to move → pos.position = player's pieces (◆)
    // Odd move count  → bot just moved or player just won → pos.position = bot's pieces
    let player_is_current = pos.get_moves() % 2 == 0;
    let mut lines = vec!["\u{1D7CF} \u{1D7D0} \u{1D7D1} \u{1D7D2} \u{1D7D3} \u{1D7D4} \u{1D7D5}".to_string()]; // 𝟏 𝟐 𝟑 𝟒 𝟓 𝟔 𝟕
    for row in (0..Position::HEIGHT).rev() {
        let mut line = String::new();
        for col in 0..Position::WIDTH {
            if col > 0 {
                line.push(' ');
            }
            let bit = 1u64 << (row + col * (Position::HEIGHT + 1));
            let ch = if pos.position & bit != 0 {
                if player_is_current { '\u{25D5}' } else { '\u{25A3}' } // ◕ ◉
            } else if pos.mask & bit != 0 {
                if player_is_current { '\u{25A3}' } else { '\u{25D5}' } // ◉ ◕
            } else {
                '\u{25A2}' // ▢
            };
            line.push(ch);
        }
        lines.push(line);
    }
    ctx.whisper_board(lines).await;
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
