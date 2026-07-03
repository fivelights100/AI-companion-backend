use axum::{extract::Multipart, http::StatusCode, Json};

use crate::{
    ai::stt::{transcribe_audio as transcribe_with_openai, SttAudioInput, SttServiceError},
    models::stt::SttResponse,
};

pub async fn transcribe_audio(mut multipart: Multipart) -> (StatusCode, Json<SttResponse>) {
    let input = match read_audio_input(&mut multipart).await {
        Ok(input) => input,
        Err(message) => return bad_request(message),
    };

    match transcribe_with_openai(input).await {
        Ok(text) => (StatusCode::OK, Json(SttResponse::ok(text))),
        Err(error) => stt_error_response(error),
    }
}

async fn read_audio_input(multipart: &mut Multipart) -> Result<SttAudioInput, String> {
    let mut audio_bytes: Option<Vec<u8>> = None;
    let mut file_name = "command.webm".to_string();
    let mut language = "ko".to_string();

    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(error) => {
                return Err(format!("multipart 요청을 읽지 못했습니다: {error}"));
            }
        };

        let field_name = field.name().unwrap_or_default().to_string();

        match field_name.as_str() {
            "file" => {
                if let Some(name) = field.file_name() {
                    file_name = name.to_string();
                }

                let bytes = field
                    .bytes()
                    .await
                    .map_err(|error| format!("오디오 파일을 읽지 못했습니다: {error}"))?;

                audio_bytes = Some(bytes.to_vec());
            }
            "language" => {
                if let Ok(value) = field.text().await {
                    let value = value.trim();
                    if !value.is_empty() {
                        language = value.to_string();
                    }
                }
            }
            _ => {}
        }
    }

    let Some(bytes) = audio_bytes else {
        return Err("file 필드가 필요합니다.".to_string());
    };

    if bytes.is_empty() {
        return Err("오디오 파일이 비어 있습니다.".to_string());
    }

    Ok(SttAudioInput {
        bytes,
        file_name,
        language,
    })
}

fn stt_error_response(error: SttServiceError) -> (StatusCode, Json<SttResponse>) {
    let status = match &error {
        SttServiceError::MissingApiKey => StatusCode::INTERNAL_SERVER_ERROR,
        SttServiceError::InvalidAudioPart(_)
        | SttServiceError::RequestFailed(_)
        | SttServiceError::InvalidResponse(_)
        | SttServiceError::ApiError(_) => StatusCode::BAD_GATEWAY,
    };

    (status, Json(SttResponse::error(error.public_message())))
}

fn bad_request(message: String) -> (StatusCode, Json<SttResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(SttResponse::error_message(message)),
    )
}
