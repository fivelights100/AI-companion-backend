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
    models::{
        chat::{ChatRequest, ChatResponse},
        files::{
            FileContentEditCandidatePage, FileCreateCandidatePage, FileDeleteCandidatePage,
            FileOpenCandidatePage, FileRenameCandidatePage, FileTransferPending,
        },
    },
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
        Intent::FileRename => chat_request_body_with_forced_tool(
            messages.clone(),
            get_tools(),
            "prepare_rename_file_or_folder",
        ),
        Intent::FileCreate => chat_request_body_with_forced_tool(
            messages.clone(),
            get_tools(),
            "prepare_create_file_or_folder",
        ),
        Intent::FileContentEdit => chat_request_body_with_forced_tool(
            messages.clone(),
            get_tools(),
            "prepare_edit_file_content",
        ),
        Intent::FileDelete => chat_request_body_with_forced_tool(
            messages.clone(),
            get_tools(),
            "prepare_delete_file_or_folder",
        ),
        Intent::FileTransfer => chat_request_body_with_forced_tool(
            messages.clone(),
            get_tools(),
            "prepare_transfer_file_or_folder",
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
            return response_with_pending(reply, audio_base64, tool_summary.schedule_changed, tool_summary.ledger_changed)
                .with_open_candidates(pending_file_open_candidates);
        }

        if let Some(pending_file_rename_candidates) = tool_summary.pending_file_rename_candidates {
            let reply = build_file_rename_candidates_reply(&pending_file_rename_candidates);
            let audio_base64 = create_tts(openai.http_client(), &reply).await;
            return response_with_pending(reply, audio_base64, tool_summary.schedule_changed, tool_summary.ledger_changed)
                .with_rename_candidates(pending_file_rename_candidates);
        }

        if let Some(pending_file_create_candidates) = tool_summary.pending_file_create_candidates {
            let reply = build_file_create_candidates_reply(&pending_file_create_candidates);
            let audio_base64 = create_tts(openai.http_client(), &reply).await;
            return response_with_pending(reply, audio_base64, tool_summary.schedule_changed, tool_summary.ledger_changed)
                .with_create_candidates(pending_file_create_candidates);
        }

        if let Some(pending_file_content_edit_candidates) = tool_summary.pending_file_content_edit_candidates {
            let reply = build_file_content_edit_candidates_reply(&pending_file_content_edit_candidates);
            let audio_base64 = create_tts(openai.http_client(), &reply).await;
            return response_with_pending(reply, audio_base64, tool_summary.schedule_changed, tool_summary.ledger_changed)
                .with_content_edit_candidates(pending_file_content_edit_candidates);
        }

        if let Some(pending_file_delete_candidates) = tool_summary.pending_file_delete_candidates {
            let reply = build_file_delete_candidates_reply(&pending_file_delete_candidates);
            let audio_base64 = create_tts(openai.http_client(), &reply).await;
            return response_with_pending(reply, audio_base64, tool_summary.schedule_changed, tool_summary.ledger_changed)
                .with_delete_candidates(pending_file_delete_candidates);
        }

        if let Some(pending_file_transfer_candidates) = tool_summary.pending_file_transfer_candidates {
            let reply = build_file_transfer_candidates_reply(&pending_file_transfer_candidates);
            let audio_base64 = create_tts(openai.http_client(), &reply).await;
            return response_with_pending(reply, audio_base64, tool_summary.schedule_changed, tool_summary.ledger_changed)
                .with_transfer_candidates(pending_file_transfer_candidates);
        }

        let second_request = chat_request_body(messages);
        let second_response = match openai.chat_completion(&second_request).await {
            Ok(response) => response,
            Err(error) => {
                eprintln!("🚨 OpenAI 2차 응답 실패: {error}");
                return response_with_pending(
                    FALLBACK_REPLY.to_string(),
                    String::new(),
                    tool_summary.schedule_changed,
                    tool_summary.ledger_changed,
                );
            }
        };

        let reply = extract_reply_text(&second_response).unwrap_or(FALLBACK_REPLY);
        let tts_text = build_tts_text(reply, &intent);
        let audio_base64 = create_tts(openai.http_client(), &tts_text).await;

        return response_with_pending(
            reply.to_string(),
            audio_base64,
            tool_summary.schedule_changed,
            tool_summary.ledger_changed,
        );
    }

    if let Some(reply) = assistant_message["content"].as_str() {
        let tts_text = build_tts_text(reply, &intent);
        let audio_base64 = create_tts(openai.http_client(), &tts_text).await;
        return response_with_pending(reply.to_string(), audio_base64, false, false);
    }

    ChatResponse::fallback(FALLBACK_REPLY)
}

