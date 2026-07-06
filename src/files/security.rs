use std::path::Path;

pub const ALLOWED_OPEN_EXTENSIONS: &[&str] = &[
    "pdf", "txt", "md", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "png", "jpg",
    "jpeg", "gif", "mp3", "mp4", "zip", "js", "ts", "rs", "py", "json", "yaml",
    "html", "css",
];

pub fn validate_query(query: &str, empty_message: &str) -> Option<String> {
    let query = query.trim();

    if query.is_empty() {
        return Some(empty_message.to_string());
    }

    if query.chars().count() > 128 {
        return Some("검색어는 128자 이하만 허용됩니다.".to_string());
    }

    if contains_control_chars(query) {
        return Some("검색어에 허용되지 않는 제어 문자가 포함되어 있습니다.".to_string());
    }

    None
}

pub fn validate_root_path(root_path: Option<&str>) -> Option<String> {
    let Some(root_path) = root_path else {
        return None;
    };

    let root_path = root_path.trim();

    if root_path.is_empty() {
        return Some("검색 범위 폴더가 비어 있습니다.".to_string());
    }

    if contains_control_chars(root_path) {
        return Some("검색 범위 폴더에 허용되지 않는 제어 문자가 포함되어 있습니다.".to_string());
    }

    let path = Path::new(root_path);
    if !path.exists() || !path.is_dir() {
        return Some("검색 범위 폴더가 존재하지 않거나 폴더가 아닙니다.".to_string());
    }

    None
}

pub fn validate_search_extension(extension: Option<&str>) -> Option<String> {
    let Some(extension) = extension else {
        return None;
    };

    let extension = normalize_extension(extension);
    let valid = !extension.is_empty()
        && extension.len() <= 16
        && extension.chars().all(|character| character.is_ascii_alphanumeric());

    if valid {
        None
    } else {
        Some("확장자는 영문/숫자 16자 이하만 허용됩니다.".to_string())
    }
}

pub fn validate_open_extension(extension: Option<&str>) -> Option<String> {
    let Some(extension) = extension else {
        return None;
    };

    let extension = normalize_extension(extension);

    if extension.is_empty() {
        return Some("확장자 값이 비어 있습니다.".to_string());
    }

    if is_allowed_open_extension(&extension) {
        None
    } else {
        Some(format!(
            "현재 안전 정책상 .{} 파일은 열 수 없습니다. 실행 파일, 스크립트 실행 파일, 바로가기 파일은 허용하지 않습니다.",
            extension
        ))
    }
}

pub fn validate_path_string(path: &str) -> Result<(), String> {
    if contains_control_chars(path) {
        Err("경로에 허용되지 않는 제어 문자가 포함되어 있어 열 수 없습니다.".to_string())
    } else {
        Ok(())
    }
}

pub fn normalize_extension(value: &str) -> String {
    value.trim().trim_start_matches('.').to_ascii_lowercase()
}

pub fn contains_control_chars(value: &str) -> bool {
    value.chars().any(|character| character.is_control())
}

pub fn is_allowed_open_extension(extension: &str) -> bool {
    ALLOWED_OPEN_EXTENSIONS.contains(&extension)
}

pub fn allowed_open_extensions_label() -> String {
    ALLOWED_OPEN_EXTENSIONS.join(", ")
}

pub fn extension_category(extension: &str) -> &'static str {
    match extension {
        "pdf" | "txt" | "md" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" => "문서 파일",
        "png" | "jpg" | "jpeg" | "gif" => "이미지 파일",
        "mp3" | "mp4" => "미디어 파일",
        "zip" => "압축 파일",
        "js" | "ts" | "rs" | "py" | "json" | "yaml" | "html" | "css" => "코드 파일",
        _ => "파일",
    }
}
