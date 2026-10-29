//! Tauri wrappers over the headless opencode readers in agent-core.

use agent_core::opencode::{db_path, latest_session_in, opencode_transcript_in, session_exists_in};
use agent_core::TranscriptChunk;

use crate::error::{run_blocking, CommandError};

/// Rows of an opencode agent's session that changed after `cursor` (a time in
/// ms), as JSONL: `session` when the chat already knows it, else the newest
/// top-level session in `dir` (touched since `since_ms`, when given). `path`
/// is the session id, "" until one exists.
#[tauri::command]
pub async fn opencode_transcript(
    dir: String,
    since_ms: Option<u64>,
    session: Option<String>,
    cursor: u64,
) -> Result<TranscriptChunk, CommandError> {
    run_blocking(move || {
        let Some(db) = db_path() else { return Ok(TranscriptChunk::default()) };
        Ok(opencode_transcript_in(&db, &dir, since_ms, session.as_deref(), cursor))
    })
    .await
}

/// The newest session begun in `dir` at or after `since_ms`, for learning a
/// fresh run's session id right after spawning it.
#[tauri::command]
pub async fn opencode_latest_session(dir: String, since_ms: u64) -> Result<Option<String>, CommandError> {
    run_blocking(move || {
        let Some(db) = db_path() else { return Ok(None) };
        Ok(latest_session_in(&db, &dir, since_ms))
    })
    .await
}

/// Whether `opencode --session <id>` can pick this session up.
#[tauri::command]
pub async fn opencode_session_exists(session_id: String) -> Result<bool, CommandError> {
    run_blocking(move || {
        let Some(db) = db_path() else { return Ok(false) };
        Ok(session_exists_in(&db, &session_id))
    })
    .await
}
