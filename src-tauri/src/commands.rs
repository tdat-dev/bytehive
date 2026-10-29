use serde::Serialize;
use std::path::Path;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter, State};

use crate::core::command_spec::CommandSpec;
use crate::core::registry::{self, Registry};
use crate::error::{run_blocking, CommandError};
use agent_core::{claude, muse};
pub use agent_core::transcript::{is_session_id, read_start, TranscriptChunk};
use crate::state::AppState;
use portable_pty::PtySize;

// All PTY commands are async: sync commands run on the main thread, and ConPTY
// creation (plus any wait on the registry lock while another agent is mid-
// spawn) is slow enough to visibly freeze the UI when a crew boots at once.

#[derive(Clone, Serialize)]
struct ExitPayload {
    id: String,
    code: u32,
    /// Which run of this agent id ended (see `pty_spawn`'s return value).
    run: u64,
}

/// Numbers every spawn. An agent restarted in place keeps its id; the run
/// tells its old process's late exit apart from the new one.
static NEXT_RUN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[tauri::command]
pub async fn pty_spawn(
    app: AppHandle,
    state: State<'_, AppState>,
    agent_id: String,
    program: String,
    args: Vec<String>,
    cwd: Option<String>,
    env: Option<Vec<(String, String)>>,
    cols: u16,
    rows: u16,
    // On plain pipes instead of a terminal: for an agent that talks in JSON
    // lines (Claude Code's stream-json). Its output arrives as whole lines.
    pipe: Option<bool>,
    // Raw-bytes channel: PTY output is streamed as binary (ArrayBuffer on the JS
    // side). Sending `Vec<u8>` directly would serialize every byte as a JSON
    // number — pathologically slow under a chatty agent's output and the cause
    // of the whole-app lag when a fleet is producing a lot of terminal output.
    on_bytes: Channel<InvokeResponseBody>,
) -> Result<u64, CommandError> {
    let mut spec = CommandSpec::new(program);
    for a in args {
        spec = spec.arg(a);
    }
    spec.cwd = cwd.filter(|s| !s.is_empty());
    // portable-pty falls back to the home folder when the cwd is missing, so an
    // agent would run somewhere nobody chose. Refuse and say which folder.
    if let Some(dir) = &spec.cwd {
        if !Path::new(dir).is_dir() {
            return Err(CommandError::Failed(format!("The folder {dir} doesn't exist anymore.")));
        }
    }
    spec.env = env.unwrap_or_default();
    let size = PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    };

    let app2 = app.clone();
    let exit_id = agent_id.clone();
    let registry = state.registry.clone();
    let run = NEXT_RUN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    run_blocking(move || {
        // Not under the registry lock: ConPTY creation is slow, and every other
        // agent's keystrokes and resizes take that same lock.
        Registry::spawn_shared(
            &registry,
            agent_id,
            run,
            &spec,
            size,
            pipe.unwrap_or(false),
            move |bytes| {
                let _ = on_bytes.send(InvokeResponseBody::Raw(bytes.to_vec()));
            },
            move |code| {
                let _ = app2.emit("pty-exit", ExitPayload { id: exit_id, code, run });
            },
        )
        .map_err(CommandError::from)?;
        Ok(run)
    })
    .await
}

/// Re-attach a running agent's output stream to a NEW channel (used when a tab
/// is detached into another window: the PTY survives, only the consumer moves).
/// The agent's buffered scrollback is replayed through the channel first.
#[tauri::command]
pub async fn pty_attach(
    state: State<'_, AppState>,
    agent_id: String,
    on_bytes: Channel<InvokeResponseBody>,
) -> Result<(), CommandError> {
    let registry = state.registry.clone();
    run_blocking(move || {
        let reg = registry::lock(&registry);
        reg.attach(
            &agent_id,
            Box::new(move |bytes| {
                let _ = on_bytes.send(InvokeResponseBody::Raw(bytes.to_vec()));
            }),
        )
        .map_err(CommandError::from)
    })
    .await
}

