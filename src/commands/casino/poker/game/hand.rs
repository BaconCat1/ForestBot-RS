// © 2025 ashxudev — terminal-poker (MIT)
use super::deck::{Card, Rank};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HandRank {
    HighCard = 0,
    Pair = 1,
    TwoPair = 2,
    ThreeOfAKind = 3,
    Straight = 4,
    Flush = 5,
    FullHouse = 6,
    FourOfAKind = 7,
    StraightFlush = 8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandEvaluation {
    pub rank: HandRank,
    pub kickers: Vec<Rank>,
    pub description: String,
}

impl HandEvaluation {
    pub fn strength(&self) -> f64 {
        let base = self.rank as u32 as f64 / 8.0;
        let kicker_bonus = if !self.kickers.is_empty() {
            (self.kickers[0] as u32 as f64 - 2.0) / 12.0 * 0.1
        } else {
            0.0
        };
        (base + kicker_bonus).min(1.0)
    }
}

pub fn evaluate_hand(hole_cards: &[Card], board: &[Card]) -> HandEvaluation {
    let mut all_cards: Vec<Card> = hole_cards.to_vec();
    all_cards.extend(board);

    if all_cards.len() < 5 {
        return evaluate_partial(&all_cards);
    }

    let combos = combinations(&all_cards, 5);
    combos
        .into_iter()
        .map(|combo| evaluate_five(&combo))
        .max_by(|a, b| a.rank.cmp(&b.rank).then_with(|| a.kickers.cmp(&b.kickers)))
        .unwrap_or_else(|| HandEvaluation {
            rank: HandRank::HighCard,
            kickers: vec![],
            description: "Unknown".to_string(),
        })
}

fn evaluate_partial(cards: &[Card]) -> HandEvaluation {
    if cards.is_empty() {
        return HandEvaluation {
            rank: HandRank::HighCard,
            kickers: vec![],
            description: "No cards".to_string(),
        };
    }

    let mut rank_counts: HashMap<Rank, u8> = HashMap::new();
    for card in cards {
        *rank_counts.entry(card.rank).or_insert(0) += 1;
    }

    let mut pairs = 0;
    let mut trips = false;
    let mut highest_paired_rank = None;

    for (&rank, &count) in &rank_counts {
        match count {
            2 => {
                pairs += 1;
                if highest_paired_rank.is_none() || rank > highest_paired_rank.unwrap() {
                    highest_paired_rank = Some(rank);
                }
            }
            3 => trips = true,
            4 => {
                return HandEvaluation {
                    rank: HandRank::FourOfAKind,
                    kickers: vec![rank],
                    description: format!("Four of a kind, {}", rank_name(rank)),
                }
            }
            _ => {}
        }
    }

    if trips {
        let trip_rank = rank_counts.iter().find(|&(_, &c)| c == 3).map(|(&r, _)| r).unwrap();
        return HandEvaluation {
            rank: HandRank::ThreeOfAKind,
            kickers: vec![trip_rank],
            description: format!("Three of a kind, {}", rank_name(trip_rank)),
        };
    }

    if pairs >= 2 {
        return HandEvaluation {
            rank: HandRank::TwoPair,
            kickers: highest_paired_rank.into_iter().collect(),
            description: "Two pair".to_string(),
        };
    }

    if pairs == 1 {
        let pair_rank = highest_paired_rank.unwrap();
        return HandEvaluation {
            rank: HandRank::Pair,
            kickers: vec![pair_rank],
            description: format!("Pair of {}", rank_name(pair_rank)),
        };
    }

    let mut ranks: Vec<Rank> = cards.iter().map(|c| c.rank).collect();
    ranks.sort_by(|a, b| b.cmp(a));
    let high_card = ranks[0];
    HandEvaluation {
        rank: HandRank::HighCard,
        kickers: ranks,
        description: format!("{} high", rank_name(high_card)),
    }
}

fn evaluate_five(cards: &[Card]) -> HandEvaluation {
    let mut rank_counts: HashMap<Rank, u8> = HashMap::new();
    let mut suit_counts: HashMap<super::deck::Suit, u8> = HashMap::new();

    for card in cards {
        *rank_counts.entry(card.rank).or_insert(0) += 1;
        *suit_counts.entry(card.suit).or_insert(0) += 1;
    }

    let is_flush = suit_counts.values().any(|&c| c >= 5);

    let mut ranks: Vec<Rank> = cards.iter().map(|c| c.rank).collect();
    ranks.sort_by(|a, b| b.cmp(a));
    ranks.dedup();

    let straight_high = check_straight(&ranks);

    if is_flush {
        if let Some(high) = straight_high {
            return HandEvaluation {
                rank: HandRank::StraightFlush,
                kickers: vec![high],
                description: format!("{} high straight flush", rank_name(high)),
            };
        }
    }

    if let Some((&rank, _)) = rank_counts.iter().find(|&(_, &c)| c == 4) {
        return HandEvaluation {
            rank: HandRank::FourOfAKind,
            kickers: vec![rank],
            description: format!("Four of a kind, {}", rank_name(rank)),
        };
    }

    let trips = rank_counts.iter().find(|&(_, &c)| c == 3).map(|(&r, _)| r);
    let pair = rank_counts.iter().find(|&(_, &c)| c == 2).map(|(&r, _)| r);

    if trips.is_some() && pair.is_some() {
        return HandEvaluation {
            rank: HandRank::FullHouse,
            kickers: vec![trips.unwrap(), pair.unwrap()],
            description: format!(
                "Full house, {} full of {}",
                rank_name(trips.unwrap()),
                rank_name(pair.unwrap())
            ),
        };
    }

    if is_flush {
        return HandEvaluation {
            rank: HandRank::Flush,
            kickers: ranks.clone(),
            description: format!("{} high flush", rank_name(ranks[0])),
        };
    }

    if let Some(high) = straight_high {
        return HandEvaluation {
            rank: HandRank::Straight,
            kickers: vec![high],
            description: format!("{} high straight", rank_name(high)),
        };
    }

    if let Some(trip_rank) = trips {
        return HandEvaluation {
            rank: HandRank::ThreeOfAKind,
            kickers: vec![trip_rank],
            description: format!("Three of a kind, {}", rank_name(trip_rank)),
        };
    }

    let pairs: Vec<Rank> = rank_counts.iter().filter(|&(_, &c)| c == 2).map(|(&r, _)| r).collect();

    if pairs.len() >= 2 {
        let mut sorted_pairs = pairs.clone();
        sorted_pairs.sort_by(|a, b| b.cmp(a));
        let high_pair = sorted_pairs[0];
        let low_pair = sorted_pairs[1];
        return HandEvaluation {
            rank: HandRank::TwoPair,
            kickers: sorted_pairs,
            description: format!("Two pair, {} and {}", rank_name(high_pair), rank_name(low_pair)),
        };
    }

    if pairs.len() == 1 {
        return HandEvaluation {
            rank: HandRank::Pair,
            kickers: vec![pairs[0]],
            description: format!("Pair of {}", rank_name(pairs[0])),
        };
    }

    HandEvaluation {
        rank: HandRank::HighCard,
        kickers: ranks,
        description: format!(
            "{} high",
            rank_name(cards.iter().map(|c| c.rank).max().unwrap())
        ),
    }
}

fn check_straight(sorted_ranks: &[Rank]) -> Option<Rank> {
    if sorted_ranks.len() < 5 {
        return None;
    }

    let values: Vec<u8> = sorted_ranks.iter().map(|r| *r as u8).collect();
    if values.contains(&14) && values.contains(&2) && values.contains(&3)
        && values.contains(&4) && values.contains(&5)
    {
        return Some(Rank::Five);
    }

    for window in sorted_ranks.windows(5) {
        let vals: Vec<u8> = window.iter().map(|r| *r as u8).collect();
        if vals[0] as i8 - vals[4] as i8 == 4 {
            return Some(window[0]);
        }
    }

    None
}

fn combinations(cards: &[Card], k: usize) -> Vec<Vec<Card>> {
    if k == 0 {
        return vec![vec![]];
    }
    if cards.len() < k {
        return vec![];
    }

    let mut result = Vec::new();
    for (i, &card) in cards.iter().enumerate() {
        let rest = &cards[i + 1..];
        for mut combo in combinations(rest, k - 1) {
            combo.insert(0, card);
            result.push(combo);
        }
    }
    result
}

fn rank_name(rank: Rank) -> &'static str {
    match rank {
        Rank::Two => "twos",
        Rank::Three => "threes",
        Rank::Four => "fours",
        Rank::Five => "fives",
        Rank::Six => "sixes",
        Rank::Seven => "sevens",
        Rank::Eight => "eights",
        Rank::Nine => "nines",
        Rank::Ten => "tens",
        Rank::Jack => "jacks",
        Rank::Queen => "queens",
        Rank::King => "kings",
        Rank::Ace => "aces",
    }
}
