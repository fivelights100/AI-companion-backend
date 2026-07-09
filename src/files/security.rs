use std::path::Path;

pub const ALLOWED_OPEN_EXTENSIONS: &[&str] = &[
    "pdf", "txt", "md", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "png", "jpg",
    "jpeg", "gif", "mp3", "mp4", "zip", "js", "ts", "rs", "py", "json", "yaml",
    "html", "css", "exe", "msi", "bat", "cmd", "ps1", "vbs", "scr", "lnk", "dll", "sys", "reg",
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
            "현재 파일 시스템 설정상 .{} 파일은 열 수 없습니다.",
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
        "exe" | "msi" | "bat" | "cmd" | "ps1" | "vbs" | "scr" | "lnk" | "dll" | "sys" | "reg" => "실행 파일",
        _ => "파일",
    }
}

pub const DANGEROUS_EXTENSIONS: &[&str] = &[
    "exe", "msi", "bat", "cmd", "ps1", "vbs", "scr", "lnk", "dll", "sys", "reg",
];

pub const MAX_EDIT_TEXT_BYTES: u64 = 2 * 1024 * 1024;


pub const ALLOWED_CREATE_FILE_EXTENSIONS: &[&str] = &[
    "txt", "md", "json", "yaml", "yml", "js", "ts", "rs", "py", "html", "css",
];

pub fn is_allowed_create_file_extension(extension: &str) -> bool {
    let extension = normalize_extension(extension);
    !extension.is_empty() && ALLOWED_CREATE_FILE_EXTENSIONS.contains(&extension.as_str())
}

pub fn allowed_create_extensions_label() -> String {
    ALLOWED_CREATE_FILE_EXTENSIONS.join(", ")
}

pub fn validate_create_file_name(name: &str) -> Option<String> {
    validate_new_file_name(name).map(|message| message.replace("변경 후 이름", "생성할 이름"))
}

pub fn validate_create_content_size(content: &str) -> Option<String> {
    if content.as_bytes().len() as u64 > MAX_EDIT_TEXT_BYTES {
        Some("생성할 파일 내용은 2MB 이하만 허용됩니다.".to_string())
    } else {
        None
    }
}

pub fn is_dangerous_extension(extension: &str) -> bool {
    let extension = normalize_extension(extension);
    !extension.is_empty() && DANGEROUS_EXTENSIONS.contains(&extension.as_str())
}

pub fn validate_new_file_name(value: &str) -> Option<String> {
    let name = value.trim().trim_matches('`').trim_matches('"').trim_matches('\'').trim();

    if name.is_empty() {
        return Some("변경 후 이름이 비어 있습니다.".to_string());
    }

    if name.chars().count() > 180 {
        return Some("변경 후 이름은 180자 이하만 허용됩니다.".to_string());
    }

    if contains_control_chars(name) {
        return Some("변경 후 이름에 허용되지 않는 제어 문자가 포함되어 있습니다.".to_string());
    }

    if name == "." || name == ".." {
        return Some("변경 후 이름으로 . 또는 .. 은 사용할 수 없습니다.".to_string());
    }

    let invalid_chars = ['\\', '/', ':', '*', '?', '"', '<', '>', '|'];
    if name.chars().any(|character| invalid_chars.contains(&character)) {
        return Some("변경 후 이름에는 경로 구분자나 Windows 예약 문자를 사용할 수 없습니다.".to_string());
    }

    let trimmed_end = name.trim_end_matches(|character| character == ' ' || character == '.');
    if trimmed_end != name {
        return Some("변경 후 이름은 공백이나 점(.)으로 끝날 수 없습니다.".to_string());
    }

    let reserved = [
        "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6",
        "com7", "com8", "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6",
        "lpt7", "lpt8", "lpt9",
    ];
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();

    if reserved.contains(&stem.as_str()) {
        return Some("Windows 예약 이름은 사용할 수 없습니다.".to_string());
    }

    let extension = Path::new(name)
        .extension()
        .map(|value| normalize_extension(&value.to_string_lossy()))
        .unwrap_or_default();

    let _ = extension;

    None
}

pub fn validate_edit_existing_target(path: &Path, expected_folder: bool) -> Result<(), String> {
    validate_path_string(&path.to_string_lossy())?;

    if !path.exists() {
        return Err("대상이 더 이상 존재하지 않습니다.".to_string());
    }

    if expected_folder && !path.is_dir() {
        return Err("대상이 폴더가 아닙니다.".to_string());
    }

    if !expected_folder && !path.is_file() {
        return Err("대상이 파일이 아닙니다.".to_string());
    }

    validate_not_restricted_path(path)?;

    if !expected_folder {
        let extension = path
            .extension()
            .map(|value| normalize_extension(&value.to_string_lossy()))
            .unwrap_or_default();

        let _ = extension;
    }

    Ok(())
}

