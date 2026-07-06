use serde_json::{json, Value};

use crate::{
    ai::{
        client::{
            chat_request_body, chat_request_body_with_forced_tool, chat_request_body_with_tools,
            extract_assistant_message, extract_reply_text, OpenAiClient,
        },
        intent::{detect_intent, Intent},
        prompt::PromptManager,
        schedule_tools::run_ai_tool_calls,
        speech_sanitizer::build_tts_text,
        tools::get_tools,
        tts::text_to_speech,
    },
    models::{chat::{ChatRequest, ChatResponse}, files::FileOpenCandidatePage},
    state::AppState,
};

const FALLBACK_REPLY: &str = "앗, 잠깐 통신이 원활하지 않아. 다시 말해줄래?";

pub async fn handle_chat(state: &AppState, payload: ChatRequest) -> ChatResponse {
    let http_client = reqwest::Client::new();
    let openai_api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
    let openai = OpenAiClient::from_env(http_client.clone());

    let intent = detect_intent(&http_client, &openai_api_key, &payload.message).await;
    let mut messages = build_messages(&payload, &intent);

    let first_request = match intent {
        Intent::FileOpen => chat_request_body_with_forced_tool(
            messages.clone(),
            get_tools(),
            "prepare_open_file_or_folder",
        ),
        Intent::Schedule | Intent::Ledger | Intent::FileSearch => {
            chat_request_body_with_tools(messages.clone(), get_tools())
        }
        Intent::Chat => chat_request_body(messages.clone()),
    };

    let first_response = match openai.chat_completion(&first_request).await {
        Ok(response) => response,
        Err(error) => {
            eprintln!("🚨 OpenAI 1차 응답 실패: {error}");
            return ChatResponse::fallback(FALLBACK_REPLY);
        }
    };

    let Some(assistant_message) = extract_assistant_message(&first_response) else {
        eprintln!("🚨 OpenAI 1차 응답에 assistant message가 없습니다: {first_response}");
        return ChatResponse::fallback(FALLBACK_REPLY);
    };

    if has_tool_calls(&assistant_message) {
        let tool_summary = run_ai_tool_calls(&state.db, &assistant_message, &mut messages).await;
        if let Some(pending_file_open_candidates) = tool_summary.pending_file_open_candidates {
            let reply = build_file_open_candidates_reply(&pending_file_open_candidates);
            let audio_base64 = create_tts(openai.http_client(), &reply).await;

            return ChatResponse {
                reply,
                audio_base64,
                schedule_updated: tool_summary.schedule_changed,
                ledger_updated: tool_summary.ledger_changed,
                pending_file_open: None,
                pending_file_open_candidates: Some(pending_file_open_candidates),
            };
        }

        let second_request = chat_request_body(messages);

        let second_response = match openai.chat_completion(&second_request).await {
            Ok(response) => response,
            Err(error) => {
                eprintln!("🚨 OpenAI 2차 응답 실패: {error}");
                return ChatResponse {
                    reply: FALLBACK_REPLY.to_string(),
                    audio_base64: String::new(),
                    schedule_updated: tool_summary.schedule_changed,
                    ledger_updated: tool_summary.ledger_changed,
                    pending_file_open: None,
                    pending_file_open_candidates: None,
                };
            }
        };

        let reply = extract_reply_text(&second_response).unwrap_or(FALLBACK_REPLY);
        let tts_text = build_tts_text(reply, &intent);
        let audio_base64 = create_tts(openai.http_client(), &tts_text).await;

        return ChatResponse {
            reply: reply.to_string(),
            audio_base64,
            schedule_updated: tool_summary.schedule_changed,
            ledger_updated: tool_summary.ledger_changed,
            pending_file_open: None,
            pending_file_open_candidates: None,
        };
    }

    if let Some(reply) = assistant_message["content"].as_str() {
        let tts_text = build_tts_text(reply, &intent);
        let audio_base64 = create_tts(openai.http_client(), &tts_text).await;

        return ChatResponse {
            reply: reply.to_string(),
            audio_base64,
            schedule_updated: false,
            ledger_updated: false,
            pending_file_open: None,
            pending_file_open_candidates: None,
        };
    }

    ChatResponse::fallback(FALLBACK_REPLY)
}

fn build_messages(payload: &ChatRequest, intent: &Intent) -> Vec<Value> {
    let mut messages = vec![json!({
        "role": "system",
        "content": PromptManager::build_system_prompt(intent),
    })];

    for message in &payload.history {
        messages.push(json!({
            "role": &message.role,
            "content": &message.content,
        }));
    }

    messages.push(json!({
        "role": "user",
        "content": &payload.message,
    }));

    messages
}

fn has_tool_calls(message: &Value) -> bool {
    message["tool_calls"]
        .as_array()
        .map(|tool_calls| !tool_calls.is_empty())
        .unwrap_or(false)
}


fn build_file_open_candidates_reply(page: &FileOpenCandidatePage) -> String {
    if page.has_more {
        "화면에 후보를 띄웠어. 원하는 항목이 없으면 다음 버튼으로 더 볼 수 있어.".to_string()
    } else {
        "화면에 후보를 띄웠어. 원하는 항목을 선택해서 열어줘.".to_string()
    }
}

async fn create_tts(client: &reqwest::Client, text: &str) -> String {
    match text_to_speech(client, text).await {
        Ok(audio_base64) => audio_base64,
        Err(error) => {
            eprintln!("🚨 TTS 생성 실패: {error}");
            String::new()
        }
    }
}
