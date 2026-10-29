//! Muse sessions: logs, session lists, transcript slices.
//!
//! Logs live under `<data>/muse/sessions/YYYY/MM/DD/<id>/session.jsonl`.
//! Unlike Claude, a run can't be given its session id up front, so a UI
//! learns a new run's id from what appeared since before it spawned.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::transcript::{read_start, TranscriptChunk};

/// Most bytes one read hands back, so a long session loads in slices.
const MUSE_SLICE: u64 = 2 * 1024 * 1024;

pub fn sessions_root() -> Option<PathBuf> {
    let data = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .ok()
        .filter(|p| p.is_absolute())
        .or_else(|| {
            let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).ok()?;
            Some(Path::new(&home).join(".local").join("share"))
        })?;
    Some(data.join("muse").join("sessions"))
}

/// A Muse session id only ever names a directory we enumerate, never a path
/// we build from it, so this is just a shape guard: ULID-ish ids like
/// `01a18216-5bd8-…` (hex and dashes) pass, anything odd stays out.
pub fn is_session_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

/// Every Muse session under `root`: (id, when its log last changed). Four
/// levels deep exactly (`YYYY/MM/DD/<id>/session.jsonl`); anything nested
/// deeper (subagent logs) is not a session of its own.
pub fn session_files(root: &Path) -> Vec<(String, u64)> {
    fn dirs(dir: &Path) -> Vec<std::path::PathBuf> {
        let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
        entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect()
    }
    let mut out = Vec::new();
    for y in dirs(root) {
        for m in dirs(&y) {
            for d in dirs(&m) {
                for id_dir in dirs(&d) {
                    let Some(id) = id_dir.file_name().and_then(|s| s.to_str()) else { continue };
                    if id == "subagent" || !is_session_id(id) {
                        continue;
                    }
                    let log = id_dir.join("session.jsonl");
                    if !log.is_file() {
                        continue;
                    }
                    let ms = std::fs::metadata(&log)
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0);
                    out.push((id.to_string(), ms));
                }
            }
        }
    }
    out
}

/// One Muse conversation: its session id and when its log last changed.
#[derive(Serialize, Clone)]
pub struct MuseSession {
    pub id: String,
    pub modified_ms: u64,
}

/// The conversations Muse has, newest first. A UI diffs this against before
/// it spawned to learn a new run's session id.
pub fn sessions() -> Vec<MuseSession> {
    let Some(root) = sessions_root() else { return Vec::new() };
    sessions_in(&root)
}

pub fn sessions_in(root: &Path) -> Vec<MuseSession> {
    let mut out: Vec<MuseSession> = session_files(root)
        .into_iter()
        .map(|(id, modified_ms)| MuseSession { id, modified_ms })
        .collect();
    out.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms));
    out.truncate(200);
    out
}

/// Whether `muse resume <id>` can pick this session up.
pub fn session_exists(session_id: &str) -> bool {
    if !is_session_id(session_id) {
        return false;
    }
    let Some(root) = sessions_root() else { return false };
    session_files(&root).iter().any(|(id, _)| *id == session_id)
}

/// New complete lines of a Muse agent's session log from `offset`: the
/// agent's own session when `session_id` names one, else nothing (`path`
/// "" until one is known).
pub fn transcript(session_id: Option<&str>, offset: u64, tail_bytes: Option<u64>) -> TranscriptChunk {
    let Some(id) = session_id.filter(|s| is_session_id(s)) else { return TranscriptChunk::default() };
    let Some(root) = sessions_root() else { return TranscriptChunk::default() };
    transcript_in(&root, id, offset, tail_bytes)
}

pub fn transcript_in(root: &Path, id: &str, offset: u64, tail: Option<u64>) -> TranscriptChunk {
    use std::io::{Read, Seek, SeekFrom};
    let Some(log) = session_log(root, id) else { return TranscriptChunk::default() };
    let Ok(mut file) = std::fs::File::open(&log) else {
        return TranscriptChunk { path: id.to_string(), text: String::new(), next: offset, from: offset };
    };
    let size = file.metadata().map(|m| m.len()).unwrap_or(0);
    let start = read_start(&mut file, size, offset, tail);
    let want = size.saturating_sub(start).min(MUSE_SLICE);
    let mut buf = vec![0u8; want as usize];
    if file.seek(SeekFrom::Start(start)).is_err() || file.read_exact(&mut buf).is_err() {
        return TranscriptChunk { path: id.to_string(), text: String::new(), next: start, from: start };
    }
    // Hand back whole lines only; the rest is read next time.
    let cut = buf.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
    buf.truncate(cut);
    TranscriptChunk { path: id.to_string(), text: String::from_utf8_lossy(&buf).into_owned(), next: start + cut as u64, from: start }
}

