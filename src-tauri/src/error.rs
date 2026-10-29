//! Tauri-side errors: the shared [`CommandError`] plus the blocking-thread hop
//! sync commands need so they never freeze the window.

pub use agent_core::CommandError;

/// Run blocking work (git, ConPTY spawns, PATH probing) on a dedicated
/// blocking thread. Sync `#[tauri::command]`s execute ON the main thread, so a
/// blocking body there freezes the whole window — no repaints, no input — for
/// its full duration (spawning several isolated agents froze the app for the
/// length of 3 full `git worktree add` checkouts). Commands wrap their body in
/// this instead and stay `async`.
pub async fn run_blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, CommandError> + Send + 'static,
) -> Result<T, CommandError> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| CommandError::Failed(format!("background task failed: {e}")))?
}
