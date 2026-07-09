use std::{
    env,
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tokio::{process::Command, time::{sleep, timeout}};

use crate::{
    files::{permission, security},
    models::files::{
        FileSearchKind, FileSearchRequest, FileSearchResponse, FileSearchResult, FileSearchStatus,
    },
};

const DEFAULT_MAX_RESULTS: u8 = 20;
const HARD_MAX_RESULTS: u8 = 50;
const COMMAND_TIMEOUT_SECONDS: u64 = 10;
const EVERYTHING_START_WAIT_MS: u64 = 900;

pub async fn get_status() -> FileSearchStatus {
    let app_path = find_everything_app();
    let es_path = find_everything_cli();

    let Some(es_path) = es_path else {
        return FileSearchStatus {
            available: false,
            es_path: None,
            everything_app_path: app_path.map(|path| path.to_string_lossy().to_string()),
            everything_running: false,
            message: "Everything CLI(es.exe)를 찾지 못했습니다.".to_string(),
            install_hint: install_hint(),
        };
    };

    let mut everything_running = check_everything_available(&es_path).await;
    let mut started_in_background = false;

    if !everything_running {
        if try_start_everything_in_background(app_path.as_deref()).await {
            started_in_background = true;
            everything_running = check_everything_available(&es_path).await;
        }
    }

    let message = if everything_running && started_in_background {
        "Everything을 백그라운드로 실행했고, CLI 검색을 사용할 수 있습니다.".to_string()
    } else if everything_running {
        "Everything CLI를 사용할 수 있습니다.".to_string()
    } else if app_path.is_some() {
        "es.exe는 찾았지만 Everything 백그라운드 실행/IPC 연결 확인에 실패했습니다.".to_string()
    } else {
        "es.exe는 찾았지만 Everything 본체(Everything.exe)를 찾지 못했거나 실행 중이 아닙니다.".to_string()
    };

    FileSearchStatus {
        available: everything_running,
        es_path: Some(es_path.to_string_lossy().to_string()),
        everything_app_path: app_path.map(|path| path.to_string_lossy().to_string()),
        everything_running,
        message,
        install_hint: install_hint(),
    }
}

pub async fn search_files(request: FileSearchRequest) -> FileSearchResponse {
    if let Err(message) = permission::ensure_search_enabled() {
        return FileSearchResponse { ok: false, message, results: vec![] };
    }
    if let Some(extension) = request.extension.as_deref() {
        if let Err(message) = permission::validate_extension_allowed(Some(extension)) {
            return FileSearchResponse { ok: false, message, results: vec![] };
        }
    }
    if let Some(message) = validate_request(&request) {
        return FileSearchResponse {
            ok: false,
            message,
            results: vec![],
        };
    }

    let Some(es_path) = find_everything_cli() else {
        return FileSearchResponse {
            ok: false,
            message: "Everything CLI(es.exe)를 찾지 못했습니다. EVERYTHING_ES_PATH를 es.exe 파일로 설정하거나 tools/everything/es.exe에 배치하세요.".to_string(),
            results: vec![],
        };
    };

    if !check_everything_available(&es_path).await {
        let app_path = find_everything_app();
        let started = try_start_everything_in_background(app_path.as_deref()).await;
        if !started || !check_everything_available(&es_path).await {
            return FileSearchResponse {
                ok: false,
                message: if app_path.is_some() {
                    "Everything을 백그라운드로 실행하려고 했지만 CLI 연결이 아직 준비되지 않았습니다. 잠시 뒤 다시 검색하거나 Everything 상태를 확인하세요.".to_string()
                } else {
                    "Everything 본체가 실행 중이 아니고 Everything.exe도 찾지 못했습니다. Everything을 실행한 뒤 다시 검색하세요.".to_string()
                },
                results: vec![],
            };
        }
    }

    let max_results = request
        .max_results
        .unwrap_or(DEFAULT_MAX_RESULTS)
        .clamp(1, HARD_MAX_RESULTS);

    let export_path = make_export_path();
    let args = build_allowed_args(&request, max_results, Some(&export_path));

    let output = timeout(
        Duration::from_secs(COMMAND_TIMEOUT_SECONDS),
        Command::new(&es_path)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output(),
    )
    .await;

    let output = match output {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => {
            let _ = fs::remove_file(&export_path);
            return FileSearchResponse {
                ok: false,
                message: format!("Everything CLI 실행 실패: {error}"),
                results: vec![],
            };
        }
        Err(_) => {
            let _ = fs::remove_file(&export_path);
            return FileSearchResponse {
                ok: false,
                message: "Everything CLI 검색 시간이 초과되었습니다.".to_string(),
                results: vec![],
            };
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let _ = fs::remove_file(&export_path);
        return FileSearchResponse {
            ok: false,
            message: if stderr.is_empty() {
                "Everything CLI 검색에 실패했습니다. Everything 본체가 실행 중인지 확인하세요.".to_string()
            } else {
                format!("Everything CLI 검색 실패: {stderr}")
            },
            results: vec![],
        };
    }

    let exported = fs::read_to_string(&export_path);
    let _ = fs::remove_file(&export_path);

    let exported = match exported {
        Ok(value) => value,
        Err(error) => {
            return FileSearchResponse {
                ok: false,
                message: format!("Everything 검색 결과 파일을 읽지 못했습니다: {error}"),
                results: vec![],
            };
        }
    };

    let results = exported
        .lines()
        .filter_map(parse_result_line)
        .filter(|result| result.is_folder || permission::is_extension_allowed(result.extension.as_deref()))
        .take(max_results as usize)
        .collect::<Vec<_>>();

    FileSearchResponse {
        ok: true,
        message: if results.is_empty() {
            "검색 결과가 없습니다. Everything 앱 검색어와 동일한 검색어로 다시 시도하거나 검색 범위/확장자 조건을 줄여보세요.".to_string()
        } else {
            format!("{}개의 결과를 찾았습니다.", results.len())
        },
        results,
    }
}

fn validate_request(request: &FileSearchRequest) -> Option<String> {
    security::validate_query(&request.query, "검색어가 비어 있습니다.")
        .or_else(|| security::validate_search_extension(request.extension.as_deref()))
        .or_else(|| security::validate_root_path(request.root_path.as_deref()))
}

fn build_allowed_args(request: &FileSearchRequest, max_results: u8, export_path: Option<&Path>) -> Vec<String> {
    let mut args = vec![
        "-n".to_string(),
        max_results.to_string(),
        "-timeout".to_string(),
        "5000".to_string(),
    ];

    if request.match_path.unwrap_or(false) {
        args.push("-p".to_string());
    }

    if let Some(root_path) = request.root_path.as_deref() {
        let root_path = root_path.trim();
        if !root_path.is_empty() {
            args.push("-path".to_string());
            args.push(root_path.to_string());
        }
    }

    match request.kind.clone().unwrap_or_default() {
        FileSearchKind::File => args.push("/a-d".to_string()),
        FileSearchKind::Folder => args.push("/ad".to_string()),
        FileSearchKind::Any => {}
    }

    if let Some(export_path) = export_path {
        args.push("-export-txt".to_string());
        args.push(export_path.to_string_lossy().to_string());
    }

    let mut search_terms = split_safe_search_terms(request.query.trim());

    if let Some(extension) = request.extension.as_deref() {
        let extension = security::normalize_extension(extension);
        if !extension.is_empty() {
            search_terms.push(format!("ext:{extension}"));
        }
    }

    args.extend(search_terms);
    args
}

fn split_safe_search_terms(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .filter(|part| !part.trim().is_empty())
        .map(|part| part.trim_matches('"').to_string())
        .filter(|part| !part.is_empty())
        .collect()
}

fn parse_result_line(line: &str) -> Option<FileSearchResult> {
    let path = line.trim().trim_matches('\u{feff}');
    if path.is_empty() {
        return None;
    }

    let path_obj = Path::new(path);
    let name = path_obj
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());
    let extension = path_obj
        .extension()
        .map(|value| value.to_string_lossy().to_string());
    let is_folder = path_obj.is_dir() || (extension.is_none() && !path.contains('.'));

    Some(FileSearchResult {
        path: path.to_string(),
        name,
        is_folder,
        extension,
    })
}


fn find_everything_cli() -> Option<PathBuf> {
    if let Ok(path) = env::var("EVERYTHING_ES_PATH") {
        let path = PathBuf::from(path.trim());
        if is_named_executable_file(&path, &["es.exe", "es"]) {
            return Some(path);
        }
    }

    let mut candidates = vec![];

    if let Ok(current_dir) = env::current_dir() {
        candidates.push(current_dir.join("tools/everything/es.exe"));
        candidates.push(current_dir.join("tools/es.exe"));
        candidates.push(current_dir.join("everything/es.exe"));
    }

    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.push(exe_dir.join("tools/everything/es.exe"));
            candidates.push(exe_dir.join("tools/es.exe"));
            candidates.push(exe_dir.join("everything/es.exe"));
        }
    }

    let path_env = env::var_os("PATH").unwrap_or_default();
    for path_dir in env::split_paths(&path_env) {
        candidates.push(path_dir.join("es.exe"));
        candidates.push(path_dir.join("es"));
    }

    candidates
        .into_iter()
        .find(|path| is_named_executable_file(path, &["es.exe", "es"]))
}

