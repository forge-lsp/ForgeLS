//! Minimal http surface. Backing client to be added later.

#[derive(Debug, Clone)]
pub struct HttpClient {
    user_agent: String,
}

impl HttpClient {
    pub fn new(user_agent: impl Into<String>) -> Self {
        Self {
            user_agent: user_agent.into(),
        }
    }

    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }
}
