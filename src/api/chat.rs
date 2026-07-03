// src/api/chat.rs

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use crate::AppState; // main.rs에 있는 AppState를 가져옵니다.

// (주의: 기존 main.rs에 있던 구조체 이름이 다르면 그에 맞게 수정해 주세요)
#[derive(Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct ChatRequest {
    pub message: String,
    pub history: Vec<ChatMessage>,
}

// 🌟 AI 두뇌 역할을 수행하는 핵심 로직
pub async fn chat_with_ai(
    State(state): State<AppState>,
    Json(payload): Json<ChatRequest>
) -> Json<serde_json::Value> {
    let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
    let client = reqwest::Client::new();

    let intent = crate::ai::router::detect_intent(&client, &api_key, &payload.message).await;
    let system_prompt = crate::ai::prompt::PromptManager::build_system_prompt(&intent);

    let mut messages = vec![serde_json::json!({ "role": "system", "content": system_prompt })];
    for msg in payload.history { 
        messages.push(serde_json::json!({ "role": msg.role, "content": msg.content })); 
    }
    messages.push(serde_json::json!({ "role": "user", "content": payload.message }));

    let tools = crate::ai::tools::get_tools();

    let mut request_body = serde_json::json!({
        "model": "gpt-4o-mini",
        "messages": messages,
    });

    if intent == crate::ai::router::Intent::Schedule {
        if let Some(obj) = request_body.as_object_mut() {
            obj.insert("tools".to_string(), tools);
            obj.insert("tool_choice".to_string(), serde_json::json!("auto"));
        }
    }

    let res = client.post("https://api.openai.com/v1/chat/completions")
        .bearer_auth(&api_key)
        .json(&request_body)
        .send()
        .await;

    if let Ok(response) = res {
        if let Ok(json) = response.json::<serde_json::Value>().await {
            let message = &json["choices"][0]["message"];

            if let Some(tool_calls) = message["tool_calls"].as_array() {
                messages.push(message.clone());
                let mut schedule_changed = false;

                for tool_call in tool_calls {
                    let tool_call_id = tool_call["id"].as_str().unwrap_or("");
                    let name = tool_call["function"]["name"].as_str().unwrap_or("");
                    let args_str = tool_call["function"]["arguments"].as_str().unwrap_or("{}");
                    let args: serde_json::Value = serde_json::from_str(args_str).unwrap_or(serde_json::json!({}));

                    let mut tool_result = String::new();

                    if name == "add_schedule" {
                        let title = args["title"].as_str().unwrap_or("");
                        let date_str = args["event_date"].as_str().unwrap_or("");
                        let time = args["event_time"]
                            .as_str()
                            .and_then(|s| chrono::NaiveTime::parse_from_str(s, "%H:%M:%S").ok());
                        let location = args["location"].as_str();
                        let memo = args["memo"].as_str();
                        
                        if title.is_empty() || date_str.is_empty() {
                            tool_result = "시스템 거절: 필수 정보(제목, 날짜)가 누락되었습니다. 사용자에게 되물어보세요.".to_string();
                        } else {
                            if let Ok(event_date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                                let _ = crate::db::schedules::add_schedule(&state.db, title, event_date, time, location, memo).await;
                                tool_result = format!("일정 '{}' 추가 완료", title);
                                schedule_changed = true; 
                            } else {
                                tool_result = "시스템 거절: 날짜 형식이 잘못되었습니다.".to_string();
                            }
                        }
                    } else if name == "get_schedules" {
                        if let Ok(sch_list) = crate::db::schedules::get_schedules(&state.db).await {
                            tool_result = if sch_list.is_empty() { 
                                "등록된 일정이 없습니다.".to_string() 
                            } else { 
                                sch_list.join("\n") 
                            };
                        }
                    } else if name == "delete_schedule" {
                        let keyword = args["keyword"].as_str().unwrap_or("");
                        if let Ok(rows_affected) = crate::db::schedules::delete_schedule(&state.db, keyword).await {
                            if rows_affected > 0 {
                                tool_result = format!("'{}' 관련 일정 삭제 성공", keyword);
                                schedule_changed = true; 
                            } else {
                                tool_result = format!("'{}' 관련 일정을 찾지 못했습니다.", keyword);
                            }
                        }
                    }

                    messages.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": tool_call_id,
                        "content": tool_result
                    }));
                }

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
                            // 🌟 AI 답변 완성 즉시 백엔드에서 소리 파일까지 구워냅니다.
                            let audio_base64 = crate::ai::tts::text_to_speech(&client, final_reply).await.unwrap_or_default();

                            return Json(serde_json::json!({ 
                                "reply": final_reply,
                                "audio_base64": audio_base64, // 🌟 오디오 데이터 동시 탑재
                                "schedule_updated": schedule_changed 
                            }));
                        }
                    }
                }
            }

            if let Some(reply) = message["content"].as_str() {
                // 🌟 일반 대화 역시 즉시 음성을 합성합니다.
                let audio_base64 = crate::ai::tts::text_to_speech(&client, reply).await.unwrap_or_default();

                return Json(serde_json::json!({ 
                    "reply": reply,
                    "audio_base64": audio_base64, // 🌟 오디오 데이터 동시 탑재
                    "schedule_updated": false 
                }));
            }
        }
    }

    Json(serde_json::json!({ 
        "reply": "앗, 잠깐 통신이 원활하지 않아. 다시 말해줄래?",
        "schedule_updated": false 
    }))
}