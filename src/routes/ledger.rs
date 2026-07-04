use axum::{extract::{Path, State}, Json};

use crate::{
    db::ledger::{create_ledger_entry, delete_ledger_entry_by_id, list_ledger_entries},
    models::ledger::{CreateLedgerEntry, LedgerEntry},
    state::AppState,
};

pub async fn get_ledger_entries(State(state): State<AppState>) -> Json<Vec<LedgerEntry>> {
    match list_ledger_entries(&state.db).await {
        Ok(entries) => Json(entries),
        Err(error) => {
            eprintln!("DB 가계부 조회 에러: {error}");
            Json(vec![])
        }
    }
}

pub async fn add_ledger_entry(
    State(state): State<AppState>,
    Json(payload): Json<CreateLedgerEntry>,
) -> Json<serde_json::Value> {
    match create_ledger_entry(&state.db, &payload).await {
        Ok(id) => Json(serde_json::json!({
            "id": id,
            "message": "가계부 기록이 추가되었습니다."
        })),
        Err(error) => Json(serde_json::json!({ "error": error.to_string() })),
    }
}

pub async fn delete_ledger_entry(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Json<serde_json::Value> {
    match delete_ledger_entry_by_id(&state.db, id).await {
        Ok(rows_affected) if rows_affected > 0 => Json(serde_json::json!({ "message": "삭제되었습니다." })),
        Ok(_) => Json(serde_json::json!({ "message": "삭제할 가계부 기록을 찾지 못했습니다." })),
        Err(error) => Json(serde_json::json!({ "error": error.to_string() })),
    }
}
