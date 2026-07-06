use reqwest::Client;
use serde_json::{json, Value};

pub const DEFAULT_CHAT_MODEL: &str = "gpt-4o-mini";
const OPENAI_CHAT_COMPLETIONS_URL: &str = "https://api.openai.com/v1/chat/completions";

#[derive(Clone)]
pub struct OpenAiClient {
    client: Client,
    api_key: String,
}

impl OpenAiClient {
    pub fn from_env(client: Client) -> Self {
        Self {
            client,
            api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
        }
    }

    pub fn http_client(&self) -> &Client {
        &self.client
    }

    pub async fn chat_completion(&self, request_body: &Value) -> Result<Value, String> {
        if self.api_key.trim().is_empty() {
            return Err("OPENAI_API_KEY가 설정되지 않았습니다.".to_string());
        }

        let response = self
            .client
            .post(OPENAI_CHAT_COMPLETIONS_URL)
            .bearer_auth(&self.api_key)
            .json(request_body)
            .send()
            .await
            .map_err(|error| format!("OpenAI 요청 실패: {error}"))?;

        let status = response.status();
        let body_text = response
            .text()
            .await
            .map_err(|error| format!("OpenAI 응답 본문 읽기 실패: {error}"))?;

        if !status.is_success() {
            return Err(format!("OpenAI 응답 오류({status}): {body_text}"));
        }

        serde_json::from_str::<Value>(&body_text)
            .map_err(|error| format!("OpenAI 응답 JSON 파싱 실패: {error}"))
    }
}

pub fn chat_request_body(messages: Vec<Value>) -> Value {
    json!({
        "model": DEFAULT_CHAT_MODEL,
        "messages": messages,
    })
}

pub fn chat_request_body_with_tools(messages: Vec<Value>, tools: Value) -> Value {
    json!({
        "model": DEFAULT_CHAT_MODEL,
        "messages": messages,
        "tools": tools,
        "tool_choice": "auto",
    })
}


pub fn chat_request_body_with_forced_tool(messages: Vec<Value>, tools: Value, tool_name: &str) -> Value {
    json!({
        "model": DEFAULT_CHAT_MODEL,
        "messages": messages,
        "tools": tools,
        "tool_choice": {
            "type": "function",
            "function": { "name": tool_name }
        },
    })
}

pub fn extract_assistant_message(response: &Value) -> Option<Value> {
    response["choices"][0]["message"].as_object()?;
    Some(response["choices"][0]["message"].clone())
}

pub fn extract_reply_text(response: &Value) -> Option<&str> {
    response["choices"][0]["message"]["content"].as_str()
}
