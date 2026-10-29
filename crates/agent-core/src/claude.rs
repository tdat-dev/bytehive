//! Claude Code sessions: transcripts, session lists, live processes.
//!
//! Transcripts live under `~/.claude/projects/<slug>/<id>.jsonl`, where the
//! slug is the workspace path with every non-alphanumeric a dash.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::transcript::{is_session_id, read_start, TranscriptChunk};

/// Most bytes one read hands back, so a long session loads in slices.
const TRANSCRIPT_SLICE: u64 = 2 * 1024 * 1024;

/// Mangle a workspace path the way Claude Code names its transcript folder:
/// every non-alphanumeric character becomes '-'. e.g. `D:\maestro` -> `D--maestro`.
pub fn project_slug(dir: &str) -> String {
    dir.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// Where Claude Code keeps the transcripts of sessions run in `dir`.
pub fn project_dir(dir: &str) -> Option<PathBuf> {
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).ok()?;
    Some(Path::new(&home).join(".claude").join("projects").join(project_slug(dir)))
}

fn projects_root() -> Option<PathBuf> {
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).ok()?;
    Some(Path::new(&home).join(".claude").join("projects"))
}

/// New lines of a Claude session's transcript: the session's own file when
/// `session_id` names one, else the newest transcript written since `since_ms`
/// (an agent started by hand). Returns the complete lines after `offset`.
pub fn transcript(dir: &str, session_id: Option<&str>, since_ms: Option<u64>, offset: u64, tail_bytes: Option<u64>) -> TranscriptChunk {
    let Some(root) = projects_root() else { return TranscriptChunk::default() };
    transcript_in(&root, dir, session_id, since_ms, offset, tail_bytes)
}

pub fn transcript_in(root: &Path, dir: &str, session_id: Option<&str>, since_ms: Option<u64>, offset: u64, tail_bytes: Option<u64>) -> TranscriptChunk {
    use std::io::{Read, Seek, SeekFrom};
    let proj = root.join(project_slug(dir));
    let path = match session_id {
        // A session id is a UUID; anything else never reaches the filesystem.
        Some(id) if !id.is_empty() && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') => {
            proj.join(format!("{id}.jsonl"))
        }
        _ => {
            let since = since_ms.unwrap_or(0);
            let Ok(entries) = std::fs::read_dir(&proj) else {
                return TranscriptChunk::default();
            };
            let mut best: Option<(u128, std::path::PathBuf)> = None;
            for ent in entries.flatten() {
                let p = ent.path();
                if p.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                    continue;
                }
                let Ok(meta) = ent.metadata() else { continue };
                let Ok(modified) = meta.modified() else { continue };
                let ms = modified
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0);
                if ms < since as u128 {
                    continue;
                }
                if best.as_ref().map_or(true, |(b, _)| ms > *b) {
                    best = Some((ms, p));
                }
            }
            match best {
                Some((_, p)) => p,
                None => return TranscriptChunk::default(),
            }
        }
    };
    let Ok(mut file) = std::fs::File::open(&path) else {
        return TranscriptChunk { path: String::new(), text: String::new(), next: offset, from: offset };
    };
    let size = file.metadata().map(|m| m.len()).unwrap_or(0);
    let start = read_start(&mut file, size, offset, tail_bytes);
    let want = (size - start).min(TRANSCRIPT_SLICE);
    let mut buf = vec![0u8; want as usize];
    if file.seek(SeekFrom::Start(start)).is_err() || file.read_exact(&mut buf).is_err() {
        return TranscriptChunk { path: path.to_string_lossy().into_owned(), text: String::new(), next: start, from: start };
    }
    // Hand back whole lines only; the rest is read next time.
    let cut = match buf.iter().rposition(|&b| b == b'\n') {
        Some(i) => i + 1,
        None => 0,
    };
    buf.truncate(cut);
    TranscriptChunk {
        path: path.to_string_lossy().into_owned(),
        text: String::from_utf8_lossy(&buf).into_owned(),
        next: start + cut as u64,
        from: start,
    }
}

/// Whether Claude Code has a transcript for this session in `dir`, i.e.
/// whether `claude --resume <id>` can pick it up.
pub fn session_exists(dir: &str, session_id: &str) -> bool {
    is_session_id(session_id)
        && project_dir(dir).map_or(false, |p| p.join(format!("{session_id}.jsonl")).is_file())
}