/// A session's log: the `<id>/session.jsonl` under any date shard. The id
/// never reaches the filesystem unvalidated.
pub fn session_log(root: &Path, id: &str) -> Option<std::path::PathBuf> {
    if !is_session_id(id) {
        return None;
    }
    for y in crate::codex::sorted_dirs(root) {
        for m in crate::codex::sorted_dirs(&y) {
            for d in crate::codex::sorted_dirs(&m) {
                let log = d.join(id).join("session.jsonl");
                if log.is_file() {
                    return Some(log);
                }
            }
        }
    }
    None
}

/// A Muse conversation for the History list: its id, opening prompt, last
/// activity, and message count.
#[derive(Serialize, Clone)]
pub struct MuseListedSession {
    pub id: String,
    pub title: String,
    pub modified_ms: u64,
    pub messages: u64,
}

/// Muse conversations, newest first, for the History list.
pub fn list(limit: usize) -> Vec<MuseListedSession> {
    let Some(root) = sessions_root() else { return Vec::new() };
    list_in(&root, limit)
}

pub fn list_in(root: &Path, limit: usize) -> Vec<MuseListedSession> {
    let mut files = session_files(root);
    files.sort_by(|a, b| b.1.cmp(&a.1));
    files.truncate(limit.min(300));
    let mut out = Vec::new();
    for (id, ms) in files {
        let Some(log) = session_log(root, &id) else { continue };
        let (title, messages) = session_head(&log);
        out.push(MuseListedSession { id, title, modified_ms: ms, messages });
    }
    out
}

/// A session's title (its first user prompt) and message count, from at most
/// the first 4 MB: enough for any real head, bounded for giant logs.
pub fn session_head(log: &Path) -> (String, u64) {
    const HEAD: u64 = 4 * 1024 * 1024;
    let Ok(mut file) = std::fs::File::open(log) else { return (String::new(), 0) };
    let mut buf = Vec::new();
    {
        use std::io::Read;
        if file.by_ref().take(HEAD).read_to_end(&mut buf).is_err() {
            return (String::new(), 0);
        }
    }
    let text = String::from_utf8_lossy(&buf);
    let mut title = String::new();
    let mut messages = 0u64;
    for line in text.split('\n') {
        let Some(k) = line.find("\"event\":{\"kind\":\"") else { continue };
        let rest = &line[k + 17..];
        if rest.starts_with("assistant_message_committed\"") {
            messages += 1;
        } else if rest.starts_with("started\"") {
            if let Some(p) = prompt_of(line, false) {
                if title.is_empty() {
                    title = p;
                }
                messages += 1;
            }
        } else if rest.starts_with("inbox_item_queued\"") {
            if let Some(p) = prompt_of(line, true) {
                if title.is_empty() {
                    title = p;
                }
                messages += 1;
            }
        }
    }
    (title, messages)
}

/// A line's user prompt, if it carries one: a run's opening prompt, or a
/// message steered in mid-run. Background deliveries are not the user.
pub fn prompt_of(line: &str, queued: bool) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let ev = v.get("payload")?.get("event")?;
    if queued {
        if ev.get("source")?.get("source")?.as_str()? != "user_steer" {
            return None;
        }
        let p = ev.get("payload").and_then(|p| p.get("prompt")).and_then(|p| p.as_str()).unwrap_or("");
        let b = ev.get("body").and_then(|b| b.as_str()).unwrap_or("");
        let t = if !p.trim().is_empty() { p } else { b };
        return if t.trim().is_empty() { None } else { Some(trim_title(t)) };
    }
    let p = ev.get("prompt")?.as_str()?;
    if p.trim().is_empty() { None } else { Some(trim_title(p)) }
}

/// A prompt as a list title: one line, at most 80 characters.
pub fn trim_title(t: &str) -> String {
    let one = t.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() <= 80 {
        return one;
    }
    let mut s: String = one.chars().take(79).collect();
    s.push('…');
    s
}


#[cfg(test)]
mod tests {
    use super::*;

