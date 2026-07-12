use rand::prelude::SliceRandom;
use rand::Rng;

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;

use super::casino::chips_str;

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["battleship", "bs"],
    description: "Battleship vs bot. !bs <chips> | !bs <coord> (e.g. a5, j0) | !bs board | !bs own | !bs forfeit",
    whitelisted: false,
    bridge_ok: true,
    execute,
};

const MIN_STAKE: i64 = 50;

// Ships in placement order: (name, length)
const SHIPS: &[(&str, usize)] = &[
    ("Carrier", 5),
    ("Battleship", 4),
    ("Cruiser", 3),
    ("Submarine", 3),
    ("Destroyer", 2),
];

const OPPONENTS: &[(&str, Diff)] = &[
    ("Glass Joe",     Diff::Random),
    ("Piston Honda",  Diff::Hunt),
    ("Bald Bull",     Diff::Target),
    ("Soda Popinski", Diff::Parity),
    ("Mike Tyson",    Diff::Density),
];

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Clone)]
enum Diff {
    Random,   // random untried cells
    Hunt,     // shoot near previous hits (±2, from reference)
    Target,   // proper hunt/target: queue adjacent cells after a hit
    Parity,   // target + checkerboard parity hunt phase
    Density,  // probability density map
}

// Board cell encoding (my_board and enemy_board):
//   0      = empty water
//   1–5    = intact ship (ship index + 1)
//   11–15  = hit ship cell
//   21–25  = sunk ship cell

#[derive(Clone, Copy, PartialEq)]
enum Shot { Unknown, Miss, Hit, Sunk }

pub struct BattleshipSession {
    my_board: [u8; 100],       // player's ships + bot's hits
    enemy_board: [u8; 100],    // bot's ships (hidden)
    shots: [Shot; 100],        // player's knowledge of enemy board
    my_ships_alive: [bool; 5],
    enemy_ships_alive: [bool; 5],
    bot_prev_shots: Vec<usize>,
    bot_targets: Vec<usize>,   // hunt/target queue
    pub stake: i64,
    difficulty: Diff,
    opponent: &'static str,
}

impl BattleshipSession {
    fn new(stake: i64, opponent: &'static str, difficulty: Diff) -> Self {
        let mut rng = rand::thread_rng();
        Self {
            my_board: place_ships(&mut rng),
            enemy_board: place_ships(&mut rng),
            shots: [Shot::Unknown; 100],
            my_ships_alive: [true; 5],
            enemy_ships_alive: [true; 5],
            bot_prev_shots: Vec::new(),
            bot_targets: Vec::new(),
            stake,
            difficulty,
            opponent,
        }
    }
}

// ── Board setup ───────────────────────────────────────────────────────────────

fn place_ships(rng: &mut impl Rng) -> [u8; 100] {
    let mut board = [0u8; 100];
    for (idx, &(_, len)) in SHIPS.iter().enumerate() {
        let val = idx as u8 + 1;
        loop {
            let horiz = rng.gen_bool(0.5);
            let (r, c) = if horiz {
                (rng.gen_range(0..10usize), rng.gen_range(0..(10 - len)))
            } else {
                (rng.gen_range(0..(10 - len)), rng.gen_range(0..10usize))
            };
            let cells: Vec<usize> = (0..len)
                .map(|i| if horiz { r * 10 + c + i } else { (r + i) * 10 + c })
                .collect();
            if cells.iter().all(|&p| board[p] == 0) {
                for &p in &cells { board[p] = val; }
                break;
            }
        }
    }
    board
}

// ── Coordinate helpers ────────────────────────────────────────────────────────

// Rows: a–j (lowercase letters), cols: 1–9 then 0 (for col 10).
// e.g. pos 0 = "a1", pos 99 = "j0"
fn pos_to_coord(pos: usize) -> String {
    let row_c = (b'a' + (pos / 10) as u8) as char;
    let col_c = match pos % 10 { 9 => '0', c => (b'1' + c as u8) as char };
    format!("{}{}", row_c, col_c)
}

fn parse_coord(s: &str) -> Option<usize> {
    if s.len() != 2 { return None; }
    let b = s.as_bytes();
    let row = b[0].to_ascii_lowercase().checked_sub(b'a')? as usize;
    if row >= 10 { return None; }
    let col = match b[1] {
        b'0' => 9,
        c @ b'1'..=b'9' => (c - b'1') as usize,
        _ => return None,
    };
    Some(row * 10 + col)
}

// ── Rendering ─────────────────────────────────────────────────────────────────

