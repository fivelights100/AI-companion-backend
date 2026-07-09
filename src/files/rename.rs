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

use tokio::sync::Mutex;

use crate::{
    files::{everything, permission, security},
    models::files::{
        FileRenameCandidate, FileRenameCandidatePage, FileRenameConfirmRequest,
        FileRenameConfirmResponse, FileRenameConfirmation, FileRenameKind, FileRenameNextRequest,
        FileRenamePrepareRequest, FileRenamePrepareResponse, FileRenamePreviewRequest,
        FileRenamePreviewResponse, FileSearchKind, FileSearchRequest, FileSearchResult,
    },
};

const RENAME_CONFIRM_TTL_SECONDS: u64 = 120;
pub const RENAME_CANDIDATE_PAGE_SIZE: usize = 7;
const RENAME_SEARCH_MAX_RESULTS: u8 = 50;

static PENDING_RENAME_CANDIDATES: OnceLock<Mutex<HashMap<String, StoredRenameCandidate>>> = OnceLock::new();
static PENDING_RENAME_REQUESTS: OnceLock<Mutex<HashMap<String, StoredRenameRequest>>> = OnceLock::new();
static PENDING_RENAME_CONFIRMATIONS: OnceLock<Mutex<HashMap<String, StoredRenameConfirmation>>> = OnceLock::new();
static RENAME_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
struct StoredRenameCandidate {
    candidate: FileRenameCandidate,
    new_name: String,
    created_at: SystemTime,
}

#[derive(Debug, Clone)]
struct StoredRenameRequest {
    candidates: Vec<FileRenameCandidate>,
    new_name: String,
    created_at: SystemTime,
}

#[derive(Debug, Clone)]
struct StoredRenameConfirmation {
    confirmation: FileRenameConfirmation,
    created_at: SystemTime,
}

pub async fn prepare_rename_target(request: FileRenamePrepareRequest) -> FileRenamePrepareResponse {
    if let Err(message) = permission::ensure_modify_enabled() {
        return rejected_response("permission_denied", message);
    }
    if let Some(message) = validate_prepare_request(&request) {
        return rejected_response("rejected", message);
    }

    let requested_kind = request.kind.clone().unwrap_or_default();
    let search_kind = match requested_kind {
        FileRenameKind::File => FileSearchKind::File,
        FileRenameKind::Folder => FileSearchKind::Folder,
        FileRenameKind::Any => FileSearchKind::Any,
    };

    let search_response = everything::search_files(FileSearchRequest {
        query: request.query.trim().to_string(),
        root_path: request.root_path.clone(),
        extension: request.extension.clone(),
        kind: Some(search_kind),
        max_results: Some(request.max_results.unwrap_or(RENAME_SEARCH_MAX_RESULTS).clamp(1, RENAME_SEARCH_MAX_RESULTS)),
        match_path: Some(false),
    })
    .await;

    if !search_response.ok {
        return rejected_response("search_failed", search_response.message);
    }

    let new_name = normalize_new_name(&request.new_name);
    let mut rejected_reasons = Vec::new();
    let mut rename_candidates = Vec::new();

    for result in search_response.results {
        match build_rename_candidate(&result, &new_name) {
            Ok(candidate) => rename_candidates.push(candidate),
            Err(message) => {
                if rejected_reasons.len() < 3 {
                    rejected_reasons.push(message);
                }
            }
        }
    }

    if rename_candidates.is_empty() {
        let message = if rejected_reasons.is_empty() {
            "이름을 변경할 파일 또는 폴더를 찾지 못했습니다.".to_string()
        } else {
            format!(
                "검색 결과는 있었지만 이름을 변경할 수 있는 항목이 없었습니다. 첫 번째 거절 사유: {}",
                rejected_reasons.first().cloned().unwrap_or_default()
            )
        };
        return rejected_response("not_found", message);
    }

    store_rename_candidates(rename_candidates, new_name).await
}

pub async fn next_rename_candidates(request: FileRenameNextRequest) -> FileRenamePrepareResponse {
    if let Err(message) = permission::ensure_modify_enabled() {
        return rejected_response("permission_denied", message);
    }
    match next_page(&request.request_id, request.offset.unwrap_or(0)).await {
        Ok(page) => response_from_page(page),
        Err(message) => rejected_response("not_found", message),
    }
}

