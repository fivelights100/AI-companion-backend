use reqwest::multipart::{Form, Part};
use serde_json::Value;

const OPENAI_AUDIO_TRANSCRIPTIONS_URL: &str = "https://api.openai.com/v1/audio/transcriptions";
const DEFAULT_STT_MODEL: &str = "whisper-1";
const DEFAULT_AUDIO_MIME: &str = "audio/webm";

#[derive(Debug)]
pub struct SttAudioInput {
    pub bytes: Vec<u8>,
    pub file_name: String,
    pub language: String,
}

#[derive(Debug)]
pub enum SttServiceError {
    MissingApiKey,
    InvalidAudioPart(String),
    RequestFailed(String),
    InvalidResponse(String),
    ApiError(Value),
}

impl SttServiceError {
    pub fn public_message(&self) -> Value {
        match self {
            SttServiceError::MissingApiKey => {
                Value::String("OPENAI_API_KEY가 설정되지 않았습니다.".to_string())
            }
            SttServiceError::InvalidAudioPart(message)
            | SttServiceError::RequestFailed(message)
            | SttServiceError::InvalidResponse(message) => Value::String(message.clone()),
            SttServiceError::ApiError(value) => value.clone(),
        }
    }
}

pub async fn transcribe_audio(input: SttAudioInput) -> Result<String, SttServiceError> {
    let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
    if api_key.trim().is_empty() {
        return Err(SttServiceError::MissingApiKey);
    }

    let audio_part = Part::bytes(input.bytes)
        .file_name(input.file_name)
        .mime_str(DEFAULT_AUDIO_MIME)
        .map_err(|error| SttServiceError::InvalidAudioPart(format!(
            "오디오 MIME 설정에 실패했습니다: {error}"
        )))?;

    let form = Form::new()
        .part("file", audio_part)
        .text("model", DEFAULT_STT_MODEL)
        .text("language", input.language);

    let client = reqwest::Client::new();
    let response = client
        .post(OPENAI_AUDIO_TRANSCRIPTIONS_URL)
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|error| SttServiceError::RequestFailed(format!(
            "OpenAI STT 요청에 실패했습니다: {error}"
        )))?;

    let status = response.status();
    let json = response
        .json::<Value>()
        .await
        .map_err(|error| SttServiceError::InvalidResponse(format!(
            "OpenAI STT 응답을 파싱하지 못했습니다: {error}"
        )))?;

    if !status.is_success() {
        return Err(SttServiceError::ApiError(json));
    }

    Ok(json["text"].as_str().unwrap_or_default().to_string())
}
