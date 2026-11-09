//! When an agent finishes a piece of work, say so: done, ready to review, or
//! stopped with an error (finishnote.ts). The "needs you" notification covers
//! an agent waiting on a question; this covers the end of a run.
//!
//! A run is one stretch of work: it opens when the agent starts working and
//! stays open through quiet moments and questions, and ends when the agent has
//! sat idle, with changes, or stopped for a few seconds. Short runs are noise
//! (a reply to "hi", a slash command) and say nothing. Finishes that land
//! within a few seconds of each other go out as one notification. No UI here:
//! inbox/notices.rs wires it up.

use std::collections::HashMap;

use supervise::task_state::TaskState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinishKind {
    Done,
    Review,
    Crash,
}

/// What a finish check needs to know about one agent right now.
pub struct FinishSnap {
    pub id: String,
    pub name: String,
    pub project: String,
    pub state: TaskState,
    /// Sitting at its plan's usage limit: Limit Guard tells you that.
    pub limited: bool,
    /// Files changed on its branch, when known.
    pub files: Option<u32>,
    /// Exit code of its process once it stopped, when known.
    pub exit_code: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Finish {
    pub id: String,
    pub name: String,
    pub project: String,
    pub kind: FinishKind,
    pub files: Option<u32>,
    pub exit_code: Option<u32>,
    /// How long the run took, ms.
    pub ran_ms: u64,
    /// When it finished, ms.
    pub at: u64,
}

/// Runs shorter than this say nothing.
pub const MIN_RUN_MS: u64 = 20_000;
/// How long an agent must stay quiet before its run counts as over: a pause
/// between two tool calls is not the end of the job.
pub const SETTLE_MS: u64 = 5_000;
/// A burst closes after this long without another finish...
pub const GROUP_MS: u64 = 3_000;
/// ...or this long after its first one, whichever comes first.
pub const GROUP_MAX_MS: u64 = 10_000;

struct Run {
    start: u64,
    /// When it stopped working, while that may still be a pause.
    quiet_since: Option<u64>,
}

#[derive(Default)]
pub struct FinishWatch {
    runs: HashMap<String, Run>,
}

impl FinishWatch {
    /// Look at every agent once (each tick). The runs that just ended with
    /// something worth telling.
    pub fn step(&mut self, snaps: &[FinishSnap], now: u64) -> Vec<Finish> {
        let mut out = Vec::new();
        for s in snaps {
            if s.state == TaskState::Working {
                self.runs.entry(s.id.clone()).or_insert(Run { start: now, quiet_since: None }).quiet_since = None;
                continue;
            }
            let Some(run) = self.runs.get_mut(&s.id) else { continue };
            // A question mid-task is part of the same run (needs-you speaks for it).
            if matches!(s.state, TaskState::Needs | TaskState::Blocked) {
                run.quiet_since = None;
                continue;
            }
            let quiet = *run.quiet_since.get_or_insert(now);
            let gone = matches!(s.state, TaskState::Stopped | TaskState::Merged);
            if !gone && now - quiet < SETTLE_MS {
                continue;
            }
            let start = run.start;
            self.runs.remove(&s.id);
            let Some(kind) = kind_of(s) else { continue };
            if quiet - start < MIN_RUN_MS {
                continue;
            }
            out.push(Finish { id: s.id.clone(), name: s.name.clone(), project: s.project.clone(), kind, files: s.files, exit_code: s.exit_code, ran_ms: quiet - start, at: quiet });
        }
        self.runs.retain(|id, _| snaps.iter().any(|s| &s.id == id));
        out
    }
}

/// What an ended run comes to, or None when there is nothing to tell.
fn kind_of(s: &FinishSnap) -> Option<FinishKind> {
    match s.state {
        TaskState::Stopped => s.exit_code.is_some_and(|c| c != 0).then_some(FinishKind::Crash),
        TaskState::Merged => None,
        _ if s.limited => None,
        TaskState::Review => Some(FinishKind::Review),
        TaskState::Idle => Some(FinishKind::Done),
        _ => None,
    }
}

/// Collects finishes into bursts: several agents done at once is one notification.
#[derive(Default)]
pub struct Burst {
    items: Vec<Finish>,
    first: u64,
    latest: u64,
}

impl Burst {
    pub fn add(&mut self, list: Vec<Finish>, now: u64) {
        if list.is_empty() {
            return;
        }
        if self.items.is_empty() {
            self.first = now;
        }
        self.latest = now;
        for f in list {
            // The same agent twice in one burst: the newer finish says it.
            self.items.retain(|x| x.id != f.id);
            self.items.push(f);
        }
    }

