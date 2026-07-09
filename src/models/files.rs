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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileRenameKind {
    Any,
    File,
    Folder,
}

impl Default for FileRenameKind {
    fn default() -> Self {
        Self::Any
    }
}

#[derive(Debug, Deserialize)]
pub struct FileRenamePrepareRequest {
    pub query: String,
    pub new_name: String,
    pub root_path: Option<String>,
    pub extension: Option<String>,
    pub kind: Option<FileRenameKind>,
    pub max_results: Option<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileRenameCandidate {
    pub id: String,
    pub name: String,
    pub path: String,
    pub parent_path: String,
    pub is_folder: bool,
    pub extension: Option<String>,
    pub category: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileRenameCandidatePage {
    pub request_id: String,
    pub candidates: Vec<FileRenameCandidate>,
    pub has_more: bool,
    pub next_offset: Option<usize>,
    pub page_size: usize,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct FileRenamePrepareResponse {
    pub ok: bool,
    pub status: String,
    pub message: String,
    pub candidates: Vec<FileRenameCandidate>,
    pub candidate_page: Option<FileRenameCandidatePage>,
}

#[derive(Debug, Deserialize)]
pub struct FileRenameNextRequest {
    pub request_id: String,
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct FileRenamePreviewRequest {
    pub candidate_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileRenameConfirmation {
    pub edit_id: String,
    pub operation: String,
    pub target_kind: String,
    pub before_name: String,
    pub after_name: String,
    pub before_path: String,
    pub after_path: String,
    pub is_folder: bool,
    pub warning: String,
}

#[derive(Debug, Serialize)]
pub struct FileRenamePreviewResponse {
    pub ok: bool,
    pub status: String,
    pub message: String,
    pub confirmation: Option<FileRenameConfirmation>,
}

#[derive(Debug, Deserialize)]
pub struct FileRenameConfirmRequest {
    pub edit_id: String,
}

#[derive(Debug, Serialize)]
pub struct FileRenameConfirmResponse {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileCreateKind {
    File,
    Folder,
}

impl Default for FileCreateKind {
    fn default() -> Self {
        Self::File
    }
}

#[derive(Debug, Deserialize)]
pub struct FileCreatePrepareRequest {
    pub query: String,
    pub name: String,
    pub kind: Option<FileCreateKind>,
    pub content: Option<String>,
    pub root_path: Option<String>,
    pub max_results: Option<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileCreateCandidate {
    pub id: String,
    pub name: String,
    pub path: String,
    pub parent_path: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileCreateCandidatePage {
    pub request_id: String,
    pub candidates: Vec<FileCreateCandidate>,
    pub has_more: bool,
    pub next_offset: Option<usize>,
    pub page_size: usize,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct FileCreatePrepareResponse {
    pub ok: bool,
    pub status: String,
    pub message: String,
    pub candidates: Vec<FileCreateCandidate>,
    pub candidate_page: Option<FileCreateCandidatePage>,
}

#[derive(Debug, Deserialize)]
pub struct FileCreateNextRequest {
    pub request_id: String,
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct FileCreatePreviewRequest {
    pub candidate_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileCreateConfirmation {
    pub edit_id: String,
    pub operation: String,
    pub target_kind: String,
    pub parent_path: String,
    pub target_name: String,
    pub target_path: String,
    pub is_folder: bool,
    pub before: String,
    pub after: String,
    #[serde(skip_serializing)]
    pub content: String,
    pub warning: String,
}

#[derive(Debug, Serialize)]
pub struct FileCreatePreviewResponse {
    pub ok: bool,
    pub status: String,
    pub message: String,
    pub confirmation: Option<FileCreateConfirmation>,
}

#[derive(Debug, Deserialize)]
pub struct FileCreateConfirmRequest {
    pub edit_id: String,
}

#[derive(Debug, Serialize)]
pub struct FileCreateConfirmResponse {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct FileContentEditPrepareRequest {
    pub query: String,
    pub instruction: String,
    pub root_path: Option<String>,
    pub extension: Option<String>,
    pub max_results: Option<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileContentEditCandidate {
    pub id: String,
    pub name: String,
    pub path: String,
    pub parent_path: String,
    pub extension: Option<String>,
    pub category: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileContentEditCandidatePage {
    pub request_id: String,
    pub candidates: Vec<FileContentEditCandidate>,
    pub has_more: bool,
    pub next_offset: Option<usize>,
    pub page_size: usize,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct FileContentEditPrepareResponse {
    pub ok: bool,
    pub status: String,
    pub message: String,
    pub candidates: Vec<FileContentEditCandidate>,
    pub candidate_page: Option<FileContentEditCandidatePage>,
}

#[derive(Debug, Deserialize)]
pub struct FileContentEditNextRequest {
    pub request_id: String,
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct FileContentEditPreviewRequest {
    pub candidate_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileContentEditConfirmation {
    pub edit_id: String,
    pub operation: String,
    pub target_kind: String,
    pub target_name: String,
    pub target_path: String,
    pub extension: Option<String>,
    pub before_content: String,
    pub after_content: String,
    pub warning: String,
}

#[derive(Debug, Serialize)]
pub struct FileContentEditPreviewResponse {
    pub ok: bool,
    pub status: String,
    pub message: String,
    pub confirmation: Option<FileContentEditConfirmation>,
}

#[derive(Debug, Deserialize)]
pub struct FileContentEditConfirmRequest {
    pub edit_id: String,
}

#[derive(Debug, Serialize)]
pub struct FileContentEditConfirmResponse {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileDeleteKind {
    Any,
    File,
    Folder,
}

impl Default for FileDeleteKind {
    fn default() -> Self {
        Self::Any
    }
}

#[derive(Debug, Deserialize)]
pub struct FileDeletePrepareRequest {
    pub query: String,
    pub root_path: Option<String>,
    pub extension: Option<String>,
    pub kind: Option<FileDeleteKind>,
    pub max_results: Option<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileDeleteCandidate {
    pub id: String,
    pub name: String,
    pub path: String,
    pub parent_path: String,
    pub is_folder: bool,
    pub extension: Option<String>,
    pub category: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileDeleteCandidatePage {
    pub request_id: String,
    pub candidates: Vec<FileDeleteCandidate>,
    pub has_more: bool,
    pub next_offset: Option<usize>,
    pub page_size: usize,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct FileDeletePrepareResponse {
    pub ok: bool,
    pub status: String,
    pub message: String,
    pub candidates: Vec<FileDeleteCandidate>,
    pub candidate_page: Option<FileDeleteCandidatePage>,
}

#[derive(Debug, Deserialize)]
pub struct FileDeleteNextRequest {
    pub request_id: String,
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct FileDeletePreviewRequest {
    pub candidate_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileDeleteConfirmation {
    pub delete_id: String,
    pub operation: String,
    pub delete_method: String,
    pub target_kind: String,
    pub target_name: String,
    pub target_path: String,
    pub parent_path: String,
    pub is_folder: bool,
    pub warning: String,
}

#[derive(Debug, Serialize)]
pub struct FileDeletePreviewResponse {
    pub ok: bool,
    pub status: String,
    pub message: String,
    pub confirmation: Option<FileDeleteConfirmation>,
}

#[derive(Debug, Deserialize)]
pub struct FileDeleteConfirmRequest {
    pub delete_id: String,
}

#[derive(Debug, Serialize)]
pub struct FileDeleteConfirmResponse {
    pub ok: bool,
    pub message: String,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileTransferOperation {
    Copy,
    Move,
}

impl FileTransferOperation {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Copy => "복사",
            Self::Move => "이동",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileTransferKind {
    Any,
    File,
    Folder,
}

impl Default for FileTransferKind {
    fn default() -> Self {
        Self::Any
    }
}

#[derive(Debug, Deserialize)]
pub struct FileTransferPrepareRequest {
    pub operation: FileTransferOperation,
    pub source_query: String,
    pub destination_query: String,
    pub root_path: Option<String>,
    pub extension: Option<String>,
    pub kind: Option<FileTransferKind>,
    pub max_results: Option<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileTransferSourceCandidate {
    pub id: String,
    pub name: String,
    pub path: String,
    pub parent_path: String,
    pub is_folder: bool,
    pub extension: Option<String>,
    pub category: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileTransferDestinationCandidate {
    pub id: String,
    pub name: String,
    pub path: String,
    pub parent_path: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileTransferSourceCandidatePage {
    pub request_id: String,
    pub operation: FileTransferOperation,
    pub candidates: Vec<FileTransferSourceCandidate>,
    pub has_more: bool,
    pub next_offset: Option<usize>,
    pub page_size: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileTransferDestinationCandidatePage {
    pub request_id: String,
    pub operation: FileTransferOperation,
    pub candidates: Vec<FileTransferDestinationCandidate>,
    pub has_more: bool,
    pub next_offset: Option<usize>,
    pub page_size: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileTransferPending {
    pub request_id: String,
    pub operation: FileTransferOperation,
    pub source_page: FileTransferSourceCandidatePage,
    pub destination_page: FileTransferDestinationCandidatePage,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct FileTransferPrepareResponse {
    pub ok: bool,
    pub status: String,
    pub message: String,
    pub pending: Option<FileTransferPending>,
}

#[derive(Debug, Deserialize)]
pub struct FileTransferSourceNextRequest {
    pub request_id: String,
    pub offset: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct FileTransferSourceNextResponse {
    pub ok: bool,
    pub status: String,
    pub message: String,
    pub source_page: Option<FileTransferSourceCandidatePage>,
}

#[derive(Debug, Deserialize)]
pub struct FileTransferDestinationNextRequest {
    pub request_id: String,
    pub offset: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct FileTransferDestinationNextResponse {
    pub ok: bool,
    pub status: String,
    pub message: String,
    pub destination_page: Option<FileTransferDestinationCandidatePage>,
}

#[derive(Debug, Deserialize)]
pub struct FileTransferPreviewRequest {
    pub request_id: String,
    pub source_id: String,
    pub destination_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileTransferConfirmation {
    pub transfer_id: String,
    pub operation: String,
    pub target_kind: String,
    pub source_name: String,
    pub source_path: String,
    pub destination_folder_path: String,
    pub destination_path: String,
    pub is_folder: bool,
    pub warning: String,
}

#[derive(Debug, Serialize)]
pub struct FileTransferPreviewResponse {
    pub ok: bool,
    pub status: String,
    pub message: String,
    pub confirmation: Option<FileTransferConfirmation>,
}

#[derive(Debug, Deserialize)]
pub struct FileTransferConfirmRequest {
    pub transfer_id: String,
}

#[derive(Debug, Serialize)]
pub struct FileTransferConfirmResponse {
    pub ok: bool,
    pub message: String,
}
