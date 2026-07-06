use serde::{Deserialize, Serialize};

use crate::models::files::{FileOpenCandidate, FileOpenCandidatePage};

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
    pub ledger_updated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_file_open: Option<FileOpenCandidate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_file_open_candidates: Option<FileOpenCandidatePage>,
}

impl ChatResponse {
    pub fn fallback(reply: &str) -> Self {
        Self {
            reply: reply.to_string(),
            audio_base64: String::new(),
            schedule_updated: false,
            ledger_updated: false,
            pending_file_open: None,
            pending_file_open_candidates: None,
        }
    }
}
