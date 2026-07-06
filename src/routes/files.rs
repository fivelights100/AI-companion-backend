use axum::Json;

use crate::{
    files::{everything, opener},
    models::files::{FileOpenConfirmRequest, FileOpenConfirmResponse, FileOpenNextRequest, FileOpenPrepareRequest, FileOpenPrepareResponse, FileSearchRequest},
};

pub async fn file_search_status() -> Json<crate::models::files::FileSearchStatus> {
    Json(everything::get_status().await)
}

pub async fn search_files(Json(payload): Json<FileSearchRequest>) -> Json<crate::models::files::FileSearchResponse> {
    Json(everything::search_files(payload).await)
}

pub async fn prepare_open_file_or_folder(
    Json(payload): Json<FileOpenPrepareRequest>,
) -> Json<FileOpenPrepareResponse> {
    Json(opener::prepare_open_target(payload).await)
}

pub async fn next_open_file_or_folder_candidates(
    Json(payload): Json<FileOpenNextRequest>,
) -> Json<FileOpenPrepareResponse> {
    Json(opener::next_open_candidates(payload).await)
}

pub async fn confirm_open_file_or_folder(
    Json(payload): Json<FileOpenConfirmRequest>,
) -> Json<FileOpenConfirmResponse> {
    Json(opener::confirm_open_target(payload).await)
}
