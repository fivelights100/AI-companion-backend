use serde_json::{json, Value};

use crate::{
    ai::{
        client::{
            chat_request_body, chat_request_body_with_tools, extract_assistant_message,
            extract_reply_text, OpenAiClient,
        },
        intent::{detect_intent, Intent},
        prompt::PromptManager,
        schedule_tools::run_schedule_tool_calls,
        tools::get_tools,
        tts::text_to_speech,
    },
    models::chat::{ChatRequest, ChatResponse},
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
        Intent::Schedule => chat_request_body_with_tools(messages.clone(), get_tools()),
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
        let tool_summary = run_schedule_tool_calls(&state.db, &assistant_message, &mut messages).await;
        let second_request = chat_request_body(messages);

        let second_response = match openai.chat_completion(&second_request).await {
            Ok(response) => response,
            Err(error) => {
                eprintln!("🚨 OpenAI 2차 응답 실패: {error}");
                return ChatResponse::fallback(FALLBACK_REPLY);
            }
        };

        let reply = extract_reply_text(&second_response).unwrap_or(FALLBACK_REPLY);
        let audio_base64 = create_tts(openai.http_client(), reply).await;

        return ChatResponse {
            reply: reply.to_string(),
            audio_base64,
            schedule_updated: tool_summary.schedule_changed,
        };
    }

    if let Some(reply) = assistant_message["content"].as_str() {
        let audio_base64 = create_tts(openai.http_client(), reply).await;

        return ChatResponse {
            reply: reply.to_string(),
            audio_base64,
            schedule_updated: false,
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

async fn create_tts(client: &reqwest::Client, text: &str) -> String {
    match text_to_speech(client, text).await {
        Ok(audio_base64) => audio_base64,
        Err(error) => {
            eprintln!("🚨 TTS 생성 실패: {error}");
            String::new()
        }
    }
}