/// A Claude Code process that is running now, as it describes itself in
/// `~/.claude/sessions/<pid>.json`. Its session id there follows the
/// conversation it is in: a /clear or a /resume inside it moves it on.
#[derive(Serialize, Clone)]
pub struct LiveSession {
    pub pid: u32,
    pub session_id: String,
    pub cwd: String,
    pub started_ms: u64,
}

pub fn live_sessions() -> Vec<LiveSession> {
    let Some(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).ok() else { return Vec::new() };
    live_sessions_in(&Path::new(&home).join(".claude").join("sessions"))
}

pub fn live_sessions_in(dir: &Path) -> Vec<LiveSession> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    entries
        .flatten()
        .filter(|e| e.path().extension().map_or(false, |x| x == "json"))
        .filter_map(|e| {
            let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(e.path()).ok()?).ok()?;
            let session_id = v.get("sessionId")?.as_str()?.to_string();
            if !is_session_id(&session_id) {
                return None;
            }
            Some(LiveSession {
                pid: u32::try_from(v.get("pid")?.as_u64()?).ok()?,
                session_id,
                cwd: v.get("cwd").and_then(|c| c.as_str()).unwrap_or("").to_string(),
                started_ms: v.get("startedAt").and_then(|s| s.as_u64()).unwrap_or(0),
            })
        })
        .collect()
}

/// One earlier conversation, for "resume a conversation".
#[derive(Serialize, Clone)]
pub struct SessionInfo {
    pub id: String,
    pub modified_ms: u64,
    /// Claude's own title for it, else your first message.
    pub title: String,
    pub messages: u32,
    /// The folder it ran in: `claude --resume` only picks it up from there.
    pub cwd: String,
}

/// The conversations Claude Code has in `dir`, newest first. Sessions with
/// nothing you typed (a local /cost, a /model) are left out. Only the start of
/// each file is read: the title and the first message are near the top.
pub fn sessions(dir: &str) -> Vec<SessionInfo> {
    let Some(proj) = project_dir(dir) else { return Vec::new() };
    sessions_in(&proj)
}

pub fn sessions_in(proj: &Path) -> Vec<SessionInfo> {
    let mut out: Vec<SessionInfo> = session_files(proj).into_iter().filter_map(|(path, ms)| session_info(&path, ms)).collect();
    out.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms));
    out.truncate(40);
    out
}

/// The newest conversations in every folder, so one begun elsewhere can be
/// carried on (in its own folder).
pub fn sessions_everywhere(limit: usize) -> Vec<SessionInfo> {
    let Some(root) = projects_root() else { return Vec::new() };
    let Ok(projects) = std::fs::read_dir(&root) else { return Vec::new() };
    let mut files: Vec<(std::path::PathBuf, u64)> = projects.flatten().filter(|p| p.path().is_dir()).flat_map(|p| session_files(&p.path())).collect();
    // Only the newest files are opened: a few extra for the ones with nothing typed.
    files.sort_by(|a, b| b.1.cmp(&a.1));
    files.truncate(limit * 2);
    let mut out: Vec<SessionInfo> = files.into_iter().filter_map(|(path, ms)| session_info(&path, ms)).collect();
    out.truncate(limit);
    out
}

/// Every conversation run in one of `roots` or a folder inside it, newest
/// first, for reading them back.
pub fn sessions_under(projects: &Path, roots: &[&str], limit: usize) -> Vec<SessionInfo> {
    // Claude names a folder's transcripts after the folder with every other
    // character a dash, so a folder inside a root is named after it too. That
    // picks the candidates; the folder each session says it ran in decides.
    let slugs: Vec<String> = roots
        .iter()
        .map(|r| project_slug(r.trim_end_matches(['\\', '/'])).to_ascii_lowercase())
        .filter(|s| !s.trim_matches('-').is_empty())
        .collect();
    let Ok(dirs) = std::fs::read_dir(projects) else { return Vec::new() };
    let mut files: Vec<(std::path::PathBuf, u64)> = dirs
        .flatten()
        .filter(|d| {
            let name = d.file_name().to_string_lossy().to_ascii_lowercase();
            d.path().is_dir() && slugs.iter().any(|s| name == *s || name.starts_with(&format!("{s}-")))
        })
        .flat_map(|d| session_files(&d.path()))
        .collect();
    files.sort_by(|a, b| b.1.cmp(&a.1));
    let mut out = Vec::new();
    for (path, ms) in files {
        if out.len() >= limit {
            break;
        }
        if let Some(s) = session_info(&path, ms) {
            if roots.iter().any(|r| path_under(&s.cwd, r)) {
                out.push(s);
            }
        }
    }
    out
}