const BS_ROW_LABELS: [char; 10] = [
    '\u{1D41A}', '\u{1D41B}', '\u{1D41C}', '\u{1D41D}', '\u{1D41E}',
    '\u{1D41F}', '\u{1D420}', '\u{1D421}', '\u{1D422}', '\u{1D423}',
]; // 𝐚–𝐣, all 3.5px unifont
const BS_HEADER: &str = "\u{25A2} \u{1D7CF} \u{1D7D0} \u{1D7D1} \u{1D7D2} \u{1D7D3} \u{1D7D4} \u{1D7D5} \u{1D7D6} \u{1D7D7} \u{1D7CE}";
// ▢ 𝟏 𝟐 𝟑 𝟒 𝟓 𝟔 𝟕 𝟖 𝟗 𝟎

fn render_enemy_board(session: &BattleshipSession) -> Vec<String> {
    let mut lines = vec![BS_HEADER.to_string()];
    for row in 0..10usize {
        let prefix = BS_ROW_LABELS[row];
        let row_str = (0..10)
            .map(|col| match session.shots[row * 10 + col] {
                Shot::Unknown => '\u{25A2}', // ▢ water
                Shot::Miss    => '\u{25CC}', // ◌ miss
                Shot::Hit     => '\u{25D5}', // ◕ hit
                Shot::Sunk    => '\u{25A3}', // ▣ sunk
            })
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        lines.push(format!("{} {}", prefix, row_str));
    }
    lines
}

fn render_own_board(session: &BattleshipSession) -> Vec<String> {
    let mut lines = vec![BS_HEADER.to_string()];
    for row in 0..10usize {
        let prefix = BS_ROW_LABELS[row];
        let row_str = (0..10)
            .map(|col| match session.my_board[row * 10 + col] {
                0            => '\u{25A2}', // ▢ water
                v if v >= 21 => '\u{25A3}', // ▣ sunk
                v if v >= 11 => '\u{25D5}', // ◕ hit
                _            => '\u{25C8}', // ◈ ship
            })
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        lines.push(format!("{} {}", prefix, row_str));
    }
    lines
}

// ── Game logic ────────────────────────────────────────────────────────────────

// Returns the result message, or Err(()) if already shot.
fn player_fire(session: &mut BattleshipSession, pos: usize) -> Result<String, ()> {
    if session.shots[pos] != Shot::Unknown { return Err(()); }
    let v = session.enemy_board[pos];
    if v == 0 {
        session.shots[pos] = Shot::Miss;
        return Ok(format!("Miss at {}.", pos_to_coord(pos)));
    }
    let ship_val = v;
    session.enemy_board[pos] = ship_val + 10;
    if !session.enemy_board.iter().any(|&c| c == ship_val) {
        // ship sunk — mark all hit cells, reveal to player
        for cell in session.enemy_board.iter_mut() {
            if *cell == ship_val + 10 { *cell = ship_val + 20; }
        }
        session.enemy_ships_alive[ship_val as usize - 1] = false;
        for (i, &c) in session.enemy_board.iter().enumerate() {
            if c == ship_val + 20 { session.shots[i] = Shot::Sunk; }
        }
        Ok(format!("Hit! Sank {}'s {}!", session.opponent, SHIPS[ship_val as usize - 1].0))
    } else {
        session.shots[pos] = Shot::Hit;
        Ok(format!("Hit at {}!", pos_to_coord(pos)))
    }
}

fn bot_fire(session: &mut BattleshipSession, pos: usize) -> String {
    let v = session.my_board[pos];
    if v == 0 {
        return format!("{} misses at {}.", session.opponent, pos_to_coord(pos));
    }
    let ship_val = v;
    session.my_board[pos] = ship_val + 10;
    if matches!(session.difficulty, Diff::Target | Diff::Parity) {
        add_adjacent_targets(&mut session.bot_targets, &session.bot_prev_shots, pos);
    }
    if !session.my_board.iter().any(|&c| c == ship_val) {
        for cell in session.my_board.iter_mut() {
            if *cell == ship_val + 10 { *cell = ship_val + 20; }
        }
        session.my_ships_alive[ship_val as usize - 1] = false;
        format!("{} sank your {}!", session.opponent, SHIPS[ship_val as usize - 1].0)
    } else {
        format!("{} hit at {}!", session.opponent, pos_to_coord(pos))
    }
}

fn add_adjacent_targets(targets: &mut Vec<usize>, prev: &[usize], pos: usize) {
    let (row, col) = (pos / 10, pos % 10);
    for (dr, dc) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
        let (nr, nc) = (row as i32 + dr, col as i32 + dc);
        if nr >= 0 && nr < 10 && nc >= 0 && nc < 10 {
            let np = nr as usize * 10 + nc as usize;
            if !prev.contains(&np) && !targets.contains(&np) {
                targets.push(np);
            }
        }
    }
}