pub async fn preview_rename_target(request: FileRenamePreviewRequest) -> FileRenamePreviewResponse {
    if let Err(message) = permission::ensure_modify_enabled() {
        return preview_rejected(message);
    }
    cleanup_expired_rename_state().await;

    let candidate_id = request.candidate_id.trim();
    if candidate_id.is_empty() {
        return preview_rejected("이름 변경 후보 ID가 비어 있습니다.".to_string());
    }

    let Some(stored) = pending_rename_candidates_store()
        .lock()
        .await
        .get(candidate_id)
        .cloned()
    else {
        return preview_rejected("이름 변경 후보가 만료되었거나 찾을 수 없습니다. 다시 요청해 주세요.".to_string());
    };

    match build_rename_confirmation(&stored.candidate, &stored.new_name) {
        Ok(mut confirmation) => {
            confirmation.edit_id = make_rename_id("rename-confirm");
            let edit_id = confirmation.edit_id.clone();
            let now = SystemTime::now();

            pending_rename_confirmations_store().lock().await.insert(
                edit_id,
                StoredRenameConfirmation {
                    confirmation: confirmation.clone(),
                    created_at: now,
                },
            );

            FileRenamePreviewResponse {
                ok: true,
                status: "confirmation_ready".to_string(),
                message: "변경 내용을 화면에 띄웠습니다. 사용자가 적용을 눌러야 실제로 이름이 변경됩니다.".to_string(),
                confirmation: Some(confirmation),
            }
        }
        Err(message) => preview_rejected(message),
    }
}

pub async fn confirm_rename_target(request: FileRenameConfirmRequest) -> FileRenameConfirmResponse {
    if let Err(message) = permission::ensure_modify_enabled() {
        return FileRenameConfirmResponse { ok: false, message };
    }
    cleanup_expired_rename_state().await;

    let edit_id = request.edit_id.trim();
    if edit_id.is_empty() {
        return FileRenameConfirmResponse {
            ok: false,
            message: "이름 변경 확인 ID가 비어 있습니다.".to_string(),
        };
    }

    let Some(stored) = pending_rename_confirmations_store().lock().await.remove(edit_id) else {
        return FileRenameConfirmResponse {
            ok: false,
            message: "이름 변경 요청이 만료되었거나 찾을 수 없습니다. 다시 요청해 주세요.".to_string(),
        };
    };

    if let Err(message) = revalidate_confirmation(&stored.confirmation) {
        return FileRenameConfirmResponse { ok: false, message };
    }

    let before_path = PathBuf::from(&stored.confirmation.before_path);
    let after_path = PathBuf::from(&stored.confirmation.after_path);

    match fs::rename(&before_path, &after_path) {
        Ok(()) => FileRenameConfirmResponse {
            ok: true,
            message: if stored.confirmation.is_folder {
                "폴더 이름을 변경했습니다.".to_string()
            } else {
                "파일 이름을 변경했습니다.".to_string()
            },
        },
        Err(error) => FileRenameConfirmResponse {
            ok: false,
            message: format!("이름 변경 실패: {error}"),
        },
    }
}

fn validate_prepare_request(request: &FileRenamePrepareRequest) -> Option<String> {
    security::validate_query(&request.query, "이름을 변경할 파일/폴더 검색어가 비어 있습니다.")
        .or_else(|| security::validate_root_path(request.root_path.as_deref()))
        .or_else(|| security::validate_search_extension(request.extension.as_deref()))
        .or_else(|| security::validate_new_file_name(&request.new_name))
}

fn normalize_new_name(value: &str) -> String {
    value.trim().trim_matches('`').trim_matches('"').trim_matches('\'').trim().to_string()
}

fn build_rename_candidate(result: &FileSearchResult, new_name: &str) -> Result<FileRenameCandidate, String> {
    security::validate_path_string(&result.path)?;

    let path = Path::new(&result.path);
    let is_folder = path.is_dir() || result.is_folder;
    let effective_new_name = effective_new_name_for(path, new_name, is_folder);

    if is_folder {
        security::validate_edit_existing_target(path, true)?;
        permission::validate_path_allowed_by_user_blacklist(path)?;
        validate_target_after_rename(path, &effective_new_name, true)?;

        return Ok(FileRenameCandidate {
            id: String::new(),
            name: result.name.clone(),
            path: result.path.clone(),
            parent_path: parent_path_string(path),
            is_folder: true,
            extension: None,
            category: "폴더".to_string(),
        });
    }

    security::validate_edit_existing_target(path, false)?;
    permission::validate_path_allowed_by_user_blacklist(path)?;
    validate_target_after_rename(path, &effective_new_name, false)?;

    let extension = result
        .extension
        .as_deref()
        .map(security::normalize_extension)
        .or_else(|| path.extension().map(|value| security::normalize_extension(&value.to_string_lossy())))
        .unwrap_or_default();

    Ok(FileRenameCandidate {
        id: String::new(),
        name: result.name.clone(),
        path: result.path.clone(),
        parent_path: parent_path_string(path),
        is_folder: false,
        extension: if extension.is_empty() { None } else { Some(extension.clone()) },
        category: if extension.is_empty() { "파일".to_string() } else { security::extension_category(&extension).to_string() },
    })
}

