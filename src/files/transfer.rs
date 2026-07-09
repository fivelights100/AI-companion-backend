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
        FileSearchKind, FileSearchRequest, FileSearchResult, FileTransferConfirmRequest,
        FileTransferConfirmResponse, FileTransferConfirmation, FileTransferDestinationCandidate,
        FileTransferDestinationNextRequest, FileTransferDestinationNextResponse, FileTransferKind,
        FileTransferOperation, FileTransferPending, FileTransferPrepareRequest,
        FileTransferPrepareResponse, FileTransferPreviewRequest, FileTransferPreviewResponse,
        FileTransferSourceCandidate, FileTransferSourceNextRequest, FileTransferSourceNextResponse,
    },
};

const TRANSFER_CONFIRM_TTL_SECONDS: u64 = 120;
pub const TRANSFER_CANDIDATE_PAGE_SIZE: usize = 7;
const TRANSFER_SEARCH_MAX_RESULTS: u8 = 50;
const MAX_TRANSFER_FOLDER_ENTRIES: usize = 500;
const MAX_TRANSFER_FOLDER_BYTES: u64 = 200 * 1024 * 1024;

static PENDING_TRANSFER_REQUESTS: OnceLock<Mutex<HashMap<String, StoredTransferRequest>>> = OnceLock::new();
static PENDING_TRANSFER_CONFIRMATIONS: OnceLock<Mutex<HashMap<String, StoredTransferConfirmation>>> = OnceLock::new();
static TRANSFER_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
struct StoredTransferRequest {
    operation: FileTransferOperation,
    source_candidates: Vec<FileTransferSourceCandidate>,
    destination_candidates: Vec<FileTransferDestinationCandidate>,
    created_at: SystemTime,
}

#[derive(Debug, Clone)]
struct StoredTransferConfirmation {
    confirmation: FileTransferConfirmation,
    created_at: SystemTime,
}

pub async fn prepare_transfer_target(request: FileTransferPrepareRequest) -> FileTransferPrepareResponse {
    if let Err(message) = permission::ensure_modify_enabled() {
        return rejected_response("permission_denied", message);
    }
    if let Some(message) = validate_prepare_request(&request) {
        return rejected_response("rejected", message);
    }

    let operation = request.operation.clone();
    let source_candidates_result = search_source_candidates(&request).await;
    let destination_candidates_result = search_destination_candidates(&request).await;

    let source_candidates = match source_candidates_result {
        Ok(candidates) => candidates,
        Err(message) => return rejected_response("source_not_found", message),
    };

    let destination_candidates = match destination_candidates_result {
        Ok(candidates) => candidates,
        Err(message) => return rejected_response("destination_not_found", message),
    };

    if source_candidates.is_empty() {
        return rejected_response("source_not_found", "복사/이동할 파일 또는 폴더 후보를 찾지 못했습니다.".to_string());
    }

    if destination_candidates.is_empty() {
        return rejected_response("destination_not_found", "복사/이동할 목적지 폴더 후보를 찾지 못했습니다.".to_string());
    }

    store_transfer_request(operation, source_candidates, destination_candidates).await
}

pub async fn next_transfer_source_candidates(request: FileTransferSourceNextRequest) -> FileTransferSourceNextResponse {
    if let Err(message) = permission::ensure_modify_enabled() {
        return FileTransferSourceNextResponse { ok: false, status: "permission_denied".to_string(), message, source_page: None };
    }
    cleanup_expired_transfer_state().await;

    match get_source_page(&request.request_id, request.offset.unwrap_or(0)).await {
        Ok(source_page) => FileTransferSourceNextResponse {
            ok: true,
            status: "candidates_ready".to_string(),
            message: "다음 원본 후보를 가져왔습니다.".to_string(),
            source_page: Some(source_page),
        },
        Err(message) => FileTransferSourceNextResponse {
            ok: false,
            status: "not_found".to_string(),
            message,
            source_page: None,
        },
    }
}

