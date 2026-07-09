use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemSettings {
    pub enabled: bool,
    pub terms: FilesystemTermsSettings,
    pub permissions: FilesystemPermissions,
    pub safety: FilesystemSafetySettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemTermsSettings {
    pub show_terms_modal: bool,
    pub accepted: bool,
    pub accepted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemPermissions {
    pub search: bool,
    pub modify: bool,
    pub delete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemSafetySettings {
    pub fixed_blocked_paths: Vec<String>,
    pub user_blocked_paths: Vec<String>,
    pub allowed_extensions: Vec<String>,
    pub extension_groups: Vec<FilesystemExtensionGroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemExtensionGroup {
    pub id: String,
    pub label: String,
    pub extensions: Vec<String>,
    pub locked: bool,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FilesystemSettingsUpdateRequest {
    pub enabled: Option<bool>,
    pub terms: Option<FilesystemTermsUpdate>,
    pub permissions: Option<FilesystemPermissionsUpdate>,
    pub safety: Option<FilesystemSafetyUpdate>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FilesystemTermsUpdate {
    pub show_terms_modal: Option<bool>,
    pub accepted: Option<bool>,
    pub accepted_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FilesystemPermissionsUpdate {
    pub modify: Option<bool>,
    pub delete: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FilesystemSafetyUpdate {
    pub user_blocked_paths: Option<Vec<String>>,
    pub allowed_extensions: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FilesystemSettingsResponse {
    pub ok: bool,
    pub message: String,
    pub settings: FilesystemSettings,
}

impl Default for FilesystemSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            terms: FilesystemTermsSettings {
                show_terms_modal: true,
                accepted: false,
                accepted_at: None,
            },
            permissions: FilesystemPermissions {
                search: true,
                modify: false,
                delete: false,
            },
            safety: FilesystemSafetySettings {
                fixed_blocked_paths: default_fixed_blocked_paths(),
                user_blocked_paths: Vec::new(),
                allowed_extensions: vec!["txt".to_string()],
                extension_groups: default_extension_groups(),
            },
        }
    }
}

pub fn default_fixed_blocked_paths() -> Vec<String> {
    vec!["C:\\Windows".to_string()]
}

pub fn default_extension_groups() -> Vec<FilesystemExtensionGroup> {
    vec![
        FilesystemExtensionGroup {
            id: "executable".to_string(),
            label: "실행".to_string(),
            extensions: vec!["exe", "msi", "bat", "cmd", "ps1", "vbs", "scr", "lnk", "dll", "sys", "reg"].into_iter().map(str::to_string).collect(),
            locked: false,
            description: "사용자가 직접 허용 여부를 선택합니다.".to_string(),
        },
        FilesystemExtensionGroup {
            id: "text".to_string(),
            label: "텍스트".to_string(),
            extensions: vec!["txt", "md", "json", "yaml", "yml", "js", "ts", "rs", "py", "html", "css"].into_iter().map(str::to_string).collect(),
            locked: false,
            description: "텍스트/코드 파일".to_string(),
        },
        FilesystemExtensionGroup {
            id: "archive".to_string(),
            label: "압축".to_string(),
            extensions: vec!["zip"].into_iter().map(str::to_string).collect(),
            locked: false,
            description: "압축 파일".to_string(),
        },
        FilesystemExtensionGroup {
            id: "audio".to_string(),
            label: "소리".to_string(),
            extensions: vec!["mp3"].into_iter().map(str::to_string).collect(),
            locked: false,
            description: "오디오 파일".to_string(),
        },
        FilesystemExtensionGroup {
            id: "video".to_string(),
            label: "영상".to_string(),
            extensions: vec!["mp4"].into_iter().map(str::to_string).collect(),
            locked: false,
            description: "비디오 파일".to_string(),
        },
        FilesystemExtensionGroup {
            id: "image".to_string(),
            label: "사진".to_string(),
            extensions: vec!["png", "jpg", "jpeg", "gif"].into_iter().map(str::to_string).collect(),
            locked: false,
            description: "이미지 파일".to_string(),
        },
    ]
}
