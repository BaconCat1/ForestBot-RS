#[allow(dead_code)]
pub fn resolve(message: &str) -> Option<String> {
    message.split_once(':').map(|(sender, _)| sender.to_owned())
}
