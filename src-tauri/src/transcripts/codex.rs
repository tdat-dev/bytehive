//! Codex keeps each session as a rollout under
//! `~/.codex/sessions/YYYY/MM/DD/rollout-<time>-<id>.jsonl`. Its first line
//! (`session_meta`) says the folder it ran in, which is how an agent's own
//! rollout is found.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::commands::TranscriptChunk;
use crate::error::{run_blocking, CommandError};

/// Most bytes one read hands back, so a long session loads in slices.
const SLICE: u64 = 2 * 1024 * 1024;
/// Day folders looked through, newest first: an agent's rollout is recent.
const DAYS_SCANNED: usize = 14;
/// How much of a rollout's first line is read to find its folder.
const HEAD_BYTES: usize = 64 * 1024;

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

pub(crate) fn sessions_root() -> Option<PathBuf> {
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).ok()?;
    Some(Path::new(&home).join(".codex").join("sessions"))
}

fn codex_transcript_in(root: &Path, dir: &str, since_ms: Option<u64>, path: Option<&str>, offset: u64, tail: Option<u64>) -> TranscriptChunk {
    // A known path is only trusted inside the sessions folder.
    let known = path
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .filter(|p| p.is_file() && p.starts_with(root));
    let file = match known.or_else(|| find_rollout(root, dir, since_ms.unwrap_or(0))) {
        Some(p) => p,
        None => return TranscriptChunk { path: String::new(), text: String::new(), next: offset, from: offset },
    };
    read_slice(&file, offset, tail)
}

/// Slashes one way, no trailing slash, any case: how two folder names compare.
fn norm(p: &str) -> String {
    p.replace('\\', "/").trim_end_matches('/').to_lowercase()
}

/// The newest rollout whose session ran in `dir`, modified at or after `since_ms`.
pub(crate) fn find_rollout(root: &Path, dir: &str, since_ms: u64) -> Option<PathBuf> {
    let want = norm(dir);
    // YYYY/MM/DD folders, newest first.
    let mut days: Vec<PathBuf> = Vec::new();
    for y in sorted_dirs(root) {
        for m in sorted_dirs(&y) {
            for d in sorted_dirs(&m) {
                days.push(d);
            }
        }
    }
    days.sort();
    days.reverse();
    let mut best: Option<(u128, PathBuf)> = None;
    for day in days.into_iter().take(DAYS_SCANNED) {
        let Ok(entries) = std::fs::read_dir(&day) else { continue };
        for ent in entries.flatten() {
            let p = ent.path();
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !(name.starts_with("rollout-") && name.ends_with(".jsonl")) {
                continue;
            }
            let ms = ent
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis())
                .unwrap_or(0);
            if ms < since_ms as u128 || best.as_ref().is_some_and(|(b, _)| ms <= *b) {
                continue;
            }
            if session_cwd(&p).is_some_and(|c| norm(&c) == want) {
                best = Some((ms, p));
            }
        }
        // The newest day with a match wins; older days can't be newer.
        if best.is_some() {
            break;
        }
    }
    best.map(|(_, p)| p)
}

/// The session id from a rollout file name (`rollout-<time>-<id>.jsonl`):
/// the same id `codex resume` takes. None for anything shaped otherwise.
pub(crate) fn rollout_id(path: &Path) -> Option<String> {
    let name = path.file_name().and_then(|n| n.to_str())?;
    let rest = name.strip_prefix("rollout-")?.strip_suffix(".jsonl")?;
    let cut = rest.len().checked_sub(36)?;
    if !rest.is_char_boundary(cut) || (cut > 0 && !rest[..cut].ends_with('-')) {
        return None;
    }
    let id = &rest[cut..];
    crate::commands::is_session_id(id).then(|| id.to_string())
}

/// The newest session begun in `dir` at or after `since_ms`, for learning a
/// fresh run's session id right after spawning it.
#[tauri::command]
pub async fn codex_latest_session(dir: String, since_ms: u64) -> Result<Option<String>, CommandError> {
    run_blocking(move || {
        let Some(root) = sessions_root() else { return Ok(None) };
        Ok(find_rollout(&root, &dir, since_ms).and_then(|p| rollout_id(&p)))
    })
    .await
}

/// Whether `codex resume <id>` can pick this session up: its rollout is still
/// on disk, or the session index still names it.
#[tauri::command]
pub async fn codex_session_exists(session_id: String) -> Result<bool, CommandError> {
    run_blocking(move || {
        if !crate::commands::is_session_id(&session_id) {
            return Ok(false);
        }
        let Some(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).ok() else { return Ok(false) };
        let codex = Path::new(&home).join(".codex");
        if rollout_with_id(&codex.join("sessions"), &session_id) {
            return Ok(true);
        }
        let idx = codex.join("session_index.jsonl");
        Ok(std::fs::read_to_string(&idx).map_or(false, |t| t.contains(&format!("\"id\":\"{session_id}\""))))
    })
    .await
}

