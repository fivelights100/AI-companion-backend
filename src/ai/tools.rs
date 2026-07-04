// src/ai/tools.rs

use serde_json::{json, Value};

pub fn get_tools() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": "add_schedule",
                "description": "새로운 일정을 데이터베이스에 추가합니다.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string", "description": "일정의 제목 (예: 회의, 치과 예약, 친구 약속)" },
                        "event_date": { "type": "string", "description": "YYYY-MM-DD 형식의 날짜 (명확하지 않으면 절대 임의로 채우지 말 것)" },
                        "event_time": { "type": "string", "description": "HH:MM:SS 형식의 시간 (선택 사항)" },
                        "location": { "type": "string", "description": "일정 장소 (선택 사항)" },
                        "memo": { "type": "string", "description": "일정에 대한 추가 메모나 설명" }
                    },
                    "required": ["title", "event_date"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "get_schedules",
                "description": "사용자의 앞으로의 일정을 모두 조회해서 가져옵니다."
            }
        },
        {
            "type": "function",
            "function": {
                "name": "delete_schedule",
                "description": "일정을 삭제합니다.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "keyword": { "type": "string", "description": "삭제할 일정의 제목이나 키워드" }
                    },
                    "required": ["keyword"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "add_ledger_entry",
                "description": "사용자의 가계부에 수입 또는 지출 기록을 추가합니다. 필수 정보가 부족하면 이 도구를 호출하지 말고 먼저 사용자에게 되물어봐야 합니다.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "entry_date": { "type": "string", "description": "YYYY-MM-DD 형식의 발생 일자. 오늘/어제 같은 표현은 현재 날짜 기준으로 변환합니다." },
                        "entry_time": { "type": "string", "description": "HH:MM:SS 형식의 발생 시간. 불명확하면 생략합니다." },
                        "entry_type": { "type": "string", "enum": ["income", "expense"], "description": "수입이면 income, 지출이면 expense" },
                        "amount": { "type": "integer", "description": "금액. 원 단위 숫자만 사용합니다." },
                        "category": { "type": "string", "description": "카테고리. 예: 식비, 카페, 교통, 월급, 쇼핑, 문화, 의료, 기타" },
                        "place": { "type": "string", "description": "장소. 예: 강남역, 스타벅스, 회사. 없으면 생략합니다." },
                        "people": { "type": "string", "description": "함께한 사람 또는 정산 대상. 여러 명이면 쉼표로 구분합니다. 없으면 생략합니다." },
                        "is_settled": { "type": "boolean", "description": "정산 완료면 true, 미정산이면 false. 정산과 관련 없거나 불명확하면 생략합니다." },
                        "memo": { "type": "string", "description": "추가 메모. 없으면 생략합니다." }
                    },
                    "required": ["entry_date", "entry_type", "amount", "category"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "get_ledger_entries",
                "description": "최근 가계부 기록을 조회해서 가져옵니다."
            }
        },
        {
            "type": "function",
            "function": {
                "name": "delete_ledger_entry",
                "description": "키워드와 관련된 가계부 기록을 삭제합니다.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "keyword": { "type": "string", "description": "삭제할 가계부 기록의 카테고리, 장소, 인원, 메모 키워드" }
                    },
                    "required": ["keyword"]
                }
            }
        }
    ])
}
