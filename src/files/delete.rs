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
        FileDeleteCandidate, FileDeleteCandidatePage, FileDeleteConfirmRequest,
        FileDeleteConfirmResponse, FileDeleteConfirmation, FileDeleteKind, FileDeleteNextRequest,
        FileDeletePrepareRequest, FileDeletePrepareResponse, FileDeletePreviewRequest,
        FileDeletePreviewResponse, FileSearchKind, FileSearchRequest, FileSearchResult,
    },
};

const DELETE_CONFIRM_TTL_SECONDS: u64 = 120;
pub const DELETE_CANDIDATE_PAGE_SIZE: usize = 7;
const DELETE_SEARCH_MAX_RESULTS: u8 = 50;
const MAX_DELETE_FOLDER_ENTRIES: usize = 200;

static PENDING_DELETE_CANDIDATES: OnceLock<Mutex<HashMap<String, StoredDeleteCandidate>>> = OnceLock::new();
static PENDING_DELETE_REQUESTS: OnceLock<Mutex<HashMap<String, StoredDeleteRequest>>> = OnceLock::new();
static PENDING_DELETE_CONFIRMATIONS: OnceLock<Mutex<HashMap<String, StoredDeleteConfirmation>>> = OnceLock::new();
static DELETE_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
struct StoredDeleteCandidate {
    candidate: FileDeleteCandidate,
    created_at: SystemTime,
}

#[derive(Debug, Clone)]
struct StoredDeleteRequest {
    candidates: Vec<FileDeleteCandidate>,
    created_at: SystemTime,
}

#[derive(Debug, Clone)]
struct StoredDeleteConfirmation {
    confirmation: FileDeleteConfirmation,
    created_at: SystemTime,
}

pub async fn prepare_delete_target(request: FileDeletePrepareRequest) -> FileDeletePrepareResponse {
    if let Err(message) = permission::ensure_delete_enabled() {
        return rejected_response("permission_denied", message);
    }
    if let Some(message) = validate_prepare_request(&request) {
        return rejected_response("rejected", message);
    }

    let requested_kind = request.kind.clone().unwrap_or_default();
    let search_kind = match requested_kind {
        FileDeleteKind::File => FileSearchKind::File,
        FileDeleteKind::Folder => FileSearchKind::Folder,
        FileDeleteKind::Any => FileSearchKind::Any,
    };

    let search_response = everything::search_files(FileSearchRequest {
        query: request.query.trim().to_string(),
        root_path: request.root_path.clone(),
        extension: request.extension.clone(),
        kind: Some(search_kind),
        max_results: Some(
            request
                .max_results
                .unwrap_or(DELETE_SEARCH_MAX_RESULTS)
                .clamp(1, DELETE_SEARCH_MAX_RESULTS),
        ),
        match_path: Some(false),
    })
    .await;

    if !search_response.ok {
        return rejected_response("search_failed", search_response.message);
    }

    let mut rejected_reasons = Vec::new();
    let mut delete_candidates = Vec::new();

    for result in search_response.results {
        match build_delete_candidate(&result) {
            Ok(candidate) => delete_candidates.push(candidate),
            Err(message) => {
                if rejected_reasons.len() < 3 {
                    rejected_reasons.push(message);
                }
            }
        }
    }

    if delete_candidates.is_empty() {
        let message = if rejected_reasons.is_empty() {
            "삭제할 파일 또는 폴더를 찾지 못했습니다.".to_string()
        } else {
            format!(
                "검색 결과는 있었지만 삭제할 수 있는 항목이 없었습니다. 첫 번째 거절 사유: {}",
                rejected_reasons.first().cloned().unwrap_or_default()
            )
        };
        return rejected_response("not_found", message);
    }

    store_delete_candidates(delete_candidates).await
}

pub async fn next_delete_candidates(request: FileDeleteNextRequest) -> FileDeletePrepareResponse {
    if let Err(message) = permission::ensure_delete_enabled() {
        return rejected_response("permission_denied", message);
    }
    match next_page(&request.request_id, request.offset.unwrap_or(0)).await {
        Ok(page) => response_from_page(page),
        Err(message) => rejected_response("not_found", message),
    }
}

