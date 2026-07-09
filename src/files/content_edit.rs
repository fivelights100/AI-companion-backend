use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        OnceLock,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde_json::json;
use tokio::sync::Mutex;

use crate::{
    ai::client::{extract_reply_text, OpenAiClient, DEFAULT_CHAT_MODEL},
    files::{everything, permission, security},
    models::files::{
        FileContentEditCandidate, FileContentEditCandidatePage, FileContentEditConfirmRequest,
        FileContentEditConfirmResponse, FileContentEditConfirmation, FileContentEditNextRequest,
        FileContentEditPrepareRequest, FileContentEditPrepareResponse, FileContentEditPreviewRequest,
        FileContentEditPreviewResponse, FileSearchKind, FileSearchRequest, FileSearchResult,
    },
};

const CONTENT_EDIT_CONFIRM_TTL_SECONDS: u64 = 120;
pub const CONTENT_EDIT_CANDIDATE_PAGE_SIZE: usize = 7;
const CONTENT_EDIT_SEARCH_MAX_RESULTS: u8 = 50;

static PENDING_CONTENT_EDIT_CANDIDATES: OnceLock<Mutex<HashMap<String, StoredContentEditCandidate>>> = OnceLock::new();
static PENDING_CONTENT_EDIT_REQUESTS: OnceLock<Mutex<HashMap<String, StoredContentEditRequest>>> = OnceLock::new();
static PENDING_CONTENT_EDIT_CONFIRMATIONS: OnceLock<Mutex<HashMap<String, StoredContentEditConfirmation>>> = OnceLock::new();
static CONTENT_EDIT_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
struct StoredContentEditCandidate {
    candidate: FileContentEditCandidate,
    instruction: String,
    created_at: SystemTime,
}

#[derive(Debug, Clone)]
struct StoredContentEditRequest {
    candidates: Vec<FileContentEditCandidate>,
    instruction: String,
    created_at: SystemTime,
}

#[derive(Debug, Clone)]
struct StoredContentEditConfirmation {
    confirmation: FileContentEditConfirmation,
    before_content: String,
    created_at: SystemTime,
}

pub async fn prepare_content_edit_target(
    request: FileContentEditPrepareRequest,
) -> FileContentEditPrepareResponse {
    if let Err(message) = permission::ensure_modify_enabled() {
        return rejected_response("permission_denied", message);
    }
    if let Some(message) = validate_prepare_request(&request) {
        return rejected_response("rejected", message);
    }

    let search_response = everything::search_files(FileSearchRequest {
        query: request.query.trim().to_string(),
        root_path: request.root_path.clone(),
        extension: request.extension.clone(),
        kind: Some(FileSearchKind::File),
        max_results: Some(
            request
                .max_results
                .unwrap_or(CONTENT_EDIT_SEARCH_MAX_RESULTS)
                .clamp(1, CONTENT_EDIT_SEARCH_MAX_RESULTS),
        ),
        match_path: Some(false),
    })
    .await;

    if !search_response.ok {
        return rejected_response("search_failed", search_response.message);
    }

    let instruction = normalize_instruction(&request.instruction);
    let mut rejected_reasons = Vec::new();
    let mut candidates = Vec::new();

    for result in search_response.results {
        match build_content_edit_candidate(&result) {
            Ok(candidate) => candidates.push(candidate),
            Err(message) => {
                if rejected_reasons.len() < 3 {
                    rejected_reasons.push(message);
                }
            }
        }
    }

    if candidates.is_empty() {
        let message = if rejected_reasons.is_empty() {
            "내용을 수정할 텍스트/코드 파일을 찾지 못했습니다.".to_string()
        } else {
            format!(
                "검색 결과는 있었지만 내용 수정이 가능한 파일이 없었습니다. 첫 번째 거절 사유: {}",
                rejected_reasons.first().cloned().unwrap_or_default()
            )
        };
        return rejected_response("not_found", message);
    }

    store_content_edit_candidates(candidates, instruction).await
}

pub async fn next_content_edit_candidates(
    request: FileContentEditNextRequest,
) -> FileContentEditPrepareResponse {
    if let Err(message) = permission::ensure_modify_enabled() {
        return rejected_response("permission_denied", message);
    }
    match next_page(&request.request_id, request.offset.unwrap_or(0)).await {
        Ok(page) => response_from_page(page),
        Err(message) => rejected_response("not_found", message),
    }
}

