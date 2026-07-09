use std::{
    collections::{HashMap, HashSet},
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
        FileCreateCandidate, FileCreateCandidatePage, FileCreateConfirmRequest,
        FileCreateConfirmResponse, FileCreateConfirmation, FileCreateKind, FileCreateNextRequest,
        FileCreatePrepareRequest, FileCreatePrepareResponse, FileCreatePreviewRequest,
        FileCreatePreviewResponse, FileSearchKind, FileSearchRequest, FileSearchResult,
    },
};

const CREATE_CONFIRM_TTL_SECONDS: u64 = 120;
pub const CREATE_CANDIDATE_PAGE_SIZE: usize = 7;
const CREATE_SEARCH_MAX_RESULTS: u8 = 50;

static PENDING_CREATE_CANDIDATES: OnceLock<Mutex<HashMap<String, StoredCreateCandidate>>> = OnceLock::new();
static PENDING_CREATE_REQUESTS: OnceLock<Mutex<HashMap<String, StoredCreateRequest>>> = OnceLock::new();
static PENDING_CREATE_CONFIRMATIONS: OnceLock<Mutex<HashMap<String, StoredCreateConfirmation>>> = OnceLock::new();
static CREATE_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
struct StoredCreateCandidate {
    candidate: FileCreateCandidate,
    name: String,
    kind: FileCreateKind,
    content: String,
    created_at: SystemTime,
}

#[derive(Debug, Clone)]
struct StoredCreateRequest {
    candidates: Vec<FileCreateCandidate>,
    name: String,
    kind: FileCreateKind,
    content: String,
    created_at: SystemTime,
}

#[derive(Debug, Clone)]
struct StoredCreateConfirmation {
    confirmation: FileCreateConfirmation,
    created_at: SystemTime,
}

pub async fn prepare_create_target(request: FileCreatePrepareRequest) -> FileCreatePrepareResponse {
    if let Err(message) = permission::ensure_modify_enabled() {
        return rejected_response("permission_denied", message);
    }
    if let Some(message) = validate_prepare_request(&request) {
        return rejected_response("rejected", message);
    }

    let requested_kind = request.kind.clone().unwrap_or_default();
    let name = normalize_create_name(&request.name);
    let content = request.content.clone().unwrap_or_default();

    let mut rejected_reasons = Vec::new();
    let mut folder_candidates = Vec::new();
    let mut seen_paths = HashSet::new();

    for path in special_folder_candidates(&request.query) {
        match build_create_candidate_from_path(&path) {
            Ok(candidate) => {
                if seen_paths.insert(normalize_path_for_compare(Path::new(&candidate.path))) {
                    folder_candidates.push(candidate);
                }
            }
            Err(message) => {
                if rejected_reasons.len() < 3 {
                    rejected_reasons.push(message);
                }
            }
        }
    }

    let search_response = everything::search_files(FileSearchRequest {
        query: normalize_folder_query(&request.query),
        root_path: request.root_path.clone(),
        extension: None,
        kind: Some(FileSearchKind::Folder),
        max_results: Some(request.max_results.unwrap_or(CREATE_SEARCH_MAX_RESULTS).clamp(1, CREATE_SEARCH_MAX_RESULTS)),
        match_path: Some(false),
    })
    .await;

    if search_response.ok {
        for result in search_response.results {
            match build_create_candidate(&result) {
                Ok(candidate) => {
                    if seen_paths.insert(normalize_path_for_compare(Path::new(&candidate.path))) {
                        folder_candidates.push(candidate);
                    }
                }
                Err(message) => {
                    if rejected_reasons.len() < 3 {
                        rejected_reasons.push(message);
                    }
                }
            }
        }
    } else if folder_candidates.is_empty() {
        return rejected_response("search_failed", search_response.message);
    }

    let mut create_candidates = Vec::new();
    for candidate in folder_candidates {
        match validate_create_destination(Path::new(&candidate.path), &name, &requested_kind, &content) {
            Ok(()) => create_candidates.push(candidate),
            Err(message) => {
                if rejected_reasons.len() < 3 {
                    rejected_reasons.push(message);
                }
            }
        }
    }

    if create_candidates.is_empty() {
        let message = if rejected_reasons.is_empty() {
            "생성 위치로 사용할 폴더를 찾지 못했습니다.".to_string()
        } else {
            format!(
                "검색 결과는 있었지만 생성할 수 있는 위치가 없었습니다. 첫 번째 거절 사유: {}",
                rejected_reasons.first().cloned().unwrap_or_default()
            )
        };
        return rejected_response("not_found", message);
    }

    store_create_candidates(create_candidates, name, requested_kind, content).await
}

