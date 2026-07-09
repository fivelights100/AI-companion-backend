use serde_json::{json, Value};

use crate::{
    files::transfer,
    models::files::{FileTransferPending, FileTransferPrepareRequest},
};

#[derive(Debug, Default)]
pub struct FileTransferToolResult {
    pub message: String,
    pub pending_file_transfer_candidates: Option<FileTransferPending>,
}

pub async fn prepare_transfer_from_args(args: &Value) -> FileTransferToolResult {
    let request_value = json!({
        "operation": args.get("operation").and_then(Value::as_str).unwrap_or("copy"),
        "source_query": args.get("source_query").and_then(Value::as_str).unwrap_or_default(),
        "destination_query": args.get("destination_query").and_then(Value::as_str).unwrap_or_default(),
        "root_path": args.get("root_path").and_then(Value::as_str),
        "extension": args.get("extension").and_then(Value::as_str),
        "kind": args.get("kind").and_then(Value::as_str).unwrap_or("any"),
        "max_results": args.get("max_results").and_then(Value::as_u64).unwrap_or(50),
    });

    let request = match serde_json::from_value::<FileTransferPrepareRequest>(request_value) {
        Ok(request) => request,
        Err(error) => {
            return FileTransferToolResult {
                message: format!("시스템 거절: 파일/폴더 복사·이동 인자 형식이 올바르지 않습니다: {error}"),
                pending_file_transfer_candidates: None,
            };
        }
    };

    let response = transfer::prepare_transfer_target(request).await;

    if let Some(pending) = response.pending {
        return FileTransferToolResult {
            message: "복사/이동 후보를 데스크탑 팝업으로 전달했습니다. 최종 답변에서는 파일명, 폴더명, 경로를 절대 말하지 말고 ‘화면에서 원본과 위치를 선택해줘’라고만 짧게 안내하세요.".to_string(),
            pending_file_transfer_candidates: Some(pending),
        };
    }

    FileTransferToolResult {
        message: response.message,
        pending_file_transfer_candidates: None,
    }
}
