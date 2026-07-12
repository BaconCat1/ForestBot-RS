use rand::Rng;

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;

use super::casino::chips_str;

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["checkers", "draughts"],
    description: "Checkers vs NPC. !checkers <chips> | !checkers a1 b2 | !checkers board | !checkers quit",
    whitelisted: false,
    bridge_ok: true,
    execute,
};

const MIN_STAKE: i64 = 50;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum Color { Red, Black }

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind { Man, King }

#[derive(Clone, Copy, PartialEq, Eq)]
struct Piece { color: Color, kind: Kind }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Pos { row: usize, col: usize }

#[derive(Clone)]
pub(crate) struct CheckersGame {
    board: [Option<Piece>; 64],
    current: Color,
}

#[derive(Clone, Copy)]
enum Difficulty { Random, Easy, Medium, Hard }

pub struct CheckersSession {
    pub game: CheckersGame,
    pub stake: i64,
    difficulty: Difficulty,
    opponent: &'static str,
    no_progress_ply: u32,
    position_history: Vec<([Option<Piece>; 64], Color)>,
}

const OPPONENTS: &[(&str, Difficulty)] = &[
    ("Glass Joe",     Difficulty::Random),
    ("Piston Honda",  Difficulty::Easy),
    ("Bald Bull",     Difficulty::Medium),
    ("Soda Popinski", Difficulty::Medium),
    ("Mike Tyson",    Difficulty::Hard),
];

// ── Board ─────────────────────────────────────────────────────────────────────

fn sq(row: usize, col: usize) -> usize { row * 8 + col }
fn is_dark(row: usize, col: usize) -> bool { (row + col) % 2 == 0 }
fn next_color(c: Color) -> Color { match c { Color::Red => Color::Black, Color::Black => Color::Red } }
fn is_promo_row(row: usize, color: Color) -> bool {
    match color { Color::Red => row == 7, Color::Black => row == 0 }
}

fn new_game() -> CheckersGame {
    let mut board = [None; 64];
    for row in 0..3usize {
        for col in 0..8usize {
            if is_dark(row, col) {
                board[sq(row, col)] = Some(Piece { color: Color::Red, kind: Kind::Man });
            }
        }
    }
    for row in 5..8usize {
        for col in 0..8usize {
            if is_dark(row, col) {
                board[sq(row, col)] = Some(Piece { color: Color::Black, kind: Kind::Man });
            }
        }
    }
    CheckersGame { board, current: Color::Red }
}

fn render_board(game: &CheckersGame) -> Vec<String> {
    let mut lines = vec!["# a b c d e f g h".to_string()];
    lines.extend((0..8usize).rev().map(|row| {
        let mut line = format!("{} ", row + 1);
        for col in 0..8usize {
            if col > 0 { line.push(' '); }
            line.push(match game.board[sq(row, col)] {
                Some(p) => match (p.color, p.kind) {
                    (Color::Red,   Kind::Man)  => 'r',
                    (Color::Red,   Kind::King) => 'R',
                    (Color::Black, Kind::Man)  => 'b',
                    (Color::Black, Kind::King) => 'B',
                },
                None => '-',
            });
        }
        line
    }));
    lines
}

// ── Move generation ───────────────────────────────────────────────────────────

const ALL_DIRS: [(i8, i8); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];

fn fwd_dirs(color: Color) -> &'static [(i8, i8)] {
    match color {
        Color::Red   => &[(1, 1), (1, -1)],
        Color::Black => &[(-1, 1), (-1, -1)],
    }
}

fn move_dirs(piece: Piece) -> &'static [(i8, i8)] {
    match piece.kind { Kind::Man => fwd_dirs(piece.color), Kind::King => &ALL_DIRS }
}

fn offset(pos: Pos, dr: i8, dc: i8) -> Option<Pos> {
    let r = pos.row as i8 + dr;
    let c = pos.col as i8 + dc;
    if r >= 0 && r < 8 && c >= 0 && c < 8 { Some(Pos { row: r as usize, col: c as usize }) } else { None }
}