fn build_rename_confirmation(candidate: &FileRenameCandidate, new_name: &str) -> Result<FileRenameConfirmation, String> {
    let before_path = PathBuf::from(&candidate.path);
    let effective_new_name = effective_new_name_for(&before_path, new_name, candidate.is_folder);

    security::validate_edit_existing_target(&before_path, candidate.is_folder)?;
    permission::validate_path_allowed_by_user_blacklist(&before_path)?;
    validate_target_after_rename(&before_path, &effective_new_name, candidate.is_folder)?;

    let after_path = after_path_for(&before_path, &effective_new_name)?;
    let target_kind = if candidate.is_folder { "폴더" } else { "파일" }.to_string();

    Ok(FileRenameConfirmation {
        edit_id: String::new(),
        operation: "이름 변경".to_string(),
        target_kind,
        before_name: candidate.name.clone(),
        after_name: effective_new_name,
        before_path: before_path.to_string_lossy().to_string(),
        after_path: after_path.to_string_lossy().to_string(),
        is_folder: candidate.is_folder,
        warning: if candidate.is_folder {
            "폴더 이름을 변경하면 하위 항목의 경로도 함께 변경됩니다.".to_string()
        } else {
            "적용 후 기존 파일명은 새 파일명으로 변경됩니다.".to_string()
        },
    })
}

fn revalidate_confirmation(confirmation: &FileRenameConfirmation) -> Result<(), String> {
    let before_path = PathBuf::from(&confirmation.before_path);
    let after_path = PathBuf::from(&confirmation.after_path);

    security::validate_edit_existing_target(&before_path, confirmation.is_folder)?;
    permission::validate_path_allowed_by_user_blacklist(&before_path)?;
    validate_destination_parent(&after_path)?;

    if after_path.exists() {
        return Err("변경 후 이름과 같은 항목이 이미 존재합니다.".to_string());
    }

    let actual_after_path = after_path_for(&before_path, &confirmation.after_name)?;
    if normalize_path_for_compare(&actual_after_path) != normalize_path_for_compare(&after_path) {
        return Err("변경 후 경로가 예상과 달라 이름 변경을 중단했습니다.".to_string());
    }

    validate_target_after_rename(&before_path, &confirmation.after_name, confirmation.is_folder)
}


fn effective_new_name_for(path: &Path, new_name: &str, is_folder: bool) -> String {
    let normalized = normalize_new_name(new_name);

    if is_folder || has_explicit_extension(&normalized) {
        return normalized;
    }

    let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
        return normalized;
    };

    if extension.trim().is_empty() {
        normalized
    } else {
        format!("{normalized}.{extension}")
    }
}

fn has_explicit_extension(name: &str) -> bool {
    Path::new(name).extension().is_some()
}

fn validate_target_after_rename(path: &Path, new_name: &str, is_folder: bool) -> Result<(), String> {
    security::validate_new_file_name(new_name).map_or(Ok(()), Err)?;

    let after_path = after_path_for(path, new_name)?;
    validate_destination_parent(&after_path)?;

    if after_path.exists() {
        return Err("변경 후 이름과 같은 항목이 이미 존재합니다.".to_string());
    }

    if !is_folder {
        let extension = after_path
            .extension()
            .map(|value| security::normalize_extension(&value.to_string_lossy()))
            .unwrap_or_default();
        permission::validate_extension_allowed(Some(&extension))?;
    }

    Ok(())
}


fn validate_destination_parent(after_path: &Path) -> Result<(), String> {
    let Some(parent) = after_path.parent() else {
        return Err("변경 후 경로의 상위 폴더를 확인할 수 없습니다.".to_string());
    };

    if !parent.exists() || !parent.is_dir() {
        return Err("변경 후 경로의 상위 폴더가 존재하지 않습니다.".to_string());
    }

    security::validate_not_restricted_path(parent)?;
    permission::validate_path_allowed_by_user_blacklist(parent)
}

fn after_path_for(path: &Path, new_name: &str) -> Result<PathBuf, String> {
    let Some(parent) = path.parent() else {
        return Err("상위 폴더를 확인할 수 없어 이름을 변경할 수 없습니다.".to_string());
    };

    Ok(parent.join(new_name))
}

