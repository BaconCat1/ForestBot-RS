#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Whisper {
    pub sender: String,
    pub recipient: Option<String>,
    pub message: String,
}

pub fn parse(message: &str, bot_username: &str) -> Option<Whisper> {
    let message = message.trim();
    if message.is_empty() {
        return None;
    }

    if let Some(rest) = message.strip_prefix("[PM] ") {
        let (sender, rest) = rest.split_once(" → ")?;
        let (recipient, msg) = rest.split_once(" » ")?;
        return Some(Whisper {
            sender: sender.trim().to_owned(),
            recipient: Some(recipient.trim().to_owned()),
            message: msg.trim().to_owned(),
        });
    }

    parse_from(message, "From:", "»", bot_username)
        .or_else(|| parse_from(message, "[MSG]", "-> me:", bot_username))
        .or_else(|| parse_from_phrase(message, " whispers to you:", bot_username))
        .or_else(|| parse_from_phrase(message, " whispers:", bot_username))
        .or_else(|| parse_bracket_from(message, "-> me", bot_username))
        .or_else(|| parse_bracket_from(message, "-> You", bot_username))
        .or_else(|| parse_to(message, "[MSG] me ->", ":", bot_username))
        .or_else(|| parse_to(message, "To:", ":", bot_username))
        .or_else(|| parse_to(message, "To", "»", bot_username))
        .or_else(|| parse_to_phrase(message, "You whisper to", ":", bot_username))
        .or_else(|| parse_bracket_to(message, "me ->", bot_username))
        .or_else(|| parse_bracket_to(message, "You ->", bot_username))
}

fn parse_from(message: &str, prefix: &str, separator: &str, bot_username: &str) -> Option<Whisper> {
    let rest = message.strip_prefix(prefix)?.trim();
    let (sender, msg) = rest.split_once(separator)?;
    Some(Whisper {
        sender: sender.trim().to_owned(),
        recipient: Some(bot_username.to_owned()),
        message: msg.trim().to_owned(),
    })
}

fn parse_from_phrase(message: &str, separator: &str, bot_username: &str) -> Option<Whisper> {
    let (sender, msg) = message.split_once(separator)?;
    Some(Whisper {
        sender: sender.trim().to_owned(),
        recipient: Some(bot_username.to_owned()),
        message: msg.trim().to_owned(),
    })
}

fn parse_bracket_from(message: &str, marker: &str, bot_username: &str) -> Option<Whisper> {
    let rest = message.strip_prefix('[')?;
    let (sender, rest) = rest.split_once(marker)?;
    let msg = rest.strip_prefix(']')?;
    Some(Whisper {
        sender: sender.trim().to_owned(),
        recipient: Some(bot_username.to_owned()),
        message: msg.trim().to_owned(),
    })
}

fn parse_to(message: &str, prefix: &str, separator: &str, bot_username: &str) -> Option<Whisper> {
    let rest = message.strip_prefix(prefix)?.trim();
    let (recipient, msg) = rest.split_once(separator)?;
    Some(Whisper {
        sender: bot_username.to_owned(),
        recipient: Some(recipient.trim().to_owned()),
        message: msg.trim().to_owned(),
    })
}

fn parse_to_phrase(
    message: &str,
    prefix: &str,
    separator: &str,
    bot_username: &str,
) -> Option<Whisper> {
    parse_to(message, prefix, separator, bot_username)
}

fn parse_bracket_to(message: &str, marker: &str, bot_username: &str) -> Option<Whisper> {
    let rest = message.strip_prefix('[')?;
    let rest = rest.strip_prefix(marker)?.trim();
    let (recipient, msg) = rest.split_once(']')?;
    Some(Whisper {
        sender: bot_username.to_owned(),
        recipient: Some(recipient.trim().to_owned()),
        message: msg.trim().to_owned(),
    })
}

pub fn is_standing_command(message: &str, prefix: &str) -> bool {
    let trimmed = message.trim().to_lowercase();
    let Some(command) = trimmed.strip_prefix(prefix) else {
        return false;
    };
    let alias = command.split_whitespace().next().unwrap_or_default();
    alias == "standing" || alias == "status"
}

pub fn is_self_standing_command(message: &str, prefix: &str) -> bool {
    is_standing_command(message, prefix) && message.split_whitespace().count() == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_from_whispers() {
        assert_eq!(
            parse("From: Steve » !ping", "Bot"),
            Some(Whisper {
                sender: "Steve".to_owned(),
                recipient: Some("Bot".to_owned()),
                message: "!ping".to_owned(),
            })
        );
        assert_eq!(
            parse("Steve whispers to you: !help", "Bot").unwrap().sender,
            "Steve"
        );
    }

    #[test]
    fn parses_to_whispers() {
        assert_eq!(
            parse("You whisper to Steve: hello", "Bot"),
            Some(Whisper {
                sender: "Bot".to_owned(),
                recipient: Some("Steve".to_owned()),
                message: "hello".to_owned(),
            })
        );
    }
}