fn gen_simple_moves(game: &CheckersGame) -> Vec<(Pos, Pos)> {
    let mut out = Vec::new();
    for row in 0..8usize {
        for col in 0..8usize {
            if !is_dark(row, col) { continue; }
            let Some(piece) = game.board[sq(row, col)] else { continue };
            if piece.color != game.current { continue; }
            let from = Pos { row, col };
            for &(dr, dc) in move_dirs(piece) {
                if let Some(to) = offset(from, dr, dc) {
                    if game.board[sq(to.row, to.col)].is_none() {
                        out.push((from, to));
                    }
                }
            }
        }
    }
    out
}

fn gen_jump_paths(game: &CheckersGame) -> Vec<Vec<Pos>> {
    let mut out = Vec::new();
    let mut board = game.board;
    for row in 0..8usize {
        for col in 0..8usize {
            if !is_dark(row, col) { continue; }
            let Some(piece) = game.board[sq(row, col)] else { continue };
            if piece.color != game.current { continue; }
            let from = Pos { row, col };
            board[sq(row, col)] = None;
            explore_jumps(&mut board, piece, from, vec![from], &mut out);
            board[sq(row, col)] = Some(piece);
        }
    }
    out
}

fn explore_jumps(
    board: &mut [Option<Piece>; 64],
    piece: Piece,
    from: Pos,
    path: Vec<Pos>,
    out: &mut Vec<Vec<Pos>>,
) {
    let mut extended = false;
    for &(dr, dc) in move_dirs(piece) {
        let Some(mid)  = offset(from, dr,     dc)     else { continue };
        let Some(land) = offset(from, dr * 2, dc * 2) else { continue };

        let is_enemy = board[sq(mid.row, mid.col)].map(|p| p.color != piece.color).unwrap_or(false);
        let land_free = board[sq(land.row, land.col)].is_none();
        if !is_enemy || !land_free { continue; }

        extended = true;
        let captured = board[sq(mid.row, mid.col)].take();
        let mut new_path = path.clone();
        new_path.push(land);

        let promoted = piece.kind == Kind::Man && is_promo_row(land.row, piece.color);
        if promoted {
            out.push(new_path);
        } else {
            explore_jumps(board, piece, land, new_path, out);
        }
        board[sq(mid.row, mid.col)] = captured;
    }
    if !extended && path.len() > 1 {
        out.push(path);
    }
}

fn legal_moves(game: &CheckersGame) -> Vec<Vec<Pos>> {
    let jumps = gen_jump_paths(game);
    if !jumps.is_empty() {
        jumps
    } else {
        gen_simple_moves(game).into_iter().map(|(f, t)| vec![f, t]).collect()
    }
}

fn is_game_over(game: &CheckersGame) -> bool {
    legal_moves(game).is_empty()
}

fn advance_draw_state(
    no_progress_ply: &mut u32,
    position_history: &mut Vec<([Option<Piece>; 64], Color)>,
    game: &CheckersGame,
    progress: bool,
) -> Option<&'static str> {
    if progress {
        *no_progress_ply = 0;
        position_history.clear();
    } else {
        *no_progress_ply += 1;
        if *no_progress_ply >= 80 {
            return Some("40-move rule (80 ply without capture or man advance)");
        }
    }
    let pos = (game.board, game.current);
    position_history.push(pos);
    let count = position_history.iter().filter(|&&p| p == pos).count();
    if count >= 3 {
        return Some("threefold repetition");
    }
    None
}

// ── Apply move ────────────────────────────────────────────────────────────────

