use serde_json::{json, Value};

use crate::{files::everything, models::files::FileSearchRequest};

pub async fn search_files_from_args(args: &Value) -> String {
    let request_value = json!({
        "query": args.get("query").and_then(Value::as_str).unwrap_or_default(),
        "root_path": args.get("root_path").and_then(Value::as_str),
        "extension": args.get("extension").and_then(Value::as_str),
        "kind": args.get("kind").and_then(Value::as_str).unwrap_or("any"),
        "max_results": args.get("max_results").and_then(Value::as_u64).unwrap_or(10),
        "match_path": args.get("match_path").and_then(Value::as_bool).unwrap_or(false),
    });

    let request = match serde_json::from_value::<FileSearchRequest>(request_value) {
        Ok(request) => request,
        Err(error) => {
            return format!("시스템 거절: 파일 검색 인자 형식이 올바르지 않습니다: {error}");
        }
    };

    let response = everything::search_files(request).await;
    if !response.ok {
        return response.message;
    }

    if response.results.is_empty() {
        return "검색 결과가 없습니다.".to_string();
    }

    let mut lines = vec![response.message];
    for (index, result) in response.results.iter().enumerate() {
        let kind = if result.is_folder { "폴더" } else { "파일" };
        lines.push(format!("{}. [{}] {}", index + 1, kind, result.path));
    }

    lines.join("\n")
}
