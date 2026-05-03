use crate::config::ApiConfig;

#[derive(Debug, Clone)]
pub struct ApiHandler {
    pub options: ApiConfig,
}

impl ApiHandler {
    pub fn new(options: ApiConfig) -> Self {
        Self { options }
    }
}