pub async fn preview_delete_target(request: FileDeletePreviewRequest) -> FileDeletePreviewResponse {
    if let Err(message) = permission::ensure_delete_enabled() {
        return preview_rejected(message);
    }
    cleanup_expired_delete_state().await;

    let candidate_id = request.candidate_id.trim();
    if candidate_id.is_empty() {
        return preview_rejected("삭제 후보 ID가 비어 있습니다.".to_string());
    }

    let Some(stored) = pending_delete_candidates_store()
        .lock()
        .await
        .get(candidate_id)
        .cloned()
    else {
        return preview_rejected("삭제 후보가 만료되었거나 찾을 수 없습니다. 다시 요청해 주세요.".to_string());
    };

    match build_delete_confirmation(&stored.candidate) {
        Ok(mut confirmation) => {
            confirmation.delete_id = make_delete_id("delete-confirm");
            let delete_id = confirmation.delete_id.clone();
            let now = SystemTime::now();

            pending_delete_confirmations_store().lock().await.insert(
                delete_id,
                StoredDeleteConfirmation {
                    confirmation: confirmation.clone(),
                    created_at: now,
                },
            );

            FileDeletePreviewResponse {
                ok: true,
                status: "confirmation_ready".to_string(),
                message: "삭제 내용을 화면에 띄웠습니다. 사용자가 휴지통으로 이동을 눌러야 실제로 삭제됩니다.".to_string(),
                confirmation: Some(confirmation),
            }
        }
        Err(message) => preview_rejected(message),
    }
}

pub async fn confirm_delete_target(request: FileDeleteConfirmRequest) -> FileDeleteConfirmResponse {
    if let Err(message) = permission::ensure_delete_enabled() {
        return FileDeleteConfirmResponse { ok: false, message };
    }
    cleanup_expired_delete_state().await;

    let delete_id = request.delete_id.trim();
    if delete_id.is_empty() {
        return FileDeleteConfirmResponse {
            ok: false,
            message: "삭제 확인 ID가 비어 있습니다.".to_string(),
        };
    }

    let Some(stored) = pending_delete_confirmations_store().lock().await.remove(delete_id) else {
        return FileDeleteConfirmResponse {
            ok: false,
            message: "삭제 요청이 만료되었거나 찾을 수 없습니다. 다시 요청해 주세요.".to_string(),
        };
    };

    if let Err(message) = revalidate_confirmation(&stored.confirmation) {
        return FileDeleteConfirmResponse { ok: false, message };
    }

    let target_path = PathBuf::from(&stored.confirmation.target_path);
    match trash::delete(&target_path) {
        Ok(()) => FileDeleteConfirmResponse {
            ok: true,
            message: "선택한 항목을 휴지통으로 이동했습니다.".to_string(),
        },
        Err(error) => FileDeleteConfirmResponse {
            ok: false,
            message: format!("휴지통 이동 실패: {error}"),
        },
    }
}

fn validate_prepare_request(request: &FileDeletePrepareRequest) -> Option<String> {
    security::validate_query(&request.query, "삭제할 파일/폴더 검색어가 비어 있습니다.")
        .or_else(|| security::validate_root_path(request.root_path.as_deref()))
        .or_else(|| security::validate_search_extension(request.extension.as_deref()))
}

fn build_delete_candidate(result: &FileSearchResult) -> Result<FileDeleteCandidate, String> {
    security::validate_path_string(&result.path)?;

    let path = Path::new(&result.path);
    let is_folder = path.is_dir() || result.is_folder;
    validate_delete_target(path, is_folder)?;

    let extension = if is_folder {
        None
    } else {
        result
            .extension
            .as_deref()
            .map(security::normalize_extension)
            .or_else(|| path.extension().map(|value| security::normalize_extension(&value.to_string_lossy())))
            .filter(|value| !value.is_empty())
    };

    Ok(FileDeleteCandidate {
        id: String::new(),
        name: result.name.clone(),
        path: result.path.clone(),
        parent_path: parent_path_string(path),
        is_folder,
        extension: extension.clone(),
        category: if is_folder {
            "폴더".to_string()
        } else if let Some(extension) = extension.as_deref() {
            security::extension_category(extension).to_string()
        } else {
            "파일".to_string()
        },
    })
}