// ── AI ────────────────────────────────────────────────────────────────────────

fn bot_pick_pos(session: &mut BattleshipSession, rng: &mut impl Rng) -> usize {
    // drain stale entries from front of target queue
    if matches!(session.difficulty, Diff::Target | Diff::Parity) {
        while let Some(&t) = session.bot_targets.first() {
            if !session.bot_prev_shots.contains(&t) { return t; }
            session.bot_targets.remove(0);
        }
    }

    let diff = session.difficulty.clone();
    match diff {
        Diff::Random => random_untried(&session.bot_prev_shots, rng),

        Diff::Hunt => {
            let hits: Vec<usize> = session.bot_prev_shots.iter()
                .copied()
                .filter(|&p| session.my_board[p] >= 11)
                .collect();
            if hits.is_empty() {
                return random_untried(&session.bot_prev_shots, rng);
            }
            for _ in 0..200 {
                let hit = *hits.choose(rng).unwrap();
                let dr = *[-2i32, -1, 0, 1, 2].choose(rng).unwrap();
                let dc = *[-2i32, -1, 0, 1, 2].choose(rng).unwrap();
                let nr = hit as i32 / 10 + dr;
                let nc = hit as i32 % 10 + dc;
                if nr >= 0 && nr < 10 && nc >= 0 && nc < 10 {
                    let p = nr as usize * 10 + nc as usize;
                    if !session.bot_prev_shots.contains(&p) { return p; }
                }
            }
            random_untried(&session.bot_prev_shots, rng)
        }

        Diff::Target => random_untried(&session.bot_prev_shots, rng),

        Diff::Parity => {
            let opts: Vec<usize> = (0..100)
                .filter(|&p| (p / 10 + p % 10) % 2 == 0 && !session.bot_prev_shots.contains(&p))
                .collect();
            if let Some(&p) = opts.choose(rng) { return p; }
            random_untried(&session.bot_prev_shots, rng)
        }

        Diff::Density => bot_density_pick(session, rng),
    }
}

fn random_untried(prev: &[usize], rng: &mut impl Rng) -> usize {
    loop {
        let p = rng.gen_range(0..100usize);
        if !prev.contains(&p) { return p; }
    }
}

fn bot_density_pick(session: &BattleshipSession, rng: &mut impl Rng) -> usize {
    let mut scores = [0u32; 100];
    for (ship_idx, &(_, len)) in SHIPS.iter().enumerate() {
        if !session.my_ships_alive[ship_idx] { continue; }
        for r in 0..10usize {
            for c in 0..10usize {
                // horizontal
                if c + len <= 10 {
                    let valid = (0..len).all(|i| {
                        let p = r * 10 + c + i;
                        !session.bot_prev_shots.contains(&p) || session.my_board[p] >= 11
                    });
                    if valid { for i in 0..len { scores[r * 10 + c + i] += 1; } }
                }
                // vertical
                if r + len <= 10 {
                    let valid = (0..len).all(|i| {
                        let p = (r + i) * 10 + c;
                        !session.bot_prev_shots.contains(&p) || session.my_board[p] >= 11
                    });
                    if valid { for i in 0..len { scores[(r + i) * 10 + c] += 1; } }
                }
            }
        }
    }
    let max = (0..100).filter(|&p| !session.bot_prev_shots.contains(&p)).map(|p| scores[p]).max().unwrap_or(0);
    let candidates: Vec<usize> = (0..100).filter(|&p| !session.bot_prev_shots.contains(&p) && scores[p] == max).collect();
    *candidates.choose(rng).unwrap_or(&0)
}

// ── Command ───────────────────────────────────────────────────────────────────

