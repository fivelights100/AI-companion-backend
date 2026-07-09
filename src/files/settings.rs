use std::{fs, path::{Path, PathBuf}};

use chrono::Utc;
use serde_json::Value;

use crate::models::filesystem::{
    default_extension_groups, default_fixed_blocked_paths, FilesystemSettings,
    FilesystemSettingsUpdateRequest,
};

use super::security::{contains_control_chars, normalize_extension, normalize_windows_path, is_same_or_child_path};

const SETTINGS_PATH: &str = "config/filesystem_settings.json";

#[derive(Debug, Clone, Copy)]
pub enum FilesystemPermissionKind {
    Search,
    Modify,
    Delete,
}

impl FilesystemPermissionKind {
    fn label(self) -> &'static str {
        match self {
            Self::Search => "검색",
            Self::Modify => "수정",
            Self::Delete => "삭제",
        }
    }
}

pub fn load_settings() -> FilesystemSettings {
    let path = settings_path();
    if !path.exists() {
        let settings = FilesystemSettings::default();
        let _ = save_settings(&settings);
        return settings;
    }

    let Ok(raw) = fs::read_to_string(&path) else {
        let settings = FilesystemSettings::default();
        let _ = save_settings(&settings);
        return settings;
    };

    let default_settings = FilesystemSettings::default();
    let Ok(mut value) = serde_json::from_str::<Value>(&raw) else {
        let _ = backup_invalid_settings_file(&path);
        let _ = save_settings(&default_settings);
        return default_settings;
    };

    if let Ok(default_value) = serde_json::to_value(&default_settings) {
        merge_missing_fields(&mut value, &default_value);
    }

    let mut settings = serde_json::from_value::<FilesystemSettings>(value).unwrap_or(default_settings);
    normalize_settings(&mut settings);
    let _ = save_settings(&settings);
    settings
}

pub fn save_settings(settings: &FilesystemSettings) -> Result<(), String> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("파일 시스템 설정 폴더를 만들 수 없습니다: {error}"))?;
    }

    let mut settings = settings.clone();
    normalize_settings(&mut settings);
    let serialized = serde_json::to_string_pretty(&settings)
        .map_err(|error| format!("파일 시스템 설정을 직렬화할 수 없습니다: {error}"))?;
    fs::write(&path, serialized).map_err(|error| format!("파일 시스템 설정을 저장할 수 없습니다: {error}"))
}

pub fn update_settings(update: FilesystemSettingsUpdateRequest) -> Result<FilesystemSettings, String> {
    let mut settings = load_settings();

    if let Some(terms) = update.terms {
        if let Some(show_terms_modal) = terms.show_terms_modal {
            settings.terms.show_terms_modal = show_terms_modal;
        }
        if let Some(accepted) = terms.accepted {
            settings.terms.accepted = accepted;
            if accepted && settings.terms.accepted_at.is_none() {
                settings.terms.accepted_at = Some(Utc::now().to_rfc3339());
            }
            if !accepted {
                settings.terms.accepted_at = None;
            }
        }
        if let Some(accepted_at) = terms.accepted_at {
            settings.terms.accepted_at = if accepted_at.trim().is_empty() { None } else { Some(accepted_at) };
        }
    }

    if let Some(permissions) = update.permissions {
        if let Some(modify) = permissions.modify {
            settings.permissions.modify = modify;
        }
        if let Some(delete) = permissions.delete {
            settings.permissions.delete = delete;
        }
        settings.permissions.search = true;
    }

    if let Some(safety) = update.safety {
        if let Some(paths) = safety.user_blocked_paths {
            settings.safety.user_blocked_paths = sanitize_user_blocked_paths(paths)?;
        }
        if let Some(extensions) = safety.allowed_extensions {
            settings.safety.allowed_extensions = sanitize_allowed_extensions(extensions)?;
        }
    }

    if let Some(enabled) = update.enabled {
        if enabled && settings.terms.show_terms_modal && !settings.terms.accepted {
            return Err("파일 시스템 약관에 동의해야 활성화할 수 있습니다.".to_string());
        }
        settings.enabled = enabled;
    }

    normalize_settings(&mut settings);
    save_settings(&settings)?;
    Ok(settings)
}

pub fn accept_terms_and_enable() -> Result<FilesystemSettings, String> {
    let mut settings = load_settings();
    settings.terms.accepted = true;
    settings.terms.accepted_at = Some(Utc::now().to_rfc3339());
    settings.enabled = true;
    normalize_settings(&mut settings);
    save_settings(&settings)?;
    Ok(settings)
}

pub fn ensure_permission(kind: FilesystemPermissionKind) -> Result<FilesystemSettings, String> {
    let settings = load_settings();
    if !settings.enabled {
        return Err("파일 시스템 기능이 비활성화되어 있습니다. 설정에서 파일 시스템을 활성화해 주세요.".to_string());
    }

    let allowed = match kind {
        FilesystemPermissionKind::Search => settings.permissions.search,
        FilesystemPermissionKind::Modify => settings.permissions.modify,
        FilesystemPermissionKind::Delete => settings.permissions.delete,
    };

    if !allowed {
        return Err(format!("파일 시스템 {} 권한이 꺼져 있습니다. 설정에서 권한을 켜 주세요.", kind.label()));
    }

    Ok(settings)
}