fn build_delete_confirmation(candidate: &FileDeleteCandidate) -> Result<FileDeleteConfirmation, String> {
    let target_path = PathBuf::from(&candidate.path);
    validate_delete_target(&target_path, candidate.is_folder)?;

    Ok(FileDeleteConfirmation {
        delete_id: String::new(),
        operation: "삭제".to_string(),
        delete_method: "휴지통으로 이동".to_string(),
        target_kind: if candidate.is_folder { "폴더" } else { "파일" }.to_string(),
        target_name: candidate.name.clone(),
        target_path: target_path.to_string_lossy().to_string(),
        parent_path: parent_path_string(&target_path),
        is_folder: candidate.is_folder,
        warning: if candidate.is_folder {
            format!(
                "이 폴더와 하위 항목이 함께 휴지통으로 이동됩니다. 하위 항목 수가 {}개를 넘는 폴더는 현재 삭제하지 않습니다.",
                MAX_DELETE_FOLDER_ENTRIES
            )
        } else {
            "선택한 파일이 휴지통으로 이동됩니다. 영구 삭제는 수행하지 않습니다.".to_string()
        },
    })
}

fn revalidate_confirmation(confirmation: &FileDeleteConfirmation) -> Result<(), String> {
    let target_path = PathBuf::from(&confirmation.target_path);
    validate_delete_target(&target_path, confirmation.is_folder)
}

fn validate_delete_target(path: &Path, expected_folder: bool) -> Result<(), String> {
    security::validate_path_string(&path.to_string_lossy())?;

    if !path.exists() {
        return Err("대상이 더 이상 존재하지 않습니다.".to_string());
    }

    if expected_folder && !path.is_dir() {
        return Err("대상이 폴더가 아닙니다.".to_string());
    }

    if !expected_folder && !path.is_file() {
        return Err("대상이 파일이 아닙니다.".to_string());
    }

    security::validate_not_restricted_path(path)?;
    permission::validate_path_allowed_by_user_blacklist(path)?;
    validate_not_protected_user_folder(path)?;

    if expected_folder {
        validate_folder_size_for_delete(path)?;
    } else {
        let extension = path
            .extension()
            .map(|value| security::normalize_extension(&value.to_string_lossy()))
            .unwrap_or_default();
        permission::validate_extension_allowed(Some(&extension))?;
    }

    Ok(())
}

fn validate_not_protected_user_folder(path: &Path) -> Result<(), String> {
    if !path.is_dir() {
        return Ok(());
    }

    let Some(profile) = std::env::var_os("USERPROFILE").map(PathBuf::from) else {
        return Ok(());
    };

    let protected = [
        profile.clone(),
        profile.join("Desktop"),
        profile.join("Downloads"),
        profile.join("Documents"),
        profile.join("Pictures"),
        profile.join("Music"),
        profile.join("Videos"),
        profile.join("OneDrive").join("Desktop"),
        profile.join("OneDrive").join("Documents"),
    ];

    for protected_path in protected {
        if paths_refer_to_same_existing_location(path, &protected_path) {
            return Err("사용자 홈/바탕화면/다운로드/문서 같은 주요 폴더 자체는 삭제할 수 없습니다.".to_string());
        }
    }

    Ok(())
}

fn validate_folder_size_for_delete(path: &Path) -> Result<(), String> {
    let mut count = 0usize;
    count_folder_entries(path, &mut count)?;

    if count > MAX_DELETE_FOLDER_ENTRIES {
        Err(format!(
            "하위 항목이 {}개를 초과하는 폴더는 현재 삭제할 수 없습니다.",
            MAX_DELETE_FOLDER_ENTRIES
        ))
    } else {
        Ok(())
    }
}

fn count_folder_entries(path: &Path, count: &mut usize) -> Result<(), String> {
    if *count > MAX_DELETE_FOLDER_ENTRIES {
        return Ok(());
    }

    let entries = fs::read_dir(path).map_err(|error| format!("폴더 내용을 확인할 수 없습니다: {error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("폴더 항목을 확인할 수 없습니다: {error}"))?;
        *count += 1;
        if *count > MAX_DELETE_FOLDER_ENTRIES {
            return Ok(());
        }

        let entry_path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            count_folder_entries(&entry_path, count)?;
        }
    }

    Ok(())
}

fn paths_refer_to_same_existing_location(left: &Path, right: &Path) -> bool {
    let Ok(left) = left.canonicalize() else {
        return false;
    };
    let Ok(right) = right.canonicalize() else {
        return false;
    };

    normalize_path_for_compare(&left) == normalize_path_for_compare(&right)
}

fn parent_path_string(path: &Path) -> String {
    path.parent()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default()
}

