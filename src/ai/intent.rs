use reqwest::Client;
use serde_json::{json, Value};

#[derive(Debug, PartialEq)]
pub enum Intent {
    Chat,
    Schedule,
    Ledger,
    FileSearch,
    FileOpen,
}

pub async fn detect_intent(client: &Client, api_key: &str, message: &str) -> Intent {
    if looks_like_file_open_request(message) {
        return Intent::FileOpen;
    }

    let system_prompt = "사용자의 메시지를 읽고 의도를 분류해.\n1. 일정 확인, 달력, 예약, 약속, 회의, 일정 추가/삭제 등 일정과 관련된 내용이면 'Schedule'이라고 대답해.\n2. 가계부, 돈 사용 기록, 지출, 수입, 소비, 결제, 영수증, 정산, 카테고리, 금액 기록과 관련된 내용이면 'Ledger'라고 대답해.\n3. 내 컴퓨터의 파일, 폴더, 경로, 확장자, 문서, 이미지, 프로젝트 파일을 찾아달라는 내용이면 'FileSearch'라고 대답해.\n4. 내 컴퓨터의 폴더나 파일을 열어달라, 실행해달라, 보여달라, 열람해달라, explorer로 열어달라는 내용이면 'FileOpen'이라고 대답해.\n5. 단순 인사, 잡담, 기술적인 질문 등 그 외의 대화면 'Chat'이라고 대답해.\n반드시 'Schedule', 'Ledger', 'FileSearch', 'FileOpen', 'Chat' 다섯 중 하나의 단어만 출력해.";

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
                if content.contains("FileOpen") {
                    return Intent::FileOpen;
                }
                if content.contains("FileSearch") {
                    return Intent::FileSearch;
                }
            }
        }
    }

    Intent::Chat
}


fn looks_like_file_open_request(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    let mentions_file_target = [
        "파일", "폴더", "경로", "문서", "이미지", "사진", "pdf", "txt", "md", "doc",
        "docx", "xls", "xlsx", "ppt", "pptx", "png", "jpg", "jpeg", "gif", "mp3",
        "mp4", "zip", "js", "ts", "rs", "py", "json", "yaml", "html", "css",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword));

    let asks_to_open = [
        "열어", "열기", "열어줘", "열어 줘", "실행", "보여줘", "보여 줘", "열람",
        "explorer", "탐색기", "open", "run",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword));

    mentions_file_target && asks_to_open
}
