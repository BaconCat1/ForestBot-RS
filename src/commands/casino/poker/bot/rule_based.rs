// © 2025 ashxudev — terminal-poker (MIT)
use super::super::game::actions::Action;
use super::super::game::deck::{Card, Suit};
use super::super::game::hand::evaluate_hand;
use super::super::game::state::{GamePhase, GameState, Player, BIG_BLIND};

use super::draws::detect_draws;
use super::preflop::preflop_strength;

use rand::Rng;

#[derive(Debug, Clone, Copy)]
enum BoardTexture {
    Dry,
    Medium,
    Wet,
}

#[derive(Debug, Clone, Copy)]
enum BetSize {
    Small,
    Medium,
    Large,
}

impl BetSize {
    fn pot_fraction(self) -> f64 {
        match self {
            BetSize::Small => 0.30,
            BetSize::Medium => 0.60,
            BetSize::Large => 0.85,
        }
    }
}

pub struct RuleBasedBot {
    pub aggression: f64,
}

impl RuleBasedBot {
    pub fn new(aggression: f64) -> Self {
        Self { aggression: aggression.clamp(0.0, 1.0) }
    }

    pub fn decide(&self, state: &GameState) -> Action {
        match state.phase {
            GamePhase::Preflop => self.decide_preflop(state),
            GamePhase::Flop | GamePhase::Turn => self.decide_postflop(state),
            GamePhase::River => self.decide_river(state),
            _ => Action::Check,
        }
    }

    fn decide_preflop(&self, state: &GameState) -> Action {
        let strength = preflop_strength(&state.bot_cards);
        let to_call = state.amount_to_call(Player::Bot);
        let available = state.available_actions();
        let stack = state.bot_stack;
        let bot_bet = state.bot_bet;
        let max_bet = bot_bet + stack;

        let mut rng = rand::thread_rng();
        let noise: f64 = rng.gen_range(-0.05..0.05);
        let aggression_adj = (self.aggression - 0.5) * 0.10;
        let adjusted = strength + aggression_adj + noise;

        if to_call == 0 {
            if adjusted > 0.70 && self.aggression > 0.2 {
                return self.preflop_raise(3.0, state);
            }
            if adjusted > 0.55 && self.aggression > 0.3 {
                return self.preflop_raise(2.5, state);
            }
            if adjusted > 0.45 && self.aggression > 0.5 && rng.gen_bool(0.25) {
                return self.preflop_raise(2.5, state);
            }
            return Action::Check;
        }

        let facing_raise = state.last_aggressor.is_some();

        if !facing_raise {
            if adjusted > 0.50 && self.aggression > 0.15 {
                let mult = if adjusted > 0.80 { 3.0 } else { 2.5 };
                return self.preflop_raise(mult, state);
            }
            if adjusted > 0.35 {
                return self.make_call(to_call, stack, bot_bet);
            }
            if self.aggression > 0.7 && rng.gen_bool(0.08) {
                return self.preflop_raise(3.0, state);
            }
            return Action::Fold;
        }

        if adjusted > 0.80 {
            if let Some(min_raise) = available.min_raise {
                let raise_to = ((state.player_bet as f64) * 3.0) as u32;
                let raise_to = raise_to.max(min_raise);
                if raise_to >= max_bet { return Action::AllIn(max_bet); }
                return Action::Raise(raise_to);
            }
            return self.make_call(to_call, stack, bot_bet);
        }

        if adjusted > 0.65 {
            if available.min_raise.is_some() && self.aggression > 0.5 && rng.gen_bool(0.25) {
                let min_raise = available.min_raise.unwrap();
                let raise_to = ((state.player_bet as f64) * 2.5) as u32;
                let raise_to = raise_to.max(min_raise);
                if raise_to < max_bet { return Action::Raise(raise_to); }
            }
            return self.make_call(to_call, stack, bot_bet);
        }

        if adjusted > 0.50 {
            return self.make_call(to_call, stack, bot_bet);
        }

        if adjusted > 0.35 && to_call <= BIG_BLIND * 3 {
            return self.make_call(to_call, stack, bot_bet);
        }

        if self.aggression > 0.7 && rng.gen_bool(0.05) {
            if let Some(min_raise) = available.min_raise {
                let raise_to = (BIG_BLIND * 7).max(min_raise);
                if raise_to < max_bet { return Action::Raise(raise_to); }
            }
        }

        Action::Fold
    }