fn response_with_pending(
    reply: String,
    audio_base64: String,
    schedule_updated: bool,
    ledger_updated: bool,
) -> ChatResponse {
    ChatResponse {
        reply,
        audio_base64,
        schedule_updated,
        ledger_updated,
        pending_file_open: None,
        pending_file_open_candidates: None,
        pending_file_rename_candidates: None,
        pending_file_create_candidates: None,
        pending_file_content_edit_candidates: None,
        pending_file_delete_candidates: None,
        pending_file_transfer_candidates: None,
    }
}

trait PendingChatResponseExt {
    fn with_open_candidates(self, page: FileOpenCandidatePage) -> Self;
    fn with_rename_candidates(self, page: FileRenameCandidatePage) -> Self;
    fn with_create_candidates(self, page: FileCreateCandidatePage) -> Self;
    fn with_content_edit_candidates(self, page: FileContentEditCandidatePage) -> Self;
    fn with_delete_candidates(self, page: FileDeleteCandidatePage) -> Self;
    fn with_transfer_candidates(self, pending: FileTransferPending) -> Self;
}

impl PendingChatResponseExt for ChatResponse {
    fn with_open_candidates(mut self, page: FileOpenCandidatePage) -> Self {
        self.pending_file_open_candidates = Some(page);
        self
    }

    fn with_rename_candidates(mut self, page: FileRenameCandidatePage) -> Self {
        self.pending_file_rename_candidates = Some(page);
        self
    }

    fn with_create_candidates(mut self, page: FileCreateCandidatePage) -> Self {
        self.pending_file_create_candidates = Some(page);
        self
    }

    fn with_content_edit_candidates(mut self, page: FileContentEditCandidatePage) -> Self {
        self.pending_file_content_edit_candidates = Some(page);
        self
    }

    fn with_delete_candidates(mut self, page: FileDeleteCandidatePage) -> Self {
        self.pending_file_delete_candidates = Some(page);
        self
    }

    fn with_transfer_candidates(mut self, pending: FileTransferPending) -> Self {
        self.pending_file_transfer_candidates = Some(pending);
        self
    }
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

fn build_file_content_edit_candidates_reply(page: &FileContentEditCandidatePage) -> String {
    if page.has_more {
        "화면에 내용 수정 후보를 띄웠어. 원하는 파일이 없으면 다음 버튼으로 더 볼 수 있어.".to_string()
    } else {
        "화면에 내용 수정 후보를 띄웠어. 원하는 파일을 선택해줘.".to_string()
    }
}

fn build_file_create_candidates_reply(page: &FileCreateCandidatePage) -> String {
    if page.has_more {
        "화면에 생성 위치 후보를 띄웠어. 원하는 위치가 없으면 다음 버튼으로 더 볼 수 있어.".to_string()
    } else {
        "화면에 생성 위치 후보를 띄웠어. 원하는 위치를 선택해줘.".to_string()
    }
}

fn build_file_rename_candidates_reply(page: &FileRenameCandidatePage) -> String {
    if page.has_more {
        "화면에 이름 변경 후보를 띄웠어. 원하는 항목이 없으면 다음 버튼으로 더 볼 수 있어.".to_string()
    } else {
        "화면에 이름 변경 후보를 띄웠어. 원하는 항목을 선택해줘.".to_string()
    }
}

fn build_file_delete_candidates_reply(page: &FileDeleteCandidatePage) -> String {
    if page.has_more {
        "화면에 삭제 후보를 띄웠어. 원하는 항목이 없으면 다음 버튼으로 더 볼 수 있어.".to_string()
    } else {
        "화면에 삭제 후보를 띄웠어. 원하는 항목을 선택해줘.".to_string()
    }
}

fn build_file_transfer_candidates_reply(pending: &FileTransferPending) -> String {
    if pending.source_page.has_more || pending.destination_page.has_more {
        "화면에 복사/이동 후보를 띄웠어. 원본과 위치를 선택해줘. 원하는 항목이 없으면 다음 버튼으로 더 볼 수 있어.".to_string()
    } else {
        "화면에 복사/이동 후보를 띄웠어. 원본과 위치를 선택해줘.".to_string()
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