#[tauri::command]
pub async fn pty_input(
    state: State<'_, AppState>,
    agent_id: String,
    data: String,
) -> Result<(), CommandError> {
    let registry = state.registry.clone();
    run_blocking(move || {
        let mut reg = registry::lock(&registry);
        reg.write_input(&agent_id, data.as_bytes())
            .map_err(CommandError::from)
    })
    .await
}

#[tauri::command]
pub async fn pty_resize(
    state: State<'_, AppState>,
    agent_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), CommandError> {
    let registry = state.registry.clone();
    run_blocking(move || {
        let reg = registry::lock(&registry);
        reg.resize(&agent_id, cols, rows).map_err(CommandError::from)
    })
    .await
}

#[tauri::command]
pub async fn pty_kill(state: State<'_, AppState>, agent_id: String) -> Result<(), CommandError> {
    let registry = state.registry.clone();
    run_blocking(move || {
        let gone = registry::lock(&registry).take(&agent_id);
        drop(gone); // tear down after releasing the lock
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn pty_kill_all(state: State<'_, AppState>) -> Result<(), CommandError> {
    let registry = state.registry.clone();
    run_blocking(move || {
        let gone = registry::lock(&registry).drain();
        drop(gone); // tear down after releasing the lock
        Ok(())
    })
    .await
}

/// Start recording an agent's terminal output to `path` (a JSONL "cast" file).
/// The frontend passes an absolute path under `<workspace>/.maestro/recordings`;
/// the parent directory is created if needed.
#[tauri::command]
pub async fn record_start(
    state: State<'_, AppState>,
    agent_id: String,
    path: String,
) -> Result<(), CommandError> {
    let registry = state.registry.clone();
    run_blocking(move || {
        if let Some(parent) = Path::new(&path).parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CommandError::Failed(format!("cannot create recordings dir: {e}")))?;
        }
        let reg = registry::lock(&registry);
        reg.record_start(&agent_id, &path)
            .map_err(CommandError::from)
    })
    .await
}

/// Stop recording an agent's output and flush its file.
#[tauri::command]
pub async fn record_stop(state: State<'_, AppState>, agent_id: String) -> Result<(), CommandError> {
    let registry = state.registry.clone();
    run_blocking(move || {
        let reg = registry::lock(&registry);
        reg.record_stop(&agent_id);
        Ok(())
    })
    .await
}

/// Read a recording file back for the replay player. Capped so a runaway
/// recording can't blow up memory; the player tolerates a truncated tail.
#[tauri::command]
pub async fn record_read(path: String) -> Result<String, CommandError> {
    const MAX_RECORDING_BYTES: u64 = 64 * 1024 * 1024; // 64 MiB
    run_blocking(move || {
        let meta = std::fs::metadata(&path).map_err(|e| CommandError::Failed(e.to_string()))?;
        if meta.len() > MAX_RECORDING_BYTES {
            return Err(CommandError::Failed("recording too large to open (>64 MB)".into()));
        }
        std::fs::read_to_string(&path).map_err(|e| CommandError::Failed(e.to_string()))
    })
    .await
}

/// Token usage for one model, summed across a workspace's Claude transcripts.
#[derive(Serialize, Default, Clone)]
pub struct ModelUsage {
    model: String,
    input_tokens: u64,
    output_tokens: u64,
    cache_creation: u64,
    cache_read: u64,
    messages: u64,
}


/// Sum Claude Code token usage for a workspace folder by reading the session
/// transcripts Claude writes under `~/.claude/projects/<slug>/*.jsonl`. These
/// are REAL usage numbers (not scraped from the terminal), but Claude-only and
/// per-workspace (a transcript folder isn't split by Maestro agent). Returns an
/// empty list when no transcripts exist for the folder.
#[tauri::command]
pub async fn claude_usage(dir: String) -> Result<Vec<ModelUsage>, CommandError> {
    run_blocking(move || Ok(claude_usage_impl(&dir))).await
}

fn claude_usage_impl(dir: &str) -> Vec<ModelUsage> {
    let Some(proj) = claude::project_dir(dir) else { return Vec::new() };
    let entries = match std::fs::read_dir(&proj) {
        Ok(e) => e,
        Err(_) => return Vec::new(), // no transcripts for this folder yet
    };
    let paths: Vec<std::path::PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("jsonl"))
        .collect();
    let mut total: std::collections::HashMap<String, ModelUsage> = std::collections::HashMap::new();
    for p in paths {
        for u in file_usage(&p) {
            let e = total.entry(u.model.clone()).or_insert_with(|| ModelUsage { model: u.model.clone(), ..Default::default() });
            e.input_tokens += u.input_tokens;
            e.output_tokens += u.output_tokens;
            e.cache_creation += u.cache_creation;
            e.cache_read += u.cache_read;
            e.messages += u.messages;
        }
    }
    sorted_usage(total.into_values().collect())
}

