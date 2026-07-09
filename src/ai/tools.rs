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
        },
        {
            "type": "function",
            "function": {
                "name": "search_files",
                "description": "사용자 컴퓨터의 파일/폴더 이름을 Everything CLI로 읽기 전용 검색합니다. 실행, 수정, 삭제, 이동, 복사 같은 작업은 절대 수행하지 않습니다.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "찾고 싶은 파일/폴더 이름 핵심 키워드. 필수." },
                        "root_path": { "type": "string", "description": "검색을 제한할 폴더 경로. 사용자가 명확히 말한 경우에만 사용." },
                        "extension": { "type": "string", "description": "확장자 필터. 예: pdf, txt, png. 점(.) 없이 작성." },
                        "kind": { "type": "string", "enum": ["any", "file", "folder"], "description": "파일만 찾으면 file, 폴더만 찾으면 folder, 불명확하면 any." },
                        "max_results": { "type": "integer", "minimum": 1, "maximum": 50, "description": "최대 결과 개수. 기본 10." },
                        "match_path": { "type": "boolean", "description": "파일명뿐 아니라 전체 경로에서도 검색어를 찾을지 여부." }
                    },
                    "required": ["query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "prepare_open_file_or_folder",
                "description": "사용자 컴퓨터에서 Everything 검색으로 파일/폴더 위치를 확인한 뒤, 실제 열기 전 데스크탑 확인 팝업을 준비합니다. 이 도구는 절대 즉시 실행/수정/삭제하지 않습니다.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "열고 싶은 파일/폴더 이름 핵심 키워드. 필수." },
                        "root_path": { "type": "string", "description": "검색을 제한할 폴더 경로. 사용자가 명확히 말한 경우에만 사용." },
                        "extension": { "type": "string", "description": "열 파일의 확장자. 예: pdf, txt, png, rs. 점(.) 없이 작성. 폴더 열기에는 생략." },
                        "kind": { "type": "string", "enum": ["any", "file", "folder"], "description": "파일을 열면 file, 폴더를 열면 folder, 불명확하면 any." },
                        "max_results": { "type": "integer", "minimum": 1, "maximum": 50, "description": "후보 확인용 최대 검색 결과 개수. 서버는 후보 팝업에 7개씩 나누어 표시합니다. 기본 50." }
                    },
                    "required": ["query"]
                }
            }
        }
        ,{
            "type": "function",
            "function": {
                "name": "prepare_rename_file_or_folder",
                "description": "사용자 컴퓨터에서 Everything 검색으로 파일/폴더 이름 변경 대상 후보를 찾고, 실제 변경 전 데스크탑 후보 선택 및 변경 전/후 확인 팝업을 준비합니다. 이 도구는 절대 즉시 변경하지 않습니다.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "이름을 변경할 파일/폴더 이름 핵심 키워드. 필수." },
                        "new_name": { "type": "string", "description": "변경 후 파일명 또는 폴더명. 경로나 슬래시 없이 이름만 작성. 파일 대상에서 확장자를 생략하면 서버가 기존 확장자를 자동 보존합니다. 필수." },
                        "root_path": { "type": "string", "description": "검색을 제한할 폴더 경로. 사용자가 명확히 말한 경우에만 사용." },
                        "extension": { "type": "string", "description": "대상 파일의 확장자. 예: pdf, txt, png, rs. 점(.) 없이 작성. 폴더에는 생략." },
                        "kind": { "type": "string", "enum": ["any", "file", "folder"], "description": "파일이면 file, 폴더면 folder, 불명확하면 any." },
                        "max_results": { "type": "integer", "minimum": 1, "maximum": 50, "description": "후보 확인용 최대 검색 결과 개수. 서버는 후보 팝업에 7개씩 나누어 표시합니다. 기본 50." }
                    },
                    "required": ["query", "new_name"]
                }
            }
        }
        ,{
            "type": "function",
            "function": {
                "name": "prepare_edit_file_content",
                "description": "사용자 컴퓨터에서 Everything 검색으로 텍스트/코드 파일 내용 수정 대상 후보를 찾고, 실제 저장 전 데스크탑 후보 선택 및 변경 전/후 비교 팝업을 준비합니다. 이 도구는 절대 즉시 저장/삭제/실행하지 않습니다. 내용 수정 가능한 파일은 txt, md, json, yaml, yml, js, ts, rs, py, html, css로 제한됩니다.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "내용을 수정할 파일 이름 핵심 키워드. 필수." },
                        "instruction": { "type": "string", "description": "사용자가 원하는 수정 지시 전체. 예: 오늘 할 일 한 줄 추가, debug 값을 true로 변경. 필수." },
                        "root_path": { "type": "string", "description": "검색을 제한할 폴더 경로. 사용자가 명확히 말한 경우에만 사용." },
                        "extension": { "type": "string", "description": "대상 파일 확장자. 예: md, txt, json, py. 점(.) 없이 작성." },
                        "max_results": { "type": "integer", "minimum": 1, "maximum": 50, "description": "후보 확인용 최대 검색 결과 개수. 서버는 후보 팝업에 7개씩 나누어 표시합니다. 기본 50." }
                    },
                    "required": ["query", "instruction"]
                }
            }
        }

        ,{
            "type": "function",
            "function": {
                "name": "prepare_create_file_or_folder",
                "description": "사용자 컴퓨터에서 Everything 검색으로 생성 위치 폴더 후보를 찾고, 실제 생성 전 데스크탑 후보 선택 및 생성 내용 확인 팝업을 준비합니다. 이 도구는 절대 즉시 생성/수정/삭제하지 않습니다. 생성 가능한 파일은 txt, md, json, yaml, yml, js, ts, rs, py, html, css로 제한됩니다.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "생성 위치로 사용할 폴더 이름 또는 위치 키워드. 예: 바탕화면, 다운로드, 문서, 프로젝트 폴더. 필수." },
                        "name": { "type": "string", "description": "생성할 파일명 또는 폴더명. 경로나 슬래시 없이 이름만 작성. 파일 생성 시 확장자를 반드시 포함. 필수." },
                        "kind": { "type": "string", "enum": ["file", "folder"], "description": "파일 생성이면 file, 폴더 생성이면 folder." },
                        "content": { "type": "string", "description": "파일 생성 시 넣을 텍스트 내용. 폴더 생성에는 비움. 파일 내용은 2MB 이하." },
                        "root_path": { "type": "string", "description": "검색을 제한할 폴더 경로. 사용자가 명확히 말한 경우에만 사용." },
                        "max_results": { "type": "integer", "minimum": 1, "maximum": 50, "description": "후보 확인용 최대 검색 결과 개수. 서버는 후보 팝업에 7개씩 나누어 표시합니다. 기본 50." }
                    },
                    "required": ["query", "name", "kind"]
                }
            }
        }
        ,{
            "type": "function",
            "function": {
                "name": "prepare_delete_file_or_folder",
                "description": "사용자 컴퓨터에서 Everything 검색으로 삭제 대상 파일/폴더 후보를 찾고, 실제 삭제 전 데스크탑 후보 선택 및 휴지통 이동 확인 팝업을 준비합니다. 이 도구는 절대 즉시 삭제하지 않습니다. 삭제는 영구 삭제가 아니라 휴지통 이동으로만 수행됩니다.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "삭제할 파일/폴더 이름 핵심 키워드. 필수." },
                        "root_path": { "type": "string", "description": "검색을 제한할 폴더 경로. 사용자가 명확히 말한 경우에만 사용." },
                        "extension": { "type": "string", "description": "대상 파일의 확장자. 예: txt, md, png. 점(.) 없이 작성. 폴더에는 생략." },
                        "kind": { "type": "string", "enum": ["any", "file", "folder"], "description": "파일이면 file, 폴더면 folder, 불명확하면 any." },
                        "max_results": { "type": "integer", "minimum": 1, "maximum": 50, "description": "후보 확인용 최대 검색 결과 개수. 서버는 후보 팝업에 7개씩 나누어 표시합니다. 기본 50." }
                    },
                    "required": ["query"]
                }
            }
        }
        ,{
            "type": "function",
            "function": {
                "name": "prepare_transfer_file_or_folder",
                "description": "사용자 컴퓨터에서 Everything 검색으로 복사/이동할 원본과 목적지 폴더 후보를 찾고, 실제 복사/이동 전 데스크탑 후보 선택 및 확인 팝업을 준비합니다. 이 도구는 절대 즉시 복사/이동하지 않습니다. 이동은 현재 같은 드라이브 안에서만 허용됩니다.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "operation": { "type": "string", "enum": ["copy", "move"], "description": "복사 요청이면 copy, 이동/옮기기 요청이면 move." },
                        "source_query": { "type": "string", "description": "복사/이동할 파일 또는 폴더 이름 핵심 키워드. 필수." },
                        "destination_query": { "type": "string", "description": "목적지 폴더 이름 또는 위치 키워드. 예: 바탕화면, 다운로드, 문서, 프로젝트 폴더. 필수." },
                        "root_path": { "type": "string", "description": "원본/목적지 검색을 제한할 폴더 경로. 사용자가 명확히 말한 경우에만 사용." },
                        "extension": { "type": "string", "description": "원본 파일 확장자. 예: txt, md, png. 점(.) 없이 작성. 폴더에는 생략." },
                        "kind": { "type": "string", "enum": ["any", "file", "folder"], "description": "원본이 파일이면 file, 폴더면 folder, 불명확하면 any." },
                        "max_results": { "type": "integer", "minimum": 1, "maximum": 50, "description": "후보 확인용 최대 검색 결과 개수. 서버는 후보 팝업에 7개씩 나누어 표시합니다. 기본 50." }
                    },
                    "required": ["operation", "source_query", "destination_query"]
                }
            }
        }




    ])
}
