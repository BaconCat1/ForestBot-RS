use std::collections::VecDeque;

use rand::Rng;

use crate::commands::{CommandContext, CommandDefinition, CommandFuture};
use crate::structure::endpoints::endpoints::CasinoAdjustErr;

use super::chips_str;

pub const COMMAND: CommandDefinition = CommandDefinition {
    names: &["mines", "minesweeper"],
    description: "Minesweeper with stacking multiplier. !mines <chips> | !mines <coord> (e.g. a3) | !mines f<coord> | !mines cash | !mines board | !mines quit",
    whitelisted: false,
    execute,
};

const BOARD_SIZE: usize = 10;
const MINE_COUNT: usize = 20;
const TOTAL_CELLS: usize = BOARD_SIZE * BOARD_SIZE;
const TOTAL_SAFE: usize = TOTAL_CELLS - MINE_COUNT; // 80
const MIN_STAKE: i64 = 25;
const HOUSE_EDGE: f64 = 0.97;

const ROW_LABELS: [char; 10] = [
    '\u{1D41A}', '\u{1D41B}', '\u{1D41C}', '\u{1D41D}', '\u{1D41E}',
    '\u{1D41F}', '\u{1D420}', '\u{1D421}', '\u{1D422}', '\u{1D423}',
]; // 𝐚–𝐣

// Same header format as battleship: ▢ 𝟏 𝟐 𝟑 𝟒 𝟓 𝟔 𝟕 𝟖 𝟗 𝟎
const HEADER: &str = "\u{25A2} \u{1D7CF} \u{1D7D0} \u{1D7D1} \u{1D7D2} \u{1D7D3} \u{1D7D4} \u{1D7D5} \u{1D7D6} \u{1D7D7} \u{1D7CE}";

const CHAR_UNREVEALED: char = '\u{25A2}'; // ▢
const CHAR_FLAG:       char = '\u{25C8}'; // ◈
const CHAR_MINE:       char = '\u{25D5}'; // ◕

// neighbor count 0–8: ◌ 𝟏 𝟐 𝟑 𝟒 𝟓 𝟔 𝟕 𝟖
const NEIGHBOR_CHARS: [char; 9] = [
    '\u{25CC}',  // 0 ◌
    '\u{1D7CF}', // 1 𝟏
    '\u{1D7D0}', // 2 𝟐
    '\u{1D7D1}', // 3 𝟑
    '\u{1D7D2}', // 4 𝟒
    '\u{1D7D3}', // 5 𝟓
    '\u{1D7D4}', // 6 𝟔
    '\u{1D7D5}', // 7 𝟕
    '\u{1D7D6}', // 8 𝟖
];

#[derive(Clone, Copy, PartialEq)]
enum CellKind {
    Safe(u8), // neighbor count 0–8
    Mine,
}

#[derive(Clone, Copy)]
struct Cell {
    kind:     CellKind,
    revealed: bool,
    flagged:  bool,
}

impl Cell {
    const fn blank() -> Self {
        Self { kind: CellKind::Safe(0), revealed: false, flagged: false }
    }
}

pub struct MinesGame {
    cells:             [Cell; TOTAL_CELLS],
    pub stake:         i64,
    pub multiplier:    f64,
    pub safe_revealed: usize,
    generated:         bool,
}

impl MinesGame {
    fn new(stake: i64) -> Self {
        Self {
            cells:         [Cell::blank(); TOTAL_CELLS],
            stake,
            multiplier:    1.0,
            safe_revealed: 0,
            generated:     false,
        }
    }

