use rand::rngs::OsRng;
use rand::seq::SliceRandom;
use std::sync::Mutex;
use std::time::Instant;

const SHOE_LIFETIME_SECS: u64 = 3600;

// Single shared shuffled multi-deck shoe for blackjack/baccarat -- one shoe per
// game, dealt to every player, same as a real table. Ranks only (1..=13, Ace=1) --
// neither game's scoring cares about suit, unlike poker's Deck/Card/Suit
// (game/deck.rs), which this reuses the shuffle-and-deal mechanism from. Starts
// empty and unshuffled: the first deal a player ever asks for is what triggers the
// initial shuffle (and its notice), rather than a silent shuffle happening at bot
// startup before anyone's watching. Reshuffles the same way once SHOE_LIFETIME_SECS
// elapses or the shoe runs dry.
pub struct CardShoe {
    cards: Vec<u8>,
    index: usize,
    deck_count: u32,
    game_name: &'static str,
    shuffled_at: Instant,
}

impl CardShoe {
    fn new(deck_count: u32, game_name: &'static str) -> Self {
        Self { cards: Vec::new(), index: 0, deck_count, game_name, shuffled_at: Instant::now() }
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

    // Forces the shoe empty so the next deal() reshuffles and announces it --
    // used by the whitelist-only `!bj clear`/`!baccarat clear` admin command to
    // test/demo the reshuffle notice on demand instead of waiting for natural
    // exhaustion or the 1hr timer.
    fn clear(&mut self) {
        self.cards.clear();
        self.index = 0;
    }

    // Deals one card, transparently shuffling first if the shoe is brand new,
    // expired, or ran dry. Returns a shuffle notice exactly when a (re)shuffle just
    // happened -- `None` on an ordinary deal from an already-live shoe.
    fn deal(&mut self) -> (u8, Option<String>) {
        let expired = self.shuffled_at.elapsed().as_secs() >= SHOE_LIFETIME_SECS;
        let exhausted = self.index >= self.cards.len(); // also true for a fresh, never-shuffled shoe
        let notice = if expired || exhausted {
            self.shuffle();
            let n = self.deck_count.max(1);
            Some(format!("Shuffling {n} Deck{} for {}...", if n == 1 { "" } else { "s" }, self.game_name))
        } else {
            None
        };
        let card = self.cards[self.index];
        self.index += 1;
        (card, notice)
    }
}

pub type Shoe = Mutex<CardShoe>;

pub fn new_shoe(deck_count: u32, game_name: &'static str) -> Shoe {
    Mutex::new(CardShoe::new(deck_count, game_name))
}

// Deals one card from the shared shoe.
pub fn deal_one(shoe: &Shoe) -> (u8, Option<String>) {
    shoe.lock().expect("shoe lock").deal()
}

// Forces the shoe empty; the next deal_one() call reshuffles and announces it.
pub fn clear_shoe(shoe: &Shoe) {
    shoe.lock().expect("shoe lock").clear();
}
