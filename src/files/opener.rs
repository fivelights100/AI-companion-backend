use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::{
    files::{candidates, everything, permission, security},
    models::files::{
        FileOpenCandidate, FileOpenConfirmRequest, FileOpenConfirmResponse, FileOpenKind,
        FileOpenNextRequest, FileOpenPrepareRequest, FileOpenPrepareResponse, FileSearchKind,
        FileSearchRequest, FileSearchResult,
    },
};

const OPEN_SEARCH_MAX_RESULTS: u8 = 50;

pub async fn prepare_open_target(request: FileOpenPrepareRequest) -> FileOpenPrepareResponse {
    if let Err(message) = permission::ensure_search_enabled() {
        return rejected_response("permission_denied", message);
    }
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
        max_results: Some(request.max_results.unwrap_or(OPEN_SEARCH_MAX_RESULTS).clamp(1, OPEN_SEARCH_MAX_RESULTS)),
        match_path: Some(false),
    })
    .await;

    if !search_response.ok {
        return rejected_response("search_failed", search_response.message);
    }

    let mut rejected_count = 0usize;
    let mut openable_candidates = Vec::new();

    for result in search_response.results {
        match build_openable_candidate(&result) {
            Ok(candidate) => openable_candidates.push(candidate),
            Err(_) => rejected_count += 1,
        }
    }

    if openable_candidates.is_empty() {
        let message = if rejected_count > 0 {
            format!(
                "검색 결과는 있었지만, 현재 안전 정책에서 열 수 있는 파일/폴더가 없었습니다. 허용 확장자는 {}입니다.",
                security::allowed_open_extensions_label()
            )
        } else {
            "열 수 있는 파일 또는 폴더를 찾지 못했습니다.".to_string()
        };

        return rejected_response("not_found", message);
    }

    candidates::store_candidates(openable_candidates).await
}

pub async fn next_open_candidates(request: FileOpenNextRequest) -> FileOpenPrepareResponse {
    if let Err(message) = permission::ensure_search_enabled() {
        return rejected_response("permission_denied", message);
    }
    match candidates::next_page(&request.request_id, request.offset.unwrap_or(0)).await {
        Ok(page) => candidates::candidates_response_from_page(page),
        Err(message) => rejected_response("not_found", message),
    }
}

pub async fn confirm_open_target(request: FileOpenConfirmRequest) -> FileOpenConfirmResponse {
    if let Err(message) = permission::ensure_search_enabled() {
        return FileOpenConfirmResponse { ok: false, message };
    }
    let candidate_id = request.candidate_id.trim();

    if candidate_id.is_empty() {
        return FileOpenConfirmResponse {
            ok: false,
            message: "열기 후보 ID가 비어 있습니다.".to_string(),
        };
    }

    let Some(candidate) = candidates::take_candidate(candidate_id).await else {
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
    security::validate_query(&request.query, "열 파일/폴더 검색어가 비어 있습니다.")
        .or_else(|| security::validate_root_path(request.root_path.as_deref()))
        .or_else(|| security::validate_open_extension(request.extension.as_deref()))
}

fn build_openable_candidate(result: &FileSearchResult) -> Result<FileOpenCandidate, String> {
    security::validate_path_string(&result.path)?;

    let path = Path::new(&result.path);
    let parent_path = path
        .parent()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();

    let is_folder = path.is_dir() || result.is_folder;

    if is_folder {
        if !path.exists() || !path.is_dir() {
            return Err("폴더가 존재하지 않습니다.".to_string());
        }
        permission::validate_path_allowed_by_user_blacklist(path)?;

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
        .map(security::normalize_extension)
        .or_else(|| path.extension().map(|value| security::normalize_extension(&value.to_string_lossy())))
        .unwrap_or_default();

    if !security::is_allowed_open_extension(&extension) {
        return Err(format!("허용되지 않은 파일 확장자입니다: .{extension}"));
    }
    permission::validate_extension_allowed(Some(&extension))?;

    Ok(FileOpenCandidate {
        id: String::new(),
        name: result.name.clone(),
        path: result.path.clone(),
        parent_path,
        is_folder: false,
        extension: Some(extension.clone()),
        category: security::extension_category(&extension).to_string(),
        requires_confirmation: true,
    })
}

fn revalidate_candidate(candidate: &FileOpenCandidate) -> Result<(), String> {
    security::validate_path_string(&candidate.path)?;

    let path = Path::new(&candidate.path);

    if candidate.is_folder {
        if path.exists() && path.is_dir() {
            permission::validate_path_allowed_by_user_blacklist(path)?;
            return Ok(());
        }
        return Err("폴더가 더 이상 존재하지 않거나 폴더가 아닙니다.".to_string());
    }

    if !path.exists() || !path.is_file() {
        return Err("파일이 더 이상 존재하지 않거나 파일이 아닙니다.".to_string());
    }

    let extension = path
        .extension()
        .map(|value| security::normalize_extension(&value.to_string_lossy()))
        .unwrap_or_default();

    if !security::is_allowed_open_extension(&extension) {
        return Err(format!("현재 안전 정책상 .{extension} 파일은 열 수 없습니다."));
    }
    permission::validate_extension_allowed(Some(&extension))?;
    permission::validate_path_allowed_by_user_blacklist(path)?;

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
