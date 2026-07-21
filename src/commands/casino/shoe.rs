use rand::rngs::OsRng;
use rand::seq::SliceRandom;
use std::sync::Mutex;
use std::time::Instant;

const SHOE_LIFETIME_SECS: u64 = 3600;

// Single shared shuffled multi-deck shoe for blackjack/baccarat -- one shoe per
// game, dealt to every player, same as a real table. Ranks only (1..=13, Ace=1) --
// neither game's scoring cares about suit, unlike poker's Deck/Card/Suit
// (game/deck.rs), which this reuses the shuffle-and-deal mechanism from. Reshuffles
// automatically once SHOE_LIFETIME_SECS elapses or the shoe runs dry; deal() surfaces
// that back to the caller so it can be whispered to the player.
pub struct CardShoe {
    cards: Vec<u8>,
    index: usize,
    deck_count: u32,
    shuffled_at: Instant,
}

impl CardShoe {
    fn new(deck_count: u32) -> Self {
        let mut shoe = Self { cards: Vec::new(), index: 0, deck_count, shuffled_at: Instant::now() };
        shoe.shuffle();
        shoe
    }

    fn shuffle(&mut self) {
        let mut cards = Vec::with_capacity(52 * self.deck_count.max(1) as usize);
        for _ in 0..self.deck_count.max(1) {
            for rank in 1u8..=13 {
                for _ in 0..4 {
                    cards.push(rank);
                }
            }
        }
        cards.shuffle(&mut OsRng);
        self.cards = cards;
        self.index = 0;
        self.shuffled_at = Instant::now();
    }

    // Deals one card, transparently reshuffling first if the shoe expired or ran
    // dry. Returns a shuffle notice exactly when a reshuffle just happened --
    // `None` on an ordinary deal from an already-live shoe.
    fn deal(&mut self) -> (u8, Option<String>) {
        let expired = self.shuffled_at.elapsed().as_secs() >= SHOE_LIFETIME_SECS;
        let exhausted = self.index >= self.cards.len();
        let notice = if expired || exhausted {
            self.shuffle();
            let n = self.deck_count.max(1);
            Some(format!("Shuffling {n} deck{}.", if n == 1 { "" } else { "s" }))
        } else {
            None
        };
        let card = self.cards[self.index];
        self.index += 1;
        (card, notice)
    }
}

pub type Shoe = Mutex<CardShoe>;

pub fn new_shoe(deck_count: u32) -> Shoe {
    Mutex::new(CardShoe::new(deck_count))
}

// Deals one card from the shared shoe.
pub fn deal_one(shoe: &Shoe) -> (u8, Option<String>) {
    shoe.lock().expect("shoe lock").deal()
}