pub fn ensure_search_enabled() -> Result<FilesystemSettings, String> {
    ensure_permission(FilesystemPermissionKind::Search)
}

pub fn ensure_modify_enabled() -> Result<FilesystemSettings, String> {
    ensure_permission(FilesystemPermissionKind::Modify)
}

pub fn ensure_delete_enabled() -> Result<FilesystemSettings, String> {
    ensure_permission(FilesystemPermissionKind::Delete)
}

pub fn validate_extension_allowed(extension: Option<&str>) -> Result<(), String> {
    let Some(extension) = extension else {
        return Ok(());
    };
    let extension = normalize_extension(extension);
    if extension.is_empty() {
        return Ok(());
    }
    let settings = load_settings();
    if settings.safety.allowed_extensions.iter().any(|value| value == &extension) {
        Ok(())
    } else {
        Err(format!("파일 시스템 설정에서 .{extension} 확장자가 허용되어 있지 않습니다."))
    }
}

pub fn is_extension_allowed(extension: Option<&str>) -> bool {
    validate_extension_allowed(extension).is_ok()
}

pub fn validate_path_allowed_by_user_blacklist(path: &Path) -> Result<(), String> {
    let settings = load_settings();
    let normalized = normalize_windows_path(&path.to_string_lossy());
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let canonical_normalized = normalize_windows_path(&canonical.to_string_lossy());

    for blocked_path in settings.safety.user_blocked_paths {
        let blocked = normalize_windows_path(&blocked_path);
        if blocked.is_empty() {
            continue;
        }
        if is_same_or_child_path(&normalized, &blocked) || is_same_or_child_path(&canonical_normalized, &blocked) {
            return Err("파일 시스템 설정의 사용자 제한 경로에 포함되어 있어 작업할 수 없습니다.".to_string());
        }
    }

    Ok(())
}

pub fn validate_path_and_extension_for_settings(path: &Path, is_folder: bool) -> Result<(), String> {
    validate_path_allowed_by_user_blacklist(path)?;
    if !is_folder {
        let extension = path.extension().map(|value| normalize_extension(&value.to_string_lossy()));
        validate_extension_allowed(extension.as_deref())?;
    }
    Ok(())
}

fn backup_invalid_settings_file(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }

    let timestamp = Utc::now().format("%Y%m%d%H%M%S");
    let backup_path = path.with_extension(format!("json.invalid-{timestamp}"));
    fs::copy(path, &backup_path)
        .map(|_| ())
        .map_err(|error| format!("손상된 파일 시스템 설정을 백업할 수 없습니다: {error}"))
}

fn merge_missing_fields(value: &mut Value, defaults: &Value) {
    match (value, defaults) {
        (Value::Object(current), Value::Object(default_map)) => {
            for (key, default_value) in default_map {
                match current.get_mut(key) {
                    Some(current_value) => merge_missing_fields(current_value, default_value),
                    None => {
                        current.insert(key.clone(), default_value.clone());
                    }
                }
            }
        }
        _ => {}
    }
}

fn settings_path() -> PathBuf {
    std::env::var("FILESYSTEM_SETTINGS_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(SETTINGS_PATH))
}

fn normalize_settings(settings: &mut FilesystemSettings) {
    settings.permissions.search = true;
    settings.safety.fixed_blocked_paths = default_fixed_blocked_paths();
    settings.safety.extension_groups = default_extension_groups();
    settings.safety.allowed_extensions = sanitize_allowed_extensions(settings.safety.allowed_extensions.clone()).unwrap_or_else(|_| vec!["txt".to_string()]);
    settings.safety.user_blocked_paths = sanitize_user_blocked_paths(settings.safety.user_blocked_paths.clone()).unwrap_or_default();
}

fn sanitize_user_blocked_paths(paths: Vec<String>) -> Result<Vec<String>, String> {
    let mut sanitized = Vec::new();
    for path in paths {
        let value = path.trim().trim_matches('"').trim_matches('\'').to_string();
        if value.is_empty() {
            continue;
        }
        if contains_control_chars(&value) {
            return Err("사용자 제한 경로에 허용되지 않는 제어 문자가 포함되어 있습니다.".to_string());
        }
        if sanitized.iter().all(|existing| existing != &value) {
            sanitized.push(value);
        }
    }
    Ok(sanitized)
}

fn sanitize_allowed_extensions(extensions: Vec<String>) -> Result<Vec<String>, String> {
    let mut sanitized = Vec::new();
    for extension in extensions {
        let extension = normalize_extension(&extension);
        if extension.is_empty() {
            continue;
        }
        let valid = extension.len() <= 16 && extension.chars().all(|character| character.is_ascii_alphanumeric());
        if !valid {
            return Err("확장자는 영문/숫자 16자 이하만 저장할 수 있습니다.".to_string());
        }
        if sanitized.iter().all(|existing| existing != &extension) {
            sanitized.push(extension);
        }
    }
    Ok(sanitized)
}