pub async fn next_create_candidates(request: FileCreateNextRequest) -> FileCreatePrepareResponse {
    if let Err(message) = permission::ensure_modify_enabled() {
        return rejected_response("permission_denied", message);
    }
    match next_page(&request.request_id, request.offset.unwrap_or(0)).await {
        Ok(page) => response_from_page(page),
        Err(message) => rejected_response("not_found", message),
    }
}

pub async fn preview_create_target(request: FileCreatePreviewRequest) -> FileCreatePreviewResponse {
    if let Err(message) = permission::ensure_modify_enabled() {
        return preview_rejected(message);
    }
    cleanup_expired_create_state().await;

    let candidate_id = request.candidate_id.trim();
    if candidate_id.is_empty() {
        return preview_rejected("생성 위치 후보 ID가 비어 있습니다.".to_string());
    }

    let Some(stored) = pending_create_candidates_store()
        .lock()
        .await
        .get(candidate_id)
        .cloned()
    else {
        return preview_rejected("생성 위치 후보가 만료되었거나 찾을 수 없습니다. 다시 요청해 주세요.".to_string());
    };

    match build_create_confirmation(&stored.candidate, &stored.name, &stored.kind, &stored.content) {
        Ok(mut confirmation) => {
            confirmation.edit_id = make_create_id("create-confirm");
            let edit_id = confirmation.edit_id.clone();
            let now = SystemTime::now();

            pending_create_confirmations_store().lock().await.insert(
                edit_id,
                StoredCreateConfirmation {
                    confirmation: confirmation.clone(),
                    created_at: now,
                },
            );

            FileCreatePreviewResponse {
                ok: true,
                status: "confirmation_ready".to_string(),
                message: "생성 내용을 화면에 띄웠습니다. 사용자가 적용을 눌러야 실제로 생성됩니다.".to_string(),
                confirmation: Some(confirmation),
            }
        }
        Err(message) => preview_rejected(message),
    }
}

pub async fn confirm_create_target(request: FileCreateConfirmRequest) -> FileCreateConfirmResponse {
    if let Err(message) = permission::ensure_modify_enabled() {
        return FileCreateConfirmResponse { ok: false, message };
    }
    cleanup_expired_create_state().await;

    let edit_id = request.edit_id.trim();
    if edit_id.is_empty() {
        return FileCreateConfirmResponse {
            ok: false,
            message: "생성 확인 ID가 비어 있습니다.".to_string(),
        };
    }

    let Some(stored) = pending_create_confirmations_store().lock().await.remove(edit_id) else {
        return FileCreateConfirmResponse {
            ok: false,
            message: "생성 요청이 만료되었거나 찾을 수 없습니다. 다시 요청해 주세요.".to_string(),
        };
    };

    if let Err(message) = revalidate_confirmation(&stored.confirmation) {
        return FileCreateConfirmResponse { ok: false, message };
    }

    let target_path = PathBuf::from(&stored.confirmation.target_path);
    let result = if stored.confirmation.is_folder {
        fs::create_dir(&target_path).map_err(|error| format!("폴더 생성 실패: {error}"))
    } else {
        fs::write(&target_path, stored.confirmation.content.as_bytes())
            .map_err(|error| format!("파일 생성 실패: {error}"))
    };

    match result {
        Ok(()) => FileCreateConfirmResponse {
            ok: true,
            message: if stored.confirmation.is_folder {
                "폴더를 생성했습니다.".to_string()
            } else {
                "파일을 생성했습니다.".to_string()
            },
        },
        Err(message) => FileCreateConfirmResponse { ok: false, message },
    }
}

