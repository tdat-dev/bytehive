//! Tauri wrappers over the headless Codex readers in agent-core.

use agent_core::codex::{codex_transcript_in, latest_session_in, session_exists_in, sessions_root, store_root};
use agent_core::TranscriptChunk;

use crate::error::{run_blocking, CommandError};

/// New lines of a Codex agent's rollout: `path` when the chat already knows
/// it, else the newest rollout begun in `dir` (since `since_ms`, when given).
#[tauri::command]
pub async fn codex_transcript(
    dir: String,
    since_ms: Option<u64>,
    path: Option<String>,
    offset: u64,
    tail_bytes: Option<u64>,
) -> Result<TranscriptChunk, CommandError> {
    run_blocking(move || {
        let Some(root) = sessions_root() else { return Ok(TranscriptChunk::default()) };
        Ok(codex_transcript_in(&root, &dir, since_ms, path.as_deref(), offset, tail_bytes))
    })
    .await
}

/// The newest session begun in `dir` at or after `since_ms`, for learning a
/// fresh run's session id right after spawning it.
#[tauri::command]
pub async fn codex_latest_session(dir: String, since_ms: u64) -> Result<Option<String>, CommandError> {
    run_blocking(move || {
        let Some(root) = sessions_root() else { return Ok(None) };
        Ok(latest_session_in(&root, &dir, since_ms))
    })
    .await
}

/// Whether `codex resume <id>` can pick this session up: its rollout is still
/// on disk, or the session index still names it.
#[tauri::command]
pub async fn codex_session_exists(session_id: String) -> Result<bool, CommandError> {
    run_blocking(move || {
        let Some(codex) = store_root() else { return Ok(false) };
        Ok(session_exists_in(&codex, &session_id))
    })
    .await
}