/// Whether a session that ran in `cwd` belongs to the project at `root`.
pub fn path_under(cwd: &str, root: &str) -> bool {
    let norm = |p: &str| {
        let p = p.replace('/', "\\");
        let p = p.trim_end_matches('\\');
        if cfg!(windows) { p.to_lowercase() } else { p.to_string() }
    };
    let (c, r) = (norm(cwd), norm(root));
    !r.is_empty() && (c == r || c.starts_with(&format!("{r}\\")))
}

/// The session transcripts in one project folder, with when each last changed.
pub fn session_files(proj: &Path) -> Vec<(std::path::PathBuf, u64)> {
    let Ok(entries) = std::fs::read_dir(proj) else { return Vec::new() };
    entries
        .flatten()
        .filter_map(|ent| {
            let path = ent.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                return None;
            }
            let id = path.file_stem().and_then(|s| s.to_str())?;
            if !is_session_id(id) {
                return None;
            }
            let ms = ent
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            Some((path, ms))
        })
        .collect()
}

/// What the start of a transcript says: its title, how many messages you
/// typed, the folder it ran in. None when you typed nothing.
pub fn session_info(path: &Path, modified_ms: u64) -> Option<SessionInfo> {
    use std::io::Read;
    let id = path.file_stem().and_then(|s| s.to_str())?.to_string();
    let file = std::fs::File::open(path).ok()?;
    let mut head = Vec::new();
    let _ = file.take(768 * 1024).read_to_end(&mut head);
    let text = String::from_utf8_lossy(&head);
    let mut title: Option<String> = None;
    let mut first: Option<String> = None;
    let mut cwd = String::new();
    let mut messages = 0u32;
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        if cwd.is_empty() {
            if let Some(c) = v.get("cwd").and_then(|c| c.as_str()) {
                cwd = c.to_string();
            }
        }
        match v.get("type").and_then(|t| t.as_str()) {
            Some("ai-title") => {
                if let Some(t) = v.get("aiTitle").and_then(|t| t.as_str()) {
                    title = Some(t.to_string());
                }
            }
            Some("user") => {
                if v.get("isMeta").and_then(|x| x.as_bool()) == Some(true) {
                    continue;
                }
                let human = v.get("origin").and_then(|o| o.get("kind")).and_then(|k| k.as_str()).map_or(true, |k| k == "human");
                let content = v.get("message").and_then(|m| m.get("content"));
                let said = match content {
                    Some(serde_json::Value::String(s)) => Some(s.clone()),
                    Some(serde_json::Value::Array(parts)) => parts
                        .iter()
                        .find(|p| p.get("type").and_then(|t| t.as_str()) == Some("text"))
                        .and_then(|p| p.get("text").and_then(|t| t.as_str()).map(str::to_string)),
                    _ => None,
                };
                let Some(said) = said else { continue };
                if !human || said.starts_with('<') || said.starts_with("[Request interrupted") {
                    continue; // commands, their output, caveats, tool results
                }
                messages += 1;
                if first.is_none() {
                    first = Some(said.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(90).collect());
                }
            }
            _ => {}
        }
    }
    if messages == 0 {
        return None;
    }
    Some(SessionInfo { id, modified_ms, title: title.or(first).unwrap_or_default(), messages, cwd })
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_matches_observed_folder() {
        // Verified against the real folder Claude Code created for D:\maestro.
        assert_eq!(project_slug("D:\\maestro"), "D--maestro");
        assert_eq!(project_slug("C:\\Users\\a\\proj"), "C--Users-a-proj");
    }

    #[test]
    fn live_sessions_are_read_from_claudes_pid_files() {
        let dir = std::env::temp_dir().join(format!("maestro-live-sessions-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("20916.json"), r#"{"pid":20916,"sessionId":"7f19387c-fc1f-4bc7-af25-0e3ba5fcf7e2","cwd":"D:\\app","startedAt":1792395112932,"status":"busy"}"#).unwrap();
        std::fs::write(dir.join("1.json"), r#"{"pid":1,"sessionId":"../../etc","cwd":"D:\\app"}"#).unwrap();
        std::fs::write(dir.join("2.json"), "not json").unwrap();
        std::fs::write(dir.join("20916.abc.key"), "x").unwrap();
        let live = live_sessions_in(&dir);
        assert_eq!(live.len(), 1);
        assert_eq!((live[0].pid, live[0].session_id.as_str(), live[0].cwd.as_str(), live[0].started_ms), (20916, "7f19387c-fc1f-4bc7-af25-0e3ba5fcf7e2", "D:\\app", 1792395112932));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_session_says_its_folder_title_and_what_you_typed() {
        let dir = std::env::temp_dir().join(format!("maestro-sessions-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let id = "11111111-2222-3333-4444-555555555555";
        let lines = [
            r#"{"type":"user","cwd":"D:\\zoldify","origin":{"kind":"human"},"message":{"content":"fix   the build"}}"#,
            r#"{"type":"user","cwd":"D:\\zoldify","origin":{"kind":"task-notification"},"message":{"content":"done"}}"#,
            r#"{"type":"ai-title","aiTitle":"Fix the build"}"#,
        ];
        let path = dir.join(format!("{id}.jsonl"));
        std::fs::write(&path, lines.join("\n")).unwrap();
        let s = session_info(&path, 7).unwrap();
        assert_eq!((s.id.as_str(), s.title.as_str(), s.messages, s.cwd.as_str(), s.modified_ms), (id, "Fix the build", 1, "D:\\zoldify", 7));
        // nothing typed: not a conversation to resume
        let empty = dir.join("22222222-2222-3333-4444-555555555555.jsonl");
        std::fs::write(&empty, r#"{"type":"user","origin":{"kind":"task-notification"},"message":{"content":"x"}}"#).unwrap();
        assert!(session_info(&empty, 1).is_none());
        assert_eq!(session_files(&dir).len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_projects_sessions_include_its_worktrees_but_not_a_lookalike() {
        let projects = std::env::temp_dir().join(format!("maestro-sessions-under-{}", std::process::id()));
        let write = |folder: &str, id: &str, cwd: &str| {
            let d = projects.join(folder);
            std::fs::create_dir_all(&d).unwrap();
            let cwd = cwd.replace('\\', r"\\");
            std::fs::write(d.join(format!("{id}.jsonl")), format!(r#"{{"type":"user","cwd":"{cwd}","origin":{{"kind":"human"}},"message":{{"content":"hi"}}}}"#)).unwrap();
        };
        write("D--app", "11111111-0000-0000-0000-000000000001", r"D:\app");
        write("D--app--claude-worktrees-x", "11111111-0000-0000-0000-000000000002", r"D:\app\.claude\worktrees\x");
        write("D--app-two", "11111111-0000-0000-0000-000000000003", r"D:\app-two");
        write("D---maestro-worktrees-app-abc123-maestro-bob-1-xyz", "11111111-0000-0000-0000-000000000004", r"D:\.maestro-worktrees\app-abc123\maestro-bob-1-xyz");
        let roots = [r"D:\app\", r"D:\.maestro-worktrees\app-abc123"];
        let ids: std::collections::BTreeSet<String> = sessions_under(&projects, &roots, 50).into_iter().map(|s| s.id).collect();
        assert_eq!(ids.len(), 3);
        assert!(!ids.contains("11111111-0000-0000-0000-000000000003"));
        assert_eq!(sessions_under(&projects, &[r"D:\app"], 1).len(), 1);
        let _ = std::fs::remove_dir_all(&projects);
    }

    #[test]
    fn a_folder_is_under_itself_and_its_children_only() {
        assert!(path_under(r"D:\app", r"D:\app\"));
        assert!(path_under("D:/app/.claude/worktrees/x", r"D:\app"));
        assert!(!path_under(r"D:\app-two", r"D:\app"));
        assert!(!path_under(r"D:\app", ""));
    }
}