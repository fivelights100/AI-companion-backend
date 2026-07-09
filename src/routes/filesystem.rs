use axum::Json;
use chrono::Utc;

use crate::{
    files::settings,
    models::filesystem::{FilesystemSettingsResponse, FilesystemSettingsUpdateRequest},
};

pub async fn get_filesystem_settings() -> Json<FilesystemSettingsResponse> {
    Json(FilesystemSettingsResponse {
        ok: true,
        message: "파일 시스템 설정을 불러왔습니다.".to_string(),
        settings: settings::load_settings(),
    })
}

pub async fn update_filesystem_settings(
    Json(payload): Json<FilesystemSettingsUpdateRequest>,
) -> Json<FilesystemSettingsResponse> {
    match settings::update_settings(payload) {
        Ok(settings) => Json(FilesystemSettingsResponse {
            ok: true,
            message: "파일 시스템 설정을 저장했습니다.".to_string(),
            settings,
        }),
        Err(message) => Json(FilesystemSettingsResponse {
            ok: false,
            message,
            settings: settings::load_settings(),
        }),
    }
}

pub async fn accept_filesystem_terms() -> Json<FilesystemSettingsResponse> {
    match settings::accept_terms_and_enable() {
        Ok(settings) => Json(FilesystemSettingsResponse {
            ok: true,
            message: format!("파일 시스템 약관에 동의했습니다. 동의 시간: {}", Utc::now().to_rfc3339()),
            settings,
        }),
        Err(message) => Json(FilesystemSettingsResponse {
            ok: false,
            message,
            settings: settings::load_settings(),
        }),
    }
}
