//! Notices when a working agent is stuck: looping the same failure, or busy
//! for a long while without its branch moving. Reads the conversation plus the
//! branch's diff fingerprint. Advisory only: it reports evidence, it never
//! stops anything.

use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::task_state::TaskState;
use crate::types::{ChatItem, StepItem};

/// Same failing action this many times in the recent window counts as a loop.
pub const LOOP_COUNT: usize = 3;
/// Completed steps after your last message looked at for a loop.
pub const LOOP_WINDOW: usize = 12;
/// A branch this long unchanged, while steps land without touching files, counts as a stall.
pub const STALL_MS: u64 = 10 * 60_000;
/// Finished steps in a row that touched no files, needed to call it a stall.
pub const STALL_STEPS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StuckKind {
    Loop,
    Stall,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StuckEvidence {
    pub kind: StuckKind,
    /// Plain words with the count ("same failure 3 times: FAIL x").
    pub text: String,
    pub count: usize,
    /// The repeated error line, or "stall".
    pub sample: String,
    /// ms the first matching step ran, 0 when unknown.
    pub since: u64,
}

/// Finished steps since you last spoke: a new instruction restarts the count.
fn fresh_steps(items: &[ChatItem]) -> Vec<&StepItem> {
    let mut start = 0;
    for (i, it) in items.iter().enumerate().rev() {
        if it.kind() == "user" {
            start = i + 1;
            break;
        }
    }
    items[start..].iter().filter_map(|it| match it {
        ChatItem::Step(s) if s.done => Some(s),
        _ => None,
    }).collect()
}

fn digits_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[0-9][0-9.,:]*").unwrap())
}

fn spaces_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\s+").unwrap())
}

fn first_line(output: Option<&str>) -> String {
    output
        .unwrap_or("")
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l).trim())
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_string()
}

/// First error line, with run-specific numbers blanked so "failed after 3
/// attempts" and "failed after 4 attempts" read as the same failure.
fn err_line_of(s: &StepItem) -> String {
    let first = first_line(s.output.as_deref()).to_lowercase();
    let flat = spaces_re().replace_all(&first, " ");
    let blanked = digits_re().replace_all(&flat, "#");
    if blanked.chars().count() > 160 {
        blanked.chars().take(160).collect()
    } else {
        blanked.to_string()
    }
}

