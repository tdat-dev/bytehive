//! Saving sessions: every 20 s, each agent's session is read, cut into segments,
//! and only what changed since the last round is sealed and sent. Runs on a
//! thread of its own; the UI only hands it the list of agents (`set_sources`)
//! and reads `status`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use super::crypto::{Keys, seal, sid};
use super::items::{Meta, digest, segments, title};
use super::relay::{Fail, Put, Segment};

/// One agent's session to save.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub cli: String,
    pub session: String,
    /// Where the CLI ran: its transcript is filed under this folder.
    pub run_dir: String,
    pub agent: String,
    pub project: String,
    pub started: u64,
}

/// What was sent per session: the header's digest and each segment's.
#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
pub struct Sent {
    sessions: HashMap<String, SentSession>,
}

#[derive(Serialize, Deserialize, Default, Debug, PartialEq, Clone)]
struct SentSession {
    meta: String,
    segs: Vec<String>,
}

/// The line under Remote in Settings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Status {
    pub saved: usize,
    pub last_ok_ms: Option<u64>,
    pub error: Option<String>,
}

static SOURCES: LazyLock<Mutex<Vec<Source>>> = LazyLock::new(|| Mutex::new(Vec::new()));
static STATUS: LazyLock<Mutex<Status>> = LazyLock::new(|| Mutex::new(Status::default()));
static STARTED: std::sync::Once = std::sync::Once::new();

pub fn set_sources(list: Vec<Source>) {
    *SOURCES.lock().unwrap_or_else(|p| p.into_inner()) = list;
}

pub fn status() -> Status {
    STATUS.lock().unwrap_or_else(|p| p.into_inner()).clone()
}

/// Puts per call: the relay takes up to 20 segments in one.
const PER_PUT: usize = 20;

/// One round over `sources`. `read` gives a session's items (None: nothing to save
/// yet); `put` sends to the relay. Returns how many sessions are saved now.
/// The reader and the sender are passed in so the tests and the e2e run use the real round.
#[allow(clippy::too_many_arguments)]
pub fn round(
    keys: &Keys,
    machine: &str,
    machine_name: &str,
    sources: &[Source],
    sent: &mut Sent,
    now_ms: u64,
    read: &mut dyn FnMut(&Source) -> Option<Vec<crate::transcript::Item>>,
    put: &mut dyn FnMut(&Put) -> Result<(), Fail>,
) -> Result<usize, Fail> {
    let mut saved = 0;
    for s in sources {
        let Some(items) = read(s) else { continue };
        if items.is_empty() {
            continue;
        }
        let id = sid(keys, machine, &s.cli, &s.session);
        let segs = segments(&items);
        let meta = Meta {
            title: title(&items),
            cli: s.cli.clone(),
            project: s.project.clone(),
            machine: machine_name.to_string(),
            agent: s.agent.clone(),
            started: s.started,
            updated: now_ms,
            items: items.len(),
        };
        // The header changes with every round's time: compare it without that.
        let meta_print = digest(&format!("{}|{}|{}|{}|{}", meta.title, meta.agent, meta.project, meta.items, segs.len()));
        let before = sent.sessions.get(&id).cloned().unwrap_or_default();
        let changed: Vec<usize> = (0..segs.len()).filter(|&n| before.segs.get(n) != Some(&digest(&segs[n]))).collect();
        if changed.is_empty() && before.meta == meta_print {
            saved += 1;
            continue;
        }
        let meta_sealed = seal(keys, &format!("{id}/meta"), &serde_json::to_vec(&meta).unwrap_or_default()).map_err(Fail::Net)?;
        // An unchanged session that only needs its header still sends one put.
        let batches: Vec<&[usize]> = if changed.is_empty() { vec![&[]] } else { changed.chunks(PER_PUT).collect() };
        for batch in batches {
            let mut out = Vec::with_capacity(batch.len());
            for &n in batch {
                out.push(Segment { n, sealed: seal(keys, &format!("{id}/{n}"), segs[n].as_bytes()).map_err(Fail::Net)? });
            }
            put(&Put { sid: id.clone(), meta: meta_sealed.clone(), updated: now_ms, segments: out })?;
        }
        sent.sessions.insert(id, SentSession { meta: meta_print, segs: segs.iter().map(|s| digest(s)).collect() });
        saved += 1;
    }
    Ok(saved)
}

fn sent_path() -> Option<std::path::PathBuf> {
    crate::fleet::data_dir().map(|d| d.join("remote-sent.json"))
}

fn load_sent() -> Sent {
    sent_path().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save_sent(s: &Sent) {
    if let (Some(p), Ok(b)) = (sent_path(), serde_json::to_vec(s)) {
        let _ = std::fs::write(p, b);
    }
}

/// Forget what was sent: after a new passphrase everything goes again.
pub fn reset_sent() {
    save_sent(&Sent::default());
}

/// A session's chat items, for the CLIs whose transcript is read so far.
fn read_items(s: &Source) -> Option<Vec<crate::transcript::Item>> {
    match s.cli.as_str() {
        "claude" => crate::transcript::read_session(&s.run_dir, &s.session, None).ok().map(|r| r.chat.items),
        "codex" => super::codex::read(&s.session),
        "opencode" => super::opencode::read(&s.session),
        _ => None,
    }
}

fn machine_name() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "This computer".into())
}

/// Start the saving thread once; it does nothing while Remote is off.
pub fn start() {
    STARTED.call_once(|| {
        std::thread::spawn(|| {
            let mut sent = load_sent();
            loop {
                let saved = super::secret::load();
                if let (true, Some(token), Some(keys)) = (saved.on, saved.token.clone(), saved.keys()) {
                    let sources = SOURCES.lock().unwrap_or_else(|p| p.into_inner()).clone();
                    let now = crate::fleet::now_ms();
                    let r = round(&keys, &saved.machine, &machine_name(), &sources, &mut sent, now, &mut read_items, &mut |p| super::relay::put(&token, p));
                    let mut st = STATUS.lock().unwrap_or_else(|p| p.into_inner());
                    match r {
                        Ok(n) => {
                            *st = Status { saved: n, last_ok_ms: Some(now), error: None };
                            save_sent(&sent);
                        }
                        Err(e) => st.error = Some(e.words()),
                    }
                }
                std::thread::sleep(Duration::from_secs(20));
            }
        });
    });
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;
