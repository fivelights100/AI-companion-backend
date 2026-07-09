use reqwest::Client;
use serde_json::{json, Value};

#[derive(Debug, PartialEq)]
pub enum Intent {
    Chat,
    Schedule,
    Ledger,
    FileSearch,
    FileOpen,
    FileRename,
    FileCreate,
    FileContentEdit,
    FileDelete,
    FileTransfer,
}

pub async fn detect_intent(client: &Client, api_key: &str, message: &str) -> Intent {
    if looks_like_file_transfer_request(message) {
        return Intent::FileTransfer;
    }

    if looks_like_file_delete_request(message) {
        return Intent::FileDelete;
    }

    if looks_like_file_content_edit_request(message) {
        return Intent::FileContentEdit;
    }

    if looks_like_file_create_request(message) {
        return Intent::FileCreate;
    }

    if looks_like_file_rename_request(message) {
        return Intent::FileRename;
    }

    if looks_like_file_open_request(message) {
        return Intent::FileOpen;
    }

    let system_prompt = "사용자의 메시지를 읽고 의도를 분류해.
1. 일정 확인, 달력, 예약, 약속, 회의, 일정 추가/삭제 등 일정과 관련된 내용이면 'Schedule'이라고 대답해.
2. 가계부, 돈 사용 기록, 지출, 수입, 소비, 결제, 영수증, 정산, 카테고리, 금액 기록과 관련된 내용이면 'Ledger'이라고 대답해.
3. 내 컴퓨터의 파일, 폴더, 경로, 확장자, 문서, 이미지, 프로젝트 파일을 찾아달라는 내용이면 'FileSearch'이라고 대답해.
4. 내 컴퓨터의 폴더나 파일을 열어달라, 실행해달라, 보여달라, 열람해달라, explorer로 열어달라는 내용이면 'FileOpen'이라고 대답해.
5. 내 컴퓨터의 폴더나 파일 이름을 변경/이름 바꾸기/rename 해달라는 내용이면 'FileRename'이라고 대답해.
6. 내 컴퓨터의 특정 위치에 새 파일이나 새 폴더를 생성/만들기 해달라는 내용이면 'FileCreate'라고 대답해.
7. 내 컴퓨터의 텍스트/코드 파일 내용을 수정/추가/변경해달라는 내용이면 'FileContentEdit'이라고 대답해.
8. 내 컴퓨터의 파일이나 폴더를 다른 폴더로 복사/copy/이동/move/옮겨달라는 내용이면 'FileTransfer'라고 대답해.
9. 내 컴퓨터의 파일이나 폴더 자체를 삭제/지우기/휴지통으로 이동해달라는 내용이면 'FileDelete'이라고 대답해.
10. 단순 인사, 잡담, 기술적인 질문 등 그 외의 대화면 'Chat'이라고 대답해.
반드시 'Schedule', 'Ledger', 'FileSearch', 'FileOpen', 'FileRename', 'FileCreate', 'FileContentEdit', 'FileDelete', 'FileTransfer', 'Chat' 열 개 중 하나의 단어만 출력해.";

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
                if content.contains("FileTransfer") {
                    return Intent::FileTransfer;
                }
                if content.contains("FileDelete") {
                    return Intent::FileDelete;
                }
                if content.contains("FileOpen") {
                    return Intent::FileOpen;
                }
                if content.contains("FileContentEdit") {
                    return Intent::FileContentEdit;
                }
                if content.contains("FileCreate") {
                    return Intent::FileCreate;
                }
                if content.contains("FileRename") {
                    return Intent::FileRename;
                }
                if content.contains("FileSearch") {
                    return Intent::FileSearch;
                }
            }
        }
    }

    Intent::Chat
}




fn looks_like_file_transfer_request(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();

    let mentions_file_target = [
        "파일", "폴더", "디렉터리", "디렉토리", "경로", "문서", "이미지", "사진",
        "pdf", "txt", "md", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "png",
        "jpg", "jpeg", "gif", "mp3", "mp4", "zip", "js", "ts", "rs", "py",
        "json", "yaml", "yml", "html", "css",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword));

    let mentions_destination = [
        "로 복사", "에 복사", "로 이동", "에 이동", "로 옮", "에 옮", "바탕화면", "데스크탑",
        "desktop", "다운로드", "downloads", "문서", "documents", "위치", "목적지",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword));

    let asks_to_transfer = [
        "복사", "복제", "카피", "copy", "duplicate", "사본", "이동", "옮겨", "옮기", "move",
        "relocate",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword));

    mentions_file_target && asks_to_transfer && mentions_destination
}