pub async fn next_transfer_destination_candidates(
    request: FileTransferDestinationNextRequest,
) -> FileTransferDestinationNextResponse {
    if let Err(message) = permission::ensure_modify_enabled() {
        return FileTransferDestinationNextResponse { ok: false, status: "permission_denied".to_string(), message, destination_page: None };
    }
    cleanup_expired_transfer_state().await;

    match get_destination_page(&request.request_id, request.offset.unwrap_or(0)).await {
        Ok(destination_page) => FileTransferDestinationNextResponse {
            ok: true,
            status: "candidates_ready".to_string(),
            message: "다음 목적지 후보를 가져왔습니다.".to_string(),
            destination_page: Some(destination_page),
        },
        Err(message) => FileTransferDestinationNextResponse {
            ok: false,
            status: "not_found".to_string(),
            message,
            destination_page: None,
        },
    }
}

pub async fn preview_transfer_target(request: FileTransferPreviewRequest) -> FileTransferPreviewResponse {
    if let Err(message) = permission::ensure_modify_enabled() {
        return preview_rejected(message);
    }
    cleanup_expired_transfer_state().await;

    let request_id = request.request_id.trim();
    let source_id = request.source_id.trim();
    let destination_id = request.destination_id.trim();

    if request_id.is_empty() || source_id.is_empty() || destination_id.is_empty() {
        return preview_rejected("원본 또는 목적지 후보 정보가 비어 있습니다.".to_string());
    }

    let Some(stored) = pending_transfer_requests_store()
        .lock()
        .await
        .get(request_id)
        .cloned()
    else {
        return preview_rejected("복사/이동 요청이 만료되었거나 찾을 수 없습니다. 다시 요청해 주세요.".to_string());
    };

    let Some(source) = stored
        .source_candidates
        .iter()
        .find(|candidate| candidate.id == source_id)
        .cloned()
    else {
        return preview_rejected("선택한 원본 후보를 찾을 수 없습니다. 다시 요청해 주세요.".to_string());
    };

    let Some(destination) = stored
        .destination_candidates
        .iter()
        .find(|candidate| candidate.id == destination_id)
        .cloned()
    else {
        return preview_rejected("선택한 목적지 후보를 찾을 수 없습니다. 다시 요청해 주세요.".to_string());
    };

    match build_transfer_confirmation(&stored.operation, &source, &destination) {
        Ok(mut confirmation) => {
            confirmation.transfer_id = make_transfer_id("transfer-confirm");
            let transfer_id = confirmation.transfer_id.clone();
            let now = SystemTime::now();

            pending_transfer_confirmations_store().lock().await.insert(
                transfer_id,
                StoredTransferConfirmation {
                    confirmation: confirmation.clone(),
                    created_at: now,
                },
            );

            FileTransferPreviewResponse {
                ok: true,
                status: "confirmation_ready".to_string(),
                message: "복사/이동 내용을 화면에 띄웠습니다. 사용자가 적용을 눌러야 실제로 진행됩니다.".to_string(),
                confirmation: Some(confirmation),
            }
        }
        Err(message) => preview_rejected(message),
    }
}

pub async fn confirm_transfer_target(request: FileTransferConfirmRequest) -> FileTransferConfirmResponse {
    if let Err(message) = permission::ensure_modify_enabled() {
        return FileTransferConfirmResponse { ok: false, message };
    }
    cleanup_expired_transfer_state().await;

    let transfer_id = request.transfer_id.trim();
    if transfer_id.is_empty() {
        return FileTransferConfirmResponse {
            ok: false,
            message: "복사/이동 확인 ID가 비어 있습니다.".to_string(),
        };
    }

    let Some(stored) = pending_transfer_confirmations_store()
        .lock()
        .await
        .remove(transfer_id)
    else {
        return FileTransferConfirmResponse {
            ok: false,
            message: "복사/이동 요청이 만료되었거나 찾을 수 없습니다. 다시 요청해 주세요.".to_string(),
        };
    };

    if let Err(message) = revalidate_confirmation(&stored.confirmation) {
        return FileTransferConfirmResponse { ok: false, message };
    }

    let source_path = PathBuf::from(&stored.confirmation.source_path);
    let destination_path = PathBuf::from(&stored.confirmation.destination_path);

    let result = match stored.confirmation.operation.as_str() {
        "복사" => copy_target(&source_path, &destination_path, stored.confirmation.is_folder),
        "이동" => fs::rename(&source_path, &destination_path)
            .map_err(|error| format!("이동 실패: {error}")),
        _ => Err("알 수 없는 복사/이동 작업입니다.".to_string()),
    };

    match result {
        Ok(()) => FileTransferConfirmResponse {
            ok: true,
            message: if stored.confirmation.operation == "이동" {
                "선택한 항목을 이동했습니다.".to_string()
            } else {
                "선택한 항목을 복사했습니다.".to_string()
            },
        },
        Err(message) => FileTransferConfirmResponse { ok: false, message },
    }
}