/// Token usage of one Claude session (one agent's conversation), by model.
#[tauri::command]
pub async fn claude_session_usage(dir: String, session_id: String) -> Result<Vec<ModelUsage>, CommandError> {
    run_blocking(move || {
        if !is_session_id(&session_id) {
            return Ok(Vec::new());
        }
        Ok(claude::project_dir(&dir).map(|p| file_usage(&p.join(format!("{session_id}.jsonl")))).unwrap_or_default())
    })
    .await
}

fn sorted_usage(mut out: Vec<ModelUsage>) -> Vec<ModelUsage> {
    out.sort_by(|a, b| (b.input_tokens + b.output_tokens).cmp(&(a.input_tokens + a.output_tokens)));
    out
}

/// What one transcript has used so far, and how far it has been read.
/// Transcripts only grow (a long session runs to hundreds of MB), so each
/// call reads just the lines added since the last one, a line at a time.
#[derive(Default)]
struct FileUsage {
    /// Bytes read so far (always at a line's end).
    read: u64,
    /// The last few reply ids: Claude Code writes a reply over several
    /// consecutive lines, each carrying the whole reply's usage, so a reply
    /// must count once, not once per line.
    recent: std::collections::VecDeque<String>,
    by_model: std::collections::HashMap<String, ModelUsage>,
}

const RECENT_IDS: usize = 32;