pub fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let sender = ctx.sender.to_string();
        let raw_arg = ctx.args.first().copied().unwrap_or("");
        let arg = raw_arg.to_ascii_lowercase();
        let arg = arg.as_str();

        let has_session = ctx.state.battleship_games.lock().unwrap().contains_key(&sender);

        if !has_session {
            let chips = match arg.parse::<i64>() {
                Ok(n) if n >= MIN_STAKE => n,
                Ok(_) => {
                    ctx.whisper(format!("Minimum stake: {} chips.", MIN_STAKE));
                    return Ok(());
                }
                _ => {
                    ctx.whisper("Usage: !bs <chips>. No active game.");
                    return Ok(());
                }
            };
            match ctx.state.api.casino_adjust(&sender, -chips).await {
                Err(CasinoAdjustErr::InsufficientFunds(have)) => {
                    ctx.whisper(format!("Not enough chips (have {}).", chips_str(have)));
                    return Ok(());
                }
                Err(e) => { ctx.whisper(format!("Error: {e:?}")); return Ok(()); }
                Ok(_) => {}
            }
            let mut rng = rand::thread_rng();
            let choice = OPPONENTS.choose(&mut rng).unwrap();
            let session = BattleshipSession::new(chips, choice.0, choice.1.clone());
            let board_lines = render_enemy_board(&session);
            ctx.state.battleship_games.lock().unwrap().insert(sender.clone(), session);
            ctx.whisper(format!("Battleship vs {} | Stake: {}", choice.0, chips_str(chips)));
            ctx.whisper("Ships placed randomly. Fire: !bs <coord> (e.g. a5, j0) | !bs own | !bs forfeit");
            for line in board_lines { ctx.whisper(&line); }
            return Ok(());
        }

        match arg {
            "board" => {
                let lines = {
                    let games = ctx.state.battleship_games.lock().unwrap();
                    render_enemy_board(games.get(&sender).unwrap())
                };
                for line in lines { ctx.whisper(&line); }
                return Ok(());
            }
            "own" => {
                let lines = {
                    let games = ctx.state.battleship_games.lock().unwrap();
                    render_own_board(games.get(&sender).unwrap())
                };
                for line in lines { ctx.whisper(&line); }
                return Ok(());
            }
            "forfeit" | "quit" => {
                let stake = ctx.state.battleship_games.lock().unwrap().remove(&sender).unwrap().stake;
                ctx.state.api.casino_jackpot_rake(stake).await;
                ctx.whisper(format!("Forfeited. {} chips to jackpot.", chips_str(stake)));
                return Ok(());
            }
            _ => {}
        }

        let pos = match parse_coord(arg) {
            Some(p) => p,
            None => {
                ctx.whisper("Invalid coord. Use e.g. !bs a5 or !bs j0.");
                return Ok(());
            }
        };

        enum Outcome {
            AlreadyShot,
            Win  { stake: i64, opponent: &'static str, player_msg: String, board_lines: Vec<String> },
            Lose { stake: i64, opponent: &'static str, player_msg: String, bot_msg: String, board_lines: Vec<String> },
            Continue { player_msg: String, bot_msg: String, board_lines: Vec<String> },
        }

        let outcome = {
            let mut rng = rand::thread_rng();
            let mut games = ctx.state.battleship_games.lock().unwrap();
            let session = games.get_mut(&sender).unwrap();

            match player_fire(session, pos) {
                Err(()) => Outcome::AlreadyShot,
                Ok(player_msg) => {
                    if session.enemy_ships_alive.iter().all(|&a| !a) {
                        let stake = session.stake;
                        let opponent = session.opponent;
                        let board_lines = render_enemy_board(session);
                        games.remove(&sender);
                        Outcome::Win { stake, opponent, player_msg, board_lines }
                    } else {
                        let bot_pos = bot_pick_pos(session, &mut rng);
                        session.bot_prev_shots.push(bot_pos);
                        let bot_msg = bot_fire(session, bot_pos);

                        if session.my_ships_alive.iter().all(|&a| !a) {
                            let stake = session.stake;
                            let opponent = session.opponent;
                            let board_lines = render_enemy_board(session);
                            games.remove(&sender);
                            Outcome::Lose { stake, opponent, player_msg, bot_msg, board_lines }
                        } else {
                            let board_lines = render_enemy_board(session);
                            Outcome::Continue { player_msg, bot_msg, board_lines }
                        }
                    }
                }
            }
        };

        match outcome {
            Outcome::AlreadyShot => {
                ctx.whisper("Already fired there.");
            }
            Outcome::Win { stake, opponent, player_msg, board_lines } => {
                ctx.state.api.casino_adjust(&sender, stake * 2).await.unwrap_or(0);
                ctx.whisper(&player_msg);
                ctx.whisper(format!("All of {opponent}'s ships sunk! Win: {}!", chips_str(stake * 2)));
                for line in board_lines { ctx.whisper(&line); }
            }
            Outcome::Lose { stake, opponent, player_msg, bot_msg, board_lines } => {
                ctx.whisper(&player_msg);
                ctx.whisper(&bot_msg);
                ctx.state.api.casino_jackpot_rake(stake).await;
                ctx.whisper(format!("{opponent} sank all your ships. {} chips to jackpot.", chips_str(stake)));
                for line in board_lines { ctx.whisper(&line); }
            }
            Outcome::Continue { player_msg, bot_msg, board_lines } => {
                ctx.whisper(&player_msg);
                ctx.whisper(&bot_msg);
                for line in board_lines { ctx.whisper(&line); }
            }
        }

        Ok(())
    })
}
