#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedChat {
    pub username: String,
    pub message: String,
}

const DEFAULT_FORMATS: &[&str] = &[
    "<{username}> {message}",
    "{username}: {message}",
    "[{skip}] {username} >> {message}",
    "[{skip}] {username} » {message}",
    "{username} » {message}",
    "{username}: {message}",
];

pub fn parse(message: &str, configured_formats: &[String]) -> Option<ParsedChat> {
    let cleaned = normalize_message(message);
    if cleaned.is_empty() {
        return None;
    }

    configured_formats
        .iter()
        .filter_map(|format| parse_with_format(&cleaned, format))
        .next()
        .or_else(|| {
            DEFAULT_FORMATS
                .iter()
                .filter_map(|format| parse_with_format(&cleaned, format))
                .next()
        })
}

fn parse_with_format(message: &str, format: &str) -> Option<ParsedChat> {
    let username_token = "{username}";
    let message_token = "{message}";

    let username_index = format.find(username_token)?;
    let message_index = format.find(message_token)?;
    if username_index > message_index {
        return None;
    }

    let before_username = &format[..username_index];
    let between = &format[username_index + username_token.len()..message_index];
    let after_message = &format[message_index + message_token.len()..];

    let mut rest = strip_prefix_with_skip(message, before_username)?;
    let between_literal = literal_without_skip(between);
    let username_end = rest.find(&between_literal)?;
    let username = normalize_username(rest[..username_end].trim());
    rest = &rest[username_end + between_literal.len()..];

    let message = if after_message.is_empty() {
        rest
    } else {
        let message_end = rest.rfind(after_message)?;
        &rest[..message_end]
    }
    .trim();

    if username.is_empty() || message.is_empty() {
        return None;
    }

    Some(ParsedChat {
        username,
        message: message.to_owned(),
    })
}

pub fn normalize_username(raw_username: &str) -> String {
    let cleaned = raw_username
        .trim()
        .trim_matches(|ch: char| matches!(ch, '<' | '>' | '[' | ']'));

    cleaned
        .split_whitespace()
        .rev()
        .find(|part| is_minecraft_username(part))
        .unwrap_or(cleaned)
        .to_owned()
}

fn is_minecraft_username(value: &str) -> bool {
    let len = value.chars().count();
    (1..=16).contains(&len)
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn strip_literal<'a>(message: &'a str, literal: &str) -> Option<&'a str> {
    let literal = literal_without_skip(literal);
    if literal.is_empty() {
        return Some(message);
    }

    message.strip_prefix(&literal)
}

fn strip_prefix_with_skip<'a>(message: &'a str, literal: &str) -> Option<&'a str> {
    let Some(skip_index) = literal.find("{skip}") else {
        return strip_literal(message, literal);
    };

    let before_skip = literal[..skip_index].trim();
    let after_skip = literal[skip_index + "{skip}".len()..].trim();
    let mut rest = if before_skip.is_empty() {
        message
    } else {
        message.strip_prefix(before_skip)?
    };

    if !after_skip.is_empty() {
        let after_skip_index = rest.find(after_skip)?;
        rest = &rest[after_skip_index + after_skip.len()..];
    }

    Some(rest.trim_start())
}

fn literal_without_skip(literal: &str) -> String {
    literal.replace("{skip}", "").trim().to_owned()
}

fn normalize_message(message: &str) -> String {
    let mut cleaned = String::with_capacity(message.len());
    let normalized = message.replace("\u{00c2}\u{00bb}", "»").replace(">>", "»");
    let mut chars = normalized.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '&' || ch == '§' {
            chars.next();
            continue;
        }

        cleaned.push(ch);
    }

    cleaned.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_arrow_chat() {
        assert_eq!(
            parse("Digital_10 » he doesnt parse that yet", &[]),
            Some(ParsedChat {
                username: "Digital_10".to_owned(),
                message: "he doesnt parse that yet".to_owned(),
            })
        );
    }

    #[test]
    fn parses_colon_chat() {
        assert_eq!(
            parse("Digital_10: !ping", &[]),
            Some(ParsedChat {
                username: "Digital_10".to_owned(),
                message: "!ping".to_owned(),
            })
        );
    }

    #[test]
    fn parses_configured_skip_prefix() {
        assert_eq!(
            parse("[world] Digital_10 >> !help", &[]),
            Some(ParsedChat {
                username: "Digital_10".to_owned(),
                message: "!help".to_owned(),
            })
        );
    }

    #[test]
    fn parses_clan_prefix_before_username() {
        assert_eq!(
            parse("RSP DaddyPayMe: !ping", &[]),
            Some(ParsedChat {
                username: "DaddyPayMe".to_owned(),
                message: "!ping".to_owned(),
            })
        );
    }

    #[test]
    fn normalizes_split_sender_with_clan_prefix() {
        assert_eq!(normalize_username("RSP DaddyPayMe"), "DaddyPayMe");
        assert_eq!(normalize_username("FCHOA Fwuffian"), "Fwuffian");
    }
}
