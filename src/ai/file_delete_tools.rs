use serde_json::Value;

use crate::models::files::{FileDeleteCandidatePage, FileDeletePrepareRequest};

#[derive(Debug, Default)]
pub struct FileDeleteToolResult {
    pub message: String,
    pub pending_file_delete_candidates: Option<FileDeleteCandidatePage>,
}

pub async fn prepare_delete_from_args(args: &Value) -> FileDeleteToolResult {
    let query = args["query"].as_str().unwrap_or_default().trim().to_string();
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
    let kind = args["kind"]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let max_results = args["max_results"]
        .as_u64()
        .map(|value| value.clamp(1, 50) as u8);

    let request_value = serde_json::json!({
        "query": query,
        "root_path": root_path,
        "extension": extension,
        "kind": kind.unwrap_or_else(|| "any".to_string()),
        "max_results": max_results,
    });

    let request = match serde_json::from_value::<FileDeletePrepareRequest>(request_value) {
        Ok(request) => request,
        Err(error) => {
            return FileDeleteToolResult {
                message: format!("시스템 거절: 파일/폴더 삭제 인자 형식이 올바르지 않습니다: {error}"),
                pending_file_delete_candidates: None,
            };
        }
    };

    let response = crate::files::delete::prepare_delete_target(request).await;

    if response.ok {
        FileDeleteToolResult {
            message: response.message,
            pending_file_delete_candidates: response.candidate_page,
        }
    } else {
        FileDeleteToolResult {
            message: response.message,
            pending_file_delete_candidates: None,
        }
    }
}
