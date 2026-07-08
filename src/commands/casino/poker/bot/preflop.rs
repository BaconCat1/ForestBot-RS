// © 2025 ashxudev — terminal-poker (MIT)
use super::super::game::deck::{Card, Rank};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PreflopTier {
    Trash,
    Marginal,
    Playable,
    Strong,
    Premium,
}

impl PreflopTier {
    pub fn base_strength(self) -> f64 {
        match self {
            PreflopTier::Premium => 0.90,
            PreflopTier::Strong => 0.75,
            PreflopTier::Playable => 0.60,
            PreflopTier::Marginal => 0.45,
            PreflopTier::Trash => 0.25,
        }
    }
}

const P: u8 = 1;
const S: u8 = 2;
const L: u8 = 3;
const M: u8 = 4;
const T: u8 = 5;

#[rustfmt::skip]
const PAIR_TIER: [u8; 13] = [
    M, M, M, M, L, L, L, L, S, S, P, P, P,
];

#[rustfmt::skip]
const SUITED: [[u8; 13]; 13] = [
    //  2  3  4  5  6  7  8  9  T  J  Q  K  A
    [0, T, T, T, T, T, T, T, T, T, T, M, L], // low=2
    [0, 0, M, T, T, T, T, T, T, T, T, M, L], // low=3
    [0, 0, 0, M, M, T, T, T, T, T, T, M, L], // low=4
    [0, 0, 0, 0, M, M, M, T, T, T, T, M, L], // low=5
    [0, 0, 0, 0, 0, M, L, M, T, T, T, M, L], // low=6
    [0, 0, 0, 0, 0, 0, L, L, M, T, T, M, L], // low=7
    [0, 0, 0, 0, 0, 0, 0, L, L, M, M, M, L], // low=8
    [0, 0, 0, 0, 0, 0, 0, 0, L, L, M, L, L], // low=9
    [0, 0, 0, 0, 0, 0, 0, 0, 0, L, L, L, S], // low=T
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, L, S, S], // low=J
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, S, S], // low=Q
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, P], // low=K
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // low=A
];

#[rustfmt::skip]
const OFFSUIT: [[u8; 13]; 13] = [
    //  2  3  4  5  6  7  8  9  T  J  Q  K  A
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // high=2
    [T, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // high=3
    [T, M, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // high=4
    [T, T, M, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // high=5
    [T, T, T, M, 0, 0, 0, 0, 0, 0, 0, 0, 0], // high=6
    [T, T, T, T, M, 0, 0, 0, 0, 0, 0, 0, 0], // high=7
    [T, T, T, T, T, M, 0, 0, 0, 0, 0, 0, 0], // high=8
    [T, T, T, T, T, T, M, 0, 0, 0, 0, 0, 0], // high=9
    [T, T, T, T, T, T, T, M, 0, 0, 0, 0, 0], // high=T
    [T, T, T, T, T, T, T, T, M, 0, 0, 0, 0], // high=J
    [T, T, T, T, T, T, T, T, M, M, 0, 0, 0], // high=Q
    [T, T, T, T, T, T, T, T, M, M, L, 0, 0], // high=K
    [T, T, T, M, M, M, M, M, L, L, S, P, 0], // high=A
];

fn tier_from_code(code: u8) -> PreflopTier {
    match code {
        P => PreflopTier::Premium,
        S => PreflopTier::Strong,
        L => PreflopTier::Playable,
        M => PreflopTier::Marginal,
        _ => PreflopTier::Trash,
    }
}

fn rank_index(rank: Rank) -> usize {
    (rank as u8 - 2) as usize
}

pub fn classify_preflop(cards: &[Card]) -> PreflopTier {
    assert_eq!(cards.len(), 2, "classify_preflop requires exactly 2 cards");

    let r0 = cards[0].rank;
    let r1 = cards[1].rank;
    let suited = cards[0].suit == cards[1].suit;

    if r0 == r1 {
        return tier_from_code(PAIR_TIER[rank_index(r0)]);
    }

    let (high, low) = if r0 > r1 { (r0, r1) } else { (r1, r0) };
    let hi = rank_index(high);
    let lo = rank_index(low);

    let code = if suited { SUITED[lo][hi] } else { OFFSUIT[hi][lo] };
    tier_from_code(code)
}

pub fn preflop_strength(cards: &[Card]) -> f64 {
    let tier = classify_preflop(cards);
    let base = tier.base_strength();

    let high_rank = cards[0].rank.max(cards[1].rank);
    let low_rank = cards[0].rank.min(cards[1].rank);
    let kicker_bonus =
        (high_rank as u8 - 2) as f64 / 12.0 * 0.04
        + (low_rank as u8 - 2) as f64 / 12.0 * 0.01;

    (base + kicker_bonus).min(1.0)
}
