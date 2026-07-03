use base64::{engine::general_purpose, Engine as _};
use reqwest::Client;
use serde_json::json;

const ELEVENLABS_TTS_MODEL: &str = "eleven_multilingual_v2";

pub async fn text_to_speech(client: &Client, text: &str) -> Result<String, String> {
    let api_key = std::env::var("ELEVENLABS_API_KEY").unwrap_or_default();
    let voice_id = std::env::var("ELEVENLABS_VOICE_ID").unwrap_or_default();

    if api_key.trim().is_empty() || voice_id.trim().is_empty() {
        return Ok(String::new());
    }

    let url = format!("https://api.elevenlabs.io/v1/text-to-speech/{voice_id}");
    let body = json!({
        "text": text,
        "model_id": ELEVENLABS_TTS_MODEL,
        "voice_settings": {
            "stability": 0.5,
            "similarity_boost": 0.75
        }
    });

    let response = client
        .post(&url)
        .header("xi-api-key", api_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|error| format!("ElevenLabs 요청 실패: {error}"))?;

    let status = response.status();

    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("ElevenLabs 응답 오류({status}): {error_text}"));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("ElevenLabs 응답 읽기 실패: {error}"))?;

    Ok(general_purpose::STANDARD.encode(bytes))
}
