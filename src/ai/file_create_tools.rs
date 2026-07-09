use serde_json::{json, Value};

use crate::{
    files::create,
    models::files::{FileCreateCandidatePage, FileCreatePrepareRequest},
};

#[derive(Debug, Default)]
pub struct FileCreateToolResult {
    pub message: String,
    pub pending_file_create_candidates: Option<FileCreateCandidatePage>,
}

pub async fn prepare_create_from_args(args: &Value) -> FileCreateToolResult {
    let request_value = json!({
        "query": args.get("query").and_then(Value::as_str).unwrap_or_default(),
        "name": args.get("name").and_then(Value::as_str).unwrap_or_default(),
        "kind": args.get("kind").and_then(Value::as_str).unwrap_or("file"),
        "content": args.get("content").and_then(Value::as_str).unwrap_or_default(),
        "root_path": args.get("root_path").and_then(Value::as_str),
        "max_results": args.get("max_results").and_then(Value::as_u64).unwrap_or(50),
    });

    let request = match serde_json::from_value::<FileCreatePrepareRequest>(request_value) {
        Ok(request) => request,
        Err(error) => {
            return FileCreateToolResult {
                message: format!("시스템 거절: 파일/폴더 생성 인자 형식이 올바르지 않습니다: {error}"),
                pending_file_create_candidates: None,
            };
        }
    };

    let response = create::prepare_create_target(request).await;

    if let Some(page) = response.candidate_page {
        return FileCreateToolResult {
            message: "생성 위치 후보를 데스크탑 팝업으로 전달했습니다. 최종 답변에서는 파일명, 폴더명, 경로, 생성 내용은 절대 말하지 말고 ‘화면의 후보 중 생성 위치를 선택해줘’라고만 짧게 안내하세요.".to_string(),
            pending_file_create_candidates: Some(page),
        };
    }

    FileCreateToolResult {
        message: response.message,
        pending_file_create_candidates: None,
    }
}
