use axum::{extract::{Path, State}, Json};

use crate::{
    db::schedules::{create_schedule, delete_schedule_by_id, list_schedules},
    models::schedule::{CreateSchedule, Schedule},
    state::AppState,
};

pub async fn get_schedules(State(state): State<AppState>) -> Json<Vec<Schedule>> {
    match list_schedules(&state.db).await {
        Ok(schedules) => Json(schedules),
        Err(error) => {
            eprintln!("DB 조회 에러: {error}");
            Json(vec![])
        }
    }
}

pub async fn add_schedule(
    State(state): State<AppState>,
    Json(payload): Json<CreateSchedule>,
) -> Json<serde_json::Value> {
    match create_schedule(&state.db, &payload).await {
        Ok(id) => Json(serde_json::json!({
            "id": id,
            "message": "성공적으로 추가되었습니다."
        })),
        Err(error) => Json(serde_json::json!({ "error": error.to_string() })),
    }
}

pub async fn delete_schedule(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Json<serde_json::Value> {
    match delete_schedule_by_id(&state.db, id).await {
        Ok(rows_affected) if rows_affected > 0 => Json(serde_json::json!({ "message": "삭제되었습니다." })),
        Ok(_) => Json(serde_json::json!({ "message": "삭제할 일정을 찾지 못했습니다." })),
        Err(error) => Json(serde_json::json!({ "error": error.to_string() })),
    }
}