    fn generate(&mut self, avoid: usize) {
        let ar = (avoid / BOARD_SIZE) as isize;
        let ac = (avoid % BOARD_SIZE) as isize;
        let mut safe = Vec::with_capacity(9);
        for dr in -1isize..=1 {
            for dc in -1isize..=1 {
                let r = ar + dr;
                let c = ac + dc;
                if (0..BOARD_SIZE as isize).contains(&r) && (0..BOARD_SIZE as isize).contains(&c) {
                    safe.push(r as usize * BOARD_SIZE + c as usize);
                }
            }
        }
        let mut rng = rand::thread_rng();
        let mut placed = 0;
        while placed < MINE_COUNT {
            let pos = rng.gen_range(0..TOTAL_CELLS);
            if safe.contains(&pos) || self.cells[pos].kind == CellKind::Mine { continue; }
            self.cells[pos].kind = CellKind::Mine;
            placed += 1;
        }
        for pos in 0..TOTAL_CELLS {
            if self.cells[pos].kind == CellKind::Mine { continue; }
            let count = neighbors(pos).iter()
                .filter(|&&n| self.cells[n].kind == CellKind::Mine)
                .count();
            self.cells[pos].kind = CellKind::Safe(count as u8);
        }
        self.generated = true;
    }

    // One multiplier step per click; safe_revealed advances by flood count.
    // p = probability the clicked cell was safe at time of click.
    fn apply_click(&mut self, flood_count: usize) {
        let unrevealed    = TOTAL_CELLS - self.safe_revealed;
        let safe_remaining = TOTAL_SAFE - self.safe_revealed;
        if safe_remaining == 0 || unrevealed == 0 { return; }
        let p = safe_remaining as f64 / unrevealed as f64;
        self.multiplier *= HOUSE_EDGE / p;
        self.safe_revealed += flood_count;
    }

    // Returns number of newly revealed safe cells, or None on mine hit.
    fn sweep(&mut self, pos: usize) -> Option<usize> {
        if !self.generated { self.generate(pos); }
        let cell = self.cells[pos];
        if cell.revealed || cell.flagged { return Some(0); }
        if cell.kind == CellKind::Mine {
            self.cells[pos].revealed = true;
            return None;
        }
        let mut count = 0usize;
        let mut queue = VecDeque::new();
        self.cells[pos].revealed = true;
        count += 1;
        queue.push_back(pos);
        // BFS flood-fill through zero-neighbor cells
        while let Some(cur) = queue.pop_front() {
            if !matches!(self.cells[cur].kind, CellKind::Safe(0)) { continue; }
            for n in neighbors(cur) {
                let nc = self.cells[n];
                if nc.revealed || nc.flagged || nc.kind == CellKind::Mine { continue; }
                self.cells[n].revealed = true;
                count += 1;
                queue.push_back(n);
            }
        }
        self.apply_click(count);
        Some(count)
    }

    fn payout(&self) -> i64 {
        (self.stake as f64 * self.multiplier) as i64
    }

    fn is_victory(&self) -> bool {
        self.safe_revealed >= TOTAL_SAFE
    }

    fn render(&self, reveal_mines: bool) -> Vec<String> {
        let mut lines = Vec::with_capacity(BOARD_SIZE + 1);
        lines.push(HEADER.to_string());
        for row in 0..BOARD_SIZE {
            let prefix = ROW_LABELS[row];
            let mut row_str = String::with_capacity(BOARD_SIZE * 2);
            for col in 0..BOARD_SIZE {
                if col > 0 { row_str.push(' '); }
                let cell = self.cells[row * BOARD_SIZE + col];
                let ch = if reveal_mines && cell.kind == CellKind::Mine {
                    CHAR_MINE
                } else if cell.flagged {
                    CHAR_FLAG
                } else if !cell.revealed {
                    CHAR_UNREVEALED
                } else {
                    match cell.kind {
                        CellKind::Mine    => CHAR_MINE,
                        CellKind::Safe(n) => NEIGHBOR_CHARS[n as usize],
                    }
                };
                row_str.push(ch);
            }
            lines.push(format!("{prefix} {row_str}"));
        }
        lines
    }
}

fn neighbors(pos: usize) -> Vec<usize> {
    let row = pos / BOARD_SIZE;
    let col = pos % BOARD_SIZE;
    let mut out = Vec::with_capacity(8);
    for dr in -1isize..=1 {
        for dc in -1isize..=1 {
            if dr == 0 && dc == 0 { continue; }
            let r = row as isize + dr;
            let c = col as isize + dc;
            if (0..BOARD_SIZE as isize).contains(&r) && (0..BOARD_SIZE as isize).contains(&c) {
                out.push(r as usize * BOARD_SIZE + c as usize);
            }
        }
    }
    out
}

