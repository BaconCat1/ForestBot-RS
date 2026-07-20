use rand::Rng;
use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;
use super::casino::{balance_str, chips_str, format_alimony};

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["reversi", "othello"],
    description: "Reversi vs NPC. !reversi <chips> | !reversi a1 | !reversi board | !reversi quit",
    whitelisted: false,
    execute,
};

const MIN_STAKE: i64 = 50;

// cells: 0=empty, 1=player, 2=cpu
#[derive(Clone)]
pub struct ReversiSession {
    pub cells: [u8; 64],
    #[allow(dead_code)]
    pub current: u8, // 1=player, 2=cpu
    pub stake: i64,
    difficulty: Diff,
    opponent: &'static str,
}

#[derive(Clone, Copy)]
enum Diff {
    Random,
    Greedy,
    Minimax(u8),
}

const OPPONENTS: &[(&str, Diff)] = &[
    ("Glass Joe",     Diff::Random),
    ("Piston Honda",  Diff::Greedy),
    ("Bald Bull",     Diff::Minimax(3)),
    ("Soda Popinski", Diff::Minimax(4)),
    ("Mike Tyson",    Diff::Minimax(5)),
];

// 8-direction deltas (dr, dc)
const DIRS: [(i8, i8); 8] = [
    (-1, -1), (-1, 0), (-1, 1),
    ( 0, -1),          ( 0, 1),
    ( 1, -1), ( 1, 0), ( 1, 1),
];

fn step(pos: u8, dr: i8, dc: i8, n: u8) -> Option<u8> {
    let row = (pos / 8) as i8 + dr * n as i8;
    let col = (pos % 8) as i8 + dc * n as i8;
    if row < 0 || row > 7 || col < 0 || col > 7 {
        return None;
    }
    Some(row as u8 * 8 + col as u8)
}

// Returns list of positions that would be flipped if `player` plays at `pos`
fn flips_for(cells: &[u8; 64], pos: u8, player: u8) -> Vec<u8> {
    let opp = if player == 1 { 2 } else { 1 };
    let mut result = Vec::new();
    if cells[pos as usize] != 0 {
        return result;
    }
    for &(dr, dc) in &DIRS {
        let mut line: Vec<u8> = Vec::new();
        let mut n = 1u8;
        loop {
            match step(pos, dr, dc, n) {
                None => break,
                Some(p) => {
                    if cells[p as usize] == opp {
                        line.push(p);
                        n += 1;
                    } else if cells[p as usize] == player && !line.is_empty() {
                        result.extend_from_slice(&line);
                        break;
                    } else {
                        break;
                    }
                }
            }
        }
    }
    result
}

fn gen_moves(cells: &[u8; 64], player: u8) -> Vec<u8> {
    (0..64u8).filter(|&p| !flips_for(cells, p, player).is_empty()).collect()
}

fn apply_move(cells: &mut [u8; 64], pos: u8, player: u8) {
    let to_flip = flips_for(cells, pos, player);
    cells[pos as usize] = player;
    for p in to_flip {
        cells[p as usize] = player;
    }
}

