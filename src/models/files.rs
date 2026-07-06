use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileSearchKind {
    Any,
    File,
    Folder,
}

impl Default for FileSearchKind {
    fn default() -> Self {
        Self::Any
    }
}

#[derive(Debug, Deserialize)]
pub struct FileSearchRequest {
    pub query: String,
    pub root_path: Option<String>,
    pub extension: Option<String>,
    pub kind: Option<FileSearchKind>,
    pub max_results: Option<u8>,
    pub match_path: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct FileSearchResult {
    pub path: String,
    pub name: String,
    pub is_folder: bool,
    pub extension: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FileSearchResponse {
    pub ok: bool,
    pub message: String,
    pub results: Vec<FileSearchResult>,
}

#[derive(Debug, Serialize)]
pub struct FileSearchStatus {
    pub available: bool,
    pub es_path: Option<String>,
    pub everything_app_path: Option<String>,
    pub everything_running: bool,
    pub message: String,
    pub install_hint: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileOpenKind {
    Any,
    File,
    Folder,
}

impl Default for FileOpenKind {
    fn default() -> Self {
        Self::Any
    }
}

#[derive(Debug, Deserialize)]
pub struct FileOpenPrepareRequest {
    pub query: String,
    pub root_path: Option<String>,
    pub extension: Option<String>,
    pub kind: Option<FileOpenKind>,
    pub max_results: Option<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileOpenCandidate {
    pub id: String,
    pub name: String,
    pub path: String,
    pub parent_path: String,
    pub is_folder: bool,
    pub extension: Option<String>,
    pub category: String,
    pub requires_confirmation: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileOpenCandidatePage {
    pub request_id: String,
    pub candidates: Vec<FileOpenCandidate>,
    pub has_more: bool,
    pub next_offset: Option<usize>,
    pub page_size: usize,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct FileOpenPrepareResponse {
    pub ok: bool,
    pub status: String,
    pub message: String,
    pub candidates: Vec<FileOpenCandidate>,
    pub candidate_page: Option<FileOpenCandidatePage>,
}

#[derive(Debug, Deserialize)]
pub struct FileOpenNextRequest {
    pub request_id: String,
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct FileOpenConfirmRequest {
    pub candidate_id: String,
}

#[derive(Debug, Serialize)]
pub struct FileOpenConfirmResponse {
    pub ok: bool,
    pub message: String,
}
