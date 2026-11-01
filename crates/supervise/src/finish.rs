//! When an agent finishes a piece of work, say so: done, ready to review, or
//! stopped with an error. "Needs you" covers an agent waiting on a question;
//! this covers the end of a run.
//!
//! A run is one stretch of work: it opens when the agent starts working,
//! stays open through quiet moments and questions, and ends when the agent
//! has sat idle, with changes, or stopped for a few seconds. Short runs are
//! noise (a reply to "hi") and say nothing. Finishes within a few seconds of
//! each other go out as one notification. Port of the web app's `finishnote.ts`.

use std::collections::HashMap;

use crate::task_state::TaskState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinishKind {
    Crash,
    Review,
    Done,
}

/// What a finish check needs to know about one agent right now.
#[derive(Clone, Debug)]
pub struct FinishSnap {
    pub pane_id: String,
    pub name: String,
    pub project: String,
    pub state: TaskState,
    /// It sits at its plan's usage limit: the limit notice says that.
    pub limited: bool,
    pub files: Option<u32>,
    pub exit_code: Option<u32>,
    /// You (or Maestro) asked it to stop just now.
    pub stop_asked: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Finish {
    pub pane_id: String,
    pub name: String,
    pub project: String,
    pub kind: FinishKind,
    pub files: Option<u32>,
    pub exit_code: Option<u32>,
    pub ran_ms: u64,
    pub at: u64,
}

/// Runs shorter than this say nothing.
pub const MIN_RUN_MS: u64 = 20_000;
/// How long an agent must stay quiet before its run counts as over.
pub const SETTLE_MS: u64 = 5_000;
/// A burst closes after this long without another finish...
pub const GROUP_MS: u64 = 3_000;
/// ...or this long after its first one, whichever comes first.
pub const GROUP_MAX_MS: u64 = 10_000;

struct Run {
    start: u64,
    quiet_since: Option<u64>,
}

#[derive(Default)]
pub struct FinishWatch {
    runs: HashMap<String, Run>,
}

fn kind_of(s: &FinishSnap) -> Option<FinishKind> {
    match s.state {
        TaskState::Stopped => (s.exit_code.is_some_and(|c| c != 0) && !s.stop_asked).then_some(FinishKind::Crash),
        _ if s.state == TaskState::Merged || s.limited => None,
        TaskState::Review => Some(FinishKind::Review),
        TaskState::Idle => Some(FinishKind::Done),
        _ => None,
    }
}

impl FinishWatch {
    /// Look at every agent once (each tick); the runs that just ended with
    /// something worth telling.
    pub fn step(&mut self, snaps: &[FinishSnap], now: u64) -> Vec<Finish> {
        let mut out = Vec::new();
        for s in snaps {
            if s.state == TaskState::Working {
                self.runs.entry(s.pane_id.clone()).or_insert(Run { start: now, quiet_since: None }).quiet_since = None;
                continue;
            }
            let Some(run) = self.runs.get_mut(&s.pane_id) else { continue };
            // A question mid-task is part of the same run.
            if s.state == TaskState::Needs {
                run.quiet_since = None;
                continue;
            }
            let quiet = *run.quiet_since.get_or_insert(now);
            let gone = matches!(s.state, TaskState::Stopped | TaskState::Merged);
            if !gone && now - quiet < SETTLE_MS {
                continue;
            }
            let start = run.start;
            self.runs.remove(&s.pane_id);
            let Some(kind) = kind_of(s) else { continue };
            if quiet - start < MIN_RUN_MS {
                continue;
            }
            out.push(Finish { pane_id: s.pane_id.clone(), name: s.name.clone(), project: s.project.clone(), kind, files: s.files, exit_code: s.exit_code, ran_ms: quiet - start, at: quiet });
        }
        self.runs.retain(|id, _| snaps.iter().any(|s| &s.pane_id == id));
        out
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
            self.items.retain(|x| x.pane_id != f.pane_id);
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
}

/// "45 s", "4 min", "1 h 20 min".
pub fn ran_for(ms: u64) -> String {
    let s = (ms + 500) / 1000;
    if s < 60 {
        return format!("{s} s");
    }
    let m = (s + 30) / 60;
    if m < 60 {
        format!("{m} min")
    } else if m % 60 > 0 {
        format!("{} h {} min", m / 60, m % 60)
    } else {
        format!("{} h", m / 60)
    }
}

fn files_text(n: Option<u32>) -> String {
    match n {
        Some(1) => "1 file changed".into(),
        Some(n) if n > 0 => format!("{n} files changed"),
        _ => "changes to review".into(),
    }
}

/// The notification for a burst, and which agent a click opens: the one that
/// most wants you (an error, then changes to review), first to finish.
pub fn finish_note(list: &[Finish]) -> (String, String, String) {
    let mut order: Vec<&Finish> = list.iter().collect();
    order.sort_by_key(|f| (f.kind as u8, f.at));
    let top = order[0];
    if order.len() == 1 {
        let (title, body) = match top.kind {
            FinishKind::Review => (format!("{} is ready to review", top.name), format!("{} in {}, after {}.", files_text(top.files), top.project, ran_for(top.ran_ms))),
            FinishKind::Crash => (format!("{} stopped with an error", top.name), format!("It exited with code {} after {}, in {}.", top.exit_code.unwrap_or(1), ran_for(top.ran_ms), top.project)),
            FinishKind::Done => (format!("{} is done", top.name), format!("Worked {} in {}.", ran_for(top.ran_ms), top.project)),
        };
        return (title, body, top.pane_id.clone());
    }
    let mut lines: Vec<String> = order
        .iter()
        .take(3)
        .map(|f| {
            let what = match f.kind {
                FinishKind::Review => format!("ready to review, {}", files_text(f.files)),
                FinishKind::Crash => "stopped with an error".into(),
                FinishKind::Done => "done".into(),
            };
            format!("{}: {what}", f.name)
        })
        .collect();
    if order.len() > 3 {
        lines.push(format!("and {} more", order.len() - 3));
    }
    (format!("{} agents finished", order.len()), lines.join("\n"), top.pane_id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(id: &str, state: TaskState) -> FinishSnap {
        FinishSnap { pane_id: id.into(), name: id.to_uppercase(), project: "maestro".into(), state, limited: false, files: Some(2), exit_code: None, stop_asked: false }
    }

    #[test]
    fn a_long_run_that_settles_is_told_once() {
        let mut w = FinishWatch::default();
        assert!(w.step(&[snap("a", TaskState::Working)], 0).is_empty());
        assert!(w.step(&[snap("a", TaskState::Needs)], 10_000).is_empty(), "a question is part of the run");
        assert!(w.step(&[snap("a", TaskState::Working)], 20_000).is_empty());
        assert!(w.step(&[snap("a", TaskState::Review)], 30_000).is_empty(), "settling");
        let f = w.step(&[snap("a", TaskState::Review)], 35_000);
        assert_eq!(f.len(), 1);
        assert_eq!((f[0].kind, f[0].ran_ms), (FinishKind::Review, 30_000));
        assert!(w.step(&[snap("a", TaskState::Review)], 40_000).is_empty(), "told once");
    }

    #[test]
    fn short_runs_merges_and_asked_stops_say_nothing() {
        let mut w = FinishWatch::default();
        w.step(&[snap("a", TaskState::Working)], 0);
        w.step(&[snap("a", TaskState::Idle)], 5_000);
        assert!(w.step(&[snap("a", TaskState::Idle)], 11_000).is_empty(), "a 5 s run is noise");
        w.step(&[snap("b", TaskState::Working)], 0);
        assert!(w.step(&[snap("b", TaskState::Merged)], 60_000).is_empty());
        let mut stopped = snap("c", TaskState::Stopped);
        stopped.exit_code = Some(1);
        w.step(&[snap("c", TaskState::Working)], 0);
        assert_eq!(w.step(&[stopped.clone()], 60_000)[0].kind, FinishKind::Crash);
        stopped.stop_asked = true;
        w.step(&[snap("c", TaskState::Working)], 100_000);
        assert!(w.step(&[stopped], 200_000).is_empty(), "you asked it to stop");
    }

    #[test]
    fn bursts_group_and_name_the_most_urgent() {
        let f = |id: &str, kind, at| Finish { pane_id: id.into(), name: id.into(), project: "p".into(), kind, files: Some(1), exit_code: Some(2), ran_ms: 125_000, at };
        let mut b = Burst::default();
        b.add(vec![f("Ana", FinishKind::Done, 1)], 0);
        b.add(vec![f("Bob", FinishKind::Crash, 2)], 2_000);
        assert!(b.take(4_000).is_none(), "still growing");
        let list = b.take(5_000).unwrap();
        let (title, body, open) = finish_note(&list);
        assert_eq!(title, "2 agents finished");
        assert_eq!(body, "Bob: stopped with an error\nAna: done");
        assert_eq!(open, "Bob");
        let (t1, b1, _) = finish_note(&[f("Ana", FinishKind::Review, 1)]);
        assert_eq!((t1.as_str(), b1.as_str()), ("Ana is ready to review", "1 file changed in p, after 2 min."));
        assert_eq!(ran_for(4_800_000), "1 h 20 min");
        assert_eq!(ran_for(45_000), "45 s");
    }
}