fn parent_path_string(path: &Path) -> String {
    path.parent()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn normalize_path_for_compare(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\").to_ascii_lowercase()
}

async fn store_rename_candidates(mut candidates: Vec<FileRenameCandidate>, new_name: String) -> FileRenamePrepareResponse {
    cleanup_expired_rename_state().await;

    let request_id = make_rename_id("rename-request");

    for candidate in &mut candidates {
        candidate.id = make_rename_id("rename-candidate");
    }

    let now = SystemTime::now();

    {
        let mut candidate_store = pending_rename_candidates_store().lock().await;
        for candidate in &candidates {
            candidate_store.insert(
                candidate.id.clone(),
                StoredRenameCandidate {
                    candidate: candidate.clone(),
                    new_name: new_name.clone(),
                    created_at: now,
                },
            );
        }
    }

    {
        let mut request_store = pending_rename_requests_store().lock().await;
        request_store.insert(
            request_id.clone(),
            StoredRenameRequest {
                candidates: candidates.clone(),
                new_name,
                created_at: now,
            },
        );
    }

    response_from_page(build_candidate_page(request_id, &candidates, 0))
}

async fn next_page(request_id: &str, offset: usize) -> Result<FileRenameCandidatePage, String> {
    cleanup_expired_rename_state().await;

    let request_id = request_id.trim();
    if request_id.is_empty() {
        return Err("후보 목록 요청 ID가 비어 있습니다.".to_string());
    }

    let store = pending_rename_requests_store().lock().await;
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

fn response_from_page(page: FileRenameCandidatePage) -> FileRenamePrepareResponse {
    FileRenamePrepareResponse {
        ok: true,
        status: "candidates_ready".to_string(),
        message: page.message.clone(),
        candidates: page.candidates.clone(),
        candidate_page: Some(page),
    }
}

fn build_candidate_page(
    request_id: String,
    candidates: &[FileRenameCandidate],
    offset: usize,
) -> FileRenameCandidatePage {
    let start = offset.min(candidates.len());
    let end = (start + RENAME_CANDIDATE_PAGE_SIZE).min(candidates.len());
    let page_candidates = candidates[start..end].to_vec();
    let has_more = end < candidates.len();
    let next_offset = has_more.then_some(end);

    FileRenameCandidatePage {
        request_id,
        candidates: page_candidates,
        has_more,
        next_offset,
        page_size: RENAME_CANDIDATE_PAGE_SIZE,
        message: "화면에 이름을 변경할 후보를 띄웠습니다. 사용자가 항목을 선택해야 다음 단계로 진행됩니다.".to_string(),
    }
}

fn rejected_response(status: &str, message: String) -> FileRenamePrepareResponse {
    FileRenamePrepareResponse {
        ok: false,
        status: status.to_string(),
        message,
        candidates: vec![],
        candidate_page: None,
    }
}

fn preview_rejected(message: String) -> FileRenamePreviewResponse {
    FileRenamePreviewResponse {
        ok: false,
        status: "rejected".to_string(),
        message,
        confirmation: None,
    }
}

async fn cleanup_expired_rename_state() {
    let now = SystemTime::now();

    {
        let mut store = pending_rename_candidates_store().lock().await;
        store.retain(|_, stored| {
            now.duration_since(stored.created_at)
                .map(|elapsed| elapsed <= Duration::from_secs(RENAME_CONFIRM_TTL_SECONDS))
                .unwrap_or(false)
        });
    }

    {
        let mut store = pending_rename_requests_store().lock().await;
        store.retain(|_, stored| {
            now.duration_since(stored.created_at)
                .map(|elapsed| elapsed <= Duration::from_secs(RENAME_CONFIRM_TTL_SECONDS))
                .unwrap_or(false)
        });
    }

    {
        let mut store = pending_rename_confirmations_store().lock().await;
        store.retain(|_, stored| {
            now.duration_since(stored.created_at)
                .map(|elapsed| elapsed <= Duration::from_secs(RENAME_CONFIRM_TTL_SECONDS))
                .unwrap_or(false)
        });
    }
}

fn pending_rename_candidates_store() -> &'static Mutex<HashMap<String, StoredRenameCandidate>> {
    PENDING_RENAME_CANDIDATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn pending_rename_requests_store() -> &'static Mutex<HashMap<String, StoredRenameRequest>> {
    PENDING_RENAME_REQUESTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn pending_rename_confirmations_store() -> &'static Mutex<HashMap<String, StoredRenameConfirmation>> {
    PENDING_RENAME_CONFIRMATIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn make_rename_id(prefix: &str) -> String {
    let counter = RENAME_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();

    format!("{prefix}-{timestamp}-{counter}")
}
