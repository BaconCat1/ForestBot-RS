use rustrict::{Censor, Trie, Type};

use crate::{config::load_word_list, structure::mineflayer::bot::AzaleaState};

const BAD_WORDS_PATH: &str = "./json/bad_words.json";
const WORD_WHITELIST_PATH: &str = "./json/word_whitelist.json";

// Outbound chat censoring threshold: broad, so mild profanity still gets starred out.
const CENSOR_THRESHOLD: Type = Type::INAPPROPRIATE;
// content_flagged mod-alert threshold: severe only, so mods aren't paged for mild stuff --
// just the kind of thing the !ai slur-spam incident should have caught.
const FLAG_THRESHOLD: Type = Type::SEVERE;

/// Builds the merged profanity trie: rustrict's built-in dictionary (substring/leetspeak-aware,
/// fixes the whole-token-match bypass from the !ai incident), with json/bad_words.json entries
/// layered in as PROFANE|SEVERE and json/word_whitelist.json entries layered in as SAFE
/// overrides (false-positive exceptions admins add as they come up).
/// Leaks the built trie -- rebuilds only happen on !censor/!wordwhitelist edits or !reload,
/// not per-message, so the leaked memory from prior versions is negligible.
pub async fn build_trie() -> &'static Trie {
    let mut trie = Trie::default();
    for word in load_word_list(BAD_WORDS_PATH).await.unwrap_or_default() {
        trie.set(&word.to_lowercase(), Type::PROFANE | Type::SEVERE);
    }
    for word in load_word_list(WORD_WHITELIST_PATH).await.unwrap_or_default() {
        trie.set(&word.to_lowercase(), Type::SAFE);
    }
    Box::leak(Box::new(trie))
}

/// Reloads bad_words.json/word_whitelist.json and swaps in a freshly built trie.
pub async fn rebuild(state: &AzaleaState) {
    let trie = build_trie().await;
    *state
        .profanity_trie
        .write()
        .expect("profanity_trie lock poisoned") = Some(trie);
}

pub fn censor_message(trie: &'static Trie, message: &str) -> String {
    Censor::from_str(message)
        .with_trie(trie)
        .with_censor_threshold(CENSOR_THRESHOLD)
        .censor()
}

pub fn is_severely_flagged(trie: &'static Trie, text: &str) -> bool {
    Censor::from_str(text)
        .with_trie(trie)
        .analyze()
        .is(FLAG_THRESHOLD)
}
