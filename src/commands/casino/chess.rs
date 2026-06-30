use rand::seq::SliceRandom;
use shakmaty::{Board, Chess as ChessPos, Color, File, Move, Outcome, Piece, Position, Rank, Role, Square};
use shakmaty::uci::UciMove;
use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use crate::structure::mineflayer::bot::CasinoSession;
use super::chips_str;

const MIN_STAKE: i64 = 25;
const MAX_STAKE: i64 = 5000;

// (name, ai_depth): 0=random, 1=greedy, 2-4=alpha-beta
const OPPONENTS: &[(&str, u32)] = &[
    ("Glass Joe",     0),
    ("Piston Honda",  1),
    ("Bald Bull",     2),
    ("Soda Popinski", 3),
    ("Mike Tyson",    4),
];

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["chess"],
    description: "Chess vs NPC. !chess white|black <stake> | !chess <from> <to> [promo] | !chess quit",
    whitelisted: false,
    execute,
};

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let arg0 = ctx.args.first().copied().unwrap_or("").to_ascii_lowercase();
        match arg0.as_str() {
            "" => {
                let session = {
                    let sessions = ctx.state.casino_sessions.lock().expect("lock");
                    sessions.get(ctx.sender).and_then(|s| match s {
                        CasinoSession::Chess { bet, player_color, position, opponent_name, ai_depth } => {
                            Some((*bet, *player_color, (**position).clone(), *opponent_name, *ai_depth))
                        }
                        _ => None,
                    })
                };
                match session {
                    Some((stake, player_color, position, opponent_name, _)) => {
                        show_board(&ctx, &position, player_color);
                        ctx.whisper(format!(
                            "Chess: You ({}) vs {} | Stake: {} | !chess <from> <to> or !chess quit",
                            color_name(player_color), opponent_name, chips_str(stake)
                        ));
                    }
                    None => ctx.whisper("No active chess game. Start: !chess white|black <stake>"),
                }
                Ok(())
            }
            "quit" | "q" => execute_quit(&ctx).await,
            "white" | "w" | "black" | "b" => execute_new_game(&ctx, &arg0).await,
            _ => execute_move(&ctx).await,
        }
    })
}