pub async fn preview_content_edit_target(
    request: FileContentEditPreviewRequest,
) -> FileContentEditPreviewResponse {
    if let Err(message) = permission::ensure_modify_enabled() {
        return preview_rejected(message);
    }
    cleanup_expired_content_edit_state().await;

    let candidate_id = request.candidate_id.trim();
    if candidate_id.is_empty() {
        return preview_rejected("내용 수정 후보 ID가 비어 있습니다.".to_string());
    }

    let Some(stored) = pending_content_edit_candidates_store()
        .lock()
        .await
        .get(candidate_id)
        .cloned()
    else {
        return preview_rejected("내용 수정 후보가 만료되었거나 찾을 수 없습니다. 다시 요청해 주세요.".to_string());
    };

    match build_content_edit_confirmation(&stored.candidate, &stored.instruction).await {
        Ok((mut confirmation, before_content)) => {
            confirmation.edit_id = make_content_edit_id("content-edit-confirm");
            let edit_id = confirmation.edit_id.clone();
            let now = SystemTime::now();

            pending_content_edit_confirmations_store().lock().await.insert(
                edit_id,
                StoredContentEditConfirmation {
                    confirmation: confirmation.clone(),
                    before_content,
                    created_at: now,
                },
            );

            FileContentEditPreviewResponse {
                ok: true,
                status: "confirmation_ready".to_string(),
                message: "변경 내용을 화면에 띄웠습니다. 사용자가 적용을 눌러야 실제로 저장됩니다.".to_string(),
                confirmation: Some(confirmation),
            }
        }
        Err(message) => preview_rejected(message),
    }
}

pub async fn confirm_content_edit_target(
    request: FileContentEditConfirmRequest,
) -> FileContentEditConfirmResponse {
    if let Err(message) = permission::ensure_modify_enabled() {
        return FileContentEditConfirmResponse { ok: false, message };
    }
    cleanup_expired_content_edit_state().await;

    let edit_id = request.edit_id.trim();
    if edit_id.is_empty() {
        return FileContentEditConfirmResponse {
            ok: false,
            message: "내용 수정 확인 ID가 비어 있습니다.".to_string(),
        };
    }

    let Some(stored) = pending_content_edit_confirmations_store().lock().await.remove(edit_id) else {
        return FileContentEditConfirmResponse {
            ok: false,
            message: "내용 수정 요청이 만료되었거나 찾을 수 없습니다. 다시 요청해 주세요.".to_string(),
        };
    };

    if let Err(message) = revalidate_confirmation(&stored.confirmation, &stored.before_content) {
        return FileContentEditConfirmResponse { ok: false, message };
    }

    let target_path = PathBuf::from(&stored.confirmation.target_path);
    match fs::write(&target_path, stored.confirmation.after_content.as_bytes()) {
        Ok(()) => FileContentEditConfirmResponse {
            ok: true,
            message: "파일 내용을 수정했습니다.".to_string(),
        },
        Err(error) => FileContentEditConfirmResponse {
            ok: false,
            message: format!("파일 저장 실패: {error}"),
        },
    }
}

fn validate_prepare_request(request: &FileContentEditPrepareRequest) -> Option<String> {
    security::validate_query(&request.query, "내용을 수정할 파일 검색어가 비어 있습니다.")
        .or_else(|| security::validate_root_path(request.root_path.as_deref()))
        .or_else(|| security::validate_search_extension(request.extension.as_deref()))
        .or_else(|| validate_requested_extension(request.extension.as_deref()))
        .or_else(|| security::validate_content_edit_instruction(&request.instruction))
}

fn validate_requested_extension(extension: Option<&str>) -> Option<String> {
    let Some(extension) = extension else {
        return None;
    };

    let extension = security::normalize_extension(extension);
    if !security::is_allowed_content_edit_extension(&extension) {
        return Some(format!(
            "내용 수정은 {} 확장자만 허용됩니다.",
            security::allowed_content_edit_extensions_label()
        ));
    }

    permission::validate_extension_allowed(Some(&extension)).err()
}

fn normalize_instruction(value: &str) -> String {
    value.trim().trim_matches('`').trim().to_string()
}

fn build_content_edit_candidate(result: &FileSearchResult) -> Result<FileContentEditCandidate, String> {
    security::validate_path_string(&result.path)?;

    let path = Path::new(&result.path);
    let size_bytes = security::validate_content_edit_target(path)?;
    permission::validate_path_and_extension_for_settings(path, false)?;

    let extension = result
        .extension
        .as_deref()
        .map(security::normalize_extension)
        .or_else(|| path.extension().map(|value| security::normalize_extension(&value.to_string_lossy())))
        .unwrap_or_default();

    Ok(FileContentEditCandidate {
        id: String::new(),
        name: result.name.clone(),
        path: result.path.clone(),
        parent_path: parent_path_string(path),
        extension: if extension.is_empty() { None } else { Some(extension.clone()) },
        category: if extension.is_empty() {
            "텍스트 파일".to_string()
        } else {
            security::extension_category(&extension).to_string()
        },
        size_bytes,
    })
}