fn validate_prepare_request(request: &FileCreatePrepareRequest) -> Option<String> {
    security::validate_query(&request.query, "생성 위치로 사용할 폴더 검색어가 비어 있습니다.")
        .or_else(|| security::validate_root_path(request.root_path.as_deref()))
        .or_else(|| security::validate_create_file_name(&request.name))
        .or_else(|| {
            let kind = request.kind.clone().unwrap_or_default();
            validate_create_kind_and_content(&kind, request.content.as_deref())
        })
}

fn validate_create_kind_and_content(kind: &FileCreateKind, content: Option<&str>) -> Option<String> {
    if matches!(kind, FileCreateKind::File) {
        security::validate_create_content_size(content.unwrap_or_default())
    } else {
        None
    }
}

fn normalize_create_name(value: &str) -> String {
    value.trim().trim_matches('`').trim_matches('"').trim_matches('\'').trim().to_string()
}

fn normalize_folder_query(query: &str) -> String {
    let trimmed = query.trim();
    match trimmed.to_ascii_lowercase().as_str() {
        "바탕화면" | "데스크톱" | "데스크탑" => "desktop".to_string(),
        "다운로드" => "downloads".to_string(),
        "문서" => "documents".to_string(),
        "사진" | "그림" | "이미지" => "pictures".to_string(),
        "음악" => "music".to_string(),
        "동영상" | "비디오" => "videos".to_string(),
        _ => trimmed.to_string(),
    }
}

fn special_folder_candidates(query: &str) -> Vec<PathBuf> {
    let lower = query.trim().to_ascii_lowercase();
    let Some(profile) = std::env::var_os("USERPROFILE").map(PathBuf::from) else {
        return Vec::new();
    };

    let mut paths = Vec::new();
    if contains_any(&lower, &["바탕화면", "데스크톱", "데스크탑", "desktop"]) {
        paths.push(profile.join("Desktop"));
        paths.push(profile.join("OneDrive").join("Desktop"));
    }
    if contains_any(&lower, &["다운로드", "download", "downloads"]) {
        paths.push(profile.join("Downloads"));
    }
    if contains_any(&lower, &["문서", "document", "documents"]) {
        paths.push(profile.join("Documents"));
        paths.push(profile.join("OneDrive").join("Documents"));
    }
    if contains_any(&lower, &["사진", "그림", "picture", "pictures"]) {
        paths.push(profile.join("Pictures"));
    }
    if contains_any(&lower, &["음악", "music"]) {
        paths.push(profile.join("Music"));
    }
    if contains_any(&lower, &["동영상", "비디오", "video", "videos"]) {
        paths.push(profile.join("Videos"));
    }

    paths
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn build_create_candidate(result: &FileSearchResult) -> Result<FileCreateCandidate, String> {
    security::validate_path_string(&result.path)?;
    build_create_candidate_from_path(Path::new(&result.path))
}

fn build_create_candidate_from_path(path: &Path) -> Result<FileCreateCandidate, String> {
    security::validate_path_string(&path.to_string_lossy())?;

    if !path.exists() || !path.is_dir() {
        return Err("생성 위치가 존재하지 않거나 폴더가 아닙니다.".to_string());
    }

    security::validate_not_restricted_path(path)?;
    permission::validate_path_allowed_by_user_blacklist(path)?;

    Ok(FileCreateCandidate {
        id: String::new(),
        name: path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string()),
        path: path.to_string_lossy().to_string(),
        parent_path: path
            .parent()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_default(),
        category: "생성 위치".to_string(),
    })
}

