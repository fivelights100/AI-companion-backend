// src/ai/tools.rs

// OpenAI에게 전달할 도구(Function Calling) 명세서

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
        }
    ])
}