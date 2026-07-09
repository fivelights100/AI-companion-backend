use crate::ai::intent::Intent;

pub fn build_tts_text(reply: &str, intent: &Intent) -> String {
    match intent {
        Intent::FileSearch => sanitize_for_file_context(
            reply,
            "검색 결과를 화면에 정리해뒀어. 필요한 항목이 있으면 다시 말해줘.",
        ),
        Intent::FileOpen => sanitize_for_file_context(
            reply,
            "파일이나 폴더 열기 요청을 확인했어. 화면의 안내를 보고 필요한 대상을 더 구체적으로 말해줘.",
        ),
        Intent::FileRename => sanitize_for_file_context(
            reply,
            "파일이나 폴더 이름 변경 요청을 확인했어. 화면의 안내를 보고 진행해줘.",
        ),
        Intent::FileCreate => sanitize_for_file_context(
            reply,
            "파일이나 폴더 생성 요청을 확인했어. 화면의 안내를 보고 진행해줘.",
        ),
        Intent::FileContentEdit => sanitize_for_file_context(
            reply,
            "파일 내용 수정 요청을 확인했어. 화면의 안내를 보고 진행해줘.",
        ),
        Intent::FileDelete => sanitize_for_file_context(
            reply,
            "파일이나 폴더 삭제 요청을 확인했어. 화면의 안내를 보고 진행해줘.",
        ),
        Intent::FileTransfer => sanitize_for_file_context(
            reply,
            "파일이나 폴더 복사/이동 요청을 확인했어. 화면의 안내를 보고 진행해줘.",
        ),
        _ => reply.to_string(),
    }
}

fn sanitize_for_file_context(reply: &str, heavy_fallback: &str) -> String {
    if looks_like_file_or_path_heavy_reply(reply) {
        return heavy_fallback.to_string();
    }

    remove_path_like_lines(reply)
}

fn looks_like_file_or_path_heavy_reply(reply: &str) -> bool {
    let lower = reply.to_ascii_lowercase();
    let path_markers = [":\\", ":/", "\\", "경로:", "위치:", "파일명:", "폴더명:", "c:", "d:", "e:"];
    let extension_markers = [
        ".pdf", ".txt", ".md", ".doc", ".docx", ".xls", ".xlsx", ".ppt", ".pptx",
        ".png", ".jpg", ".jpeg", ".gif", ".mp3", ".mp4", ".zip", ".js", ".ts",
        ".rs", ".py", ".json", ".yaml", ".yml", ".html", ".css",
        ".exe", ".msi", ".bat", ".cmd", ".ps1", ".vbs", ".scr", ".lnk", ".dll", ".sys", ".reg",
    ];

    let path_hits = path_markers.iter().filter(|marker| lower.contains(*marker)).count();
    let extension_hits = extension_markers.iter().filter(|marker| lower.contains(*marker)).count();

    path_hits > 0 || extension_hits >= 2 || reply.lines().count() >= 4 && extension_hits > 0
}

fn remove_path_like_lines(reply: &str) -> String {
    let cleaned = reply
        .lines()
        .filter(|line| !looks_like_path_line(line))
        .map(remove_inline_code)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    if cleaned.is_empty() {
        "화면에 결과를 정리해뒀어.".to_string()
    } else {
        cleaned
    }
}

fn looks_like_path_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains(":\\")
        || lower.contains(":/")
        || lower.contains("경로:")
        || lower.contains("위치:")
        || lower.contains("파일명:")
        || lower.contains("폴더명:")
}

fn remove_inline_code(line: &str) -> String {
    let mut result = String::new();
    let mut in_backticks = false;

    for character in line.chars() {
        if character == '`' {
            in_backticks = !in_backticks;
            continue;
        }

        if !in_backticks {
            result.push(character);
        }
    }

    result
}