fn validate_prepare_request(request: &FileTransferPrepareRequest) -> Option<String> {
    security::validate_query(&request.source_query, "복사/이동할 원본 검색어가 비어 있습니다.")
        .or_else(|| security::validate_query(&request.destination_query, "복사/이동할 목적지 검색어가 비어 있습니다."))
        .or_else(|| security::validate_root_path(request.root_path.as_deref()))
        .or_else(|| security::validate_search_extension(request.extension.as_deref()))
}

async fn search_source_candidates(request: &FileTransferPrepareRequest) -> Result<Vec<FileTransferSourceCandidate>, String> {
    let search_kind = match request.kind.clone().unwrap_or_default() {
        FileTransferKind::File => FileSearchKind::File,
        FileTransferKind::Folder => FileSearchKind::Folder,
        FileTransferKind::Any => FileSearchKind::Any,
    };

    let response = everything::search_files(FileSearchRequest {
        query: request.source_query.trim().to_string(),
        root_path: request.root_path.clone(),
        extension: request.extension.clone(),
        kind: Some(search_kind),
        max_results: Some(
            request
                .max_results
                .unwrap_or(TRANSFER_SEARCH_MAX_RESULTS)
                .clamp(1, TRANSFER_SEARCH_MAX_RESULTS),
        ),
        match_path: Some(false),
    })
    .await;

    if !response.ok {
        return Err(response.message);
    }

    let mut rejected_reasons = Vec::new();
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    for result in response.results {
        match build_source_candidate(&result) {
            Ok(candidate) => {
                if seen.insert(normalize_path_for_compare(Path::new(&candidate.path))) {
                    candidates.push(candidate);
                }
            }
            Err(message) => {
                if rejected_reasons.len() < 3 {
                    rejected_reasons.push(message);
                }
            }
        }
    }

    if candidates.is_empty() {
        if let Some(reason) = rejected_reasons.first() {
            Err(format!("검색 결과는 있었지만 복사/이동할 수 있는 원본이 없었습니다. 첫 번째 거절 사유: {reason}"))
        } else {
            Err("복사/이동할 원본을 찾지 못했습니다.".to_string())
        }
    } else {
        Ok(candidates)
    }
}

async fn search_destination_candidates(request: &FileTransferPrepareRequest) -> Result<Vec<FileTransferDestinationCandidate>, String> {
    let mut rejected_reasons = Vec::new();
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    for path in special_folder_candidates(&request.destination_query) {
        match build_destination_candidate_from_path(&path) {
            Ok(candidate) => {
                if seen.insert(normalize_path_for_compare(Path::new(&candidate.path))) {
                    candidates.push(candidate);
                }
            }
            Err(message) => {
                if rejected_reasons.len() < 3 {
                    rejected_reasons.push(message);
                }
            }
        }
    }

    let response = everything::search_files(FileSearchRequest {
        query: normalize_folder_query(&request.destination_query),
        root_path: request.root_path.clone(),
        extension: None,
        kind: Some(FileSearchKind::Folder),
        max_results: Some(
            request
                .max_results
                .unwrap_or(TRANSFER_SEARCH_MAX_RESULTS)
                .clamp(1, TRANSFER_SEARCH_MAX_RESULTS),
        ),
        match_path: Some(false),
    })
    .await;

    if response.ok {
        for result in response.results {
            match build_destination_candidate(&result) {
                Ok(candidate) => {
                    if seen.insert(normalize_path_for_compare(Path::new(&candidate.path))) {
                        candidates.push(candidate);
                    }
                }
                Err(message) => {
                    if rejected_reasons.len() < 3 {
                        rejected_reasons.push(message);
                    }
                }
            }
        }
    } else if candidates.is_empty() {
        return Err(response.message);
    }

    if candidates.is_empty() {
        if let Some(reason) = rejected_reasons.first() {
            Err(format!("검색 결과는 있었지만 목적지로 사용할 수 있는 폴더가 없었습니다. 첫 번째 거절 사유: {reason}"))
        } else {
            Err("목적지 폴더를 찾지 못했습니다.".to_string())
        }
    } else {
        Ok(candidates)
    }
}

