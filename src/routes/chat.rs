use axum::{extract::State, Json};

use crate::{ai::chat_service::handle_chat, models::chat::ChatRequest, state::AppState};

pub async fn chat_with_ai(
    State(state): State<AppState>,
    Json(payload): Json<ChatRequest>,
) -> Json<crate::models::chat::ChatResponse> {
    Json(handle_chat(&state, payload).await)
}
