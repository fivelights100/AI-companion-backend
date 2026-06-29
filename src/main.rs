use axum::{
    routing::{get, post, delete},
    Router, Json, extract::{State, Path}
};
use sqlx::{postgres::PgPoolOptions, PgPool, FromRow};
use std::net::SocketAddr;
use tower_http::{services::ServeDir, cors::{CorsLayer, Any}};

#[derive(Clone)]
struct AppState {
    db: PgPool,
}

// --- 데이터 구조체들 ---
#[derive(serde::Serialize, FromRow)]
struct Schedule {
    id: i32,
    title: String,
    event_date: chrono::NaiveDate,
    event_time: Option<chrono::NaiveTime>,
    location: Option<String>,
    memo: Option<String>,
}

#[derive(serde::Deserialize)]
struct CreateSchedule {
    title: String,
    event_date: chrono::NaiveDate,
    event_time: Option<chrono::NaiveTime>,
    location: Option<String>,
    memo: Option<String>,
}

// 프론트엔드에서 넘어올 대화 내역 규격
#[derive(serde::Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(serde::Deserialize)]
struct ChatRequest {
    history: Vec<ChatMessage>,
    message: String,
}

// --- 메인 함수 ---
#[tokio::main]
async fn main() {
    // 환경변수(.env) 로드
    dotenvy::dotenv().ok();
    
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL이 설정되지 않았습니다.");
    
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .expect("Neon 데이터베이스 연결에 실패했습니다.");

    let state = AppState { db: pool };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // 🌟 여기서 .with_state(state)가 라우터 끝에 잘 붙어있어야 E0282 에러가 나지 않습니다.
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/schedules", get(get_schedules).post(add_schedule))
        .route("/api/schedules/:id", delete(delete_schedule))
        .route("/api/chat", post(chat_with_ai))
        .nest_service("/models", ServeDir::new("models"))
        .layer(cors)
        .with_state(state); // <-- 핵심입니다!

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("🚀 서버가 포트 3000에서 실행 중입니다...");
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// --- API 핸들러 함수들 ---
async fn health_check() -> &'static str { "OK" }

async fn get_schedules(State(state): State<AppState>) -> Json<Vec<Schedule>> {
    let result = sqlx::query_as::<_, Schedule>(
        "SELECT id, title, event_date, event_time, location, memo FROM schedules ORDER BY event_date ASC"
    )
    .fetch_all(&state.db)
    .await;

    match result {
        Ok(schedules) => Json(schedules),
        Err(e) => {
            eprintln!("DB 조회 에러: {:?}", e);
            Json(vec![])
        }
    }
}

async fn add_schedule(
    State(state): State<AppState>,
    Json(payload): Json<CreateSchedule>,
) -> Json<serde_json::Value> {
    let result = sqlx::query!(
        "INSERT INTO schedules (title, event_date, event_time, location, memo) VALUES ($1, $2, $3, $4, $5) RETURNING id",
        payload.title,
        payload.event_date,
        payload.event_time,
        payload.location,
        payload.memo
    )
    .fetch_one(&state.db)
    .await;

    match result {
        Ok(row) => Json(serde_json::json!({ "id": row.id, "message": "성공적으로 추가되었습니다." })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

async fn delete_schedule(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Json<serde_json::Value> {
    let result = sqlx::query!("DELETE FROM schedules WHERE id = $1", id)
        .execute(&state.db)
        .await;

    match result {
        Ok(_) => Json(serde_json::json!({ "message": "삭제되었습니다." })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

// 🌟 AI 두뇌 역할을 수행하는 핵심 로직
async fn chat_with_ai(
    State(state): State<AppState>,
    Json(payload): Json<ChatRequest>
) -> Json<serde_json::Value> {
    let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
    let client = reqwest::Client::new();

    // 1. 프롬프트 세팅 (오늘 날짜 주입)
    let system_prompt = format!(
        "너는 나의 든든하고 다정한 AI 동반자야. 친근한 반말을 써. \
        오늘 날짜는 {}야. 사용자가 일정을 물어보거나 조작하려 하면 반드시 주어진 도구(tools)를 사용해.",
        chrono::Local::now().format("%Y-%m-%d")
    );

    let mut messages = vec![serde_json::json!({ "role": "system", "content": system_prompt })];
    for msg in payload.history { messages.push(serde_json::json!({ "role": msg.role, "content": msg.content })); }
    messages.push(serde_json::json!({ "role": "user", "content": payload.message }));

    // 🌟 2. AI에게 쥐어줄 3가지 도구(추가, 조회, 삭제) 설명서
    let tools = serde_json::json!([
        {
            "type": "function",
            "function": {
                "name": "add_schedule",
                "description": "새로운 일정을 데이터베이스에 추가합니다.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string" },
                        "event_date": { "type": "string", "description": "YYYY-MM-DD 형식" },
                        "event_time": { "type": "string", "description": "HH:MM:SS 형식" },
                        "location": { "type": "string" }
                    },
                    "required": ["title", "event_date"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "get_schedules",
                "description": "사용자의 앞으로의 일정을 모두 조회해서 가져옵니다. 일정을 물어보면 이 도구를 써서 데이터를 확인하세요."
            }
        },
        {
            "type": "function",
            "function": {
                "name": "delete_schedule",
                "description": "일정을 삭제합니다. 사용자가 말한 제목의 일부(키워드)를 이용해 해당 일정을 찾아 삭제합니다.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "keyword": { "type": "string", "description": "삭제할 일정의 제목이나 키워드 (예: '제주', '회의')" }
                    },
                    "required": ["keyword"]
                }
            }
        }
    ]);

    let request_body = serde_json::json!({
        "model": "gpt-4o-mini",
        "messages": messages,
        "tools": tools,
        "tool_choice": "auto"
    });

    // 3. AI에게 첫 번째 통신 (어떤 도구를 쓸지 판단시키기)
    let res = client.post("https://api.openai.com/v1/chat/completions")
        .bearer_auth(&api_key)
        .json(&request_body)
        .send().await;

    if let Ok(response) = res {
        if let Ok(json) = response.json::<serde_json::Value>().await {
            let message = &json["choices"][0]["message"];

            // 🌟 4. AI가 도구를 사용하겠다고 결정한 경우 (핵심 2-Step 로직)
            if let Some(tool_calls) = message["tool_calls"].as_array() {
                
                // AI의 도구 사용 결정을 대화 기록에 저장
                messages.push(message.clone());

                for tool_call in tool_calls {
                    let tool_call_id = tool_call["id"].as_str().unwrap_or("");
                    let name = tool_call["function"]["name"].as_str().unwrap_or("");
                    let args_str = tool_call["function"]["arguments"].as_str().unwrap_or("{}");
                    let args: serde_json::Value = serde_json::from_str(args_str).unwrap_or(serde_json::json!({}));

                    let mut tool_result = String::new();

                    // 도구별 Rust DB 실행 로직
                    if name == "add_schedule" {
                        let title = args["title"].as_str().unwrap_or("새 일정");
                        let date_str = args["event_date"].as_str().unwrap_or("2026-01-01");
                        if let Ok(event_date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                            let _ = sqlx::query!("INSERT INTO schedules (title, event_date) VALUES ($1, $2)", title, event_date).execute(&state.db).await;
                            tool_result = format!("일정 '{}' 추가 완료", title);
                        }
                    } 
                    else if name == "get_schedules" {
                        let rows = sqlx::query!("SELECT title, event_date, event_time FROM schedules ORDER BY event_date ASC").fetch_all(&state.db).await.unwrap_or(vec![]);
                        let mut sch_list = vec![];
                        for r in rows {
                            let time = r.event_time.map(|t| t.to_string()).unwrap_or_default();
                            sch_list.push(format!("- {} ({} {})", r.title, r.event_date, time));
                        }
                        tool_result = if sch_list.is_empty() { "등록된 일정이 없습니다.".to_string() } else { sch_list.join("\n") };
                    } 
                    else if name == "delete_schedule" {
                        let keyword = args["keyword"].as_str().unwrap_or("");
                        let search_pattern = format!("%{}%", keyword);
                        let result = sqlx::query!("DELETE FROM schedules WHERE title LIKE $1", search_pattern).execute(&state.db).await;
                        
                        match result {
                            Ok(res) if res.rows_affected() > 0 => tool_result = format!("'{}' 관련 일정 삭제 성공", keyword),
                            _ => tool_result = format!("'{}' 관련 일정을 찾지 못했습니다.", keyword),
                        }
                    }

                    // 도구 실행 결과를 대화 기록에 추가하여 AI에게 읽도록 함
                    messages.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": tool_call_id,
                        "content": tool_result
                    }));
                }

                // 🌟 5. 결과를 읽은 AI에게 자연스러운 대답을 만들어 달라고 두 번째 통신 요청!
                let request_body_step2 = serde_json::json!({
                    "model": "gpt-4o-mini",
                    "messages": messages
                });

                let res2 = client.post("https://api.openai.com/v1/chat/completions")
                    .bearer_auth(&api_key)
                    .json(&request_body_step2)
                    .send().await;

                if let Ok(response2) = res2 {
                    if let Ok(json2) = response2.json::<serde_json::Value>().await {
                        if let Some(final_reply) = json2["choices"][0]["message"]["content"].as_str() {
                            return Json(serde_json::json!({ "reply": final_reply }));
                        }
                    }
                }
            }

            // 도구를 사용하지 않는 일상 대화인 경우
            if let Some(reply) = message["content"].as_str() {
                return Json(serde_json::json!({ "reply": reply }));
            }
        }
    }

    Json(serde_json::json!({ "reply": "앗, 잠깐 통신이 원활하지 않아. 다시 말해줄래?" }))
}