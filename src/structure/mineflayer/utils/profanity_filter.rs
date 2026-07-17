use rustrict::{Censor, Replacements, Trie, Type};

use crate::{config::load_word_list, structure::mineflayer::bot::AzaleaState};

const BAD_WORDS_PATH: &str = "./json/bad_words.json";
const WORD_WHITELIST_PATH: &str = "./json/word_whitelist.json";

// content_flagged mod-alert threshold: severe only, so mods aren't paged for mild stuff --
// just the kind of thing the !ai slur-spam incident should have caught.
const FLAG_THRESHOLD: Type = Type::SEVERE;

/// Maps the data-driven `censor_threshold` config value (RuntimeConfig, editable via
/// config.json + !reload, no recompile needed) to the rustrict severity bar outbound chat
/// must clear before it gets censored at all. Unrecognized values fall back to "moderate".
pub fn censor_threshold_from_config(value: &str) -> Type {
    match value.to_lowercase().as_str() {
        "mild" => Type::MILD_OR_HIGHER,
        "severe" => Type::SEVERE,
        _ => Type::MODERATE_OR_HIGHER,
    }
}

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

/// Strips rustrict leetspeak substitutions that were confirmed live to turn harmless bot output
/// into false-positive censoring, via the `trace` diagnostic binary:
/// - digit->letter (e.g. `9->g`, `0->o`): digit-heavy output (zip codes, prices) decoded into
///   real dictionary words (`90210` -> "gooch"). Digits still match themselves; only the letter
///   interpretations are removed.
/// - `#`->letter (`#->a`/`#->h`): `!faq`'s `#id/total` output has `#` decode to "h", then any
///   later `8` (digits in between are always skippable, substitution-independent) spells the
///   dictionary hit "h8". Confirmed the digit fix alone did NOT stop this -- `8` matches "h8"
///   literally, no digit substitution needed -- only stripping `#` itself closes it.
///
/// Neither strip affects literal profanity detection (real letters, unchanged).
///
/// Must run once at startup before any censoring happens and before any concurrent access to
/// `Replacements` -- call synchronously, not from a spawned task.
pub fn strip_false_positive_leetspeak() {
    // (source char, [letters it can stand in for]), read off rustrict's replacements.csv
    const SUBSTITUTIONS: &[(char, &[char])] = &[
        ('0', &['o']),
        ('1', &['i', 'l']),
        ('2', &['z']),
        ('3', &['b', 'e', 'g']),
        ('4', &['a']),
        ('5', &['s']),
        ('6', &['b', 's']),
        ('7', &['t']),
        ('8', &['b', 'h']),
        ('9', &['g', 'p', 'q']),
        ('#', &['a', 'h']),
    ];
    // Safe: called once, synchronously, at startup before any tokio::spawn'd task touches
    // censoring -- no concurrent access is possible yet.
    let replacements = unsafe { Replacements::customize_default() };
    for &(source, letters) in SUBSTITUTIONS {
        for &letter in letters {
            replacements.remove(source, letter);
            replacements.remove(source, letter.to_ascii_uppercase());
        }
    }
}

/// Reloads bad_words.json/word_whitelist.json and swaps in a freshly built trie.
pub async fn rebuild(state: &AzaleaState) {
    let trie = build_trie().await;
    *state
        .profanity_trie
        .write()
        .expect("profanity_trie lock poisoned") = Some(trie);
}

pub fn censor_message(trie: &'static Trie, message: &str, threshold: Type) -> String {
    Censor::from_str(message)
        .with_trie(trie)
        .with_censor_threshold(threshold)
        .censor()
}

pub fn is_severely_flagged(trie: &'static Trie, text: &str) -> bool {
    Censor::from_str(text)
        .with_trie(trie)
        .analyze()
        .is(FLAG_THRESHOLD)
}
