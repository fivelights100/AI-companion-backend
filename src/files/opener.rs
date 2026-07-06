use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        OnceLock,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tokio::sync::Mutex;

use crate::{
    files::everything,
    models::files::{
        FileOpenCandidate, FileOpenCandidatePage, FileOpenConfirmRequest, FileOpenConfirmResponse,
        FileOpenKind, FileOpenNextRequest, FileOpenPrepareRequest, FileOpenPrepareResponse,
        FileSearchKind, FileSearchRequest, FileSearchResult,
    },
};

const OPEN_CONFIRM_TTL_SECONDS: u64 = 120;
const OPEN_SEARCH_MAX_RESULTS: u8 = 50;
const OPEN_CANDIDATE_PAGE_SIZE: usize = 7;

static PENDING_OPEN_CANDIDATES: OnceLock<Mutex<HashMap<String, StoredOpenCandidate>>> = OnceLock::new();
static PENDING_OPEN_REQUESTS: OnceLock<Mutex<HashMap<String, StoredOpenRequest>>> = OnceLock::new();
static ID_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
struct StoredOpenCandidate {
    candidate: FileOpenCandidate,
    created_at: SystemTime,
}

#[derive(Debug, Clone)]
struct StoredOpenRequest {
    candidates: Vec<FileOpenCandidate>,
    created_at: SystemTime,
}

pub async fn prepare_open_target(request: FileOpenPrepareRequest) -> FileOpenPrepareResponse {
    if let Some(message) = validate_prepare_request(&request) {
        return rejected_response("rejected", message);
    }

    let requested_kind = request.kind.clone().unwrap_or_default();
    let search_kind = match requested_kind {
        FileOpenKind::File => FileSearchKind::File,
        FileOpenKind::Folder => FileSearchKind::Folder,
        FileOpenKind::Any => FileSearchKind::Any,
    };

    let search_response = everything::search_files(FileSearchRequest {
        query: request.query.trim().to_string(),
        root_path: request.root_path.clone(),
        extension: request.extension.clone(),
        kind: Some(search_kind),
        max_results: Some(OPEN_SEARCH_MAX_RESULTS),
        match_path: Some(false),
    })
    .await;

    if !search_response.ok {
        return rejected_response("search_failed", search_response.message);
    }

    let mut rejected_count = 0usize;
    let mut candidates = Vec::new();

    for result in search_response.results {
        match build_allowed_candidate(&result) {
            Ok(candidate) => candidates.push(candidate),
            Err(_) => rejected_count += 1,
        }
    }

    if candidates.is_empty() {
        let message = if rejected_count > 0 {
            "검색 결과는 있었지만, 현재 안전 정책에서 열 수 있는 파일/폴더가 없었습니다. 허용 확장자는 pdf, txt, md, doc, docx, xls, xlsx, ppt, pptx, png, jpg, jpeg, gif, mp3, mp4, zip, js, ts, rs, py, json, yaml, html, css입니다.".to_string()
        } else {
            "열 수 있는 파일 또는 폴더를 찾지 못했습니다.".to_string()
        };

        return rejected_response("not_found", message);
    }

    build_candidates_response(candidates, 0).await
}

pub async fn next_open_candidates(request: FileOpenNextRequest) -> FileOpenPrepareResponse {
    cleanup_expired_open_state().await;

    let request_id = request.request_id.trim();
    if request_id.is_empty() {
        return rejected_response("rejected", "후보 목록 요청 ID가 비어 있습니다.".to_string());
    }

    let offset = request.offset.unwrap_or(0);
    let store = pending_requests_store().lock().await;
    let Some(stored_request) = store.get(request_id) else {
        return rejected_response(
            "not_found",
            "후보 목록이 만료되었거나 찾을 수 없습니다. 다시 요청해 주세요.".to_string(),
        );
    };

    if offset >= stored_request.candidates.len() {
        return rejected_response(
            "not_found",
            "더 보여줄 후보가 없습니다.".to_string(),
        );
    }

    let page = build_candidate_page(request_id.to_string(), &stored_request.candidates, offset);

    FileOpenPrepareResponse {
        ok: true,
        status: "candidates_ready".to_string(),
        message: page.message.clone(),
        candidates: page.candidates.clone(),
        candidate_page: Some(page),
    }
}

