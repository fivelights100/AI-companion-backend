pub mod chat;
pub mod health;
pub mod files;
pub mod filesystem;
pub mod ledger;
pub mod schedules;
pub mod status;
pub mod stt;

use axum::{routing::{delete, get, post, put}, Router};
use tower_http::{cors::{Any, CorsLayer}, services::ServeDir};

use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(health::health_check))
        .route("/api/schedules", get(schedules::get_schedules).post(schedules::add_schedule))
        .route("/api/schedules/:id", delete(schedules::delete_schedule))
        .route("/api/ledger", get(ledger::get_ledger_entries).post(ledger::add_ledger_entry))
        .route("/api/ledger/:id", delete(ledger::delete_ledger_entry))
        .route("/api/files/status", get(files::file_search_status))
        .route("/api/files/search", post(files::search_files))
        .route("/api/files/open/prepare", post(files::prepare_open_file_or_folder))
        .route("/api/files/open/next", post(files::next_open_file_or_folder_candidates))
        .route("/api/files/open/confirm", post(files::confirm_open_file_or_folder))
        .route("/api/files/rename/prepare", post(files::prepare_rename_file_or_folder))
        .route("/api/files/rename/next", post(files::next_rename_file_or_folder_candidates))
        .route("/api/files/rename/preview", post(files::preview_rename_file_or_folder))
        .route("/api/files/rename/confirm", post(files::confirm_rename_file_or_folder))
        .route("/api/files/create/prepare", post(files::prepare_create_file_or_folder))
        .route("/api/files/create/next", post(files::next_create_file_or_folder_candidates))
        .route("/api/files/create/preview", post(files::preview_create_file_or_folder))
        .route("/api/files/create/confirm", post(files::confirm_create_file_or_folder))
        .route("/api/files/content-edit/prepare", post(files::prepare_content_edit_file))
        .route("/api/files/content-edit/next", post(files::next_content_edit_file_candidates))
        .route("/api/files/content-edit/preview", post(files::preview_content_edit_file))
        .route("/api/files/content-edit/confirm", post(files::confirm_content_edit_file))
        .route("/api/files/delete/prepare", post(files::prepare_delete_file_or_folder))
        .route("/api/files/delete/next", post(files::next_delete_file_or_folder_candidates))
        .route("/api/files/delete/preview", post(files::preview_delete_file_or_folder))
        .route("/api/files/delete/confirm", post(files::confirm_delete_file_or_folder))
        .route("/api/files/transfer/prepare", post(files::prepare_transfer_file_or_folder))
        .route("/api/files/transfer/source-next", post(files::next_transfer_source_candidates))
        .route("/api/files/transfer/destination-next", post(files::next_transfer_destination_candidates))
        .route("/api/files/transfer/preview", post(files::preview_transfer_file_or_folder))
        .route("/api/files/transfer/confirm", post(files::confirm_transfer_file_or_folder))
        .route("/api/chat", post(chat::chat_with_ai))
        .route("/api/stt", post(stt::transcribe_audio))
        .route("/api/filesystem/settings", get(filesystem::get_filesystem_settings).put(filesystem::update_filesystem_settings))
        .route("/api/filesystem/terms/accept", post(filesystem::accept_filesystem_terms))
        .route("/api/status", get(status::api_status))
        .nest_service("/models", ServeDir::new("assets/models"))
        .layer(cors)
        .with_state(state)
}
