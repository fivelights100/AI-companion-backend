use reqwest::Client;
use serde_json::{json, Value};

#[derive(Debug, PartialEq)]
pub enum Intent {
    Chat,
    Schedule,
    Ledger,
}

pub async fn detect_intent(client: &Client, api_key: &str, message: &str) -> Intent {
    let system_prompt = "사용자의 메시지를 읽고 의도를 분류해.\n1. 일정 확인, 달력, 예약, 약속, 회의, 일정 추가/삭제 등 일정과 관련된 내용이면 'Schedule'이라고 대답해.\n2. 가계부, 돈 사용 기록, 지출, 수입, 소비, 결제, 영수증, 정산, 카테고리, 금액 기록과 관련된 내용이면 'Ledger'라고 대답해.\n3. 단순 인사, 잡담, 기술적인 질문 등 그 외의 대화면 'Chat'이라고 대답해.\n반드시 'Schedule', 'Ledger', 'Chat' 셋 중 하나의 단어만 출력해.";

    let request_body = json!({
        "model": "gpt-4o-mini",
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": message}
        ],
        "temperature": 0.0,
        "max_tokens": 10
    });

    if api_key.trim().is_empty() {
        return Intent::Chat;
    }

    if let Ok(res) = client
        .post("https://api.openai.com/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&request_body)
        .send()
        .await
    {
        if let Ok(json) = res.json::<Value>().await {
            if let Some(content) = json["choices"][0]["message"]["content"].as_str() {
                let content = content.trim();
                if content.contains("Ledger") {
                    return Intent::Ledger;
                }
                if content.contains("Schedule") {
                    return Intent::Schedule;
                }
            }
        }
    }

    Intent::Chat
}
