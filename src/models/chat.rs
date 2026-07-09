use serde::{Deserialize, Serialize};

use crate::models::files::{FileContentEditCandidatePage, FileCreateCandidatePage, FileDeleteCandidatePage, FileOpenCandidate, FileOpenCandidatePage, FileRenameCandidatePage, FileTransferPending};

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_file_rename_candidates: Option<FileRenameCandidatePage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_file_create_candidates: Option<FileCreateCandidatePage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_file_content_edit_candidates: Option<FileContentEditCandidatePage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_file_delete_candidates: Option<FileDeleteCandidatePage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_file_transfer_candidates: Option<FileTransferPending>,
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
            pending_file_rename_candidates: None,
            pending_file_create_candidates: None,
            pending_file_content_edit_candidates: None,
            pending_file_delete_candidates: None,
            pending_file_transfer_candidates: None,
        }
    }
}
