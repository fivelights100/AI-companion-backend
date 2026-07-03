use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize)]
pub struct SttResponse {
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
}

impl SttResponse {
    pub fn ok(text: String) -> Self {
        Self {
            text: Some(text),
            error: None,
        }
    }

    pub fn error(error: Value) -> Self {
        Self {
            text: None,
            error: Some(error),
        }
    }

    pub fn error_message(message: impl Into<String>) -> Self {
        Self::error(Value::String(message.into()))
    }
}
