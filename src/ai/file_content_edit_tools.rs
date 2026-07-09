use serde_json::Value;

use crate::models::files::{FileContentEditCandidatePage, FileContentEditPrepareRequest};

#[derive(Debug, Default)]
pub struct FileContentEditToolResult {
    pub message: String,
    pub pending_file_content_edit_candidates: Option<FileContentEditCandidatePage>,
}

pub async fn prepare_content_edit_from_args(args: &Value) -> FileContentEditToolResult {
    let query = args["query"].as_str().unwrap_or_default().trim().to_string();
    let instruction = args["instruction"].as_str().unwrap_or_default().trim().to_string();
    let root_path = args["root_path"]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let extension = args["extension"]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let max_results = args["max_results"]
        .as_u64()
        .map(|value| value.clamp(1, 50) as u8);

    let response = crate::files::content_edit::prepare_content_edit_target(FileContentEditPrepareRequest {
        query,
        instruction,
        root_path,
        extension,
        max_results,
    })
    .await;

    if response.ok {
        FileContentEditToolResult {
            message: response.message,
            pending_file_content_edit_candidates: response.candidate_page,
        }
    } else {
        FileContentEditToolResult {
            message: response.message,
            pending_file_content_edit_candidates: None,
        }
    }
}