fn score(cells: &[u8; 64]) -> (u8, u8) {
    let player = cells.iter().filter(|&&c| c == 1).count() as u8;
    let cpu    = cells.iter().filter(|&&c| c == 2).count() as u8;
    (player, cpu)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GameResult {
    InProgress,
    PlayerWins,
    CpuWins,
    Draw,
}

fn game_result(cells: &[u8; 64]) -> GameResult {
    let pmoves = gen_moves(cells, 1);
    let cmoves = gen_moves(cells, 2);
    if !pmoves.is_empty() || !cmoves.is_empty() {
        return GameResult::InProgress;
    }
    let (p, c) = score(cells);
    match p.cmp(&c) {
        std::cmp::Ordering::Greater => GameResult::PlayerWins,
        std::cmp::Ordering::Less    => GameResult::CpuWins,
        std::cmp::Ordering::Equal   => GameResult::Draw,
    }
}

// Positional weights: corners prized, near-corner cells penalized
const WEIGHTS: [i32; 64] = [
    10, -3,  2,  2,  2,  2, -3, 10,
    -3, -4, -1, -1, -1, -1, -4, -3,
     2, -1,  1,  0,  0,  1, -1,  2,
     2, -1,  0,  1,  1,  0, -1,  2,
     2, -1,  0,  1,  1,  0, -1,  2,
     2, -1,  1,  0,  0,  1, -1,  2,
    -3, -4, -1, -1, -1, -1, -4, -3,
    10, -3,  2,  2,  2,  2, -3, 10,
];

fn evaluate(cells: &[u8; 64]) -> i32 {
    cells.iter().enumerate().map(|(i, &c)| {
        if c == 2 { WEIGHTS[i] } else if c == 1 { -WEIGHTS[i] } else { 0 }
    }).sum()
}

fn minimax(cells: &[u8; 64], depth: u8, mut alpha: i32, mut beta: i32, maximizing: bool) -> i32 {
    let result = game_result(cells);
    if result != GameResult::InProgress {
        return match result {
            GameResult::CpuWins    =>  10_000,
            GameResult::PlayerWins => -10_000,
            GameResult::Draw       =>  0,
            GameResult::InProgress => unreachable!(),
        };
    }
    if depth == 0 {
        return evaluate(cells);
    }

    let player = if maximizing { 2u8 } else { 1u8 };
    let moves = gen_moves(cells, player);

    if moves.is_empty() {
        // Turn skip: opponent has no moves either (checked above), so recurse with same depth
        return minimax(cells, depth - 1, alpha, beta, !maximizing);
    }

    if maximizing {
        let mut best = i32::MIN;
        for m in moves {
            let mut next = *cells;
            apply_move(&mut next, m, 2);
            let v = minimax(&next, depth - 1, alpha, beta, false);
            if v > best { best = v; }
            if best > alpha { alpha = best; }
            if beta <= alpha { break; }
        }
        best
    } else {
        let mut best = i32::MAX;
        for m in moves {
            let mut next = *cells;
            apply_move(&mut next, m, 1);
            let v = minimax(&next, depth - 1, alpha, beta, true);
            if v < best { best = v; }
            if best < beta { beta = best; }
            if beta <= alpha { break; }
        }
        best
    }
}

fn cpu_pick(cells: &[u8; 64], diff: Diff) -> Option<u8> {
    let moves = gen_moves(cells, 2);
    if moves.is_empty() {
        return None;
    }
    match diff {
        Diff::Random => {
            Some(moves[rand::thread_rng().gen_range(0..moves.len())])
        }
        Diff::Greedy => {
            moves.into_iter().max_by_key(|&m| {
                let mut next = *cells;
                apply_move(&mut next, m, 2);
                score(&next).1
            })
        }
        Diff::Minimax(depth) => {
            let mut best_score = i32::MIN;
            let mut best_move = moves[0];
            for m in moves {
                let mut next = *cells;
                apply_move(&mut next, m, 2);
                let v = minimax(&next, depth - 1, i32::MIN, i32::MAX, false);
                if v > best_score {
                    best_score = v;
                    best_move = m;
                }
            }
            Some(best_move)
        }
    }
}

fn init_board() -> [u8; 64] {
    let mut cells = [0u8; 64];
    // Standard Othello start: d5/e4=player(1), d4/e5=cpu(2)
    cells[27] = 2; // d4
    cells[28] = 1; // e4
    cells[35] = 1; // d5
    cells[36] = 2; // e5
    cells
}

fn pos_to_str(pos: u8) -> String {
    let col = (b'a' + pos % 8) as char;
    let row = pos / 8 + 1;
    format!("{}{}", col, row)
}

fn parse_pos(s: &str) -> Option<u8> {
    let bytes = s.as_bytes();
    if bytes.len() != 2 {
        return None;
    }
    let col = bytes[0].to_ascii_lowercase();
    let row = bytes[1];
    if !(b'a'..=b'h').contains(&col) || !(b'1'..=b'8').contains(&row) {
        return None;
    }
    Some((row - b'1') * 8 + (col - b'a'))
}

fn render_board(cells: &[u8; 64], legal: &[u8]) -> Vec<String> {
    // header: spacer + math bold a-h (all 3.5px unifont, matches piece width)
    let mut lines = vec!["\u{25A2} \u{1D41A} \u{1D41B} \u{1D41C} \u{1D41D} \u{1D41E} \u{1D41F} \u{1D420} \u{1D421}".to_string()];
    // math bold digits 1-8 for row labels (3.0-3.5px unifont)
    const BD: [char; 9] = ['\0', '\u{1D7CF}', '\u{1D7D0}', '\u{1D7D1}', '\u{1D7D2}',
                            '\u{1D7D3}', '\u{1D7D4}', '\u{1D7D5}', '\u{1D7D6}'];
    lines.extend((0..8usize).map(|row| {
        let mut line = format!("{} ", BD[row + 1]);
        for col in 0..8usize {
            if col > 0 { line.push(' '); }
            let pos = row * 8 + col;
            line.push(if legal.contains(&(pos as u8)) {
                '\u{25CC}' // ◌ legal move
            } else {
                match cells[pos] {
                    1 => '\u{25D5}', // ◕ player
                    2 => '\u{25A3}', // ▣ bot
                    _ => '\u{25A2}', // ▢ empty
                }
            });
        }
        line
    }));
    lines
}

struct TurnResult {
    skipped_cpu: bool,
    #[allow(dead_code)]
    skipped_player: bool,
    over: GameResult,
}

// After player move, run CPU turns until player can move or game ends.
// Returns whether CPU skipped (no moves), player skipped, and final game state.
fn run_cpu_turns(cells: &mut [u8; 64], diff: Diff) -> (Vec<String>, TurnResult) {
    let mut messages = Vec::new();
    loop {
        let cpu_moves = gen_moves(cells, 2);
        if cpu_moves.is_empty() {
            // CPU skips
            let player_moves = gen_moves(cells, 1);
            if player_moves.is_empty() {
                // both skip = game over
                return (messages, TurnResult { skipped_cpu: true, skipped_player: false, over: game_result(cells) });
            }
            return (messages, TurnResult { skipped_cpu: true, skipped_player: false, over: GameResult::InProgress });
        }
        let m = cpu_pick(cells, diff).unwrap();
        apply_move(cells, m, 2);
        messages.push(format!("Bot plays {}", pos_to_str(m)));

        let gr = game_result(cells);
        if gr != GameResult::InProgress {
            return (messages, TurnResult { skipped_cpu: false, skipped_player: false, over: gr });
        }

        let player_moves = gen_moves(cells, 1);
        if !player_moves.is_empty() {
            return (messages, TurnResult { skipped_cpu: false, skipped_player: false, over: GameResult::InProgress });
        }
        // Player has no moves — skip player turn, loop again for another CPU move
        messages.push("You have no legal moves. Skipping your turn.".to_string());
    }
}

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let sender = ctx.sender.to_owned();
        let args = ctx.args.clone();

        if args.is_empty() {
            ctx.whisper_success("Usage: !reversi <chips> | !reversi <a1> | !reversi board | !reversi quit");
            return Ok(());
        }

        let first = args[0].to_ascii_lowercase();

        match first.as_str() {
            "board" => {
                let info = {
                    let games = ctx.state.reversi_games.lock().unwrap();
                    games.get(&sender).map(|sess| {
                        let legal = gen_moves(&sess.cells, 1);
                        let lines = render_board(&sess.cells, &legal);
                        let (ps, cs) = score(&sess.cells);
                        (sess.opponent, ps, cs, lines)
                    })
                };
                match info {
                    None => { ctx.whisper_success("No active reversi game."); }
                    Some((opponent, ps, cs, lines)) => {
                        ctx.whisper_success(format!("Reversi vs {} | you \u{25D5} / bot \u{25A3} / \u{25CC} legal | You: {} Bot: {}", opponent, ps, cs));
                        ctx.whisper_board(lines).await;
                    }
                }
            }
            "quit" | "forfeit" => {
                quit_game(&ctx, &sender, "Forfeit").await?;
            }
            _ if first.len() == 2 && first.as_bytes()[0].is_ascii_alphabetic() && first.as_bytes()[1].is_ascii_digit() => {
                make_move(&ctx, &sender, &first).await?;
            }
            _ => {
                // Try parse as chip amount → start game
                match first.parse::<i64>() {
                    Ok(stake) => { start_game(&ctx, &sender, stake).await?; }
                    Err(_) => { ctx.whisper_success("Usage: !reversi <chips> | !reversi <a1> | !reversi board | !reversi quit"); }
                }
            }
        }

        Ok(())
    })
}