    fn preflop_raise(&self, bb_multiplier: f64, state: &GameState) -> Action {
        let available = state.available_actions();
        let stack = state.bot_stack;
        let bot_bet = state.bot_bet;
        let max_bet = bot_bet + stack;
        let raise_to = (BIG_BLIND as f64 * bb_multiplier) as u32;

        if state.amount_to_call(Player::Bot) == 0 {
            let min = available.min_bet.unwrap_or(BIG_BLIND);
            let amount = raise_to.max(min);
            if amount >= max_bet { Action::AllIn(max_bet) } else { Action::Bet(amount) }
        } else {
            let min = available.min_raise.unwrap_or(raise_to);
            let amount = raise_to.max(min);
            if amount >= max_bet { Action::AllIn(max_bet) } else { Action::Raise(amount) }
        }
    }

    fn decide_postflop(&self, state: &GameState) -> Action {
        let made = evaluate_hand(&state.bot_cards, &state.board).strength();
        let street_factor = match state.phase {
            GamePhase::Flop => 1.0,
            GamePhase::Turn => 0.5,
            _ => 0.0,
        };
        let draws = detect_draws(&state.bot_cards, &state.board);
        let draw_boost = draws.equity_boost(street_factor);
        let effective = made + draw_boost;
        let adjusted = self.adjust_strength(effective, state);
        let texture = analyze_board_texture(&state.board);
        let to_call = state.amount_to_call(Player::Bot);

        if to_call == 0 {
            self.postflop_bet_or_check(adjusted, texture, state)
        } else {
            self.postflop_facing_bet(adjusted, to_call, state)
        }
    }

    fn postflop_bet_or_check(&self, adjusted: f64, texture: BoardTexture, state: &GameState) -> Action {
        let mut rng = rand::thread_rng();

        if adjusted > 0.45 { return self.make_bet(BetSize::Large, state); }

        if adjusted > 0.25 {
            let size = match texture {
                BoardTexture::Dry => BetSize::Small,
                BoardTexture::Medium => BetSize::Medium,
                BoardTexture::Wet => BetSize::Large,
            };
            return self.make_bet(size, state);
        }

        if adjusted > 0.15 && self.aggression > 0.4 {
            return self.make_bet(BetSize::Small, state);
        }

        if adjusted < 0.10 && self.aggression > 0.6 && rng.gen_bool(0.20) {
            let size = match texture {
                BoardTexture::Dry => BetSize::Small,
                _ => BetSize::Medium,
            };
            return self.make_bet(size, state);
        }

        Action::Check
    }

    fn decide_river(&self, state: &GameState) -> Action {
        let made = evaluate_hand(&state.bot_cards, &state.board).strength();
        let adjusted = self.adjust_strength(made, state);
        let to_call = state.amount_to_call(Player::Bot);

        if to_call == 0 {
            self.river_bet_or_check(adjusted, state)
        } else {
            self.postflop_facing_bet(adjusted, to_call, state)
        }
    }

    fn river_bet_or_check(&self, adjusted: f64, state: &GameState) -> Action {
        let mut rng = rand::thread_rng();

        if adjusted > 0.45 { return self.make_bet(BetSize::Large, state); }
        if adjusted > 0.20 { return self.make_bet(BetSize::Small, state); }
        if adjusted < 0.08 && self.aggression > 0.6 && rng.gen_bool(0.15) {
            return self.make_bet(BetSize::Large, state);
        }
        Action::Check
    }

