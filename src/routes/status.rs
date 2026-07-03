use axum::{extract::State, Json};
use chrono::Utc;
use std::path::Path;

use crate::state::AppState;

pub async fn api_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let database = match sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&state.db)
        .await
    {
        Ok(_) => serde_json::json!({
            "ok": true,
            "message": "connected"
        }),
        Err(error) => serde_json::json!({
            "ok": false,
            "message": error.to_string()
        }),
    };

    let model_path = Path::new("assets/models/hiyori_ex/runtime/hiyori_free_t08.model3.json");

    Json(serde_json::json!({
        "status": "ok",
        "server_time": Utc::now().to_rfc3339(),
        "database": database,
        "services": {
            "openai_api_key": has_env("OPENAI_API_KEY"),
            "elevenlabs_api_key": has_env("ELEVENLABS_API_KEY"),
            "elevenlabs_voice_id": has_env("ELEVENLABS_VOICE_ID")
        },
        "models": {
            "hiyori_runtime": model_path.exists(),
            "hiyori_model_path": model_path.to_string_lossy()
        }
    }))
}

fn has_env(name: &str) -> bool {
    std::env::var(name)
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}