async fn build_content_edit_confirmation(
    candidate: &FileContentEditCandidate,
    instruction: &str,
) -> Result<(FileContentEditConfirmation, String), String> {
    let target_path = PathBuf::from(&candidate.path);
    security::validate_content_edit_target(&target_path)?;
    permission::validate_path_and_extension_for_settings(&target_path, false)?;

    let before_content = read_utf8_text_file(&target_path)?;
    let after_content = generate_after_content(&before_content, instruction, candidate.extension.as_deref()).await?;
    security::validate_after_content_size(&after_content)?;

    let target_name = target_path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "파일".to_string());

    let confirmation = FileContentEditConfirmation {
        edit_id: String::new(),
        operation: "내용 수정".to_string(),
        target_kind: "파일".to_string(),
        target_name,
        target_path: target_path.to_string_lossy().to_string(),
        extension: candidate.extension.clone(),
        before_content: before_content.clone(),
        after_content,
        warning: "적용하면 파일 전체 내용이 변경 후 내용으로 저장됩니다. 백업 기능은 아직 적용되지 않았습니다.".to_string(),
    };

    Ok((confirmation, before_content))
}

fn read_utf8_text_file(path: &Path) -> Result<String, String> {
    security::validate_content_edit_target(path)?;
    permission::validate_path_and_extension_for_settings(path, false)?;
    let bytes = fs::read(path).map_err(|error| format!("파일 읽기 실패: {error}"))?;
    String::from_utf8(bytes).map_err(|_| "UTF-8 텍스트로 읽을 수 없는 파일은 현재 수정할 수 없습니다.".to_string())
}

async fn generate_after_content(
    before_content: &str,
    instruction: &str,
    extension: Option<&str>,
) -> Result<String, String> {
    let openai = OpenAiClient::from_env(reqwest::Client::new());
    let extension_label = extension.unwrap_or("txt");

    let request_body = json!({
        "model": DEFAULT_CHAT_MODEL,
        "messages": [
            {
                "role": "system",
                "content": "너는 로컬 파일의 텍스트/코드 내용을 수정하는 도구야. 반드시 사용자의 지시에 맞게 수정된 전체 파일 내용만 반환해. 설명, 인사, 마크다운 코드블록, 따옴표 포장, 변경 요약은 절대 쓰지 마. 파일이 JSON/YAML/코드라면 문법을 최대한 유지해."
            },
            {
                "role": "user",
                "content": format!(
                    "확장자: .{extension_label}\n수정 지시:\n{instruction}\n\n기존 전체 파일 내용:\n---BEGIN FILE---\n{before_content}\n---END FILE---\n\n수정 후 전체 파일 내용만 반환해."
                )
            }
        ],
        "temperature": 0.2,
    });

    let response = openai.chat_completion(&request_body).await?;
    let raw = extract_reply_text(&response)
        .ok_or_else(|| "AI가 수정 결과를 반환하지 않았습니다.".to_string())?;
    let cleaned = strip_wrapping_code_fence(raw).to_string();

    if cleaned.trim().is_empty() && !before_content.trim().is_empty() {
        return Err("AI가 빈 수정 결과를 반환해서 저장을 중단했습니다.".to_string());
    }

    Ok(cleaned)
}

fn strip_wrapping_code_fence(value: &str) -> &str {
    let trimmed = value.trim();
    if !trimmed.starts_with("```") || !trimmed.ends_with("```") {
        return value;
    }

    let without_start = trimmed.trim_start_matches('`');
    let Some(first_newline) = without_start.find('\n') else {
        return value;
    };
    let body = &without_start[first_newline + 1..];
    body.strip_suffix("```").map(str::trim_end).unwrap_or(value)
}

fn revalidate_confirmation(
    confirmation: &FileContentEditConfirmation,
    expected_before_content: &str,
) -> Result<(), String> {
    let target_path = PathBuf::from(&confirmation.target_path);
    security::validate_content_edit_target(&target_path)?;
    permission::validate_path_and_extension_for_settings(&target_path, false)?;
    security::validate_after_content_size(&confirmation.after_content)?;

    let current_content = read_utf8_text_file(&target_path)?;
    if current_content != expected_before_content {
        return Err("미리보기 이후 파일 내용이 변경되어 저장을 중단했습니다. 다시 요청해 주세요.".to_string());
    }

    Ok(())
}

fn parent_path_string(path: &Path) -> String {
    path.parent()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default()
}

