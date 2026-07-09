use axum::Json;

use crate::{
    files::{everything, opener, rename, create, content_edit, delete, transfer},
    models::files::{FileOpenConfirmRequest, FileOpenConfirmResponse, FileOpenNextRequest, FileOpenPrepareRequest, FileOpenPrepareResponse, FileRenameConfirmRequest, FileRenameConfirmResponse, FileRenameNextRequest, FileRenamePrepareRequest, FileRenamePrepareResponse, FileRenamePreviewRequest, FileRenamePreviewResponse, FileSearchRequest, FileCreateConfirmRequest, FileCreateConfirmResponse, FileCreateNextRequest, FileCreatePrepareRequest, FileCreatePrepareResponse, FileCreatePreviewRequest, FileCreatePreviewResponse, FileContentEditConfirmRequest, FileContentEditConfirmResponse, FileContentEditNextRequest, FileContentEditPrepareRequest, FileContentEditPrepareResponse, FileContentEditPreviewRequest, FileContentEditPreviewResponse, FileDeleteConfirmRequest, FileDeleteConfirmResponse, FileDeleteNextRequest, FileDeletePrepareRequest, FileDeletePrepareResponse, FileDeletePreviewRequest, FileDeletePreviewResponse, FileTransferConfirmRequest, FileTransferConfirmResponse, FileTransferDestinationNextRequest, FileTransferDestinationNextResponse, FileTransferPrepareRequest, FileTransferPrepareResponse, FileTransferPreviewRequest, FileTransferPreviewResponse, FileTransferSourceNextRequest, FileTransferSourceNextResponse},
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


pub async fn prepare_rename_file_or_folder(
    Json(payload): Json<FileRenamePrepareRequest>,
) -> Json<FileRenamePrepareResponse> {
    Json(rename::prepare_rename_target(payload).await)
}

pub async fn next_rename_file_or_folder_candidates(
    Json(payload): Json<FileRenameNextRequest>,
) -> Json<FileRenamePrepareResponse> {
    Json(rename::next_rename_candidates(payload).await)
}

pub async fn preview_rename_file_or_folder(
    Json(payload): Json<FileRenamePreviewRequest>,
) -> Json<FileRenamePreviewResponse> {
    Json(rename::preview_rename_target(payload).await)
}

pub async fn confirm_rename_file_or_folder(
    Json(payload): Json<FileRenameConfirmRequest>,
) -> Json<FileRenameConfirmResponse> {
    Json(rename::confirm_rename_target(payload).await)
}


pub async fn prepare_create_file_or_folder(
    Json(payload): Json<FileCreatePrepareRequest>,
) -> Json<FileCreatePrepareResponse> {
    Json(create::prepare_create_target(payload).await)
}

pub async fn next_create_file_or_folder_candidates(
    Json(payload): Json<FileCreateNextRequest>,
) -> Json<FileCreatePrepareResponse> {
    Json(create::next_create_candidates(payload).await)
}

pub async fn preview_create_file_or_folder(
    Json(payload): Json<FileCreatePreviewRequest>,
) -> Json<FileCreatePreviewResponse> {
    Json(create::preview_create_target(payload).await)
}

pub async fn confirm_create_file_or_folder(
    Json(payload): Json<FileCreateConfirmRequest>,
) -> Json<FileCreateConfirmResponse> {
    Json(create::confirm_create_target(payload).await)
}


pub async fn prepare_content_edit_file(
    Json(payload): Json<FileContentEditPrepareRequest>,
) -> Json<FileContentEditPrepareResponse> {
    Json(content_edit::prepare_content_edit_target(payload).await)
}

pub async fn next_content_edit_file_candidates(
    Json(payload): Json<FileContentEditNextRequest>,
) -> Json<FileContentEditPrepareResponse> {
    Json(content_edit::next_content_edit_candidates(payload).await)
}

pub async fn preview_content_edit_file(
    Json(payload): Json<FileContentEditPreviewRequest>,
) -> Json<FileContentEditPreviewResponse> {
    Json(content_edit::preview_content_edit_target(payload).await)
}

pub async fn confirm_content_edit_file(
    Json(payload): Json<FileContentEditConfirmRequest>,
) -> Json<FileContentEditConfirmResponse> {
    Json(content_edit::confirm_content_edit_target(payload).await)
}


pub async fn prepare_delete_file_or_folder(
    Json(payload): Json<FileDeletePrepareRequest>,
) -> Json<FileDeletePrepareResponse> {
    Json(delete::prepare_delete_target(payload).await)
}

pub async fn next_delete_file_or_folder_candidates(
    Json(payload): Json<FileDeleteNextRequest>,
) -> Json<FileDeletePrepareResponse> {
    Json(delete::next_delete_candidates(payload).await)
}

pub async fn preview_delete_file_or_folder(
    Json(payload): Json<FileDeletePreviewRequest>,
) -> Json<FileDeletePreviewResponse> {
    Json(delete::preview_delete_target(payload).await)
}

pub async fn confirm_delete_file_or_folder(
    Json(payload): Json<FileDeleteConfirmRequest>,
) -> Json<FileDeleteConfirmResponse> {
    Json(delete::confirm_delete_target(payload).await)
}


pub async fn prepare_transfer_file_or_folder(
    Json(payload): Json<FileTransferPrepareRequest>,
) -> Json<FileTransferPrepareResponse> {
    Json(transfer::prepare_transfer_target(payload).await)
}

pub async fn next_transfer_source_candidates(
    Json(payload): Json<FileTransferSourceNextRequest>,
) -> Json<FileTransferSourceNextResponse> {
    Json(transfer::next_transfer_source_candidates(payload).await)
}

pub async fn next_transfer_destination_candidates(
    Json(payload): Json<FileTransferDestinationNextRequest>,
) -> Json<FileTransferDestinationNextResponse> {
    Json(transfer::next_transfer_destination_candidates(payload).await)
}

pub async fn preview_transfer_file_or_folder(
    Json(payload): Json<FileTransferPreviewRequest>,
) -> Json<FileTransferPreviewResponse> {
    Json(transfer::preview_transfer_target(payload).await)
}

pub async fn confirm_transfer_file_or_folder(
    Json(payload): Json<FileTransferConfirmRequest>,
) -> Json<FileTransferConfirmResponse> {
    Json(transfer::confirm_transfer_target(payload).await)
}