async fn store_delete_candidates(mut candidates: Vec<FileDeleteCandidate>) -> FileDeletePrepareResponse {
    cleanup_expired_delete_state().await;

    let request_id = make_delete_id("delete-request");

    for candidate in &mut candidates {
        candidate.id = make_delete_id("delete-candidate");
    }

    let now = SystemTime::now();

    {
        let mut candidate_store = pending_delete_candidates_store().lock().await;
        for candidate in &candidates {
            candidate_store.insert(
                candidate.id.clone(),
                StoredDeleteCandidate {
                    candidate: candidate.clone(),
                    created_at: now,
                },
            );
        }
    }

    {
        let mut request_store = pending_delete_requests_store().lock().await;
        request_store.insert(
            request_id.clone(),
            StoredDeleteRequest {
                candidates: candidates.clone(),
                created_at: now,
            },
        );
    }

    response_from_page(build_candidate_page(request_id, &candidates, 0))
}

async fn next_page(request_id: &str, offset: usize) -> Result<FileDeleteCandidatePage, String> {
    cleanup_expired_delete_state().await;

    let request_id = request_id.trim();
    if request_id.is_empty() {
        return Err("후보 목록 요청 ID가 비어 있습니다.".to_string());
    }

    let store = pending_delete_requests_store().lock().await;
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

fn response_from_page(page: FileDeleteCandidatePage) -> FileDeletePrepareResponse {
    FileDeletePrepareResponse {
        ok: true,
        status: "candidates_ready".to_string(),
        message: page.message.clone(),
        candidates: page.candidates.clone(),
        candidate_page: Some(page),
    }
}

fn build_candidate_page(
    request_id: String,
    candidates: &[FileDeleteCandidate],
    offset: usize,
) -> FileDeleteCandidatePage {
    let start = offset.min(candidates.len());
    let end = (start + DELETE_CANDIDATE_PAGE_SIZE).min(candidates.len());
    let page_candidates = candidates[start..end].to_vec();
    let has_more = end < candidates.len();
    let next_offset = has_more.then_some(end);

    FileDeleteCandidatePage {
        request_id,
        candidates: page_candidates,
        has_more,
        next_offset,
        page_size: DELETE_CANDIDATE_PAGE_SIZE,
        message: "화면에 삭제 후보를 띄웠습니다. 사용자가 항목을 선택해야 다음 단계로 진행됩니다.".to_string(),
    }
}

fn rejected_response(status: &str, message: String) -> FileDeletePrepareResponse {
    FileDeletePrepareResponse {
        ok: false,
        status: status.to_string(),
        message,
        candidates: vec![],
        candidate_page: None,
    }
}

fn preview_rejected(message: String) -> FileDeletePreviewResponse {
    FileDeletePreviewResponse {
        ok: false,
        status: "rejected".to_string(),
        message,
        confirmation: None,
    }
}

async fn cleanup_expired_delete_state() {
    let now = SystemTime::now();

    {
        let mut store = pending_delete_candidates_store().lock().await;
        store.retain(|_, stored| {
            now.duration_since(stored.created_at)
                .map(|elapsed| elapsed <= Duration::from_secs(DELETE_CONFIRM_TTL_SECONDS))
                .unwrap_or(false)
        });
    }

    {
        let mut store = pending_delete_requests_store().lock().await;
        store.retain(|_, stored| {
            now.duration_since(stored.created_at)
                .map(|elapsed| elapsed <= Duration::from_secs(DELETE_CONFIRM_TTL_SECONDS))
                .unwrap_or(false)
        });
    }

    {
        let mut store = pending_delete_confirmations_store().lock().await;
        store.retain(|_, stored| {
            now.duration_since(stored.created_at)
                .map(|elapsed| elapsed <= Duration::from_secs(DELETE_CONFIRM_TTL_SECONDS))
                .unwrap_or(false)
        });
    }
}

fn pending_delete_candidates_store() -> &'static Mutex<HashMap<String, StoredDeleteCandidate>> {
    PENDING_DELETE_CANDIDATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn pending_delete_requests_store() -> &'static Mutex<HashMap<String, StoredDeleteRequest>> {
    PENDING_DELETE_REQUESTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn pending_delete_confirmations_store() -> &'static Mutex<HashMap<String, StoredDeleteConfirmation>> {
    PENDING_DELETE_CONFIRMATIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn make_delete_id(prefix: &str) -> String {
    let counter = DELETE_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();

    format!("{prefix}-{timestamp}-{counter}")
}

fn normalize_path_for_compare(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\").to_ascii_lowercase()
}