async fn store_content_edit_candidates(
    mut candidates: Vec<FileContentEditCandidate>,
    instruction: String,
) -> FileContentEditPrepareResponse {
    cleanup_expired_content_edit_state().await;

    let request_id = make_content_edit_id("content-edit-request");

    for candidate in &mut candidates {
        candidate.id = make_content_edit_id("content-edit-candidate");
    }

    let now = SystemTime::now();

    {
        let mut candidate_store = pending_content_edit_candidates_store().lock().await;
        for candidate in &candidates {
            candidate_store.insert(
                candidate.id.clone(),
                StoredContentEditCandidate {
                    candidate: candidate.clone(),
                    instruction: instruction.clone(),
                    created_at: now,
                },
            );
        }
    }

    {
        let mut request_store = pending_content_edit_requests_store().lock().await;
        request_store.insert(
            request_id.clone(),
            StoredContentEditRequest {
                candidates: candidates.clone(),
                instruction,
                created_at: now,
            },
        );
    }

    response_from_page(build_candidate_page(request_id, &candidates, 0))
}

async fn next_page(request_id: &str, offset: usize) -> Result<FileContentEditCandidatePage, String> {
    cleanup_expired_content_edit_state().await;

    let request_id = request_id.trim();
    if request_id.is_empty() {
        return Err("후보 목록 요청 ID가 비어 있습니다.".to_string());
    }

    let store = pending_content_edit_requests_store().lock().await;
    let Some(stored_request) = store.get(request_id) else {
        return Err("후보 목록이 만료되었거나 찾을 수 없습니다. 다시 요청해 주세요.".to_string());
    };

    if offset >= stored_request.candidates.len() {
        return Err("더 보여줄 후보가 없습니다.".to_string());
    }

    Ok(build_candidate_page(
        request_id.to_string(),
        &stored_request.candidates,
        offset,
    ))
}

fn response_from_page(page: FileContentEditCandidatePage) -> FileContentEditPrepareResponse {
    FileContentEditPrepareResponse {
        ok: true,
        status: "candidates_ready".to_string(),
        message: page.message.clone(),
        candidates: page.candidates.clone(),
        candidate_page: Some(page),
    }
}

fn build_candidate_page(
    request_id: String,
    candidates: &[FileContentEditCandidate],
    offset: usize,
) -> FileContentEditCandidatePage {
    let start = offset.min(candidates.len());
    let end = (start + CONTENT_EDIT_CANDIDATE_PAGE_SIZE).min(candidates.len());
    let page_candidates = candidates[start..end].to_vec();
    let has_more = end < candidates.len();
    let next_offset = has_more.then_some(end);

    FileContentEditCandidatePage {
        request_id,
        candidates: page_candidates,
        has_more,
        next_offset,
        page_size: CONTENT_EDIT_CANDIDATE_PAGE_SIZE,
        message: "화면에 내용을 수정할 후보를 띄웠습니다. 사용자가 항목을 선택해야 다음 단계로 진행됩니다.".to_string(),
    }
}

fn rejected_response(status: &str, message: String) -> FileContentEditPrepareResponse {
    FileContentEditPrepareResponse {
        ok: false,
        status: status.to_string(),
        message,
        candidates: vec![],
        candidate_page: None,
    }
}

fn preview_rejected(message: String) -> FileContentEditPreviewResponse {
    FileContentEditPreviewResponse {
        ok: false,
        status: "rejected".to_string(),
        message,
        confirmation: None,
    }
}

async fn cleanup_expired_content_edit_state() {
    let now = SystemTime::now();

    {
        let mut store = pending_content_edit_candidates_store().lock().await;
        store.retain(|_, stored| {
            now.duration_since(stored.created_at)
                .map(|elapsed| elapsed <= Duration::from_secs(CONTENT_EDIT_CONFIRM_TTL_SECONDS))
                .unwrap_or(false)
        });
    }

    {
        let mut store = pending_content_edit_requests_store().lock().await;
        store.retain(|_, stored| {
            now.duration_since(stored.created_at)
                .map(|elapsed| elapsed <= Duration::from_secs(CONTENT_EDIT_CONFIRM_TTL_SECONDS))
                .unwrap_or(false)
        });
    }

    {
        let mut store = pending_content_edit_confirmations_store().lock().await;
        store.retain(|_, stored| {
            now.duration_since(stored.created_at)
                .map(|elapsed| elapsed <= Duration::from_secs(CONTENT_EDIT_CONFIRM_TTL_SECONDS))
                .unwrap_or(false)
        });
    }
}

fn pending_content_edit_candidates_store() -> &'static Mutex<HashMap<String, StoredContentEditCandidate>> {
    PENDING_CONTENT_EDIT_CANDIDATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn pending_content_edit_requests_store() -> &'static Mutex<HashMap<String, StoredContentEditRequest>> {
    PENDING_CONTENT_EDIT_REQUESTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn pending_content_edit_confirmations_store() -> &'static Mutex<HashMap<String, StoredContentEditConfirmation>> {
    PENDING_CONTENT_EDIT_CONFIRMATIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn make_content_edit_id(prefix: &str) -> String {
    let counter = CONTENT_EDIT_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();

    format!("{prefix}-{timestamp}-{counter}")
}