    fn postflop_facing_bet(&self, adjusted: f64, to_call: u32, state: &GameState) -> Action {
        let available = state.available_actions();
        let stack = state.bot_stack;
        let bot_bet = state.bot_bet;
        let max_bet = bot_bet + stack;
        let mut rng = rand::thread_rng();

        if adjusted > 0.35 {
            if let Some(min_raise) = available.min_raise {
                let raise_to = self.calculate_raise_size(min_raise, state.pot, stack, bot_bet);
                if raise_to >= max_bet { return Action::AllIn(max_bet); }
                return Action::Raise(raise_to);
            }
            return self.make_call(to_call, stack, bot_bet);
        }

        if adjusted > 0.20 {
            if available.min_raise.is_some() && self.aggression > 0.5 && rng.gen_bool(0.30) {
                let min_raise = available.min_raise.unwrap();
                let raise_to = self.calculate_raise_size(min_raise, state.pot, stack, bot_bet);
                if raise_to < max_bet { return Action::Raise(raise_to); }
            }
            return self.make_call(to_call, stack, bot_bet);
        }

        if adjusted > 0.12 { return self.make_call(to_call, stack, bot_bet); }

        if adjusted < 0.08 && self.aggression > 0.7 && rng.gen_bool(0.10) {
            if let Some(min_raise) = available.min_raise {
                let raise_to = self.calculate_raise_size(min_raise, state.pot, stack, bot_bet);
                if raise_to < max_bet { return Action::Raise(raise_to); }
            }
        }

        Action::Fold
    }

    fn adjust_strength(&self, effective: f64, state: &GameState) -> f64 {
        let mut rng = rand::thread_rng();
        let noise: f64 = rng.gen_range(-0.05..0.05);
        let position = if state.button == Player::Bot { 0.06 } else { -0.04 };
        let aggression_adj = (self.aggression - 0.5) * 0.12;
        effective + position + aggression_adj + noise
    }

    fn make_bet(&self, size: BetSize, state: &GameState) -> Action {
        let available = state.available_actions();
        let stack = state.bot_stack;
        let bot_bet = state.bot_bet;
        let max_bet = bot_bet + stack;

        let min_bet = match available.min_bet {
            Some(v) => v,
            None => return Action::Check,
        };

        let raw = (state.pot as f64 * size.pot_fraction()) as u32;
        let amount = raw.max(min_bet).min(stack);

        if amount >= stack { Action::AllIn(max_bet) } else { Action::Bet(amount) }
    }

    fn make_call(&self, to_call: u32, stack: u32, bot_bet: u32) -> Action {
        if to_call >= stack { Action::AllIn(bot_bet + stack) } else { Action::Call(to_call) }
    }

    fn calculate_raise_size(&self, min_raise_to: u32, pot: u32, stack: u32, bot_bet: u32) -> u32 {
        let raise_to = (pot as f64 * 0.70) as u32 + bot_bet;
        let max_bet = bot_bet + stack;
        raise_to.max(min_raise_to).min(max_bet)
    }
}

fn analyze_board_texture(board: &[Card]) -> BoardTexture {
    if board.is_empty() { return BoardTexture::Dry; }

    let mut wetness: i32 = 0;

    let mut suit_counts = [0u8; 4];
    for card in board {
        let idx = match card.suit {
            Suit::Spades => 0,
            Suit::Hearts => 1,
            Suit::Diamonds => 2,
            Suit::Clubs => 3,
        };
        suit_counts[idx] += 1;
    }
    let max_suit = *suit_counts.iter().max().unwrap();
    if max_suit >= 3 { wetness += 2; } else if max_suit == 2 { wetness += 1; }

    let mut ranks: Vec<u8> = board.iter().map(|c| c.rank as u8).collect();
    ranks.sort();
    for window in ranks.windows(2) {
        if window[1] - window[0] <= 2 { wetness += 1; }
    }

    if ranks.windows(2).any(|w| w[0] == w[1]) { wetness += 1; }

    match wetness {
        0..=1 => BoardTexture::Dry,
        2..=3 => BoardTexture::Medium,
        _ => BoardTexture::Wet,
    }
}