async fn start_game(ctx: &CommandContext<'_>, sender: &str, stake: i64) -> anyhow::Result<()> {
    {
        let games = ctx.state.reversi_games.lock().unwrap();
        if games.contains_key(sender) {
            ctx.whisper_success("Already have active reversi game. !reversi quit to forfeit.");
            return Ok(());
        }
    }

    if stake < MIN_STAKE {
        ctx.whisper_success(format!("Min stake: {}", chips_str(MIN_STAKE)));
        return Ok(());
    }

    let Some(player_uuid) = ctx.require_player_uuid().await else { return Ok(()); };

    match ctx.state.api.casino_adjust(&player_uuid, -stake).await {
        Err(CasinoAdjustErr::InsufficientFunds(have)) => {
            ctx.whisper_success(format!("Need {} but have {}.", chips_str(stake), chips_str(have)));
            return Ok(());
        }
        Err(e) => {
            ctx.whisper_success(format!("Error: {:?}", e));
            return Ok(());
        }
        Ok(_) => {}
    }

    let idx = rand::thread_rng().gen_range(0..OPPONENTS.len());
    let (opp_name, diff) = OPPONENTS[idx];

    let cells = init_board();
    let legal = gen_moves(&cells, 1);

    let sess = ReversiSession {
        cells,
        current: 1,
        stake,
        difficulty: diff,
        opponent: opp_name,
    };

    {
        let mut games = ctx.state.reversi_games.lock().unwrap();
        games.insert(sender.to_owned(), sess);
    }

    let board_lines = render_board(&cells, &legal);
    ctx.whisper_success(format!("Reversi vs {} | stake {} | you=\u{25D5} bot=\u{25A3} \u{25CC}=legal", opp_name, chips_str(stake)));
    ctx.whisper_board(board_lines).await;
    ctx.whisper_success("Your move. Format: !reversi a1");

    Ok(())
}