fn validate_create_destination(parent: &Path, name: &str, kind: &FileCreateKind, content: &str) -> Result<(), String> {
    security::validate_create_file_name(name).map_or(Ok(()), Err)?;
    security::validate_not_restricted_path(parent)?;
    permission::validate_path_allowed_by_user_blacklist(parent)?;

    if !parent.exists() || !parent.is_dir() {
        return Err("생성 위치가 존재하지 않거나 폴더가 아닙니다.".to_string());
    }

    let target_path = parent.join(name);
    if target_path.exists() {
        return Err("생성하려는 이름과 같은 항목이 이미 존재합니다.".to_string());
    }

    if matches!(kind, FileCreateKind::File) {
        let extension = target_path
            .extension()
            .map(|value| security::normalize_extension(&value.to_string_lossy()))
            .unwrap_or_default();

        if !security::is_allowed_create_file_extension(&extension) {
            return Err(format!(
                "현재 안전 정책상 .{} 파일은 생성할 수 없습니다. 허용 확장자는 {}입니다.",
                if extension.is_empty() { "확장자 없음".to_string() } else { extension.clone() },
                security::allowed_create_extensions_label()
            ));
        }
        permission::validate_extension_allowed(Some(&extension))?;

        if let Some(message) = security::validate_create_content_size(content) {
            return Err(message);
        }
    }

    Ok(())
}

fn build_create_confirmation(
    candidate: &FileCreateCandidate,
    name: &str,
    kind: &FileCreateKind,
    content: &str,
) -> Result<FileCreateConfirmation, String> {
    let parent_path = PathBuf::from(&candidate.path);
    validate_create_destination(&parent_path, name, kind, content)?;

    let target_path = parent_path.join(name);
    let is_folder = matches!(kind, FileCreateKind::Folder);

    Ok(FileCreateConfirmation {
        edit_id: String::new(),
        operation: if is_folder { "폴더 생성" } else { "파일 생성" }.to_string(),
        target_kind: if is_folder { "폴더" } else { "파일" }.to_string(),
        parent_path: parent_path.to_string_lossy().to_string(),
        target_name: name.to_string(),
        target_path: target_path.to_string_lossy().to_string(),
        is_folder,
        before: "없음".to_string(),
        after: if is_folder {
            "새 폴더가 생성됩니다.".to_string()
        } else if content.is_empty() {
            "빈 텍스트 파일이 생성됩니다.".to_string()
        } else {
            content_preview(content)
        },
        content: content.to_string(),
        warning: if is_folder {
            "적용 후 선택한 위치에 새 폴더가 생성됩니다.".to_string()
        } else {
            "적용 후 선택한 위치에 새 파일이 생성됩니다.".to_string()
        },
    })
}

fn revalidate_confirmation(confirmation: &FileCreateConfirmation) -> Result<(), String> {
    let parent_path = PathBuf::from(&confirmation.parent_path);
    let kind = if confirmation.is_folder { FileCreateKind::Folder } else { FileCreateKind::File };
    validate_create_destination(&parent_path, &confirmation.target_name, &kind, &confirmation.content)?;

    let target_path = parent_path.join(&confirmation.target_name);
    if normalize_path_for_compare(&target_path) != normalize_path_for_compare(Path::new(&confirmation.target_path)) {
        return Err("생성 경로가 예상과 달라 작업을 중단했습니다.".to_string());
    }

    Ok(())
}

fn content_preview(content: &str) -> String {
    const PREVIEW_CHARS: usize = 4000;
    let mut preview: String = content.chars().take(PREVIEW_CHARS).collect();
    if content.chars().count() > PREVIEW_CHARS {
        preview.push_str("\n…\n내용이 길어 일부만 미리보기로 표시됩니다.");
    }
    preview
}

async fn store_create_candidates(
    mut candidates: Vec<FileCreateCandidate>,
    name: String,
    kind: FileCreateKind,
    content: String,
) -> FileCreatePrepareResponse {
    cleanup_expired_create_state().await;

    let request_id = make_create_id("create-request");

    for candidate in &mut candidates {
        candidate.id = make_create_id("create-candidate");
    }

    let now = SystemTime::now();

    {
        let mut candidate_store = pending_create_candidates_store().lock().await;
        for candidate in &candidates {
            candidate_store.insert(
                candidate.id.clone(),
                StoredCreateCandidate {
                    candidate: candidate.clone(),
                    name: name.clone(),
                    kind: kind.clone(),
                    content: content.clone(),
                    created_at: now,
                },
            );
        }
    }

    {
        let mut request_store = pending_create_requests_store().lock().await;
        request_store.insert(
            request_id.clone(),
            StoredCreateRequest {
                candidates: candidates.clone(),
                name,
                kind,
                content,
                created_at: now,
            },
        );
    }

    response_from_page(build_candidate_page(request_id, &candidates, 0))
}