fn find_everything_app() -> Option<PathBuf> {
    if let Ok(path) = env::var("EVERYTHING_APP_PATH") {
        let path = PathBuf::from(path.trim());
        if is_named_executable_file(&path, &["everything.exe", "everything"]) {
            return Some(path);
        }
    }

    let mut candidates = vec![];

    if let Ok(current_dir) = env::current_dir() {
        candidates.push(current_dir.join("tools/everything/Everything.exe"));
        candidates.push(current_dir.join("everything/Everything.exe"));
    }

    if let Some(es_path) = find_everything_cli() {
        if let Some(es_dir) = es_path.parent() {
            candidates.push(es_dir.join("Everything.exe"));
        }
    }

    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.push(exe_dir.join("tools/everything/Everything.exe"));
            candidates.push(exe_dir.join("everything/Everything.exe"));
        }
    }

    for env_name in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
        if let Ok(base) = env::var(env_name) {
            candidates.push(PathBuf::from(base).join("Everything/Everything.exe"));
        }
    }

    candidates
        .into_iter()
        .find(|path| is_named_executable_file(path, &["everything.exe", "everything"]))
}

fn is_named_executable_file(path: &Path, allowed_names: &[&str]) -> bool {
    if !path.exists() || !path.is_file() {
        return false;
    }

    let Some(file_name) = path.file_name().map(|value| value.to_string_lossy().to_ascii_lowercase()) else {
        return false;
    };

    allowed_names.iter().any(|name| file_name == *name)
}

async fn check_everything_available(es_path: &Path) -> bool {
    let result = timeout(
        Duration::from_secs(4),
        Command::new(es_path)
            .args(["-n", "1", "-timeout", "3000", "*"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status(),
    )
    .await;

    matches!(result, Ok(Ok(status)) if status.success())
}

async fn try_start_everything_in_background(app_path: Option<&Path>) -> bool {
    let Some(app_path) = app_path else {
        return false;
    };

    let spawn_result = std::process::Command::new(app_path)
        .arg("-startup")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    if spawn_result.is_err() {
        return false;
    }

    sleep(Duration::from_millis(EVERYTHING_START_WAIT_MS)).await;
    true
}

fn make_export_path() -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();

    env::temp_dir().join(format!(
        "ai_companion_everything_search_{}_{}.txt",
        std::process::id(),
        timestamp
    ))
}

fn install_hint() -> String {
    "Everything 본체를 설치한 뒤 Everything.exe는 일반 실행 파일, es.exe는 CLI 파일로 지정하세요. 예: server/tools/everything/Everything.exe, server/tools/everything/es.exe 또는 EVERYTHING_APP_PATH/EVERYTHING_ES_PATH 환경변수.".to_string()
}