/// The error's own first line for the evidence, else what it ran.
fn show_line(s: &StepItem) -> String {
    let first = first_line(s.output.as_deref());
    let shown = if first.is_empty() { s.target.clone() } else { first };
    let shown = shown.trim();
    if shown.chars().count() > 80 {
        shown.chars().take(80).collect()
    } else {
        shown.to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loop {
    pub count: usize,
    pub sample: String,
    pub since: u64,
}

/// The same failure repeating in the recent steps, or none. Only failures
/// count: an agent that runs green commands over and over is busy, not
/// looping. Ties go to the failure seen last.
pub fn loop_of(items: &[ChatItem]) -> Option<Loop> {
    let fresh = fresh_steps(items);
    let tail = if fresh.len() > LOOP_WINDOW { &fresh[fresh.len() - LOOP_WINDOW..] } else { &fresh[..] };
    struct Count {
        count: usize,
        sample: String,
        since: u64,
        last: usize,
    }
    let mut counts: HashMap<String, Count> = HashMap::new();
    for (i, s) in tail.iter().enumerate() {
        if !s.error {
            continue;
        }
        let sig = format!("{}\u{2016}{}\u{2016}{}", s.tool, s.target, err_line_of(s));
        match counts.get_mut(&sig) {
            Some(cur) => {
                cur.count += 1;
                cur.sample = show_line(s);
                cur.last = i;
            }
            None => {
                counts.insert(sig, Count { count: 1, sample: show_line(s), since: s.at, last: i });
            }
        }
    }
    let mut best: Option<Count> = None;
    for c in counts.into_values() {
        if c.count < LOOP_COUNT {
            continue;
        }
        let take = match &best {
            None => true,
            Some(b) => c.count > b.count || (c.count == b.count && c.last > b.last),
        };
        if take {
            best = Some(c);
        }
    }
    best.map(|b| Loop { count: b.count, sample: b.sample, since: b.since })
}

/// A step counts as touching files when it says what changed, or its verb is
/// an edit ("Updated the plan" is not: plans aren't files).
fn touched_files(s: &StepItem) -> bool {
    if s.added.unwrap_or(0) > 0 || s.removed.unwrap_or(0) > 0 || s.diff.as_ref().is_some_and(|d| !d.is_empty()) {
        return true;
    }
    s.verb == "Edited" || s.verb == "Wrote"
}

/// The recent run of finished steps that touched no files, when it is long
/// enough to suspect a stall. None while a step is still open: a long test
/// running is not stuck.
pub fn stall_steps(items: &[ChatItem]) -> Option<Vec<StepItem>> {
    let steps: Vec<&StepItem> = items.iter().filter_map(|it| match it {
        ChatItem::Step(s) => Some(s),
        _ => None,
    }).collect();
    let newest = steps.last()?;
    if !newest.done {
        return None;
    }
    let fresh = fresh_steps(items);
    let tail = if fresh.len() > STALL_STEPS { &fresh[fresh.len() - STALL_STEPS..] } else { &fresh[..] };
    if tail.len() < STALL_STEPS || tail.iter().any(|s| touched_files(s)) {
        return None;
    }
    Some(tail.iter().map(|s| (*s).clone()).collect())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffPrint {
    pub n: u64,
    pub add: u64,
    pub del: u64,
}

#[derive(Debug, Clone)]
pub struct StuckSnap {
    /// A restart forgets the old run.
    pub run: u64,
    pub state: TaskState,
    pub items: Vec<ChatItem>,
    /// Branch diff fingerprint, or none when the diff isn't one agent's.
    pub diff: Option<DiffPrint>,
    pub now: u64,
}

#[derive(Debug)]
struct Trail {
    run: u64,
    print: Option<String>,
    stable_since: u64,
}

#[derive(Debug, Default)]
pub struct StuckWatch {
    trails: HashMap<String, Trail>,
}

impl StuckWatch {
    pub fn new() -> StuckWatch {
        StuckWatch::default()
    }

    /// What this agent's stuck verdict is now, or none. Only a working agent
    /// can be stuck; without a conversation there is nothing to read.
    pub fn step(&mut self, pane_id: &str, snap: &StuckSnap) -> Option<StuckEvidence> {
        let reset = self.trails.get(pane_id).map(|t| t.run != snap.run).unwrap_or(true);
        if reset {
            self.trails.insert(pane_id.to_string(), Trail { run: snap.run, print: None, stable_since: snap.now });
        }
        let trail = self.trails.get_mut(pane_id)?;
        let print = snap.diff.as_ref().map(|d| format!("{}:{}:{}", d.n, d.add, d.del));
        if print != trail.print {
            trail.print = print;
            trail.stable_since = snap.now;
        }
        if snap.state != TaskState::Working {
            return None;
        }
        if let Some(found) = loop_of(&snap.items) {
            return Some(StuckEvidence {
                kind: StuckKind::Loop,
                text: format!("same failure {} times: {}", found.count, found.sample),
                count: found.count,
                sample: found.sample,
                since: found.since,
            });
        }
        if snap.diff.is_none() {
            return None;
        }
        let stalled = stall_steps(&snap.items)?;
        if snap.now.saturating_sub(trail.stable_since) < STALL_MS {
            return None;
        }
        let mins = snap.now.saturating_sub(trail.stable_since) / 60_000;
        Some(StuckEvidence {
            kind: StuckKind::Stall,
            text: format!("no file changes in {mins}m across {} steps", stalled.len()),
            count: stalled.len(),
            sample: "stall".to_string(),
            since: trail.stable_since,
        })
    }

    /// Forget agents that are gone (a closed pane), or all when none is named.
    pub fn forget(&mut self, pane_id: Option<&str>) {
        match pane_id {
            Some(id) => {
                self.trails.remove(id);
            }
            None => self.trails.clear(),
        }
    }
}