async fn make_move(ctx: &CommandContext<'_>, sender: &str, pos_str: &str) -> anyhow::Result<()> {
    let pos = match parse_pos(pos_str) {
        None => { ctx.whisper_success("Invalid position. Use a1-h8."); return Ok(()); }
        Some(p) => p,
    };

    // Phase 1: apply player move under lock, clone state. Lock must not span an
    // .await -- MutexGuard is !Send, so the check/apply lives in its own block that
    // returns owned data, and every whisper_board() call happens after it closes.
    enum MoveCheck {
        NoGame,
        Illegal { lines: Vec<String>, ps: u8, cs: u8 },
        Applied { cells: [u8; 64], stake: i64, diff: Diff, opponent: &'static str },
    }

    let check = {
        let mut games = ctx.state.reversi_games.lock().unwrap();
        match games.get_mut(sender) {
            None => MoveCheck::NoGame,
            Some(sess) => {
                let legal = gen_moves(&sess.cells, 1);
                if !legal.contains(&pos) {
                    let (ps, cs) = score(&sess.cells);
                    let lines = render_board(&sess.cells, &legal);
                    MoveCheck::Illegal { lines, ps, cs }
                } else {
                    apply_move(&mut sess.cells, pos, 1);
                    MoveCheck::Applied {
                        cells: sess.cells,
                        stake: sess.stake,
                        diff: sess.difficulty,
                        opponent: sess.opponent,
                    }
                }
            }
        }
    };

    let (mut cells, stake, diff, opponent) = match check {
        MoveCheck::NoGame => {
            ctx.whisper_success("No active reversi game. Start with !reversi <chips>.");
            return Ok(());
        }
        MoveCheck::Illegal { lines, ps, cs } => {
            ctx.whisper_success(format!("{} is not a legal move.", pos_str));
            ctx.whisper_success(format!("You: {} Bot: {}", ps, cs));
            ctx.whisper_board(lines).await;
            return Ok(());
        }
        MoveCheck::Applied { cells, stake, diff, opponent } => (cells, stake, diff, opponent),
    };

    // Check game state after player move
    let gr_after_player = game_result(&cells);
    if gr_after_player != GameResult::InProgress {
        return finish_game(ctx, sender, &cells, stake, gr_after_player, opponent, &[]).await;
    }

    // Phase 2: run CPU turns (no lock held)
    let (cpu_msgs, turn_result) = run_cpu_turns(&mut cells, diff);

    // Phase 3: save back under lock
    {
        let mut games = ctx.state.reversi_games.lock().unwrap();
        if let Some(sess) = games.get_mut(sender) {
            sess.cells = cells;
        }
    }

    if turn_result.over != GameResult::InProgress {
        return finish_game(ctx, sender, &cells, stake, turn_result.over, opponent, &cpu_msgs).await;
    }

    // Show board
    for msg in &cpu_msgs { ctx.whisper_success(msg); }
    if turn_result.skipped_cpu {
        ctx.whisper_success("Bot has no legal moves. Your turn again.");
    }

    let legal = gen_moves(&cells, 1);
    let (ps, cs) = score(&cells);
    let lines = render_board(&cells, &legal);
    ctx.whisper_success(format!("You: {} Bot: {}", ps, cs));
    ctx.whisper_board(lines).await;

    if legal.is_empty() {
        // Shouldn't reach here (handled in run_cpu_turns) but guard anyway
        ctx.whisper_success("No legal moves. Game ending.");
    }

    Ok(())
}

async fn finish_game(
    ctx: &CommandContext<'_>,
    sender: &str,
    cells: &[u8; 64],
    stake: i64,
    result: GameResult,
    opponent: &str,
    cpu_msgs: &[String],
) -> anyhow::Result<()> {
    ctx.state.reversi_games.lock().unwrap().remove(sender);

    for msg in cpu_msgs { ctx.whisper_success(msg); }

    let legal_empty: Vec<u8> = Vec::new();
    let lines = render_board(cells, &legal_empty);
    let (ps, cs) = score(cells);
    ctx.whisper_board(lines).await;
    ctx.whisper_success(format!("Final score — You: {} Bot: {}", ps, cs));

    let Some(player_uuid) = ctx.require_player_uuid().await else { return Ok(()); };

    match result {
        GameResult::PlayerWins => {
            let payout = stake * 2;
            match ctx.state.api.casino_win(&player_uuid, payout).await {
                Ok(win) => {
                    let alimony_note = format_alimony(win.alimony_paid);
                    ctx.whisper_success(format!("You beat {}! +{}{alimony_note} | Balance: {}", opponent, chips_str(payout), chips_str(win.chips)));
                }
                Err(e) => {
                    eprintln!("[Reversi] payout failed for {player_uuid}: {e:?}");
                    ctx.whisper_error(format!("You beat {}, but payout failed. Contact an admin.", opponent));
                }
            }
        }
        GameResult::CpuWins => {
            ctx.state.api.casino_jackpot_rake(stake).await;
            let bal = ctx.state.api.casino_get_balance(&player_uuid).await.map(|b| b.chips);
            ctx.whisper_success(format!("{} wins. -{} | Balance: {}", opponent, chips_str(stake), balance_str(bal)));
        }
        GameResult::Draw => {
            let bal = ctx.state.api.casino_adjust(&player_uuid, stake).await.unwrap_or(0);
            ctx.whisper_success(format!("Draw! Stake returned. | Balance: {}", chips_str(bal)));
        }
        GameResult::InProgress => {}
    }

    Ok(())
}

async fn quit_game(ctx: &CommandContext<'_>, sender: &str, reason: &str) -> anyhow::Result<()> {
    let sess = {
        let mut games = ctx.state.reversi_games.lock().unwrap();
        games.remove(sender)
    };

    match sess {
        None => { ctx.whisper_success("No active reversi game."); }
        Some(s) => {
            ctx.state.api.casino_jackpot_rake(s.stake).await;
            let Some(player_uuid) = ctx.require_player_uuid().await else { return Ok(()); };
            let bal = ctx.state.api.casino_get_balance(&player_uuid).await.map(|b| b.chips);
            ctx.whisper_success(format!("{} — lost {}. Stake to jackpot. | Balance: {}", reason, chips_str(s.stake), balance_str(bal)));
        }
    }

    Ok(())
}
