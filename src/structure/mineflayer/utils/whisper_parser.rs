#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Whisper {
    pub sender: String,
    pub message: String,
}

pub fn parse(_message: &str) -> Option<Whisper> {
    None
}
