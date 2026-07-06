pub mod chat;
pub mod health;
pub mod files;
pub mod ledger;
pub mod schedules;
pub mod status;
pub mod stt;

use axum::{routing::{delete, get, post}, Router};
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
        .route("/api/chat", post(chat::chat_with_ai))
        .route("/api/stt", post(stt::transcribe_audio))
        .route("/api/status", get(status::api_status))
        .nest_service("/models", ServeDir::new("assets/models"))
        .layer(cors)
        .with_state(state)
}