async fn next_page(request_id: &str, offset: usize) -> Result<FileCreateCandidatePage, String> {
    cleanup_expired_create_state().await;

    let request_id = request_id.trim();
    if request_id.is_empty() {
        return Err("후보 목록 요청 ID가 비어 있습니다.".to_string());
    }

    let store = pending_create_requests_store().lock().await;
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

fn response_from_page(page: FileCreateCandidatePage) -> FileCreatePrepareResponse {
    FileCreatePrepareResponse {
        ok: true,
        status: "candidates_ready".to_string(),
        message: page.message.clone(),
        candidates: page.candidates.clone(),
        candidate_page: Some(page),
    }
}

fn build_candidate_page(
    request_id: String,
    candidates: &[FileCreateCandidate],
    offset: usize,
) -> FileCreateCandidatePage {
    let start = offset.min(candidates.len());
    let end = (start + CREATE_CANDIDATE_PAGE_SIZE).min(candidates.len());
    let page_candidates = candidates[start..end].to_vec();
    let has_more = end < candidates.len();
    let next_offset = has_more.then_some(end);

    FileCreateCandidatePage {
        request_id,
        candidates: page_candidates,
        has_more,
        next_offset,
        page_size: CREATE_CANDIDATE_PAGE_SIZE,
        message: "화면에 생성 위치 후보를 띄웠습니다. 사용자가 위치를 선택해야 다음 단계로 진행됩니다.".to_string(),
    }
}

fn rejected_response(status: &str, message: String) -> FileCreatePrepareResponse {
    FileCreatePrepareResponse {
        ok: false,
        status: status.to_string(),
        message,
        candidates: vec![],
        candidate_page: None,
    }
}

fn preview_rejected(message: String) -> FileCreatePreviewResponse {
    FileCreatePreviewResponse {
        ok: false,
        status: "rejected".to_string(),
        message,
        confirmation: None,
    }
}

async fn cleanup_expired_create_state() {
    let now = SystemTime::now();

    {
        let mut store = pending_create_candidates_store().lock().await;
        store.retain(|_, stored| {
            now.duration_since(stored.created_at)
                .map(|elapsed| elapsed <= Duration::from_secs(CREATE_CONFIRM_TTL_SECONDS))
                .unwrap_or(false)
        });
    }

    {
        let mut store = pending_create_requests_store().lock().await;
        store.retain(|_, stored| {
            now.duration_since(stored.created_at)
                .map(|elapsed| elapsed <= Duration::from_secs(CREATE_CONFIRM_TTL_SECONDS))
                .unwrap_or(false)
        });
    }

    {
        let mut store = pending_create_confirmations_store().lock().await;
        store.retain(|_, stored| {
            now.duration_since(stored.created_at)
                .map(|elapsed| elapsed <= Duration::from_secs(CREATE_CONFIRM_TTL_SECONDS))
                .unwrap_or(false)
        });
    }
}

fn pending_create_candidates_store() -> &'static Mutex<HashMap<String, StoredCreateCandidate>> {
    PENDING_CREATE_CANDIDATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn pending_create_requests_store() -> &'static Mutex<HashMap<String, StoredCreateRequest>> {
    PENDING_CREATE_REQUESTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn pending_create_confirmations_store() -> &'static Mutex<HashMap<String, StoredCreateConfirmation>> {
    PENDING_CREATE_CONFIRMATIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn make_create_id(prefix: &str) -> String {
    let counter = CREATE_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();

    format!("{prefix}-{timestamp}-{counter}")
}

fn normalize_path_for_compare(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\").to_ascii_lowercase()
}