fn build_source_candidate(result: &FileSearchResult) -> Result<FileTransferSourceCandidate, String> {
    security::validate_path_string(&result.path)?;

    let path = Path::new(&result.path);
    let is_folder = path.is_dir() || result.is_folder;
    validate_source_target(path, is_folder)?;

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

    Ok(FileTransferSourceCandidate {
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

fn build_destination_candidate(result: &FileSearchResult) -> Result<FileTransferDestinationCandidate, String> {
    security::validate_path_string(&result.path)?;
    build_destination_candidate_from_path(Path::new(&result.path))
}

fn build_destination_candidate_from_path(path: &Path) -> Result<FileTransferDestinationCandidate, String> {
    security::validate_path_string(&path.to_string_lossy())?;

    if !path.exists() || !path.is_dir() {
        return Err("목적지가 존재하지 않거나 폴더가 아닙니다.".to_string());
    }

    security::validate_not_restricted_path(path)?;
    permission::validate_path_allowed_by_user_blacklist(path)?;

    Ok(FileTransferDestinationCandidate {
        id: String::new(),
        name: path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string()),
        path: path.to_string_lossy().to_string(),
        parent_path: parent_path_string(path),
        category: "목적지 폴더".to_string(),
    })
}

fn validate_source_target(path: &Path, expected_folder: bool) -> Result<(), String> {
    security::validate_edit_existing_target(path, expected_folder)?;
    permission::validate_path_and_extension_for_settings(path, expected_folder)?;
    validate_not_protected_user_folder(path)?;

    if expected_folder {
        validate_folder_limits(path)?;
    }

    Ok(())
}

fn build_transfer_confirmation(
    operation: &FileTransferOperation,
    source: &FileTransferSourceCandidate,
    destination: &FileTransferDestinationCandidate,
) -> Result<FileTransferConfirmation, String> {
    let source_path = PathBuf::from(&source.path);
    let destination_folder = PathBuf::from(&destination.path);

    validate_source_target(&source_path, source.is_folder)?;
    validate_destination_folder(&destination_folder)?;

    let file_name = source_path
        .file_name()
        .ok_or_else(|| "원본 이름을 확인할 수 없습니다.".to_string())?;
    let destination_path = destination_folder.join(file_name);

    validate_destination_target(&destination_path)?;

    if source.is_folder && is_same_or_child_path(&destination_folder, &source_path) {
        return Err("폴더를 자기 자신이나 자신의 하위 폴더 안으로 복사/이동할 수 없습니다.".to_string());
    }

    if matches!(operation, FileTransferOperation::Move) && !is_same_drive(&source_path, &destination_path) {
        return Err("현재 이동 기능은 안전을 위해 같은 드라이브 안에서만 허용됩니다. 다른 드라이브로 옮기려면 복사를 사용해 주세요.".to_string());
    }

    let operation_label = match operation {
        FileTransferOperation::Copy => "복사",
        FileTransferOperation::Move => "이동",
    }
    .to_string();

    Ok(FileTransferConfirmation {
        transfer_id: String::new(),
        operation: operation_label.clone(),
        target_kind: if source.is_folder { "폴더" } else { "파일" }.to_string(),
        source_name: source.name.clone(),
        source_path: source_path.to_string_lossy().to_string(),
        destination_folder_path: destination_folder.to_string_lossy().to_string(),
        destination_path: destination_path.to_string_lossy().to_string(),
        is_folder: source.is_folder,
        warning: if source.is_folder {
            format!(
                "이 폴더와 하위 항목이 함께 {}됩니다. 하위 항목 {}개 또는 총 {}MB를 넘는 폴더는 현재 처리하지 않습니다.",
                operation_label,
                MAX_TRANSFER_FOLDER_ENTRIES,
                MAX_TRANSFER_FOLDER_BYTES / 1024 / 1024
            )
        } else if operation_label == "이동" {
            "원본 위치에서 사라지고 목적지 폴더로 이동됩니다. 현재는 같은 드라이브 이동만 허용됩니다.".to_string()
        } else {
            "원본은 그대로 두고 목적지 폴더에 같은 이름으로 복사됩니다.".to_string()
        },
    })
}

fn revalidate_confirmation(confirmation: &FileTransferConfirmation) -> Result<(), String> {
    let source_path = PathBuf::from(&confirmation.source_path);
    let destination_folder = PathBuf::from(&confirmation.destination_folder_path);
    let destination_path = PathBuf::from(&confirmation.destination_path);

    validate_source_target(&source_path, confirmation.is_folder)?;
    validate_destination_folder(&destination_folder)?;
    validate_destination_target(&destination_path)?;

    if confirmation.is_folder && is_same_or_child_path(&destination_folder, &source_path) {
        return Err("폴더를 자기 자신이나 자신의 하위 폴더 안으로 복사/이동할 수 없습니다.".to_string());
    }

    if confirmation.operation == "이동" && !is_same_drive(&source_path, &destination_path) {
        return Err("현재 이동 기능은 같은 드라이브 안에서만 허용됩니다.".to_string());
    }

    Ok(())
}

fn validate_destination_folder(path: &Path) -> Result<(), String> {
    security::validate_path_string(&path.to_string_lossy())?;
    if !path.exists() || !path.is_dir() {
        return Err("목적지 폴더가 더 이상 존재하지 않습니다.".to_string());
    }
    security::validate_not_restricted_path(path)?;
    permission::validate_path_allowed_by_user_blacklist(path)
}

fn validate_destination_target(path: &Path) -> Result<(), String> {
    security::validate_path_string(&path.to_string_lossy())?;
    let parent = path
        .parent()
        .ok_or_else(|| "목적지 상위 폴더를 확인할 수 없습니다.".to_string())?;
    security::validate_not_restricted_path(parent)?;
    permission::validate_path_allowed_by_user_blacklist(parent)?;

    if path.exists() {
        return Err("목적지에 같은 이름의 파일 또는 폴더가 이미 있습니다. 덮어쓰기는 현재 허용하지 않습니다.".to_string());
    }

    Ok(())
}

fn validate_folder_limits(path: &Path) -> Result<(), String> {
    let stats = collect_folder_stats(path)?;
    if stats.entries > MAX_TRANSFER_FOLDER_ENTRIES {
        return Err(format!("하위 항목이 {}개를 초과하는 폴더는 현재 복사/이동할 수 없습니다.", MAX_TRANSFER_FOLDER_ENTRIES));
    }
    if stats.bytes > MAX_TRANSFER_FOLDER_BYTES {
        return Err(format!("총 크기가 {}MB를 초과하는 폴더는 현재 복사/이동할 수 없습니다.", MAX_TRANSFER_FOLDER_BYTES / 1024 / 1024));
    }
    Ok(())
}

#[derive(Debug, Default)]
struct FolderStats {
    entries: usize,
    bytes: u64,
}

fn collect_folder_stats(path: &Path) -> Result<FolderStats, String> {
    let mut stats = FolderStats::default();
    collect_folder_stats_inner(path, &mut stats)?;
    Ok(stats)
}

fn collect_folder_stats_inner(path: &Path, stats: &mut FolderStats) -> Result<(), String> {
    for entry in fs::read_dir(path).map_err(|error| format!("폴더 항목을 확인할 수 없습니다: {error}"))? {
        let entry = entry.map_err(|error| format!("폴더 항목을 확인할 수 없습니다: {error}"))?;
        let entry_path = entry.path();
        stats.entries += 1;

        if stats.entries > MAX_TRANSFER_FOLDER_ENTRIES {
            return Ok(());
        }

        let metadata = entry
            .metadata()
            .map_err(|error| format!("폴더 항목 정보를 확인할 수 없습니다: {error}"))?;
        if metadata.is_dir() {
            collect_folder_stats_inner(&entry_path, stats)?;
        } else {
            stats.bytes = stats.bytes.saturating_add(metadata.len());
        }

        if stats.bytes > MAX_TRANSFER_FOLDER_BYTES {
            return Ok(());
        }
    }
    Ok(())
}

fn copy_target(source: &Path, destination: &Path, is_folder: bool) -> Result<(), String> {
    if is_folder {
        copy_dir_recursive(source, destination)
    } else {
        fs::copy(source, destination)
            .map(|_| ())
            .map_err(|error| format!("파일 복사 실패: {error}"))
    }
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir(destination).map_err(|error| format!("폴더 생성 실패: {error}"))?;

    for entry in fs::read_dir(source).map_err(|error| format!("폴더 복사 실패: {error}"))? {
        let entry = entry.map_err(|error| format!("폴더 복사 실패: {error}"))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let metadata = entry
            .metadata()
            .map_err(|error| format!("폴더 항목 복사 실패: {error}"))?;

        if metadata.is_dir() {
            copy_dir_recursive(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path)
                .map_err(|error| format!("파일 복사 실패: {error}"))?;
        }
    }

    Ok(())
}

fn validate_not_protected_user_folder(path: &Path) -> Result<(), String> {
    let normalized = normalize_path_for_compare(path);
    for protected in protected_user_folders() {
        if normalize_path_for_compare(&protected) == normalized {
            return Err("사용자 주요 폴더 자체는 복사/이동할 수 없습니다. 그 안의 개별 파일 또는 하위 폴더만 선택해 주세요.".to_string());
        }
    }
    Ok(())
}

fn protected_user_folders() -> Vec<PathBuf> {
    let Some(profile) = std::env::var_os("USERPROFILE").map(PathBuf::from) else {
        return Vec::new();
    };

    vec![
        profile.clone(),
        profile.join("Desktop"),
        profile.join("OneDrive").join("Desktop"),
        profile.join("Downloads"),
        profile.join("Documents"),
        profile.join("OneDrive").join("Documents"),
        profile.join("Pictures"),
        profile.join("Music"),
        profile.join("Videos"),
    ]
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

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn is_same_or_child_path(path: &Path, base: &Path) -> bool {
    let normalized_path = normalize_path_for_compare(path);
    let normalized_base = normalize_path_for_compare(base);
    normalized_path == normalized_base
        || normalized_path
            .strip_prefix(&normalized_base)
            .map(|rest| rest.starts_with('\\'))
            .unwrap_or(false)
}

fn is_same_drive(a: &Path, b: &Path) -> bool {
    match (drive_prefix(a), drive_prefix(b)) {
        (Some(left), Some(right)) => left.eq_ignore_ascii_case(&right),
        // 드라이브 정보를 확인할 수 없는 환경에서는 같은 파일시스템으로 간주한다.
        // 실제 Windows 앱에서는 C:, D: 같은 prefix가 잡힌다.
        _ => true,
    }
}

fn drive_prefix(path: &Path) -> Option<String> {
    let value = path.to_string_lossy();
    let bytes = value.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        Some(value[..2].to_ascii_lowercase())
    } else {
        None
    }
}

async fn store_transfer_request(
    operation: FileTransferOperation,
    mut source_candidates: Vec<FileTransferSourceCandidate>,
    mut destination_candidates: Vec<FileTransferDestinationCandidate>,
) -> FileTransferPrepareResponse {
    cleanup_expired_transfer_state().await;

    let request_id = make_transfer_id("transfer-request");

    for (index, candidate) in source_candidates.iter_mut().enumerate() {
        candidate.id = format!("{request_id}-source-{index}");
    }
    for (index, candidate) in destination_candidates.iter_mut().enumerate() {
        candidate.id = format!("{request_id}-destination-{index}");
    }

    let stored = StoredTransferRequest {
        operation: operation.clone(),
        source_candidates: source_candidates.clone(),
        destination_candidates: destination_candidates.clone(),
        created_at: SystemTime::now(),
    };

    pending_transfer_requests_store()
        .lock()
        .await
        .insert(request_id.clone(), stored);

    let source_page = build_source_page(&request_id, &operation, &source_candidates, 0);
    let destination_page = build_destination_page(&request_id, &operation, &destination_candidates, 0);
    let pending = FileTransferPending {
        request_id,
        operation,
        source_page,
        destination_page,
        message: "복사/이동 후보를 화면에 띄웠습니다.".to_string(),
    };

    FileTransferPrepareResponse {
        ok: true,
        status: "candidates_ready".to_string(),
        message: "복사/이동 후보를 화면에 띄웠습니다. 사용자가 원본과 목적지를 선택해야 실제로 진행됩니다.".to_string(),
        pending: Some(pending),
    }
}

async fn get_source_page(request_id: &str, offset: usize) -> Result<crate::models::files::FileTransferSourceCandidatePage, String> {
    let store = pending_transfer_requests_store().lock().await;
    let Some(stored) = store.get(request_id) else {
        return Err("복사/이동 요청이 만료되었거나 찾을 수 없습니다. 다시 요청해 주세요.".to_string());
    };

    Ok(build_source_page(request_id, &stored.operation, &stored.source_candidates, offset))
}

async fn get_destination_page(request_id: &str, offset: usize) -> Result<crate::models::files::FileTransferDestinationCandidatePage, String> {
    let store = pending_transfer_requests_store().lock().await;
    let Some(stored) = store.get(request_id) else {
        return Err("복사/이동 요청이 만료되었거나 찾을 수 없습니다. 다시 요청해 주세요.".to_string());
    };

    Ok(build_destination_page(request_id, &stored.operation, &stored.destination_candidates, offset))
}

fn build_source_page(
    request_id: &str,
    operation: &FileTransferOperation,
    candidates: &[FileTransferSourceCandidate],
    offset: usize,
) -> crate::models::files::FileTransferSourceCandidatePage {
    let start = offset.min(candidates.len());
    let end = (start + TRANSFER_CANDIDATE_PAGE_SIZE).min(candidates.len());
    let page_candidates = candidates[start..end].to_vec();
    let has_more = end < candidates.len();

    crate::models::files::FileTransferSourceCandidatePage {
        request_id: request_id.to_string(),
        operation: operation.clone(),
        candidates: page_candidates,
        has_more,
        next_offset: has_more.then_some(end),
        page_size: TRANSFER_CANDIDATE_PAGE_SIZE,
        message: "복사/이동할 원본 후보를 선택해 주세요.".to_string(),
    }
}

fn build_destination_page(
    request_id: &str,
    operation: &FileTransferOperation,
    candidates: &[FileTransferDestinationCandidate],
    offset: usize,
) -> crate::models::files::FileTransferDestinationCandidatePage {
    let start = offset.min(candidates.len());
    let end = (start + TRANSFER_CANDIDATE_PAGE_SIZE).min(candidates.len());
    let page_candidates = candidates[start..end].to_vec();
    let has_more = end < candidates.len();

    crate::models::files::FileTransferDestinationCandidatePage {
        request_id: request_id.to_string(),
        operation: operation.clone(),
        candidates: page_candidates,
        has_more,
        next_offset: has_more.then_some(end),
        page_size: TRANSFER_CANDIDATE_PAGE_SIZE,
        message: "복사/이동할 목적지 폴더를 선택해 주세요.".to_string(),
    }
}


fn parent_path_string(path: &Path) -> String {
    path.parent()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn normalize_path_for_compare(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

fn make_transfer_id(prefix: &str) -> String {
    let counter = TRANSFER_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("{prefix}-{millis}-{counter}")
}

fn pending_transfer_requests_store() -> &'static Mutex<HashMap<String, StoredTransferRequest>> {
    PENDING_TRANSFER_REQUESTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn pending_transfer_confirmations_store() -> &'static Mutex<HashMap<String, StoredTransferConfirmation>> {
    PENDING_TRANSFER_CONFIRMATIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

async fn cleanup_expired_transfer_state() {
    let ttl = Duration::from_secs(TRANSFER_CONFIRM_TTL_SECONDS);
    let now = SystemTime::now();

    pending_transfer_requests_store()
        .lock()
        .await
        .retain(|_, stored| now.duration_since(stored.created_at).unwrap_or_default() < ttl);

    pending_transfer_confirmations_store()
        .lock()
        .await
        .retain(|_, stored| now.duration_since(stored.created_at).unwrap_or_default() < ttl);
}

fn rejected_response(status: &str, message: String) -> FileTransferPrepareResponse {
    FileTransferPrepareResponse {
        ok: false,
        status: status.to_string(),
        message,
        pending: None,
    }
}

fn preview_rejected(message: String) -> FileTransferPreviewResponse {
    FileTransferPreviewResponse {
        ok: false,
        status: "rejected".to_string(),
        message,
        confirmation: None,
    }
}
