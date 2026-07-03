// src/ai/router.rs

// 사용자의 의도를 파악하는 라우터(문지기)

use reqwest::Client;
use serde_json::{json, Value};

#[derive(Debug, PartialEq)]
pub enum Intent {
    Chat,
    Schedule,
}

pub async fn detect_intent(client: &Client, api_key: &str, message: &str) -> Intent {
    let system_prompt = "사용자의 메시지를 읽고 의도를 분류해.\n1. 일정 확인, 달력, 예약, 추가, 삭제 등 일정과 관련된 내용이면 'Schedule'이라고 대답해.\n2. 단순 인사, 잡담, 기술적인 질문 등 그 외의 대화면 'Chat'이라고 대답해.\n반드시 'Schedule' 또는 'Chat' 둘 중 하나의 단어만 출력해.";

    let request_body = json!({
        "model": "gpt-4o-mini",
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": message}
        ],
        "temperature": 0.0,
        "max_tokens": 10
    });

    if let Ok(res) = client.post("https://api.openai.com/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&request_body)
        .send().await 
    {
        if let Ok(json) = res.json::<Value>().await {
            if let Some(content) = json["choices"][0]["message"]["content"].as_str() {
                if content.trim().contains("Schedule") {
                    return Intent::Schedule;
                }
            }
        }
    }
    
    Intent::Chat
}