    fn muse_tree() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("maestro-muse-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for id in [
            "01a18216-5bd8-7b20-9b10-9f481ee54a74",
            "01a18217-0000-4000-8000-000000000000",
        ] {
            let dir = root.join("2026").join("10").join("28").join(id);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("session.jsonl"), "{}\n").unwrap();
        }
        // A subagent log nested deeper is not a session of its own.
        let sub = root
            .join("2026")
            .join("10")
            .join("28")
            .join("01a18216-5bd8-7b20-9b10-9f481ee54a74")
            .join("subagent")
            .join("01a18218-1111-4222-8333-444444444444");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("session.jsonl"), "{}\n").unwrap();
        // An id dir without a log is a crashed start, not a session.
        std::fs::create_dir_all(
            root.join("2026").join("10").join("28").join("01a18219-2222-4333-8444-555555555555"),
        )
        .unwrap();
        root
    }

    #[test]
    fn lists_top_level_sessions_only() {
        let root = muse_tree();
        let mut ids: Vec<String> = session_files(&root).into_iter().map(|(id, _)| id).collect();
        ids.sort();
        assert_eq!(
            ids,
            vec![
                "01a18216-5bd8-7b20-9b10-9f481ee54a74".to_string(),
                "01a18217-0000-4000-8000-000000000000".to_string(),
            ]
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn id_shapes() {
        assert!(is_session_id("01a18216-5bd8-7b20-9b10-9f481ee54a74"));
        assert!(!is_session_id(""));
        assert!(!is_session_id("../evil"));
        assert!(!is_session_id("01a18216 5bd8"));
    }

    #[test]
    fn transcript_reads_whole_lines_in_slices() {
        let root = std::env::temp_dir().join(format!("maestro-muse-tx-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("2026").join("10").join("28").join("01a18216-5bd8-7b20-9b10-9f481ee54a74");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("session.jsonl"), "{\"a\":1}\n{\"b\":2}\n{\"half\":").unwrap();
        let id = "01a18216-5bd8-7b20-9b10-9f481ee54a74";
        let c = transcript_in(&root, id, 0, None);
        assert_eq!(c.path, id);
        assert_eq!(c.text, "{\"a\":1}\n{\"b\":2}\n"); // the half-written last one waits
        assert_eq!(c.from, 0);
        let c2 = transcript_in(&root, id, c.next, None);
        assert_eq!(c2.text, "");
        // unknown id: nothing yet; a hostile id never reaches the filesystem
        assert_eq!(transcript_in(&root, "00000000-0000-4000-8000-000000000000", 0, None).path, "");
        assert_eq!(session_log(&root, "../../evil"), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn session_list_titles_and_counts() {
        let root = std::env::temp_dir().join(format!("maestro-muse-list-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let a = root.join("2026").join("10").join("28").join("01a18216-5bd8-7b20-9b10-9f481ee54a74");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::write(a.join("session.jsonl"), concat!(
            "{\"id\":\"1\",\"payload\":{\"event\":{\"kind\":\"started\",\"prompt\":\"fix the login\"}}}\n",
            "{\"id\":\"2\",\"payload\":{\"event\":{\"kind\":\"started\",\"task_id\":\"t\"}}}\n",
            "{\"id\":\"3\",\"payload\":{\"event\":{\"kind\":\"assistant_message_committed\",\"message_id\":\"m\",\"text\":\"ok\"}}}\n",
            "{\"id\":\"4\",\"payload\":{\"event\":{\"kind\":\"inbox_item_queued\",\"source\":{\"source\":\"cron\"},\"payload\":{\"prompt\":\"tick\"}}}}\n",
            "{\"id\":\"5\",\"payload\":{\"event\":{\"kind\":\"inbox_item_queued\",\"source\":{\"source\":\"user_steer\"},\"payload\":{\"prompt\":\"wait\"}}}}\n",
        )).unwrap();
        let (title, messages) = session_head(&a.join("session.jsonl"));
        assert_eq!(title, "fix the login");
        assert_eq!(messages, 3); // prompt + reply + steer; bare started and cron skipped
        assert_eq!(prompt_of("{\"payload\":{\"event\":{\"kind\":\"started\",\"prompt\":\"  \"}}}", false), None);
        let long = "w ".repeat(100);
        assert!(trim_title(&long).chars().count() <= 80);
        let _ = std::fs::remove_dir_all(&root);
    }
}