// Returns Ok(true) if move makes progress (capture or man advance), Ok(false) for king-only moves.
fn apply_path(game: &mut CheckersGame, path: &[Pos]) -> Result<bool, String> {
    if path.len() < 2 {
        return Err("need at least two positions".into());
    }

    let piece = game.board[sq(path[0].row, path[0].col)]
        .ok_or_else(|| format!("no piece at {}", format_pos(path[0])))?;

    if piece.color != game.current {
        return Err("not your piece".into());
    }

    let piece_kind = piece.kind;
    let row_dist = path[0].row.abs_diff(path[1].row);
    let col_dist = path[0].col.abs_diff(path[1].col);

    let is_capture = if row_dist == 1 && col_dist == 1 {
        if path.len() != 2 {
            return Err("simple moves take exactly two positions".into());
        }
        if !gen_jump_paths(game).is_empty() {
            return Err("jump available — you must take it".into());
        }
        if !gen_simple_moves(game).iter().any(|(f, t)| *f == path[0] && *t == path[1]) {
            return Err(format!("illegal move — {} can't move to {}", format_pos(path[0]), format_pos(path[1])));
        }
        let mut p = game.board[sq(path[0].row, path[0].col)].take().unwrap();
        if p.kind == Kind::Man && is_promo_row(path[1].row, p.color) {
            p.kind = Kind::King;
        }
        game.board[sq(path[1].row, path[1].col)] = Some(p);
        false
    } else if row_dist == 2 && col_dist == 2 {
        let legal = gen_jump_paths(game);
        if !legal.iter().any(|j| j.as_slice() == path) {
            return Err("illegal jump path".into());
        }
        let mut p = game.board[sq(path[0].row, path[0].col)].take().unwrap();
        for pair in path.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let mid = Pos { row: (a.row + b.row) / 2, col: (a.col + b.col) / 2 };
            game.board[sq(mid.row, mid.col)] = None;
        }
        let final_pos = *path.last().unwrap();
        if p.kind == Kind::Man && is_promo_row(final_pos.row, p.color) {
            p.kind = Kind::King;
        }
        game.board[sq(final_pos.row, final_pos.col)] = Some(p);
        true
    } else {
        return Err("invalid move — use 'a1 b2' for a step, 'a1 c3 e5' for jumps".into());
    };

    game.current = next_color(game.current);
    Ok(is_capture || piece_kind == Kind::Man)
}

// ── NPC AI ────────────────────────────────────────────────────────────────────

fn evaluate(game: &CheckersGame) -> i32 {
    // Positive = Black (NPC) winning
    let mut score = 0i32;
    for row in 0..8usize {
        for col in 0..8usize {
            if let Some(piece) = game.board[sq(row, col)] {
                let val = match piece.kind { Kind::Man => 10, Kind::King => 17 };
                // Bonus for back row protection
                let back = match (piece.color, row) {
                    (Color::Black, 7) | (Color::Red, 0) => 2,
                    _ => 0,
                };
                // Advancement bonus (toward promotion)
                let adv = match piece.color {
                    Color::Red   => row as i32,
                    Color::Black => 7 - row as i32,
                };
                let ps = val + back + adv / 2;
                if piece.color == Color::Black { score += ps; } else { score -= ps; }
            }
        }
    }
    score
}

fn minimax(game: &CheckersGame, depth: u8, mut alpha: i32, mut beta: i32, maximizing: bool) -> i32 {
    if depth == 0 { return evaluate(game); }

    let moves = legal_moves(game);
    if moves.is_empty() {
        return if maximizing { i32::MIN / 2 } else { i32::MAX / 2 };
    }

    if maximizing {
        let mut best = i32::MIN / 2;
        for mv in &moves {
            let mut copy = game.clone();
            let _ = apply_path(&mut copy, mv);
            let s = minimax(&copy, depth - 1, alpha, beta, false);
            best = best.max(s);
            alpha = alpha.max(s);
            if beta <= alpha { break; }
        }
        best
    } else {
        let mut best = i32::MAX / 2;
        for mv in &moves {
            let mut copy = game.clone();
            let _ = apply_path(&mut copy, mv);
            let s = minimax(&copy, depth - 1, alpha, beta, true);
            best = best.min(s);
            beta = beta.min(s);
            if beta <= alpha { break; }
        }
        best
    }
}