async fn execute_new_game(ctx: &CommandContext<'_>, color_str: &str) -> anyhow::Result<()> {
    {
        let sessions = ctx.state.casino_sessions.lock().expect("lock");
        if let Some(s) = sessions.get(ctx.sender) {
            ctx.whisper(format!("Already in a {} game. Finish it first.", session_label(s)));
            return Ok(());
        }
    }

    let player_color = if color_str.starts_with('w') { Color::White } else { Color::Black };
    let stake_str = ctx.args.get(1).copied().unwrap_or("");
    let stake: i64 = match stake_str.parse() {
        Ok(n) => n,
        Err(_) => {
            ctx.whisper("Usage: !chess white|black <stake>");
            return Ok(());
        }
    };
    if stake < MIN_STAKE || stake > MAX_STAKE {
        ctx.whisper(format!("Stake must be {}-{}.", chips_str(MIN_STAKE), chips_str(MAX_STAKE)));
        return Ok(());
    }

    match ctx.state.api.casino_adjust(ctx.sender, -stake).await {
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

    let &(opponent_name, ai_depth) = OPPONENTS.choose(&mut rand::thread_rng()).unwrap();
    let position = ChessPos::default();

    ctx.state.casino_sessions.lock().expect("lock")
        .insert(ctx.sender.to_owned(), CasinoSession::Chess {
            bet: stake,
            player_color,
            position: Box::new(position.clone()),
            opponent_name,
            ai_depth,
        });

    ctx.whisper(format!(
        "Chess: You ({}) vs {} | Stake: {} | !chess <from> <to>",
        color_name(player_color), opponent_name, chips_str(stake)
    ));
    show_board(ctx, &position, player_color);

    if player_color == Color::Black {
        execute_bot_turn(ctx, position, stake, player_color, opponent_name, ai_depth).await?;
    } else {
        ctx.whisper("Your move! e.g. !chess e2 e4");
    }

    Ok(())
}

async fn execute_move(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let (stake, player_color, position, opponent_name, ai_depth) = {
        let sessions = ctx.state.casino_sessions.lock().expect("lock");
        match sessions.get(ctx.sender) {
            Some(CasinoSession::Chess { bet, player_color, position, opponent_name, ai_depth }) => {
                (*bet, *player_color, (**position).clone(), *opponent_name, *ai_depth)
            }
            Some(s) => {
                ctx.whisper(format!("In a {} game, not chess.", session_label(s)));
                return Ok(());
            }
            None => {
                ctx.whisper("No active chess game. Start: !chess white|black <stake>");
                return Ok(());
            }
        }
    };

    if position.turn() != player_color {
        ctx.whisper("Not your turn.");
        return Ok(());
    }

    let from  = ctx.args.first().copied().unwrap_or("").to_ascii_lowercase();
    let to    = ctx.args.get(1).copied().unwrap_or("").to_ascii_lowercase();
    if from.is_empty() || to.is_empty() {
        ctx.whisper("Usage: !chess <from> <to> [promo]. E.g. !chess e2 e4 or !chess e7 e8 q");
        return Ok(());
    }
    let promo   = ctx.args.get(2).copied().unwrap_or("").to_ascii_lowercase();
    let uci_str = format!("{}{}{}", from, to, promo);

    let uci: UciMove = match uci_str.parse() {
        Ok(u) => u,
        Err(_) => {
            ctx.whisper(format!("Invalid move: {}. Example: !chess e2 e4", uci_str));
            return Ok(());
        }
    };
    let player_move = match uci.to_move(&position) {
        Ok(m) => m,
        Err(_) => {
            ctx.whisper(format!("Illegal move: {}. Type !chess for board.", uci_str));
            return Ok(());
        }
    };

    let mut pos_after = position.clone();
    pos_after.play_unchecked(&player_move);
    let move_str = fmt_move(&player_move);

    if let Some(outcome) = pos_after.outcome() {
        ctx.state.casino_sessions.lock().expect("lock").remove(ctx.sender);
        show_board(ctx, &pos_after, player_color);
        return finish_game(ctx, outcome, player_color, stake, opponent_name,
            &format!("You played {}. ", move_str)).await;
    }
    if pos_after.halfmoves() >= 100 {
        ctx.state.casino_sessions.lock().expect("lock").remove(ctx.sender);
        show_board(ctx, &pos_after, player_color);
        let bal = ctx.state.api.casino_adjust(ctx.sender, stake).await.unwrap_or(0);
        ctx.whisper(format!("You played {}. Draw by 50-move rule. Stake returned. Balance: {}", move_str, chips_str(bal)));
        return Ok(());
    }

    let check = if !pos_after.checkers().is_empty() { " (check!)" } else { "" };
    ctx.whisper(format!("You played {}{} | {} thinking...", move_str, check, opponent_name));
    execute_bot_turn(ctx, pos_after, stake, player_color, opponent_name, ai_depth).await
}

async fn execute_bot_turn(
    ctx: &CommandContext<'_>,
    position: ChessPos,
    stake: i64,
    player_color: Color,
    opponent_name: &'static str,
    ai_depth: u32,
) -> anyhow::Result<()> {
    let bot_color = !player_color;
    let pos_clone = position.clone();

    let bot_move = tokio::task::spawn_blocking(move || {
        compute_ai_move(&pos_clone, ai_depth, bot_color)
    }).await.unwrap_or(None);

    let Some(bot_move) = bot_move else {
        ctx.state.casino_sessions.lock().expect("lock").remove(ctx.sender);
        let bal = ctx.state.api.casino_adjust(ctx.sender, stake).await.unwrap_or(0);
        ctx.whisper(format!("Draw! Stake returned. Balance: {}", chips_str(bal)));
        return Ok(());
    };

    let mut pos_after = position.clone();
    pos_after.play_unchecked(&bot_move);
    let bot_move_str = fmt_move(&bot_move);

    if let Some(outcome) = pos_after.outcome() {
        ctx.state.casino_sessions.lock().expect("lock").remove(ctx.sender);
        show_board(ctx, &pos_after, player_color);
        return finish_game(ctx, outcome, player_color, stake, opponent_name,
            &format!("{} played {}. ", opponent_name, bot_move_str)).await;
    }
    if pos_after.halfmoves() >= 100 {
        ctx.state.casino_sessions.lock().expect("lock").remove(ctx.sender);
        show_board(ctx, &pos_after, player_color);
        let bal = ctx.state.api.casino_adjust(ctx.sender, stake).await.unwrap_or(0);
        ctx.whisper(format!("{} played {}. Draw by 50-move rule. Stake returned. Balance: {}", opponent_name, bot_move_str, chips_str(bal)));
        return Ok(());
    }

    ctx.state.casino_sessions.lock().expect("lock")
        .insert(ctx.sender.to_owned(), CasinoSession::Chess {
            bet: stake,
            player_color,
            position: Box::new(pos_after.clone()),
            opponent_name,
            ai_depth,
        });

    let check = if !pos_after.checkers().is_empty() { " — CHECK!" } else { "" };
    show_board(ctx, &pos_after, player_color);
    ctx.whisper(format!("{} played {}{} | !chess <from> <to>", opponent_name, bot_move_str, check));
    Ok(())
}

async fn finish_game(
    ctx: &CommandContext<'_>,
    outcome: Outcome,
    player_color: Color,
    stake: i64,
    opponent_name: &'static str,
    prefix: &str,
) -> anyhow::Result<()> {
    match outcome {
        Outcome::Decisive { winner } if winner == player_color => {
            let bal = ctx.state.api.casino_adjust(ctx.sender, stake * 2).await.unwrap_or(0);
            ctx.whisper(format!("{}Checkmate! You WIN! +{} | Balance: {}", prefix, chips_str(stake), chips_str(bal)));
        }
        Outcome::Decisive { .. } => {
            ctx.state.api.casino_jackpot_rake(stake).await;
            let bal = ctx.state.api.casino_get_balance(ctx.sender).await.map(|b| b.chips).unwrap_or(0);
            ctx.whisper(format!("{}Checkmate! {} wins. -{} | Balance: {}", prefix, opponent_name, chips_str(stake), chips_str(bal)));
        }
        Outcome::Draw => {
            let bal = ctx.state.api.casino_adjust(ctx.sender, stake).await.unwrap_or(0);
            ctx.whisper(format!("{}Draw! Stake returned. Balance: {}", prefix, chips_str(bal)));
        }
    }
    Ok(())
}

async fn execute_quit(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let (stake, opponent_name) = {
        let sessions = ctx.state.casino_sessions.lock().expect("lock");
        match sessions.get(ctx.sender) {
            Some(CasinoSession::Chess { bet, opponent_name, .. }) => (*bet, *opponent_name),
            Some(s) => {
                ctx.whisper(format!("In a {} game, not chess.", session_label(s)));
                return Ok(());
            }
            None => {
                ctx.whisper("No active chess game.");
                return Ok(());
            }
        }
    };
    ctx.state.casino_sessions.lock().expect("lock").remove(ctx.sender);
    ctx.state.api.casino_jackpot_rake(stake).await;
    let bal = ctx.state.api.casino_get_balance(ctx.sender).await.map(|b| b.chips).unwrap_or(0);
    ctx.whisper(format!("Forfeited vs {}. -{} | Balance: {}", opponent_name, chips_str(stake), chips_str(bal)));
    Ok(())
}

fn show_board(ctx: &CommandContext<'_>, pos: &ChessPos, player_color: Color) {
    let board = pos.board();
    // Math Bold Small a-h (U+1D41A-1D421) = 4px, matches chess piece width
    ctx.whisper(if player_color == Color::White {
        "# \u{1D41A} \u{1D41B} \u{1D41C} \u{1D41D} \u{1D41E} \u{1D41F} \u{1D420} \u{1D421}"
    } else {
        "# \u{1D421} \u{1D420} \u{1D41F} \u{1D41E} \u{1D41D} \u{1D41C} \u{1D41B} \u{1D41A}"
    });
    let rank_iter: Vec<u32> = if player_color == Color::White { (0..8).rev().collect() } else { (0..8).collect() };
    let file_iter: Vec<u32> = if player_color == Color::White { (0..8).collect() }      else { (0..8).rev().collect() };
    for rank_idx in rank_iter {
        let rank = Rank::new(rank_idx);
        let mut line = format!("{} ", rank_idx + 1);
        for (i, &file_idx) in file_iter.iter().enumerate() {
            let sq = Square::from_coords(File::new(file_idx), rank);
            if i > 0 { line.push(' '); }
            match board.piece_at(sq) {
                Some(Piece { color, role }) => line.push(piece_char(role, color)),
                None => line.push('\u{25A2}'), // ▢ unifont 3.5px, matches King/Queen/Rook/Bishop
            }
        }
        ctx.whisper(line);
    }
}

fn fmt_move(mv: &Move) -> String {
    match mv {
        Move::Castle { king, rook } => {
            if rook.file() > king.file() { "O-O".to_owned() } else { "O-O-O".to_owned() }
        }
        Move::Normal { from, to, promotion: Some(p), .. } => format!("{}-{}={}", from, to, role_char(*p)),
        Move::Normal { from, to, .. } | Move::EnPassant { from, to } => format!("{}-{}", from, to),
        Move::Put { .. } => "?".to_owned(),
    }
}

fn compute_ai_move(pos: &ChessPos, depth: u32, bot_color: Color) -> Option<Move> {
    let mut moves: Vec<Move> = pos.legal_moves().into_iter().collect();
    if moves.is_empty() { return None; }
    if depth == 0 {
        return moves.choose(&mut rand::thread_rng()).cloned();
    }
    moves.sort_by_key(|m| if mv_is_capture(m) { 0i32 } else { 1i32 });
    moves.into_iter().max_by_key(|mv| {
        let mut next = pos.clone();
        next.play_unchecked(mv);
        alpha_beta(&next, depth - 1, i32::MIN, i32::MAX, bot_color)
    })
}

fn alpha_beta(pos: &ChessPos, depth: u32, mut alpha: i32, mut beta: i32, bot_color: Color) -> i32 {
    if let Some(outcome) = pos.outcome() {
        return match outcome {
            Outcome::Decisive { winner } if winner == bot_color => 100_000,
            Outcome::Decisive { .. } => -100_000,
            Outcome::Draw => 0,
        };
    }
    if pos.halfmoves() >= 100 { return 0; }
    if depth == 0 { return material_score(pos.board(), bot_color); }

    let mut moves: Vec<Move> = pos.legal_moves().into_iter().collect();
    moves.sort_by_key(|m| if mv_is_capture(m) { 0i32 } else { 1i32 });

    if pos.turn() == bot_color {
        let mut best = i32::MIN;
        for mv in &moves {
            let mut next = pos.clone();
            next.play_unchecked(mv);
            best = best.max(alpha_beta(&next, depth - 1, alpha, beta, bot_color));
            alpha = alpha.max(best);
            if beta <= alpha { break; }
        }
        best
    } else {
        let mut best = i32::MAX;
        for mv in &moves {
            let mut next = pos.clone();
            next.play_unchecked(mv);
            best = best.min(alpha_beta(&next, depth - 1, alpha, beta, bot_color));
            beta = beta.min(best);
            if beta <= alpha { break; }
        }
        best
    }
}

fn material_score(board: &Board, color: Color) -> i32 {
    const VALS: [(Role, i32); 5] = [
        (Role::Pawn, 100), (Role::Knight, 320), (Role::Bishop, 330),
        (Role::Rook, 500), (Role::Queen, 900),
    ];
    VALS.iter().map(|&(role, val)| {
        let mine   = (board.by_color(color)  & board.by_role(role)).count() as i32;
        let theirs = (board.by_color(!color) & board.by_role(role)).count() as i32;
        (mine - theirs) * val
    }).sum()
}

fn mv_is_capture(mv: &Move) -> bool {
    matches!(mv, Move::Normal { capture: Some(_), .. } | Move::EnPassant { .. })
}

fn piece_char(role: Role, color: Color) -> char {
    match (color, role) {
        (Color::White, Role::King)   => '♔',
        (Color::White, Role::Queen)  => '♕',
        (Color::White, Role::Rook)   => '♖',
        (Color::White, Role::Bishop) => '♗',
        (Color::White, Role::Knight) => '♘',
        (Color::White, Role::Pawn)   => '♙',
        (Color::Black, Role::King)   => '♚',
        (Color::Black, Role::Queen)  => '♛',
        (Color::Black, Role::Rook)   => '♜',
        (Color::Black, Role::Bishop) => '♝',
        (Color::Black, Role::Knight) => '♞',
        (Color::Black, Role::Pawn)   => '♟',
    }
}

fn role_char(role: Role) -> char {
    match role {
        Role::Pawn   => 'P',
        Role::Knight => 'N',
        Role::Bishop => 'B',
        Role::Rook   => 'R',
        Role::Queen  => 'Q',
        Role::King   => 'K',
    }
}

fn color_name(c: Color) -> &'static str {
    match c { Color::White => "White", Color::Black => "Black" }
}

fn session_label(s: &CasinoSession) -> &'static str {
    match s {
        CasinoSession::Craps { .. }       => "craps",
        CasinoSession::Hilo { .. }        => "hilo",
        CasinoSession::Blackjack { .. }   => "blackjack",
        CasinoSession::Poker { .. }       => "poker",
        CasinoSession::ConnectFour { .. } => "Connect Four",
        CasinoSession::Chess { .. }       => "chess",
    }
}