    /// The burst, once it is over; None while it may still grow.
    pub fn take(&mut self, now: u64) -> Option<Vec<Finish>> {
        if self.items.is_empty() || (now - self.latest < GROUP_MS && now - self.first < GROUP_MAX_MS) {
            return None;
        }
        Some(std::mem::take(&mut self.items))
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }
}

/// "45 s", "4 min", "1 h 20 min".
pub fn ran_for(ms: u64) -> String {
    let s = (ms as f64 / 1000.0).round() as u64;
    if s < 60 {
        return format!("{s} s");
    }
    let m = (s as f64 / 60.0).round() as u64;
    if m < 60 {
        return format!("{m} min");
    }
    if m % 60 != 0 { format!("{} h {} min", m / 60, m % 60) } else { format!("{} h", m / 60) }
}

fn files_text(n: Option<u32>) -> String {
    match n {
        Some(n) if n > 0 => format!("{n} file{} changed", if n == 1 { "" } else { "s" }),
        _ => "changes to review".to_string(),
    }
}

/// One agent's finish in a few words, after its name.
fn what(f: &Finish) -> String {
    match f.kind {
        FinishKind::Review => format!("ready to review, {}", files_text(f.files)),
        FinishKind::Crash => "stopped with an error".to_string(),
        FinishKind::Done => "done".to_string(),
    }
}

fn weight(k: FinishKind) -> u8 {
    match k {
        FinishKind::Crash => 0,
        FinishKind::Review => 1,
        FinishKind::Done => 2,
    }
}

/// Names a grouped notification lists before "and N more".
const LIST_MAX: usize = 3;

/// A notification: what it says, and the agent a click opens.
#[derive(Debug, PartialEq)]
pub struct Note {
    pub title: String,
    pub body: String,
    pub open: String,
}

/// The notification for a burst; a click opens the agent that most wants
/// you (an error, then changes to review), first to finish.
pub fn finish_note(list: &[Finish]) -> Option<Note> {
    let mut order: Vec<&Finish> = list.iter().collect();
    order.sort_by_key(|f| (weight(f.kind), f.at));
    let top = *order.first()?;
    let open = top.id.clone();
    if order.len() == 1 {
        let (title, body) = match top.kind {
            FinishKind::Review => (format!("{} is ready to review", top.name), format!("{} in {}, after {}.", files_text(top.files), top.project, ran_for(top.ran_ms))),
            FinishKind::Crash => (format!("{} stopped with an error", top.name), format!("It exited with code {} after {}, in {}.", top.exit_code.map(|c| c.to_string()).unwrap_or_else(|| "?".into()), ran_for(top.ran_ms), top.project)),
            FinishKind::Done => (format!("{} is done", top.name), format!("Worked {} in {}.", ran_for(top.ran_ms), top.project)),
        };
        return Some(Note { title, body, open });
    }
    let mut lines: Vec<String> = order.iter().take(LIST_MAX).map(|f| format!("{}: {}", f.name, what(f))).collect();
    if order.len() > LIST_MAX {
        lines.push(format!("and {} more", order.len() - LIST_MAX));
    }
    Some(Note { title: format!("{} agents finished", order.len()), body: lines.join("\n"), open })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(id: &str, state: TaskState) -> FinishSnap {
        FinishSnap { id: id.into(), name: id.to_uppercase(), project: "web".into(), state, limited: false, files: None, exit_code: None }
    }

    #[test]
    fn a_long_run_that_settles_is_done() {
        let mut w = FinishWatch::default();
        assert!(w.step(&[snap("a", TaskState::Working)], 0).is_empty());
        assert!(w.step(&[snap("a", TaskState::Idle)], 30_000).is_empty());
        assert!(w.step(&[snap("a", TaskState::Idle)], 33_000).is_empty());
        let got = w.step(&[snap("a", TaskState::Idle)], 35_000);
        assert_eq!(got.len(), 1);
        assert_eq!((got[0].kind, got[0].ran_ms), (FinishKind::Done, 30_000));
    }

    #[test]
    fn short_runs_and_questions_say_nothing() {
        let mut w = FinishWatch::default();
        w.step(&[snap("a", TaskState::Working)], 0);
        assert!(w.step(&[snap("a", TaskState::Idle)], 5_000).is_empty());
        assert!(w.step(&[snap("a", TaskState::Idle)], 11_000).is_empty());
        w.step(&[snap("b", TaskState::Working)], 0);
        w.step(&[snap("b", TaskState::Needs)], 25_000);
        assert!(w.step(&[snap("b", TaskState::Needs)], 60_000).is_empty());
    }

    #[test]
    fn a_crash_tells_a_stop_does_not() {
        let mut w = FinishWatch::default();
        w.step(&[snap("a", TaskState::Working), snap("b", TaskState::Working)], 0);
        let mut a = snap("a", TaskState::Stopped);
        a.exit_code = Some(1);
        let b = snap("b", TaskState::Stopped);
        let got = w.step(&[a, b], 40_000);
        assert_eq!(got.iter().map(|f| f.kind).collect::<Vec<_>>(), vec![FinishKind::Crash]);
    }

    #[test]
    fn a_burst_groups_and_puts_the_crash_first() {
        let f = |id: &str, kind, at| Finish { id: id.into(), name: id.into(), project: "web".into(), kind, files: Some(2), exit_code: Some(3), ran_ms: 60_000, at };
        let mut b = Burst::default();
        b.add(vec![f("ana", FinishKind::Done, 0)], 0);
        b.add(vec![f("bo", FinishKind::Crash, 1_000)], 1_000);
        assert!(b.take(2_000).is_none());
        let list = b.take(4_500).unwrap();
        let n = finish_note(&list).unwrap();
        assert_eq!(n.title, "2 agents finished");
        assert_eq!(n.body, "bo: stopped with an error\nana: done");
        assert_eq!(n.open, "bo");
        let one = finish_note(&[f("ana", FinishKind::Review, 0)]).unwrap();
        assert_eq!((one.title.as_str(), one.body.as_str()), ("ana is ready to review", "2 files changed in web, after 1 min."));
    }

    #[test]
    fn says_how_long() {
        assert_eq!(ran_for(45_000), "45 s");
        assert_eq!(ran_for(240_000), "4 min");
        assert_eq!(ran_for(80 * 60_000), "1 h 20 min");
        assert_eq!(ran_for(120 * 60_000), "2 h");
    }
}
