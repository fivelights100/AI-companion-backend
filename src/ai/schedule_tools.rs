use chrono::{NaiveDate, NaiveTime};
use serde_json::{json, Value};
use sqlx::PgPool;

#[derive(Debug, Default)]
pub struct ToolExecutionSummary {
    pub schedule_changed: bool,
    pub ledger_changed: bool,
}

pub async fn run_schedule_tool_calls(
    db: &PgPool,
    assistant_message: &Value,
    messages: &mut Vec<Value>,
) -> ToolExecutionSummary {
    let Some(tool_calls) = assistant_message["tool_calls"].as_array() else {
        return ToolExecutionSummary::default();
    };

    messages.push(assistant_message.clone());

    let mut summary = ToolExecutionSummary::default();

    for tool_call in tool_calls {
        let tool_call_id = tool_call["id"].as_str().unwrap_or_default();
        let name = tool_call["function"]["name"].as_str().unwrap_or_default();
        let args = parse_tool_arguments(tool_call);

        let tool_result = match name {
            "add_schedule" => {
                let result = add_schedule_from_args(db, &args).await;
                if result.changed {
                    summary.schedule_changed = true;
                }
                result.message
            }
            "get_schedules" => get_schedules_for_ai(db).await,
            "delete_schedule" => {
                let result = delete_schedule_from_args(db, &args).await;
                if result.changed {
                    summary.schedule_changed = true;
                }
                result.message
            }
            "add_ledger_entry" => {
                let result = add_ledger_entry_from_args(db, &args).await;
                if result.changed {
                    summary.ledger_changed = true;
                }
                result.message
            }
            "get_ledger_entries" => get_ledger_entries_for_ai(db).await,
            "delete_ledger_entry" => {
                let result = delete_ledger_entry_from_args(db, &args).await;
                if result.changed {
                    summary.ledger_changed = true;
                }
                result.message
            }
            other => format!("시스템 거절: 알 수 없는 도구입니다: {other}"),
        };

        messages.push(json!({
            "role": "tool",
            "tool_call_id": tool_call_id,
            "content": tool_result,
        }));
    }

    summary
}

#[derive(Debug)]
struct ToolResult {
    message: String,
    changed: bool,
}

fn parse_tool_arguments(tool_call: &Value) -> Value {
    let args_str = tool_call["function"]["arguments"]
        .as_str()
        .unwrap_or("{}");

    serde_json::from_str(args_str).unwrap_or_else(|_| json!({}))
}

async fn add_schedule_from_args(db: &PgPool, args: &Value) -> ToolResult {
    let title = args["title"].as_str().unwrap_or_default().trim();
    let date_str = args["event_date"].as_str().unwrap_or_default().trim();

    if title.is_empty() || date_str.is_empty() {
        return ToolResult {
            message: "시스템 거절: 필수 정보(제목, 날짜)가 누락되었습니다. 사용자에게 되물어보세요.".to_string(),
            changed: false,
        };
    }

    let Ok(event_date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") else {
        return ToolResult {
            message: "시스템 거절: 날짜 형식이 잘못되었습니다. YYYY-MM-DD 형식이 필요합니다.".to_string(),
            changed: false,
        };
    };

    let event_time = args["event_time"]
        .as_str()
        .and_then(parse_optional_time);
    let location = args["location"].as_str().filter(|value| !value.trim().is_empty());
    let memo = args["memo"].as_str().filter(|value| !value.trim().is_empty());

    match crate::db::schedules::add_schedule(db, title, event_date, event_time, location, memo).await {
        Ok(_) => ToolResult {
            message: format!("일정 '{title}' 추가 완료"),
            changed: true,
        },
        Err(error) => ToolResult {
            message: format!("시스템 오류: 일정 추가 실패: {error}"),
            changed: false,
        },
    }
}

async fn get_schedules_for_ai(db: &PgPool) -> String {
    match crate::db::schedules::get_schedules(db).await {
        Ok(schedules) if schedules.is_empty() => "등록된 일정이 없습니다.".to_string(),
        Ok(schedules) => schedules.join("\n"),
        Err(error) => format!("시스템 오류: 일정 조회 실패: {error}"),
    }
}

