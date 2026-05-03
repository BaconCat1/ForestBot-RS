pub fn strip_minecraft_formatting(message: &str) -> String {
    let mut stripped = String::with_capacity(message.len());
    let mut chars = message.chars();

    while let Some(ch) = chars.next() {
        if ch == '&' || ch == '§' {
            chars.next();
            continue;
        }

        stripped.push(ch);
    }

    stripped
}