impl FileUsage {
    fn add_line(&mut self, line: &str) {
        if !line.contains("\"usage\"") {
            return;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { return };
        if v.get("type").and_then(|t| t.as_str()) != Some("assistant") {
            return;
        }
        let Some(msg) = v.get("message") else { return };
        let Some(usage) = msg.get("usage") else { return };
        if let Some(id) = msg.get("id").and_then(|x| x.as_str()) {
            if self.recent.iter().any(|r| r == id) {
                return;
            }
            if self.recent.len() == RECENT_IDS {
                self.recent.pop_front();
            }
            self.recent.push_back(id.to_string());
        }
        let model = msg.get("model").and_then(|m| m.as_str()).unwrap_or("unknown");
        if model == "<synthetic>" {
            return;
        }
        let get = |k: &str| usage.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
        let e = self
            .by_model
            .entry(model.to_string())
            .or_insert_with(|| ModelUsage { model: model.to_string(), ..Default::default() });
        e.input_tokens += get("input_tokens");
        e.output_tokens += get("output_tokens");
        e.cache_creation += get("cache_creation_input_tokens");
        e.cache_read += get("cache_read_input_tokens");
        e.messages += 1;
    }

    /// Read what was added to the file since last time (whole lines only: a
    /// line still being written waits for the next call).
    fn catch_up(&mut self, path: &Path) {
        use std::io::{BufRead, Seek, SeekFrom};
        let Ok(file) = std::fs::File::open(path) else { return };
        let len = file.metadata().map(|m| m.len()).unwrap_or(0);
        if len < self.read {
            *self = FileUsage::default(); // rewritten: start over
        }
        if len == self.read {
            return;
        }
        let mut r = std::io::BufReader::with_capacity(256 * 1024, file);
        if r.seek(SeekFrom::Start(self.read)).is_err() {
            return;
        }
        let mut buf = Vec::with_capacity(64 * 1024);
        loop {
            buf.clear();
            let n = match r.read_until(b'\n', &mut buf) {
                Ok(n) => n,
                Err(_) => break,
            };
            if n == 0 || buf.last() != Some(&b'\n') {
                break;
            }
            self.read += n as u64;
            if let Ok(line) = std::str::from_utf8(&buf) {
                self.add_line(line);
            }
        }
    }
}

fn usage_cache() -> &'static std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, FileUsage>> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, FileUsage>>> = std::sync::OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// One transcript's usage by model, read on from where it was last read.
fn file_usage(path: &Path) -> Vec<ModelUsage> {
    let mut cache = usage_cache().lock().unwrap_or_else(|e| e.into_inner());
    let entry = cache.entry(path.to_path_buf()).or_default();
    entry.catch_up(path);
    sorted_usage(entry.by_model.values().cloned().collect())
}

/// Where pictures pasted into the chat composer are kept, and for how long.
const PASTE_DIR: &str = "maestro-paste";
const PASTE_KEEP_SECS: u64 = 7 * 24 * 3600;

/// Save a picture pasted into the chat composer (base64) as a file, so it can
/// reach the agent the way a dropped file does: as a path. Returns the path.
#[tauri::command]
pub async fn save_pasted_image(data: String, ext: String) -> Result<String, CommandError> {
    run_blocking(move || save_pasted_image_in(&std::env::temp_dir().join(PASTE_DIR), &data, &ext)).await
}

/// Most a file pasted into the chat may be (base64 grows it by a third on the way).
const PASTE_FILE_MAX: usize = 50 * 1024 * 1024;

/// Save a file pasted into the chat composer (copied in Explorer: the page gets
/// its content, not where it lives) under its own name, in a folder of its own
/// so two pastes of `notes.txt` don't collide. Returns the path.
#[tauri::command]
pub async fn save_pasted_file(data: String, name: String) -> Result<String, CommandError> {
    run_blocking(move || save_pasted_file_in(&std::env::temp_dir().join(PASTE_DIR), &data, &name)).await
}

fn save_pasted_file_in(dir: &Path, data: &str, name: &str) -> Result<String, CommandError> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    // Only the name: no folders, nothing Windows refuses in a file name.
    let clean: String = Path::new(name)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
        .chars()
        .map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '_' } else { c })
        .collect();
    let clean = clean.trim().trim_end_matches('.').to_string();
    let clean = if clean.is_empty() { "pasted".to_string() } else { clean };
    if data.len() / 4 * 3 > PASTE_FILE_MAX {
        return Err(CommandError::Failed(format!("{clean} is over {} MB", PASTE_FILE_MAX / 1024 / 1024)));
    }
    let bytes = STANDARD
        .decode(data.trim())
        .map_err(|e| CommandError::Failed(format!("bad file data: {e}")))?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let mut folder = dir.join(format!("files-{stamp}"));
    let mut n = 1;
    while folder.exists() {
        folder = dir.join(format!("files-{stamp}-{n}"));
        n += 1;
    }
    std::fs::create_dir_all(&folder).map_err(|e| CommandError::Failed(e.to_string()))?;
    let path = folder.join(&clean);
    std::fs::write(&path, bytes).map_err(|e| CommandError::Failed(e.to_string()))?;
    Ok(path.to_string_lossy().into_owned())
}

