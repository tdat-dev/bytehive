//! What each agent's task is doing right now, derived from facts already
//! known instead of set by hand: is the process alive, when did it last
//! print, what is on its screen, and has it changed files on its branch.

use chrono::{Local, TimeZone, Timelike};
use serde::{Deserialize, Serialize};

use crate::ask::{parse_ask, Ask, AskKind};
use crate::ask_risk::{risk_of_ask, AskRisk};
use crate::clash::ClashNote;
use crate::screens::{blocked_on_screen, limit_on_screen, BlockedHit, LimitHit};
use crate::stuck::StuckEvidence;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskState {
    Needs,
    Blocked,
    Review,
    Working,
    Idle,
    Stopped,
    Merged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TaskFacts {
    pub running: bool,
    /// ms of the last output.
    pub last_output_at: u64,
    /// Visible screen text.
    pub screen: String,
    /// Files changed on the agent's branch vs HEAD; none when unknown (no repo, not checked yet).
    pub changed_files: Option<u32>,
    /// Its branch was merged into the base (and its worktree removed) from Review.
    pub merged: bool,
    /// It has worked on something since the start (from task history).
    pub worked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskStatus {
    pub state: TaskState,
    /// Present when asking: what it is asking and which keys answer it.
    pub ask: Option<Ask>,
    /// It stopped at its plan's usage limit (and when that lifts).
    pub limit: Option<LimitHit>,
    /// Present when blocked: what only you can clear.
    pub blocked: Option<BlockedHit>,
    /// Idle after working, not before its first task.
    pub spent: bool,
    /// A working agent looping or stalled, with the evidence.
    pub stuck: Option<StuckEvidence>,
    /// Overlaps another agent's files or task, with the evidence.
    pub clash: Option<ClashNote>,
}

/// Spinners and streaming redraw several times a second; this long without a
/// byte means the agent has stopped producing and is waiting.
pub const WORKING_MS: u64 = 2500;

/// Order the queue uses: what needs you first, then what's ready for you.
pub fn state_rank(state: TaskState) -> u8 {
    match state {
        TaskState::Needs => 0,
        TaskState::Blocked => 1,
        TaskState::Review => 2,
        TaskState::Working => 3,
        TaskState::Idle => 4,
        TaskState::Stopped => 5,
        TaskState::Merged => 6,
    }
}

fn blank(state: TaskState) -> TaskStatus {
    TaskStatus { state, ask: None, limit: None, blocked: None, spent: false, stuck: None, clash: None }
}

pub fn derive_task_state(f: &TaskFacts, now: u64) -> TaskStatus {
    // A merged agent was stopped to land its branch: its job is done, not cut short.
    if !f.running {
        return blank(if f.merged { TaskState::Merged } else { TaskState::Stopped });
    }
    // A prompt on screen wins over recency: selectors redraw while you move the
    // cursor, and a question is still a question the moment it appears.
    if let Some(ask) = parse_ask(&f.screen) {
        let mut s = blank(TaskState::Needs);
        s.ask = Some(ask);
        return s;
    }
    // A login page is not working, even while it redraws: a block notice wins over recency.
    if let Some(blocked) = blocked_on_screen(&f.screen) {
        let mut s = blank(TaskState::Blocked);
        s.blocked = Some(blocked);
        return s;
    }
    if now.saturating_sub(f.last_output_at) <= WORKING_MS {
        return blank(TaskState::Working);
    }
    // quiet after hitting its plan's limit: not done, waiting for the limit to lift
    let limit = limit_on_screen(&f.screen, now);
    if matches!(f.changed_files, Some(n) if n > 0) {
        let mut s = blank(TaskState::Review);
        s.limit = limit;
        return s;
    }
    let mut s = blank(TaskState::Idle);
    s.limit = limit;
    s.spent = f.worked;
    s
}

/// "7:09 PM" today, "Thu 1:00 AM" later.
pub fn clock_of(at: u64, now: u64) -> String {
    let d = Local.timestamp_millis_opt(at as i64).single();
    let n = Local.timestamp_millis_opt(now as i64).single();
    let (d, n) = match (d, n) {
        (Some(d), Some(n)) => (d, n),
        _ => return String::new(),
    };
    let time = format!("{}:{:02} {}", d.hour12().1, d.minute(), if d.hour12().0 { "AM" } else { "PM" });
    if d.date_naive() == n.date_naive() {
        time
    } else {
        format!("{} {time}", d.format("%a"))
    }
}

/// One line for a queue row or a notification, from the agent's point of view.
pub fn status_line(name: &str, s: &TaskStatus, now: u64) -> String {
    if s.state == TaskState::Needs {
        if let Some(a) = &s.ask {
            let risk = if risk_of_ask(a.kind, a.detail.as_deref()) == AskRisk::High { " (high risk)" } else { "" };
            if a.kind == AskKind::Run {
                return match &a.detail {
                    Some(d) => format!("{name} wants to run {d}{risk}"),
                    None => format!("{name} wants to run a command{risk}"),
                };
            }
            if a.kind == AskKind::Edit {
                return match &a.detail {
                    Some(d) => format!("{name} wants to edit {d}{risk}"),
                    None => format!("{name} wants to edit a file{risk}"),
                };
            }
            return format!("{} asks: {}", name, a.prompt);
        }
    }

    if s.state == TaskState::Blocked {
        match s.blocked.as_ref().map(|b| b.kind) {
            Some(crate::screens::BlockedKind::Captcha) => return format!("{name} needs you to solve a CAPTCHA"),
            Some(crate::screens::BlockedKind::Key) => return format!("{name} needs a valid API key"),
            _ => return format!("{name} is waiting on you to sign in"),
        }
    }
    if let Some(limit) = &s.limit {
        return match limit.resets_at {
            Some(at) => format!("{name} is out of usage until {}", clock_of(at, now)),
            None => format!("{name} is out of usage"),
        };
    }
    if s.state == TaskState::Review {
        return format!("{name} is done, changes ready to review");
    }
    if s.state == TaskState::Working {
        return match &s.stuck {
            Some(stuck) => format!("{} looks stuck: {}", name, stuck.text),
            None => format!("{name} is working"),
        };
    }
    if s.state == TaskState::Stopped {
        return format!("{name} stopped");
    }
    if s.state == TaskState::Merged {
        return format!("{name}'s work is merged");
    }
    if s.state == TaskState::Idle && s.spent {
        return format!("{name} is done, with no changes to review");
    }
    format!("{name} is waiting for a task")
}