async fn delete_schedule_from_args(db: &PgPool, args: &Value) -> ToolResult {
    let keyword = args["keyword"].as_str().unwrap_or_default().trim();

    if keyword.is_empty() {
        return ToolResult {
            message: "시스템 거절: 삭제할 일정 키워드가 비어 있습니다.".to_string(),
            changed: false,
        };
    }

    match crate::db::schedules::delete_schedule(db, keyword).await {
        Ok(rows_affected) if rows_affected > 0 => ToolResult {
            message: format!("'{keyword}' 관련 일정 삭제 성공"),
            changed: true,
        },
        Ok(_) => ToolResult {
            message: format!("'{keyword}' 관련 일정을 찾지 못했습니다."),
            changed: false,
        },
        Err(error) => ToolResult {
            message: format!("시스템 오류: 일정 삭제 실패: {error}"),
            changed: false,
        },
    }
}



async fn add_ledger_entry_from_args(db: &PgPool, args: &Value) -> ToolResult {
    let date_str = args["entry_date"].as_str().unwrap_or_default().trim();
    let entry_type = args["entry_type"].as_str().unwrap_or_default().trim();
    let amount = args["amount"].as_i64().unwrap_or(-1);
    let category = args["category"].as_str().unwrap_or_default().trim();

    if date_str.is_empty() || entry_type.is_empty() || amount < 0 || category.is_empty() {
        return ToolResult {
            message: "시스템 거절: 필수 정보(일자, 수입/지출, 금액, 카테고리)가 누락되었습니다. 사용자에게 되물어보세요.".to_string(),
            changed: false,
        };
    }

    if entry_type != "income" && entry_type != "expense" {
        return ToolResult {
            message: "시스템 거절: entry_type은 income 또는 expense만 사용할 수 있습니다.".to_string(),
            changed: false,
        };
    }

    let Ok(entry_date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") else {
        return ToolResult {
            message: "시스템 거절: 날짜 형식이 잘못되었습니다. YYYY-MM-DD 형식이 필요합니다.".to_string(),
            changed: false,
        };
    };

    let entry_time = args["entry_time"]
        .as_str()
        .and_then(parse_optional_time);
    let place = args["place"].as_str().filter(|value| !value.trim().is_empty());
    let people = args["people"].as_str().filter(|value| !value.trim().is_empty());
    let is_settled = args["is_settled"].as_bool();
    let memo = args["memo"].as_str().filter(|value| !value.trim().is_empty());

    match crate::db::ledger::add_ledger_entry(
        db,
        entry_date,
        entry_time,
        entry_type,
        amount,
        category,
        place,
        people,
        is_settled,
        memo,
    ).await {
        Ok(_) => ToolResult {
            message: format!("가계부 기록 추가 완료: {entry_date} {category} {amount}원"),
            changed: true,
        },
        Err(error) => ToolResult {
            message: format!("시스템 오류: 가계부 기록 추가 실패: {error}"),
            changed: false,
        },
    }
}

async fn get_ledger_entries_for_ai(db: &PgPool) -> String {
    match crate::db::ledger::get_ledger_entries(db).await {
        Ok(entries) if entries.is_empty() => "등록된 가계부 기록이 없습니다.".to_string(),
        Ok(entries) => entries.join("\n"),
        Err(error) => format!("시스템 오류: 가계부 조회 실패: {error}"),
    }
}

async fn delete_ledger_entry_from_args(db: &PgPool, args: &Value) -> ToolResult {
    let keyword = args["keyword"].as_str().unwrap_or_default().trim();

    if keyword.is_empty() {
        return ToolResult {
            message: "시스템 거절: 삭제할 가계부 키워드가 비어 있습니다.".to_string(),
            changed: false,
        };
    }

    match crate::db::ledger::delete_ledger_entry(db, keyword).await {
        Ok(rows_affected) if rows_affected > 0 => ToolResult {
            message: format!("'{keyword}' 관련 가계부 기록 삭제 성공"),
            changed: true,
        },
        Ok(_) => ToolResult {
            message: format!("'{keyword}' 관련 가계부 기록을 찾지 못했습니다."),
            changed: false,
        },
        Err(error) => ToolResult {
            message: format!("시스템 오류: 가계부 삭제 실패: {error}"),
            changed: false,
        },
    }
}

fn parse_optional_time(value: &str) -> Option<NaiveTime> {
    let value = value.trim();

    if value.is_empty() {
        return None;
    }

    NaiveTime::parse_from_str(value, "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(value, "%H:%M"))
        .ok()
}
