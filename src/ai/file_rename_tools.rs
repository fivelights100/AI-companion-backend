use serde_json::{json, Value};

use crate::{
    files::rename,
    models::files::{FileRenameCandidatePage, FileRenamePrepareRequest},
};

#[derive(Debug, Default)]
pub struct FileRenameToolResult {
    pub message: String,
    pub pending_file_rename_candidates: Option<FileRenameCandidatePage>,
}

pub async fn prepare_rename_from_args(args: &Value) -> FileRenameToolResult {
    let request_value = json!({
        "query": args.get("query").and_then(Value::as_str).unwrap_or_default(),
        "new_name": args.get("new_name").and_then(Value::as_str).unwrap_or_default(),
        "root_path": args.get("root_path").and_then(Value::as_str),
        "extension": args.get("extension").and_then(Value::as_str),
        "kind": args.get("kind").and_then(Value::as_str).unwrap_or("any"),
        "max_results": args.get("max_results").and_then(Value::as_u64).unwrap_or(50),
    });

    let request = match serde_json::from_value::<FileRenamePrepareRequest>(request_value) {
        Ok(request) => request,
        Err(error) => {
            return FileRenameToolResult {
                message: format!("시스템 거절: 파일/폴더 이름 변경 인자 형식이 올바르지 않습니다: {error}"),
                pending_file_rename_candidates: None,
            };
        }
    };

    let response = rename::prepare_rename_target(request).await;

    if let Some(page) = response.candidate_page {
        return FileRenameToolResult {
            message: "이름을 변경할 후보를 데스크탑 팝업으로 전달했습니다. 최종 답변에서는 파일명, 폴더명, 경로, 변경 전/후 이름을 절대 말하지 말고 ‘화면의 후보 중 원하는 항목을 선택해줘’라고만 짧게 안내하세요.".to_string(),
            pending_file_rename_candidates: Some(page),
        };
    }

    FileRenameToolResult {
        message: response.message,
        pending_file_rename_candidates: None,
    }
}