pub fn validate_not_restricted_path(path: &Path) -> Result<(), String> {
    let raw = path.to_string_lossy().to_string();
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let normalized = normalize_windows_path(&canonical.to_string_lossy());
    let raw_normalized = normalize_windows_path(&raw);

    let restricted = vec!["c:\\windows".to_string()];

    for blocked in restricted {
        if blocked.ends_with("\\") {
            if normalized == blocked || raw_normalized == blocked {
                return Err("시스템 드라이브 루트는 수정할 수 없습니다.".to_string());
            }
            continue;
        }

        if is_same_or_child_path(&normalized, &blocked) || is_same_or_child_path(&raw_normalized, &blocked) {
            return Err("현재 안전 정책상 제한된 시스템 경로는 수정할 수 없습니다.".to_string());
        }
    }

    Ok(())
}

pub fn normalize_windows_path(value: &str) -> String {
    let mut normalized = value.trim().replace('/', "\\").to_ascii_lowercase();

    // Windows의 std::fs::canonicalize()는 로컬 경로도 \\?\C:\...
    // 형태의 verbatim 경로로 반환할 수 있다. 이 접두사를 UNC로 오판하면
    // 바탕화면/문서 같은 정상 사용자 경로까지 차단되므로 정규화 단계에서
    // 로컬 verbatim 접두사는 제거하고, \\?\UNC\... 형태만 실제 UNC로 유지한다.
    if let Some(stripped) = normalized.strip_prefix("\\\\?\\unc\\") {
        normalized = format!("\\\\{stripped}");
    } else if let Some(stripped) = normalized.strip_prefix("\\\\?\\") {
        normalized = stripped.to_string();
    }

    while normalized.ends_with('\\') && normalized.len() > 3 {
        normalized.pop();
    }
    normalized
}

pub fn is_same_or_child_path(path: &str, base: &str) -> bool {
    path == base || path.strip_prefix(base).map(|rest| rest.starts_with('\\')).unwrap_or(false)
}

fn is_unc_path(path: &str) -> bool {
    path.starts_with("\\\\")
}


pub const ALLOWED_CONTENT_EDIT_EXTENSIONS: &[&str] = &[
    "txt", "md", "json", "yaml", "yml", "js", "ts", "rs", "py", "html", "css",
];

pub fn is_allowed_content_edit_extension(extension: &str) -> bool {
    let extension = normalize_extension(extension);
    !extension.is_empty() && ALLOWED_CONTENT_EDIT_EXTENSIONS.contains(&extension.as_str())
}

pub fn allowed_content_edit_extensions_label() -> String {
    ALLOWED_CONTENT_EDIT_EXTENSIONS.join(", ")
}

pub fn validate_content_edit_instruction(instruction: &str) -> Option<String> {
    let instruction = instruction.trim();
    if instruction.is_empty() {
        return Some("수정 지시가 비어 있습니다.".to_string());
    }
    if instruction.chars().count() > 2000 {
        return Some("수정 지시는 2000자 이하만 허용됩니다.".to_string());
    }
    if contains_control_chars(instruction) {
        return Some("수정 지시에 허용되지 않는 제어 문자가 포함되어 있습니다.".to_string());
    }
    None
}

pub fn validate_text_file_size(path: &Path) -> Result<u64, String> {
    let metadata = path
        .metadata()
        .map_err(|error| format!("파일 정보를 확인할 수 없습니다: {error}"))?;
    let size = metadata.len();
    if size > MAX_EDIT_TEXT_BYTES {
        return Err("파일 크기가 2MB를 초과해서 현재 내용 수정 기능으로는 수정할 수 없습니다.".to_string());
    }
    Ok(size)
}

pub fn validate_after_content_size(content: &str) -> Result<(), String> {
    if content.as_bytes().len() as u64 > MAX_EDIT_TEXT_BYTES {
        Err("변경 후 내용이 2MB를 초과해서 저장할 수 없습니다.".to_string())
    } else {
        Ok(())
    }
}

pub fn validate_content_edit_target(path: &Path) -> Result<u64, String> {
    validate_edit_existing_target(path, false)?;

    let extension = path
        .extension()
        .map(|value| normalize_extension(&value.to_string_lossy()))
        .unwrap_or_default();

    if !is_allowed_content_edit_extension(&extension) {
        return Err(format!(
            "현재 내용 수정은 {} 확장자만 허용됩니다.",
            allowed_content_edit_extensions_label()
        ));
    }

    validate_text_file_size(path)
}
