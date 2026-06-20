use std::collections::HashSet;

/// Normalize common leet-speak substitutions to plain letters before matching.
fn normalize_leet(ch: char) -> char {
    match ch {
        '0' => 'o',
        '1' | '!' | '|' => 'i',
        '2' => 'z',
        '3' => 'e',
        '4' => 'a',
        '5' | '$' => 's',
        '6' => 'g',
        '7' | '+' => 't',
        '8' => 'b',
        '9' => 'g',
        '@' => 'a',
        c => c,
    }
}

/// Returns true if `text` contains any word from `bad_words` after leet-speak normalization.
pub fn contains_flagged_word(text: &str, bad_words: &[String]) -> bool {
    if bad_words.is_empty() {
        return false;
    }
    let bad_set: HashSet<String> = bad_words.iter().map(|w| w.to_lowercase()).collect();
    let normalized: String = text
        .chars()
        .map(|c| normalize_leet(c.to_lowercase().next().unwrap_or(c)))
        .collect();
    normalized
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|w| !w.is_empty())
        .any(|w| bad_set.contains(w))
}

pub fn censor_bad_words(message: &str, bad_words: &[String], word_whitelist: &[String]) -> String {
    if bad_words.is_empty() {
        return message.to_owned();
    }

    let bad_words = bad_words
        .iter()
        .map(|word| word.to_lowercase())
        .collect::<HashSet<_>>();
    let word_whitelist = word_whitelist
        .iter()
        .map(|word| word.to_lowercase())
        .collect::<HashSet<_>>();

    let mut output = String::with_capacity(message.len());
    let mut token = String::new();

    for ch in message.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            token.push(ch);
            continue;
        }

        push_censored_token(&mut output, &token, &bad_words, &word_whitelist);
        token.clear();
        output.push(ch);
    }

    push_censored_token(&mut output, &token, &bad_words, &word_whitelist);
    output
}

fn push_censored_token(
    output: &mut String,
    token: &str,
    bad_words: &HashSet<String>,
    word_whitelist: &HashSet<String>,
) {
    if token.is_empty() {
        return;
    }

    let normalized = token.to_lowercase();
    if word_whitelist.contains(&normalized) || !bad_words.contains(&normalized) {
        output.push_str(token);
        return;
    }

    let mut chars = token.chars();
    if let Some(first) = chars.next() {
        output.push(first);
        output.extend(chars.map(|_| '*'));
    }
}