fn save_pasted_image_in(dir: &Path, data: &str, ext: &str) -> Result<String, CommandError> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let ext = ext.trim_start_matches('.').to_ascii_lowercase();
    if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp") {
        return Err(CommandError::Failed(format!("not a picture type: {ext}")));
    }
    let bytes = STANDARD
        .decode(data.trim())
        .map_err(|e| CommandError::Failed(format!("bad picture data: {e}")))?;
    std::fs::create_dir_all(dir).map_err(|e| CommandError::Failed(e.to_string()))?;
    // Pastes from earlier sessions go after a week.
    if let Ok(list) = std::fs::read_dir(dir) {
        for entry in list.flatten() {
            let old = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age.as_secs() > PASTE_KEEP_SECS);
            if old {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let mut path = dir.join(format!("pasted-{stamp}.{ext}"));
    let mut n = 1;
    while path.exists() {
        path = dir.join(format!("pasted-{stamp}-{n}.{ext}"));
        n += 1;
    }
    std::fs::write(&path, bytes).map_err(|e| CommandError::Failed(e.to_string()))?;
    Ok(path.to_string_lossy().into_owned())
}


/// Read an agent's transcript under `~/.claude/projects/<slug of dir>/`: the
/// `<session_id>.jsonl` Maestro started it with, or, when that is unknown, the
/// newest transcript written since `since_ms` (an agent started by hand).
/// Returns the complete lines after `offset`.
#[tauri::command]
pub async fn claude_transcript(
    dir: String,
    session_id: Option<String>,
    since_ms: Option<u64>,
    offset: u64,
    tail_bytes: Option<u64>,
) -> Result<TranscriptChunk, CommandError> {
    run_blocking(move || Ok(claude::transcript(&dir, session_id.as_deref(), since_ms, offset, tail_bytes))).await
}



/// Whether Claude Code has a transcript for this session in `dir`, i.e.
/// whether `claude --resume <id>` can pick it up.
#[tauri::command]
pub async fn claude_session_exists(dir: String, session_id: String) -> Result<bool, CommandError> {
    run_blocking(move || Ok(claude::session_exists(&dir, &session_id))).await
}





/// The conversations Muse has, newest first. The frontend diffs this against
/// before it spawned to learn a new run's session id.
#[tauri::command]
pub async fn muse_sessions() -> Result<Vec<muse::MuseSession>, CommandError> {
    run_blocking(|| Ok(muse::sessions())).await
}

/// Whether `muse resume <id>` can pick this session up.
#[tauri::command]
pub async fn muse_session_exists(session_id: String) -> Result<bool, CommandError> {
    run_blocking(move || Ok(muse::session_exists(&session_id))).await
}

/// New complete lines of a Muse agent's session log from `offset`: the
/// agent's own session when `session_id` names one, else nothing (`path`
/// "" until one is known).
#[tauri::command]
pub async fn muse_transcript(session_id: Option<String>, offset: u64, tail_bytes: Option<u64>) -> Result<TranscriptChunk, CommandError> {
    run_blocking(move || Ok(muse::transcript(session_id.as_deref(), offset, tail_bytes))).await
}




/// Muse conversations, newest first, for the History list.
#[tauri::command]
pub async fn muse_session_list(limit: usize) -> Result<Vec<muse::MuseListedSession>, CommandError> {
    run_blocking(move || Ok(muse::list(limit))).await
}





#[tauri::command]
pub async fn claude_live_sessions() -> Result<Vec<claude::LiveSession>, CommandError> {
    run_blocking(|| Ok(claude::live_sessions())).await
}



/// The conversations Claude Code has in `dir`, newest first. Sessions with
/// nothing you typed (a local /cost, a /model) are left out. Only the start of
/// each file is read: the title and the first message are near the top.
#[tauri::command]
pub async fn claude_sessions(dir: String) -> Result<Vec<claude::SessionInfo>, CommandError> {
    run_blocking(move || Ok(claude::sessions(&dir))).await
}

/// The newest conversations in every folder, so one begun elsewhere can be
/// carried on (in its own folder).
#[tauri::command]
pub async fn claude_sessions_everywhere(limit: usize) -> Result<Vec<claude::SessionInfo>, CommandError> {
    run_blocking(move || Ok(claude::sessions_everywhere(limit.clamp(1, 200)))).await
}



/// Every conversation run in `root` or a folder inside it, or in one of its
/// agents' worktrees (kept outside the repo, see worktree.rs), newest first,
/// for reading them back.
#[tauri::command]
pub async fn claude_sessions_in(root: String, limit: usize) -> Result<Vec<claude::SessionInfo>, CommandError> {
    run_blocking(move || {
        let Some(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).ok() else { return Ok(Vec::new()) };
        // The worktrees' folder is named after a hash of the repo path as it was
        // written then (git says D:/x, Windows D:\x): look under both.
        let mut roots = vec![root.clone()];
        for form in [root.replace('/', "\\"), root.replace('\\', "/")] {
            let w = crate::worktree::worktrees_of(&form).to_string_lossy().into_owned();
            if !roots.contains(&w) {
                roots.push(w);
            }
        }
        let roots: Vec<&str> = roots.iter().map(String::as_str).collect();
        Ok(claude::sessions_under(&Path::new(&home).join(".claude").join("projects"), &roots, limit.clamp(1, 500)))
    })
    .await
}





/// Most output `run_capture` keeps: a CLI that prints more is cut here.
const CAPTURE_MAX: usize = 8 * 1024 * 1024;

/// Run a program to completion (no window, no stdin) in `cwd` and return its
/// stdout. For asking an agent CLI about itself, e.g. Claude Code's
/// `claude -p /cost --output-format stream-json --verbose`, whose first
/// event lists its commands, skills and model without calling the model.
/// Killed (with its child processes) after `timeout_ms`.
#[tauri::command]
pub async fn run_capture(
    program: String,
    args: Vec<String>,
    cwd: Option<String>,
    timeout_ms: Option<u64>,
) -> Result<String, CommandError> {
    run_blocking(move || run_capture_impl(&program, &args, cwd.as_deref(), timeout_ms.unwrap_or(60_000))).await
}

fn run_capture_impl(program: &str, args: &[String], cwd: Option<&str>, timeout_ms: u64) -> Result<String, CommandError> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let mut cmd = Command::new(program);
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    if let Some(dir) = cwd.filter(|d| !d.is_empty()) {
        if !Path::new(dir).is_dir() {
            return Err(CommandError::Failed(format!("The folder {dir} doesn't exist anymore.")));
        }
        cmd.current_dir(dir);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().map_err(|e| CommandError::Failed(format!("couldn't start {program}: {e}")))?;
    let pid = child.id();
    let mut out = child.stdout.take().ok_or_else(|| CommandError::Failed("no stdout".into()))?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 64 * 1024];
        loop {
            match out.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if buf.len() < CAPTURE_MAX {
                        buf.extend_from_slice(&chunk[..n.min(CAPTURE_MAX - buf.len())]);
                    }
                }
            }
        }
        let _ = tx.send(buf);
    });
    match rx.recv_timeout(std::time::Duration::from_millis(timeout_ms)) {
        Ok(buf) => {
            let _ = child.wait();
            Ok(String::from_utf8_lossy(&buf).into_owned())
        }
        Err(_) => {
            // Take the whole tree down: a .cmd shim leaves its node process behind.
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                let _ = Command::new("taskkill")
                    .args(["/T", "/F", "/PID", &pid.to_string()])
                    .creation_flags(0x0800_0000)
                    .status();
            }
            let _ = child.kill();
            let _ = child.wait();
            Err(CommandError::Failed(format!("{program} took longer than {} s", timeout_ms / 1000)))
        }
    }
}

