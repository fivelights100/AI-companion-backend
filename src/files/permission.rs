use std::path::Path;

use crate::models::filesystem::FilesystemSettings;

use super::settings::{self, FilesystemPermissionKind};

/// Centralized permission gate for all filesystem actions.
///
/// UI settings are an additional user-controlled filter. They do not bypass
/// operation-specific safeguards such as preview/confirm flows, size limits,
/// overwrite checks, and same-drive move restrictions.
pub fn ensure_permission(kind: FilesystemPermissionKind) -> Result<FilesystemSettings, String> {
    settings::ensure_permission(kind)
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
    settings::validate_extension_allowed(extension)
}

pub fn is_extension_allowed(extension: Option<&str>) -> bool {
    settings::is_extension_allowed(extension)
}

pub fn validate_path_allowed_by_user_blacklist(path: &Path) -> Result<(), String> {
    settings::validate_path_allowed_by_user_blacklist(path)
}

pub fn validate_path_and_extension_for_settings(path: &Path, is_folder: bool) -> Result<(), String> {
    settings::validate_path_and_extension_for_settings(path, is_folder)
}

pub fn validate_parent_path_for_new_target(parent: &Path) -> Result<(), String> {
    validate_path_allowed_by_user_blacklist(parent)
}