fn npc_pick_move(game: &CheckersGame, difficulty: Difficulty) -> Option<Vec<Pos>> {
    let mut moves = legal_moves(game);
    if moves.is_empty() { return None; }

    match difficulty {
        Difficulty::Random => {
            let idx = rand::thread_rng().gen_range(0..moves.len());
            Some(moves.swap_remove(idx))
        }
        _ => {
            let depth: u8 = match difficulty {
                Difficulty::Easy   => 2,
                Difficulty::Medium => 4,
                Difficulty::Hard   => 6,
                Difficulty::Random => unreachable!(),
            };
            let mut best_idx = 0;
            let mut best_score = i32::MIN;
            for (i, mv) in moves.iter().enumerate() {
                let mut copy = game.clone();
                let _ = apply_path(&mut copy, mv);
                let s = minimax(&copy, depth - 1, i32::MIN / 2, i32::MAX / 2, false);
                if s > best_score {
                    best_score = s;
                    best_idx = i;
                }
            }
            Some(moves.swap_remove(best_idx))
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_pos(s: &str) -> Result<Pos, String> {
    let b = s.as_bytes();
    if b.len() < 2 {
        return Err(format!("bad position '{}' — use e.g. 'a1'", s));
    }
    let col = match b[0].to_ascii_lowercase() {
        b'a' => 0, b'b' => 1, b'c' => 2, b'd' => 3,
        b'e' => 4, b'f' => 5, b'g' => 6, b'h' => 7,
        ch => return Err(format!("'{}' in '{}' isn't a valid column (a–h)", ch as char, s)),
    };
    let rank: usize = std::str::from_utf8(&b[1..])
        .unwrap_or("")
        .parse()
        .map_err(|_| format!("'{}' isn't a valid square — use e.g. d4", s))?;
    if !(1..=8).contains(&rank) {
        return Err(format!("row in '{}' must be 1–8", s));
    }
    Ok(Pos { row: rank - 1, col })
}

fn format_pos(p: Pos) -> String {
    format!("{}{}", (b'a' + p.col as u8) as char, p.row + 1)
}

fn fmt_path(path: &[Pos]) -> String {
    path.iter().map(|p| format_pos(*p)).collect::<Vec<_>>().join(" ")
}

fn move_hint(game: &CheckersGame) -> Option<String> {
    let jumps = gen_jump_paths(game);
    if !jumps.is_empty() {
        Some(format!("Jump! e.g. !checkers {}", fmt_path(&jumps[0])))
    } else {
        gen_simple_moves(game).first()
            .map(|(f, t)| format!("e.g. !checkers {} {}", format_pos(*f), format_pos(*t)))
    }
}

// ── Commands ──────────────────────────────────────────────────────────────────

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let first = ctx.args.first().copied().unwrap_or("").to_ascii_lowercase();

        match first.as_str() {
            "" | "board" => show_board(&ctx),
            "quit" | "forfeit" => quit_game(&ctx).await?,
            "help" => {
                ctx.whisper("!checkers <chips> — start | !checkers a1 b2 — move (or a1 c3 e5 for jumps) | !checkers board | !checkers quit");
            }
            _ if first.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) => {
                let chips: i64 = match first.parse() {
                    Ok(n) => n,
                    Err(_) => { ctx.whisper("Usage: !checkers <chips>"); return Ok(()); }
                };
                start_game(&ctx, chips).await?;
            }
            _ => {
                let path: Result<Vec<Pos>, String> = ctx.args.iter().map(|s| parse_pos(s)).collect();
                match path {
                    Ok(p) if p.len() >= 2 => make_move(&ctx, p).await?,
                    Ok(_)  => ctx.whisper("Specify at least two positions, e.g. !checkers c3 d4"),
                    Err(e) => ctx.whisper(format!("{} — usage: !checkers <from> <to> (e.g. c3 d4)", e)),
                }
            }
        }
        Ok(())
    })
}

