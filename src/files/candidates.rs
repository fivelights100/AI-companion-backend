use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        OnceLock,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tokio::sync::Mutex;

use crate::models::files::{FileOpenCandidate, FileOpenCandidatePage, FileOpenPrepareResponse};

const OPEN_CONFIRM_TTL_SECONDS: u64 = 120;
pub const OPEN_CANDIDATE_PAGE_SIZE: usize = 7;

static PENDING_OPEN_CANDIDATES: OnceLock<Mutex<HashMap<String, StoredOpenCandidate>>> = OnceLock::new();
static PENDING_OPEN_REQUESTS: OnceLock<Mutex<HashMap<String, StoredOpenRequest>>> = OnceLock::new();
static ID_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
struct StoredOpenCandidate {
    candidate: FileOpenCandidate,
    created_at: SystemTime,
}

#[derive(Debug, Clone)]
struct StoredOpenRequest {
    candidates: Vec<FileOpenCandidate>,
    created_at: SystemTime,
}

pub async fn store_candidates(mut candidates: Vec<FileOpenCandidate>) -> FileOpenPrepareResponse {
    cleanup_expired_open_state().await;

    let request_id = make_id("open-request");

    for candidate in &mut candidates {
        candidate.id = make_id("open-candidate");
    }

    let now = SystemTime::now();

    {
        let mut candidate_store = pending_candidates_store().lock().await;
        for candidate in &candidates {
            candidate_store.insert(
                candidate.id.clone(),
                StoredOpenCandidate {
                    candidate: candidate.clone(),
                    created_at: now,
                },
            );
        }
    }

    {
        let mut request_store = pending_requests_store().lock().await;
        request_store.insert(
            request_id.clone(),
            StoredOpenRequest {
                candidates: candidates.clone(),
                created_at: now,
            },
        );
    }

    candidates_response_from_page(build_candidate_page(request_id, &candidates, 0))
}

pub async fn next_page(request_id: &str, offset: usize) -> Result<FileOpenCandidatePage, String> {
    cleanup_expired_open_state().await;

    let request_id = request_id.trim();
    if request_id.is_empty() {
        return Err("후보 목록 요청 ID가 비어 있습니다.".to_string());
    }

    let store = pending_requests_store().lock().await;
    let Some(stored_request) = store.get(request_id) else {
        return Err("후보 목록이 만료되었거나 찾을 수 없습니다. 다시 요청해 주세요.".to_string());
    };

    if offset >= stored_request.candidates.len() {
        return Err("더 보여줄 후보가 없습니다.".to_string());
    }

    Ok(build_candidate_page(
        request_id.to_string(),
        &stored_request.candidates,
        offset,
    ))
}

pub async fn take_candidate(candidate_id: &str) -> Option<FileOpenCandidate> {
    cleanup_expired_open_state().await;

    let mut store = pending_candidates_store().lock().await;
    store.remove(candidate_id).map(|stored| stored.candidate)
}

pub fn candidates_response_from_page(page: FileOpenCandidatePage) -> FileOpenPrepareResponse {
    FileOpenPrepareResponse {
        ok: true,
        status: "candidates_ready".to_string(),
        message: page.message.clone(),
        candidates: page.candidates.clone(),
        candidate_page: Some(page),
    }
}

fn build_candidate_page(
    request_id: String,
    candidates: &[FileOpenCandidate],
    offset: usize,
) -> FileOpenCandidatePage {
    let start = offset.min(candidates.len());
    let end = (start + OPEN_CANDIDATE_PAGE_SIZE).min(candidates.len());
    let page_candidates = candidates[start..end].to_vec();
    let has_more = end < candidates.len();
    let next_offset = has_more.then_some(end);

    FileOpenCandidatePage {
        request_id,
        candidates: page_candidates,
        has_more,
        next_offset,
        page_size: OPEN_CANDIDATE_PAGE_SIZE,
        message: "화면에 열 수 있는 후보를 띄웠습니다. 사용자가 항목을 선택해야 실제로 열립니다.".to_string(),
    }
}

async fn cleanup_expired_open_state() {
    let now = SystemTime::now();

    {
        let mut store = pending_candidates_store().lock().await;
        store.retain(|_, stored| {
            now.duration_since(stored.created_at)
                .map(|elapsed| elapsed <= Duration::from_secs(OPEN_CONFIRM_TTL_SECONDS))
                .unwrap_or(false)
        });
    }

    {
        let mut store = pending_requests_store().lock().await;
        store.retain(|_, stored| {
            now.duration_since(stored.created_at)
                .map(|elapsed| elapsed <= Duration::from_secs(OPEN_CONFIRM_TTL_SECONDS))
                .unwrap_or(false)
        });
    }
}

fn pending_candidates_store() -> &'static Mutex<HashMap<String, StoredOpenCandidate>> {
    PENDING_OPEN_CANDIDATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn pending_requests_store() -> &'static Mutex<HashMap<String, StoredOpenRequest>> {
    PENDING_OPEN_REQUESTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn make_id(prefix: &str) -> String {
    let counter = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();

    format!("{prefix}-{timestamp}-{counter}")
}