async fn build_candidates_response(mut candidates: Vec<FileOpenCandidate>, offset: usize) -> FileOpenPrepareResponse {
    cleanup_expired_open_state().await;

    let request_id = make_id("open-request");

    for candidate in &mut candidates {
        candidate.id = make_id("open-candidate");
    }

    {
        let mut candidate_store = pending_candidates_store().lock().await;
        let now = SystemTime::now();
        for candidate in &candidates {
            candidate_store.insert(
                candidate.id.clone(),
                StoredOpenCandidate {
                    candidate: candidate.clone(),
                    created_at: now,
                },
            );
        }
    }

    {
        let mut request_store = pending_requests_store().lock().await;
        request_store.insert(
            request_id.clone(),
            StoredOpenRequest {
                candidates: candidates.clone(),
                created_at: SystemTime::now(),
            },
        );
    }

    let page = build_candidate_page(request_id, &candidates, offset);

    FileOpenPrepareResponse {
        ok: true,
        status: "candidates_ready".to_string(),
        message: page.message.clone(),
        candidates: page.candidates.clone(),
        candidate_page: Some(page),
    }
}

fn build_candidate_page(
    request_id: String,
    candidates: &[FileOpenCandidate],
    offset: usize,
) -> FileOpenCandidatePage {
    let start = offset.min(candidates.len());
    let end = (start + OPEN_CANDIDATE_PAGE_SIZE).min(candidates.len());
    let page_candidates = candidates[start..end].to_vec();
    let has_more = end < candidates.len();
    let next_offset = has_more.then_some(end);

    FileOpenCandidatePage {
        request_id,
        candidates: page_candidates,
        has_more,
        next_offset,
        page_size: OPEN_CANDIDATE_PAGE_SIZE,
        message: "화면에 열 수 있는 후보를 띄웠습니다. 사용자가 항목을 선택해야 실제로 열립니다.".to_string(),
    }
}

pub async fn confirm_open_target(request: FileOpenConfirmRequest) -> FileOpenConfirmResponse {
    let candidate_id = request.candidate_id.trim();

    if candidate_id.is_empty() {
        return FileOpenConfirmResponse {
            ok: false,
            message: "열기 후보 ID가 비어 있습니다.".to_string(),
        };
    }

    let Some(candidate) = take_pending_candidate(candidate_id).await else {
        return FileOpenConfirmResponse {
            ok: false,
            message: "열기 요청이 만료되었거나 찾을 수 없습니다. 다시 요청해 주세요.".to_string(),
        };
    };

    if let Err(message) = revalidate_candidate(&candidate) {
        return FileOpenConfirmResponse { ok: false, message };
    }

    let path = PathBuf::from(&candidate.path);
    let open_result = if candidate.is_folder {
        open_folder(&path)
    } else {
        open_file(&path)
    };

    match open_result {
        Ok(()) => FileOpenConfirmResponse {
            ok: true,
            message: if candidate.is_folder {
                "폴더를 열었습니다.".to_string()
            } else {
                "파일을 열었습니다.".to_string()
            },
        },
        Err(message) => FileOpenConfirmResponse { ok: false, message },
    }
}

fn rejected_response(status: &str, message: String) -> FileOpenPrepareResponse {
    FileOpenPrepareResponse {
        ok: false,
        status: status.to_string(),
        message,
        candidates: vec![],
        candidate_page: None,
    }
}

fn validate_prepare_request(request: &FileOpenPrepareRequest) -> Option<String> {
    let query = request.query.trim();

    if query.is_empty() {
        return Some("열 파일/폴더 검색어가 비어 있습니다.".to_string());
    }

    if query.chars().count() > 128 {
        return Some("검색어는 128자 이하만 허용됩니다.".to_string());
    }

    if contains_control_chars(query) {
        return Some("검색어에 허용되지 않는 제어 문자가 포함되어 있습니다.".to_string());
    }

    if let Some(root_path) = request.root_path.as_deref() {
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
    }

    if let Some(extension) = request.extension.as_deref() {
        let extension = normalize_extension(extension);
        if extension.is_empty() {
            return Some("확장자 값이 비어 있습니다.".to_string());
        }
        if !is_allowed_file_extension(&extension) {
            return Some(format!(
                "현재 안전 정책상 .{} 파일은 열 수 없습니다. 실행 파일, 스크립트 실행 파일, 바로가기 파일은 허용하지 않습니다.",
                extension
            ));
        }
    }

    None
}