// rows a–j → 0–9; cols 1–9 → 0–8, 0 → 9
fn parse_coord(s: &str) -> Option<usize> {
    let mut chars = s.chars();
    let row_ch = chars.next()?;
    let col_ch = chars.next()?;
    if chars.next().is_some() { return None; }
    let row = (row_ch as u8).wrapping_sub(b'a') as usize;
    let col = match col_ch {
        '1'..='9' => (col_ch as u8 - b'1') as usize,
        '0'       => 9,
        _         => return None,
    };
    if row >= BOARD_SIZE { return None; }
    Some(row * BOARD_SIZE + col)
}

fn execute(ctx: CommandContext<'_>) -> CommandFuture<'_> {
    Box::pin(async move {
        let sender  = ctx.sender.to_string();
        let raw_arg = ctx.args.first().copied().unwrap_or("").to_ascii_lowercase();
        let arg     = raw_arg.as_str();

        let Some(player_uuid) = ctx.require_player_uuid().await else { return Ok(()); };

        let has_session = ctx.state.mines_games.lock().unwrap().contains_key(&sender);

        if !has_session {
            let chips = match arg.parse::<i64>() {
                Ok(n) if n >= MIN_STAKE => n,
                Ok(_) => {
                    ctx.whisper_success(format!("Minimum stake: {} chips.", MIN_STAKE));
                    return Ok(());
                }
                _ => {
                    ctx.whisper_success(format!("Usage: !mines <chips>. No active game. Min: {}.", MIN_STAKE));
                    return Ok(());
                }
            };
            match ctx.state.api.casino_adjust(&player_uuid, -chips).await {
                Err(CasinoAdjustErr::InsufficientFunds(have)) => {
                    ctx.whisper_success(format!("Not enough chips (have {}).", chips_str(have)));
                    return Ok(());
                }
                Err(e) => { ctx.whisper_success(format!("Error: {e:?}")); return Ok(()); }
                Ok(_)  => {}
            }
            let session = MinesGame::new(chips);
            let board   = session.render(false);
            ctx.state.mines_games.lock().unwrap().insert(sender.clone(), session);
            ctx.whisper_success(format!("Minesweeper | {} stake | 20 mines, 10×10 board", chips_str(chips)));
            ctx.whisper_success("Reveal: !mines <coord> (e.g. a3) | Flag: !mines f<coord> | Cashout: !mines cash | !mines board | !mines quit");
            ctx.whisper_board(board).await;
            return Ok(());
        }

        match arg {
            "board" => {
                let (lines, mult_str, sr) = {
                    let games = ctx.state.mines_games.lock().unwrap();
                    let g = games.get(&sender).unwrap();
                    (g.render(false), format!("{:.3}×", g.multiplier), g.safe_revealed)
                };
                ctx.whisper_success(format!("Multiplier: {mult_str} | {sr}/{TOTAL_SAFE} safe revealed"));
                ctx.whisper_board(lines).await;
                return Ok(());
            }
            "cash" | "cashout" => {
                let (sr, payout) = {
                    let games = ctx.state.mines_games.lock().unwrap();
                    let g = games.get(&sender).unwrap();
                    (g.safe_revealed, g.payout())
                };
                if sr == 0 {
                    ctx.whisper_success("Reveal at least one safe cell before cashing out.");
                    return Ok(());
                }
                ctx.state.mines_games.lock().unwrap().remove(&sender);
                let win = ctx.state.api.casino_win(&player_uuid, payout).await.unwrap_or_default();
                let alimony_note = if win.alimony_paid > 0 { format!(" (-{} alimony)", chips_str(win.alimony_paid)) } else { String::new() };
                ctx.whisper_success(format!("Cashed out! Won {}{alimony_note}.", chips_str(payout)));
                return Ok(());
            }
            "quit" | "forfeit" => {
                let stake = ctx.state.mines_games.lock().unwrap().remove(&sender).unwrap().stake;
                ctx.state.api.casino_jackpot_rake(stake).await;
                ctx.whisper_success(format!("Forfeited. {} to jackpot.", chips_str(stake)));
                return Ok(());
            }
            _ => {}
        }

        // flag: "f<coord>"
        if let Some(coord_part) = arg.strip_prefix('f') {
            let pos = match parse_coord(coord_part) {
                Some(p) => p,
                None => {
                    ctx.whisper_success("Invalid coord. Use e.g. !mines fa3 or !mines fj0.");
                    return Ok(());
                }
            };
            let msg = {
                let mut games = ctx.state.mines_games.lock().unwrap();
                let g = games.get_mut(&sender).unwrap();
                if g.cells[pos].revealed {
                    "That cell is already revealed.".to_string()
                } else {
                    let now_flagged = !g.cells[pos].flagged;
                    g.cells[pos].flagged = now_flagged;
                    if now_flagged { format!("Flagged {coord_part}.") }
                    else { format!("Unflagged {coord_part}.") }
                }
            };
            ctx.whisper_success(&msg);
            return Ok(());
        }

        // sweep
        let pos = match parse_coord(arg) {
            Some(p) => p,
            None => {
                ctx.whisper_success("Invalid coord. e.g. !mines a3, !mines j0. Flag: !mines f<coord>.");
                return Ok(());
            }
        };

        enum Outcome {
            AlreadyRevealed,
            Mine    { stake: i64, board: Vec<String> },
            Victory { payout: i64, board: Vec<String> },
            Continue { revealed: usize, multiplier: f64, safe_revealed: usize, board: Vec<String> },
        }

        let outcome = {
            let mut games = ctx.state.mines_games.lock().unwrap();
            let g = games.get_mut(&sender).unwrap();

            if g.cells[pos].revealed {
                Outcome::AlreadyRevealed
            } else {
                match g.sweep(pos) {
                    None => {
                        // hit mine
                        let board = g.render(true);
                        let stake = g.stake;
                        games.remove(&sender);
                        Outcome::Mine { stake, board }
                    }
                    Some(_) if g.is_victory() => {
                        let payout = g.payout();
                        let board  = g.render(false);
                        games.remove(&sender);
                        Outcome::Victory { payout, board }
                    }
                    Some(newly_revealed) => {
                        let mult = g.multiplier;
                        let sr   = g.safe_revealed;
                        let board = g.render(false);
                        Outcome::Continue { revealed: newly_revealed, multiplier: mult, safe_revealed: sr, board }
                    }
                }
            }
        };

        match outcome {
            Outcome::AlreadyRevealed => {
                ctx.whisper_success("Already revealed.");
            }
            Outcome::Mine { stake, board } => {
                ctx.state.api.casino_jackpot_rake(stake).await;
                ctx.whisper_success(format!("BOOM! Hit a mine. {} to jackpot.", chips_str(stake)));
                ctx.whisper_board(board).await;
            }
            Outcome::Victory { payout, board } => {
                let win = ctx.state.api.casino_win(&player_uuid, payout).await.unwrap_or_default();
                let alimony_note = if win.alimony_paid > 0 { format!(" (-{} alimony)", chips_str(win.alimony_paid)) } else { String::new() };
                ctx.whisper_success(format!("All safe cells cleared! Won {}{alimony_note}!", chips_str(payout)));
                ctx.whisper_board(board).await;
            }
            Outcome::Continue { revealed, multiplier, safe_revealed, board } => {
                let mult_str = format!("{multiplier:.3}×");
                ctx.whisper_success(format!(
                    "+{revealed} | Multiplier: {mult_str} | {safe_revealed}/{TOTAL_SAFE} safe | !mines cash to collect"
                ));
                ctx.whisper_board(board).await;
            }
        }

        Ok(())
    })
}