/// Show or hide the system-tray icon. Driven by the frontend "Hide to tray"
/// setting so the icon only appears for users who opt in.
#[tauri::command]
pub fn set_tray_visible(app: AppHandle, visible: bool) -> Result<(), CommandError> {
    if let Some(tray) = app.tray_by_id("main") {
        tray.set_visible(visible)
            .map_err(|e| CommandError::Failed(e.to_string()))?;
    }
    Ok(())
}

/// Update the tray icon's hover tooltip (e.g. "Maestro · 3 running") so users
/// can see at a glance that agents are still alive while the window is hidden.
#[tauri::command]
pub fn set_tray_tooltip(app: AppHandle, tooltip: String) -> Result<(), CommandError> {
    if let Some(tray) = app.tray_by_id("main") {
        tray.set_tooltip(Some(tooltip))
            .map_err(|e| CommandError::Failed(e.to_string()))?;
    }
    Ok(())
}

/// Resolve a single program name the way Windows' CreateProcess / cmd would and
/// report whether it's launchable. Absolute paths just check the file exists;
/// names ending in an executable extension (.exe/.com/…) are looked up verbatim
/// in each PATH directory; bare names are probed against every PATHEXT
/// extension in each PATH directory. Extension matching is case-insensitive.
fn program_on_path(program: &str, exts: &[String]) -> bool {
    let program = program.trim();
    if program.is_empty() {
        return false;
    }

    // Absolute (or otherwise rooted) path — just check the file is there.
    let p = Path::new(program);
    if p.is_absolute() || program.contains('/') || program.contains('\\') {
        return p.is_file();
    }

    // Does the name already carry one of the executable extensions? If so we
    // look it up verbatim rather than appending more extensions.
    let has_exe_ext = exts.iter().any(|ext| {
        let ext = ext.trim_start_matches('.');
        program.len() > ext.len()
            && program
                .get(program.len() - ext.len()..)
                .map(|tail| tail.eq_ignore_ascii_case(ext))
                .unwrap_or(false)
            && program.as_bytes()[program.len() - ext.len() - 1] == b'.'
    });

    let path_dirs: Vec<_> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();

    for dir in &path_dirs {
        if has_exe_ext {
            if dir.join(program).is_file() {
                return true;
            }
        } else {
            for ext in exts {
                // PATHEXT entries already include the leading dot (".EXE").
                if dir.join(format!("{program}{ext}")).is_file() {
                    return true;
                }
            }
        }
    }
    false
}