fn build_allowed_candidate(result: &FileSearchResult) -> Result<FileOpenCandidate, String> {
    let path = Path::new(&result.path);

    if contains_control_chars(&result.path) {
        return Err("경로에 제어 문자가 포함되어 있습니다.".to_string());
    }

    let parent_path = path
        .parent()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();

    let is_folder = path.is_dir() || result.is_folder;

    if is_folder {
        if !path.exists() || !path.is_dir() {
            return Err("폴더가 존재하지 않습니다.".to_string());
        }

        return Ok(FileOpenCandidate {
            id: String::new(),
            name: result.name.clone(),
            path: result.path.clone(),
            parent_path,
            is_folder: true,
            extension: None,
            category: "폴더".to_string(),
            requires_confirmation: true,
        });
    }

    if !path.exists() || !path.is_file() {
        return Err("파일이 존재하지 않습니다.".to_string());
    }

    let extension = result
        .extension
        .as_deref()
        .map(normalize_extension)
        .or_else(|| path.extension().map(|value| normalize_extension(&value.to_string_lossy())))
        .unwrap_or_default();

    if !is_allowed_file_extension(&extension) {
        return Err(format!("허용되지 않은 파일 확장자입니다: .{extension}"));
    }

    Ok(FileOpenCandidate {
        id: String::new(),
        name: result.name.clone(),
        path: result.path.clone(),
        parent_path,
        is_folder: false,
        extension: Some(extension.clone()),
        category: extension_category(&extension).to_string(),
        requires_confirmation: true,
    })
}

async fn take_pending_candidate(candidate_id: &str) -> Option<FileOpenCandidate> {
    cleanup_expired_open_state().await;

    let mut store = pending_candidates_store().lock().await;
    store.remove(candidate_id).map(|stored| stored.candidate)
}

async fn cleanup_expired_open_state() {
    let now = SystemTime::now();

    {
        let mut store = pending_candidates_store().lock().await;
        store.retain(|_, stored| {
            now.duration_since(stored.created_at)
                .map(|elapsed| elapsed <= Duration::from_secs(OPEN_CONFIRM_TTL_SECONDS))
                .unwrap_or(false)
        });
    }

    {
        let mut store = pending_requests_store().lock().await;
        store.retain(|_, stored| {
            now.duration_since(stored.created_at)
                .map(|elapsed| elapsed <= Duration::from_secs(OPEN_CONFIRM_TTL_SECONDS))
                .unwrap_or(false)
        });
    }
}

fn pending_candidates_store() -> &'static Mutex<HashMap<String, StoredOpenCandidate>> {
    PENDING_OPEN_CANDIDATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn pending_requests_store() -> &'static Mutex<HashMap<String, StoredOpenRequest>> {
    PENDING_OPEN_REQUESTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn make_id(prefix: &str) -> String {
    let counter = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();

    format!("{prefix}-{timestamp}-{counter}")
}

fn revalidate_candidate(candidate: &FileOpenCandidate) -> Result<(), String> {
    let path = Path::new(&candidate.path);

    if contains_control_chars(&candidate.path) {
        return Err("경로에 허용되지 않는 제어 문자가 포함되어 있어 열 수 없습니다.".to_string());
    }

    if candidate.is_folder {
        if path.exists() && path.is_dir() {
            return Ok(());
        }
        return Err("폴더가 더 이상 존재하지 않거나 폴더가 아닙니다.".to_string());
    }

    if !path.exists() || !path.is_file() {
        return Err("파일이 더 이상 존재하지 않거나 파일이 아닙니다.".to_string());
    }

    let extension = path
        .extension()
        .map(|value| normalize_extension(&value.to_string_lossy()))
        .unwrap_or_default();

    if !is_allowed_file_extension(&extension) {
        return Err(format!("현재 안전 정책상 .{extension} 파일은 열 수 없습니다."));
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn open_folder(path: &Path) -> Result<(), String> {
    Command::new("explorer.exe")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("폴더 열기 실패: {error}"))
}

#[cfg(not(target_os = "windows"))]
fn open_folder(path: &Path) -> Result<(), String> {
    open::that(path).map_err(|error| format!("폴더 열기 실패: {error}"))
}

fn open_file(path: &Path) -> Result<(), String> {
    open::that(path).map_err(|error| format!("파일 열기 실패: {error}"))
}

fn normalize_extension(value: &str) -> String {
    value.trim().trim_start_matches('.').to_ascii_lowercase()
}

fn contains_control_chars(value: &str) -> bool {
    value.chars().any(|character| character.is_control())
}

fn is_allowed_file_extension(extension: &str) -> bool {
    matches!(
        extension,
        "pdf"
            | "txt"
            | "md"
            | "doc"
            | "docx"
            | "xls"
            | "xlsx"
            | "ppt"
            | "pptx"
            | "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "mp3"
            | "mp4"
            | "zip"
            | "js"
            | "ts"
            | "rs"
            | "py"
            | "json"
            | "yaml"
            | "html"
            | "css"
    )
}

fn extension_category(extension: &str) -> &'static str {
    match extension {
        "pdf" | "txt" | "md" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" => "문서 파일",
        "png" | "jpg" | "jpeg" | "gif" => "이미지 파일",
        "mp3" | "mp4" => "미디어 파일",
        "zip" => "압축 파일",
        "js" | "ts" | "rs" | "py" | "json" | "yaml" | "html" | "css" => "코드 파일",
        _ => "파일",
    }
}