/// Any rollout file under `root` (any day) carrying this session id.
/// File names only, no reads: cheap enough to scan without a day cap.
fn rollout_with_id(root: &Path, id: &str) -> bool {
    for y in sorted_dirs(root) {
        for m in sorted_dirs(&y) {
            for d in sorted_dirs(&m) {
                let Ok(entries) = std::fs::read_dir(&d) else { continue };
                for ent in entries.flatten() {
                    if rollout_id(&ent.path()).as_deref() == Some(id) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

pub(crate) fn sorted_dirs(p: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(p)
        .map(|it| it.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect())
        .unwrap_or_default();
    out.sort();
    out
}

/// The folder a rollout's session ran in, from `"cwd"` in its first line.
fn session_cwd(p: &Path) -> Option<String> {
    let mut f = std::fs::File::open(p).ok()?;
    let mut buf = vec![0u8; HEAD_BYTES];
    let n = f.read(&mut buf).ok()?;
    buf.truncate(n);
    let head = match buf.iter().position(|&b| b == b'\n') {
        Some(i) => &buf[..i],
        None => &buf[..],
    };
    let key = b"\"cwd\":";
    let at = head.windows(key.len()).position(|w| w == key)? + key.len();
    let rest = &head[at..];
    let start = rest.iter().position(|&b| !b.is_ascii_whitespace())?;
    let mut de = serde_json::Deserializer::from_slice(&rest[start..]);
    <String as serde::Deserialize>::deserialize(&mut de).ok()
}

/// Complete lines of `path` from `offset`, at most one slice.
fn read_slice(path: &Path, offset: u64, tail: Option<u64>) -> TranscriptChunk {
    let shown = path.to_string_lossy().into_owned();
    let Ok(mut file) = std::fs::File::open(path) else {
        return TranscriptChunk { path: String::new(), text: String::new(), next: offset, from: offset };
    };
    let size = file.metadata().map(|m| m.len()).unwrap_or(0);
    let start = crate::commands::read_start(&mut file, size, offset, tail);
    let want = (size - start).min(SLICE);
    let mut buf = vec![0u8; want as usize];
    if file.seek(SeekFrom::Start(start)).is_err() || file.read_exact(&mut buf).is_err() {
        return TranscriptChunk { path: shown, text: String::new(), next: start, from: start };
    }
    // Hand back whole lines only; the rest is read next time.
    let cut = buf.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
    buf.truncate(cut);
    TranscriptChunk { path: shown, text: String::from_utf8_lossy(&buf).into_owned(), next: start + cut as u64, from: start }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("maestro-codex-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn rollout(root: &Path, day: &str, name: &str, cwd: &str, body: &str) -> PathBuf {
        let d = day.split('/').fold(root.to_path_buf(), |p, part| p.join(part));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join(format!("rollout-{name}.jsonl"));
        let meta = serde_json::json!({ "type": "session_meta", "payload": { "id": name, "cwd": cwd, "base_instructions": { "text": "x".repeat(200) } } });
        std::fs::write(&p, format!("{meta}\n{body}")).unwrap();
        p
    }

    #[test]
    fn finds_the_newest_rollout_begun_in_the_folder() {
        let root = tmp("find");
        rollout(&root, "2026/09/24", "a", "D:\\app", "");
        std::thread::sleep(std::time::Duration::from_millis(20));
        let other = rollout(&root, "2026/09/25", "b", "D:\\other", "");
        std::thread::sleep(std::time::Duration::from_millis(20));
        let mine = rollout(&root, "2026/09/25", "c", "d:/APP/", "{\"type\":\"x\"}\n{\"half\":");
        let got = codex_transcript_in(&root, "D:\\app", None, None, 0, None);
        assert_eq!(got.path, mine.to_string_lossy());
        // whole lines only: the half-written last one waits
        assert!(got.text.ends_with("{\"type\":\"x\"}\n"));
        assert_eq!(got.next, got.text.len() as u64);
        // a known path is read as is
        let again = codex_transcript_in(&root, "D:\\app", None, Some(&other.to_string_lossy()), 0, None);
        assert_eq!(again.path, other.to_string_lossy());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_first_read_can_start_near_the_end_on_a_whole_line() {
        let root = tmp("tail");
        let body: String = (0..200).map(|i| format!("{{\"n\":{i}}}\n")).collect();
        let p = rollout(&root, "2026/09/25", "a", "D:/app", &body);
        let size = std::fs::metadata(&p).unwrap().len();
        let got = codex_transcript_in(&root, "D:/app", None, None, 0, Some(100));
        assert!(got.from > 0 && got.from <= size - 90);
        assert!(got.text.starts_with("{\"n\":"), "starts on a whole line: {:?}", &got.text[..20]);
        assert!(got.text.ends_with("{\"n\":199}\n"));
        assert_eq!(got.next, size);
        // later reads go on from where it got to, tail or not
        let more = codex_transcript_in(&root, "D:/app", None, None, got.next, Some(100));
        assert_eq!((more.from, more.text.as_str()), (size, ""));
        // a short file is read whole
        let whole = codex_transcript_in(&root, "D:/app", None, None, 0, Some(size + 10));
        assert_eq!(whole.from, 0);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn nothing_before_since_or_outside_the_sessions_folder() {
        let root = tmp("since");
        let p = rollout(&root, "2026/09/25", "a", "D:\\app", "");
        let got = codex_transcript_in(&root, "D:\\app", Some(u64::MAX / 2), None, 0, None);
        assert_eq!(got.path, "");
        let outside = std::env::temp_dir().join(format!("maestro-codex-outside-{}.jsonl", std::process::id()));
        std::fs::write(&outside, "{}\n").unwrap();
        let got = codex_transcript_in(&root, "D:\\nowhere", None, Some(&outside.to_string_lossy()), 0, None);
        assert_eq!(got.path, "");
        assert_eq!(session_cwd(&p).as_deref(), Some("D:\\app"));
        let _ = std::fs::remove_file(&outside);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn rollout_ids_power_resume() {
        let root = tmp("resume");
        let id = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let mine = rollout(&root, "2026/09/25", &format!("2026-09-25T10-00-00-{id}"), "D:/app", "");
        assert_eq!(rollout_id(&mine).as_deref(), Some(id));
        assert_eq!(rollout_id(Path::new("rollout-nope.jsonl")), None);
        let p = find_rollout(&root, "D:/app", 0).unwrap();
        assert_eq!(rollout_id(&p).as_deref(), Some(id));
        assert!(rollout_with_id(&root, id));
        assert!(!rollout_with_id(&root, "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