fn show_board(ctx: &CommandContext) {
    let games = ctx.state.checkers_games.lock().expect("checkers lock");
    match games.get(ctx.sender) {
        None => ctx.whisper("No active game. Start: !checkers <chips>"),
        Some(s) => {
            let turn_label = match s.game.current {
                Color::Red   => "your turn (r/R)",
                Color::Black => "bot's turn",
            };
            ctx.whisper(format!(
                "Checkers vs {} — {} | Stake: {}",
                s.opponent, turn_label, chips_str(s.stake)
            ));
            for line in render_board(&s.game) { ctx.whisper(line); }
        }
    }
}

async fn start_game(ctx: &CommandContext<'_>, stake: i64) -> anyhow::Result<()> {
    if stake < MIN_STAKE {
        ctx.whisper(format!("Min stake is {}.", chips_str(MIN_STAKE)));
        return Ok(());
    }
    {
        let games = ctx.state.checkers_games.lock().expect("checkers lock");
        if games.contains_key(ctx.sender) {
            ctx.whisper("Already in a checkers game. !checkers board / !checkers quit");
            return Ok(());
        }
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

    let idx = rand::thread_rng().gen_range(0..OPPONENTS.len());
    let (opponent, difficulty) = OPPONENTS[idx];
    let game = new_game();
    let board = render_board(&game);

    ctx.state.checkers_games.lock().expect("checkers lock")
        .insert(ctx.sender.to_owned(), CheckersSession {
            game, stake, difficulty, opponent,
            no_progress_ply: 0,
            position_history: Vec::new(),
        });

    ctx.whisper(format!(
        "Checkers vs {}! You are r/R (red, bottom). Stake: {}",
        opponent, chips_str(stake)
    ));
    for line in &board { ctx.whisper(line); }
    ctx.whisper("Move: !checkers a1 b2 | Jump: !checkers a1 c3 | Multi-jump: !checkers a1 c3 e5 | !checkers quit");
    Ok(())
}

async fn make_move(ctx: &CommandContext<'_>, path: Vec<Pos>) -> anyhow::Result<()> {
    // Phase 1: apply player move under lock, clone game for NPC think
    enum Phase1 {
        BadMove(String),
        PlayerWins { stake: i64, opponent: &'static str },
        Draw { stake: i64, reason: &'static str },
        NpcTurn {
            game: CheckersGame,
            stake: i64,
            difficulty: Difficulty,
            opponent: &'static str,
            no_progress_ply: u32,
            position_history: Vec<([Option<Piece>; 64], Color)>,
        },
    }

    let p1 = {
        let mut games = ctx.state.checkers_games.lock().expect("checkers lock");
        let session = match games.get_mut(ctx.sender) {
            Some(s) => s,
            None => return { ctx.whisper("No active game. Start: !checkers <chips>"); Ok(()) },
        };
        if session.game.current != Color::Red {
            Phase1::BadMove("Not your turn.".into())
        } else {
            match apply_path(&mut session.game, &path) {
                Err(e) => Phase1::BadMove(e),
                Ok(progress) => {
                    if let Some(reason) = advance_draw_state(
                        &mut session.no_progress_ply,
                        &mut session.position_history,
                        &session.game,
                        progress,
                    ) {
                        let stake = session.stake;
                        games.remove(ctx.sender);
                        Phase1::Draw { stake, reason }
                    } else if is_game_over(&session.game) {
                        let (stake, opponent) = (session.stake, session.opponent);
                        games.remove(ctx.sender);
                        Phase1::PlayerWins { stake, opponent }
                    } else {
                        Phase1::NpcTurn {
                            game: session.game.clone(),
                            stake: session.stake,
                            difficulty: session.difficulty,
                            opponent: session.opponent,
                            no_progress_ply: session.no_progress_ply,
                            position_history: session.position_history.clone(),
                        }
                    }
                }
            }
        }
    };

    match p1 {
        Phase1::BadMove(e) => ctx.whisper(e),
        Phase1::PlayerWins { stake, opponent } => {
            let bal = ctx.state.api.casino_adjust(ctx.sender, stake * 2).await.unwrap_or(0);
            ctx.whisper(format!("{} has no moves — you WIN! +{} | Balance: {}", opponent, chips_str(stake), chips_str(bal)));
        }
        Phase1::Draw { stake, reason } => {
            let bal = ctx.state.api.casino_adjust(ctx.sender, stake).await.unwrap_or(0);
            ctx.whisper(format!("Draw by {}. Stake returned. | Balance: {}", reason, chips_str(bal)));
        }
        Phase1::NpcTurn { mut game, stake, difficulty, opponent, mut no_progress_ply, mut position_history } => {
            // NPC computes move without holding lock (potentially slow at Hard depth)
            let Some(npc_path) = npc_pick_move(&game, difficulty) else {
                let bal = ctx.state.api.casino_adjust(ctx.sender, stake * 2).await.unwrap_or(0);
                ctx.whisper(format!("{} has no moves — you WIN! +{} | Balance: {}", opponent, chips_str(stake), chips_str(bal)));
                return Ok(());
            };

            let npc_progress = apply_path(&mut game, &npc_path).expect("NPC picked a legal move");
            let draw_reason = advance_draw_state(&mut no_progress_ply, &mut position_history, &game, npc_progress);

            // Update session under lock
            {
                let mut games = ctx.state.checkers_games.lock().expect("checkers lock");
                if let Some(session) = games.get_mut(ctx.sender) {
                    session.game = game.clone();
                    session.no_progress_ply = no_progress_ply;
                    session.position_history = position_history;
                } else {
                    return Ok(()); // player quit during NPC think
                }
            }

            ctx.whisper(format!("{}: {}", opponent, fmt_path(&npc_path)));

            if let Some(reason) = draw_reason {
                ctx.state.checkers_games.lock().expect("checkers lock").remove(ctx.sender);
                let bal = ctx.state.api.casino_adjust(ctx.sender, stake).await.unwrap_or(0);
                ctx.whisper(format!("Draw by {}. Stake returned. | Balance: {}", reason, chips_str(bal)));
            } else if is_game_over(&game) {
                ctx.state.checkers_games.lock().expect("checkers lock").remove(ctx.sender);
                ctx.state.api.casino_jackpot_rake(stake).await;
                let bal = ctx.state.api.casino_get_balance(ctx.sender).await
                    .map(|b| b.chips).unwrap_or(0);
                ctx.whisper(format!("You have no moves — {} wins. -{} | Balance: {}", opponent, chips_str(stake), chips_str(bal)));
            } else {
                for line in render_board(&game) { ctx.whisper(line); }
                if let Some(hint) = move_hint(&game) { ctx.whisper(hint); }
            }
        }
    }
    Ok(())
}

async fn quit_game(ctx: &CommandContext<'_>) -> anyhow::Result<()> {
    let removed = ctx.state.checkers_games.lock().expect("checkers lock").remove(ctx.sender);
    match removed {
        None => ctx.whisper("No active game."),
        Some(s) => {
            ctx.state.api.casino_jackpot_rake(s.stake).await;
            let bal = ctx.state.api.casino_get_balance(ctx.sender).await
                .map(|b| b.chips).unwrap_or(0);
            ctx.whisper(format!("Forfeited vs {}. -{} | Balance: {}", s.opponent, chips_str(s.stake), chips_str(bal)));
        }
    }
    Ok(())
}
