use serde_json::{json, Value};

use crate::{
    files::opener,
    models::files::{FileOpenCandidatePage, FileOpenPrepareRequest},
};

#[derive(Debug, Default)]
pub struct FileOpenToolResult {
    pub message: String,
    pub pending_file_open_candidates: Option<FileOpenCandidatePage>,
}

pub async fn prepare_open_from_args(args: &Value) -> FileOpenToolResult {
    let request_value = json!({
        "query": args.get("query").and_then(Value::as_str).unwrap_or_default(),
        "root_path": args.get("root_path").and_then(Value::as_str),
        "extension": args.get("extension").and_then(Value::as_str),
        "kind": args.get("kind").and_then(Value::as_str).unwrap_or("any"),
        "max_results": args.get("max_results").and_then(Value::as_u64).unwrap_or(50),
    });

    let request = match serde_json::from_value::<FileOpenPrepareRequest>(request_value) {
        Ok(request) => request,
        Err(error) => {
            return FileOpenToolResult {
                message: format!("시스템 거절: 파일/폴더 열기 인자 형식이 올바르지 않습니다: {error}"),
                pending_file_open_candidates: None,
            };
        }
    };

    let response = opener::prepare_open_target(request).await;

    if let Some(page) = response.candidate_page {
        return FileOpenToolResult {
            message: "열 수 있는 후보를 데스크탑 팝업으로 전달했습니다. 최종 답변에서는 파일명, 폴더명, 경로를 절대 말하지 말고 ‘화면의 후보 중 원하는 항목을 선택해줘’라고만 짧게 안내하세요.".to_string(),
            pending_file_open_candidates: Some(page),
        };
    }

    FileOpenToolResult {
        message: response.message,
        pending_file_open_candidates: None,
    }
}