fn looks_like_file_delete_request(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();

    // 파일 내용 일부를 지우라는 요청은 파일 자체 삭제가 아니라 내용 수정 쪽으로 보낸다.
    let looks_like_content_delete = ["내용", "본문", "문구", "줄", "라인", "텍스트에서", "코드에서", "json에서", "md에서"]
        .iter()
        .any(|keyword| lower.contains(keyword));
    let explicitly_whole_target = ["파일", "폴더", "디렉터리", "디렉토리", "문서", "이미지", "사진", "압축", "zip"]
        .iter()
        .any(|keyword| lower.contains(keyword));

    if looks_like_content_delete && !explicitly_whole_target {
        return false;
    }

    let mentions_file_target = [
        "파일", "폴더", "디렉터리", "디렉토리", "경로", "문서", "이미지", "사진",
        "pdf", "txt", "md", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "png",
        "jpg", "jpeg", "gif", "mp3", "mp4", "zip", "js", "ts", "rs", "py",
        "json", "yaml", "yml", "html", "css",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword));

    let asks_to_delete = [
        "삭제", "삭제해", "삭제해줘", "삭제해 줘", "지워", "지워줘", "지워 줘",
        "제거", "제거해", "제거해줘", "없애", "없애줘", "휴지통", "delete",
        "remove", "trash",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword));

    mentions_file_target && asks_to_delete
}

fn looks_like_file_create_request(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();

    let mentions_file_target = [
        "파일", "폴더", "디렉터리", "디렉토리", "문서", "메모", "텍스트", "코드",
        "txt", "md", "json", "yaml", "yml", "js", "ts", "rs", "py", "html", "css",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword));

    let mentions_location = [
        "바탕화면", "데스크탑", "desktop", "다운로드", "download", "downloads", "문서",
        "documents", "폴더에", "경로에", "위치에",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword));

    let asks_to_create = [
        "생성", "만들", "만들어", "만들어줘", "만들어 줘", "새 파일", "새파일",
        "새 폴더", "새폴더", "작성", "create", "make", "new file", "new folder",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword));

    asks_to_create && (mentions_file_target || mentions_location)
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


fn looks_like_file_rename_request(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    let mentions_file_target = [
        "파일", "폴더", "경로", "문서", "이미지", "사진", "pdf", "txt", "md", "doc",
        "docx", "xls", "xlsx", "ppt", "pptx", "png", "jpg", "jpeg", "gif", "mp3",
        "mp4", "zip", "js", "ts", "rs", "py", "json", "yaml", "html", "css",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword));

    let asks_to_rename = [
        "이름 변경", "이름변경", "이름 바", "이름을 바", "이름 바꿔", "이름을 바꿔",
        "이름 바꾸", "이름을 바꾸", "파일명", "폴더명", "rename", "리네임",
        "변경해", "변경 해", "바꿔줘", "바꿔 줘",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword));

    mentions_file_target && asks_to_rename
}

fn looks_like_file_content_edit_request(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();

    // 생성/이름 변경 요청은 "적어줘/바꿔줘/변경" 같은 표현이 겹치므로 내용 수정으로 잡지 않는다.
    if [
        "생성", "만들", "새 파일", "새파일", "새 폴더", "새폴더", "create", "make",
        "new file", "new folder", "이름", "파일명", "폴더명", "rename", "리네임",
    ]
        .iter()
        .any(|keyword| lower.contains(keyword))
    {
        return false;
    }

    let mentions_text_file_target = [
        "txt", "md", "json", "yaml", "yml", "js", "ts", "rs", "py", "html", "css",
        "메모", "텍스트", "코드",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword));

    let mentions_content = ["내용", "본문", "문구", "줄", "텍스트", "코드"]
        .iter()
        .any(|keyword| lower.contains(keyword));

    let asks_to_edit_content = [
        "내용 수정", "내용을 수정", "내용 바", "내용을 바", "내용 변경", "내용을 변경",
        "추가해", "추가 해", "추가해줘", "추가해 줘", "적어줘", "적어 줘",
        "써줘", "써 줘", "수정해", "수정 해", "수정해줘", "수정해 줘",
        "바꿔줘", "바꿔 줘", "변경해", "변경 해", "고쳐줘", "고쳐 줘",
        "replace", "append", "add", "edit", "modify", "update",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword));

    asks_to_edit_content && (mentions_text_file_target || mentions_content)
}