/// For each program name, report whether it resolves on PATH (see
/// `program_on_path`). Pure std — no per-item shelling out — so the wizard can
/// batch-check every preset's binary in one round-trip to gray out the ones
/// that aren't installed.
#[tauri::command]
pub async fn programs_on_path(programs: Vec<String>) -> Result<Vec<bool>, CommandError> {
    // PATH can contain slow/network dirs; probe off the main thread.
    run_blocking(move || programs_on_path_impl(programs)).await
}

fn programs_on_path_impl(programs: Vec<String>) -> Result<Vec<bool>, CommandError> {
    // PATHEXT decides which extensions a bare name can resolve to. Default to
    // the documented Windows set when it's unset, and uppercase-normalize for
    // tidy case-insensitive comparisons.
    let raw = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
    let exts: Vec<String> = raw
        .split(';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| {
            let s = if s.starts_with('.') {
                s.to_string()
            } else {
                format!(".{s}")
            };
            s.to_ascii_uppercase()
        })
        .collect();

    Ok(programs
        .iter()
        .map(|p| program_on_path(p, &exts))
        .collect())
}

#[cfg(test)]
mod tests {
    #[test]
    fn usage_counts_a_reply_once_and_reads_only_what_was_added() {
        let line = |id: &str, model: &str| format!(
            "{{\"type\":\"assistant\",\"message\":{{\"id\":\"{id}\",\"model\":\"{model}\",\"usage\":{{\"input_tokens\":10,\"output_tokens\":5,\"cache_read_input_tokens\":100,\"cache_creation_input_tokens\":20}},\"content\":[]}}}}\n"
        );
        let dir = std::env::temp_dir().join(format!("maestro-usage-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("s.jsonl");
        // a reply over two lines, another reply, a half-written line
        std::fs::write(&path, [line("m1", "claude-opus-5-5"), line("m1", "claude-opus-5-5"), line("m2", "claude-opus-5-5"), "{\"type\":\"assi".to_string()].concat()).unwrap();
        let first = super::file_usage(&path);
        assert_eq!((first[0].messages, first[0].output_tokens, first[0].cache_read), (2, 10, 200));
        // the half line finishes, more replies come
        let mut rest = std::fs::read_to_string(&path).unwrap();
        rest.truncate(rest.len() - "{\"type\":\"assi".len());
        rest += &[line("m3", "claude-haiku-4-5"), line("m4", "<synthetic>"), line("m5", "claude-opus-5-5")].concat();
        std::fs::write(&path, rest).unwrap();
        let rows = super::file_usage(&path);
        let opus = rows.iter().find(|r| r.model == "claude-opus-5-5").unwrap();
        assert_eq!((opus.messages, opus.input_tokens, opus.cache_creation), (3, 30, 60));
        assert_eq!(rows.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    use super::*;

    fn default_exts() -> Vec<String> {
        ".COM;.EXE;.BAT;.CMD"
            .split(';')
            .map(|s| s.to_string())
            .collect()
    }

    #[test]
    fn programs_finds_cmd_and_misses_fake() {
        let out = programs_on_path_impl(vec![
            "cmd.exe".into(),
            "cmd".into(),
            "definitely-not-a-real-cli-xyz".into(),
        ])
        .unwrap();
        assert_eq!(out.len(), 3);
        // cmd.exe (verbatim) and cmd (via PATHEXT) both live in System32.
        assert!(out[0], "cmd.exe should resolve on PATH");
        assert!(out[1], "cmd should resolve via PATHEXT");
        assert!(!out[2], "a bogus name must not resolve");
    }


    #[test]
    fn empty_name_is_not_found() {
        assert!(!program_on_path("", &default_exts()));
        assert!(!program_on_path("   ", &default_exts()));
    }

    #[test]
    fn absolute_path_checks_file_existence() {
        // A directory is not a file, and a bogus absolute path is absent.
        assert!(!program_on_path(
            "C:\\Windows\\System32\\no-such-binary.exe",
            &default_exts()
        ));
    }
}


#[cfg(test)]
mod paste_tests {
    use super::*;

    #[test]
    fn a_pasted_picture_is_saved_as_its_own_file() {
        let dir = std::env::temp_dir().join(format!("maestro-paste-test-{}", std::process::id()));
        let a = save_pasted_image_in(&dir, "iVBORw0KGgo=", "PNG").unwrap();
        let b = save_pasted_image_in(&dir, "iVBORw0KGgo=", ".png").unwrap();
        assert_ne!(a, b);
        assert!(a.ends_with(".png"));
        assert_eq!(std::fs::read(&a).unwrap(), b"\x89PNG\r\n\x1a\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_pasted_file_keeps_its_name_in_a_folder_of_its_own() {
        let dir = std::env::temp_dir().join(format!("maestro-paste-test3-{}", std::process::id()));
        let a = save_pasted_file_in(&dir, "aGk=", "notes.txt").unwrap();
        let b = save_pasted_file_in(&dir, "aGk=", "notes.txt").unwrap();
        assert!(a.ends_with("notes.txt") && b.ends_with("notes.txt"));
        assert_ne!(a, b);
        assert_eq!(std::fs::read(&a).unwrap(), b"hi");
        // a name can't climb out of the folder or carry what Windows refuses
        let c = save_pasted_file_in(&dir, "aGk=", "..\\..\\evil:name?.txt").unwrap();
        assert!(Path::new(&c).starts_with(&dir));
        assert!(c.ends_with("evil_name_.txt"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_pictures_and_real_base64_are_saved() {
        let dir = std::env::temp_dir().join(format!("maestro-paste-test2-{}", std::process::id()));
        assert!(save_pasted_image_in(&dir, "iVBORw0KGgo=", "exe").is_err());
        assert!(save_pasted_image_in(&dir, "not base64!", "png").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}