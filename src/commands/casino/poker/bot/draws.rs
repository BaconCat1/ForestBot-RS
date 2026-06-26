// © 2025 ashxudev — terminal-poker (MIT)
use std::collections::HashSet;

use super::super::game::deck::{Card, Suit};

#[derive(Debug, Clone, Default)]
pub struct DrawInfo {
    pub flush_draw: bool,
    pub oesd: bool,
    pub gutshot: bool,
    pub overcards: u8,
    pub backdoor_flush: bool,
    pub backdoor_straight: bool,
}

impl DrawInfo {
    pub fn equity_boost(&self, street_factor: f64) -> f64 {
        let mut boost = 0.0;
        if self.flush_draw { boost += 0.18 * street_factor; }
        if self.oesd { boost += 0.14 * street_factor; }
        else if self.gutshot { boost += 0.08 * street_factor; }
        boost += self.overcards as f64 * 0.04 * street_factor;
        if self.backdoor_flush { boost += 0.03 * street_factor; }
        if self.backdoor_straight { boost += 0.02 * street_factor; }
        boost
    }
}

pub fn detect_draws(hole_cards: &[Card], board: &[Card]) -> DrawInfo {
    if board.is_empty() {
        return DrawInfo::default();
    }

    let mut info = DrawInfo::default();
    detect_flush_draws(hole_cards, board, &mut info);
    detect_straight_draws(hole_cards, board, &mut info);
    detect_overcards(hole_cards, board, &mut info);
    info
}

fn detect_flush_draws(hole_cards: &[Card], board: &[Card], info: &mut DrawInfo) {
    let suits = [Suit::Spades, Suit::Hearts, Suit::Diamonds, Suit::Clubs];

    for &suit in &suits {
        let hole_count = hole_cards.iter().filter(|c| c.suit == suit).count();
        let board_count = board.iter().filter(|c| c.suit == suit).count();
        let total = hole_count + board_count;

        if hole_count == 0 { continue; }

        if total == 4 {
            info.flush_draw = true;
        } else if total == 3 && board.len() == 3 {
            info.backdoor_flush = true;
        }
    }
}

fn detect_straight_draws(hole_cards: &[Card], board: &[Card], info: &mut DrawInfo) {
    let all_cards: Vec<&Card> = hole_cards.iter().chain(board.iter()).collect();

    let mut rank_set: HashSet<u8> = HashSet::new();
    for card in &all_cards {
        let v = card.rank as u8;
        rank_set.insert(v);
        if v == 14 { rank_set.insert(1); }
    }

    let mut hole_rank_values: HashSet<u8> = HashSet::new();
    for card in hole_cards {
        let v = card.rank as u8;
        hole_rank_values.insert(v);
        if v == 14 { hole_rank_values.insert(1); }
    }

    for base in 1..=10u8 {
        let window: Vec<u8> = (base..base + 5).collect();
        let present: Vec<u8> = window.iter().copied().filter(|v| rank_set.contains(v)).collect();
        let missing: Vec<u8> = window.iter().copied().filter(|v| !rank_set.contains(v)).collect();

        let hole_in_window = window.iter().any(|v| hole_rank_values.contains(v));

        if present.len() == 5 { continue; }

        if present.len() == 4 && missing.len() == 1 && hole_in_window {
            let gap = missing[0];
            if gap == window[0] || gap == window[4] {
                let is_open_ended = if gap == window[0] {
                    base + 5 <= 14
                } else {
                    base >= 2
                };
                if is_open_ended { info.oesd = true; } else { info.gutshot = true; }
            } else {
                info.gutshot = true;
            }
        }

        if present.len() == 3 && board.len() == 3 && hole_in_window {
            info.backdoor_straight = true;
        }
    }
}

fn detect_overcards(hole_cards: &[Card], board: &[Card], info: &mut DrawInfo) {
    let max_board_rank = board.iter().map(|c| c.rank as u8).max().unwrap_or(0);
    let count = hole_cards.iter().filter(|c| (c.rank as u8) > max_board_rank).count();
    info.overcards = count as u8;
}
