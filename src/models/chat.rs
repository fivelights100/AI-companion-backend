use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    pub history: Vec<ChatMessage>,
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub reply: String,
    pub audio_base64: String,
    pub schedule_updated: bool,
}

impl ChatResponse {
    pub fn fallback(reply: &str) -> Self {
        Self {
            reply: reply.to_string(),
            audio_base64: String::new(),
            schedule_updated: false,
        }
    }
}
