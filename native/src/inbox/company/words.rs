//! What the Company says (rail.ts, model.ts, taskstate.ts statusLine), word for word.

use crate::company::needs::PlanDetail;
use crate::company::types::JobStatus;
use supervise::ask::AskKind;
use supervise::task_state::{TaskState, TaskStatus};

pub(super) const NEEDS_YOU: &str = "Needs you";
pub(super) const JOBS: &str = "Jobs";
pub(super) const JOBS_META: &str = "Tokens used of the cap";
pub(super) const NOTHING_NEEDS_YOU: &str = "Nothing needs you.";
pub(super) const NO_JOB: &str = "No job is running.";
pub(super) const RESUME_DETAIL: &str = "Its people stopped when ByteHive closed. Resume starts them again.";

pub(super) fn resume_title(job: &str) -> String {
    format!("Nobody is at work on: {job}")
}
pub(super) const STARTING: &str = "Starting...";
pub(super) const GIVE_JOB: &str = "Give the Founder a job";
pub(super) const SEND: &str = "Send";
pub(super) const SEND_HINT: &str = "Enter sends. Shift+Enter starts a new line.";
pub(super) const WHY_LABEL: &str = "Why send it back?";
pub(super) const WHY_HINT: &str = "For example: fewer hires, and leave QA for later";
pub(super) const NOTE_NEEDED: &str = "Send back needs a note. To approve instead, close the note. To drop the job, use Cancel job in its menu.";
pub(super) const CLOSE_NOTE: &str = "Close the note";
pub(super) const DETAILS: &str = "Details";
pub(super) const KEEP_JOB: &str = "Keep job";
pub(super) const COMPANY: &str = "Company";
pub(super) const BYTEHIVE: &str = "ByteHive";
pub(super) const NOBODY_HERE: &str = "Nobody works here yet.";
pub(super) const SOMETHING_WRONG: &str = "Something went wrong.";
pub(super) const INBOX_TIP: &str = "Inbox (Ctrl+Shift+O)";
pub(super) const BACK_TIP: &str = "Back to the Company (Ctrl+Shift+O)";
pub(super) const INBOX: &str = "Inbox";
pub(super) const LOCKED: &str = "The Company is running in another ByteHive window.";
/// Examples for an empty box: what a first job can sound like.
pub(super) const EXAMPLES: [&str; 3] = ["Add a pricing page", "Fix the login", "Write the docs"];
/// What Add 1M tokens adds to a paused Job's cap.
pub(super) const RAISE_BY: u64 = 1_000_000;

pub(super) fn plural(n: u64, w: &str) -> String {
    format!("{n} {w}{}", if n == 1 { "" } else { "s" })
}

fn with_article(hire: &str) -> String {
    let vowel = hire.chars().next().is_some_and(|c| "aeiouAEIOU".contains(c));
    format!("{} {hire}", if vowel { "an" } else { "a" })
}

fn listed(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// One decimal, half away from zero, and no ".0": JS `+x.toFixed(1)`.
fn tenths(n: u64, unit: u64) -> String {
    let t = (n + unit / 20) / (unit / 10);
    if t % 10 == 0 { (t / 10).to_string() } else { format!("{}.{}", t / 10, t % 10) }
}

/// 5,000,000 as 5M, and 1,200,000 as 1.2M.
pub(super) fn tokens_short(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{}M", tenths(n, 1_000_000))
    } else if n >= 1000 {
        format!("{}K", tenths(n, 1000))
    } else {
        n.to_string()
    }
}

/// 5000000 as 5,000,000: `toLocaleString("en-US")`.
pub(super) fn grouped(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// The whole plan as one sentence: it reads as a proposal, and the list stays folded under it.
pub(super) fn plan_sentence(p: &PlanDetail) -> String {
    let cards: usize = p.floors.iter().map(|f| f.cards.len()).sum();
    let wants = if p.hires.is_empty() {
        "The Founder wants to do this with the people you have".to_string()
    } else {
        let hires: Vec<String> = p.hires.iter().map(|h| with_article(h)).collect();
        format!("The Founder wants to hire {}", listed(&hires))
    };
    format!("{wants}, {}, up to {} tokens.", plural(cards as u64, "card"), tokens_short(p.token_cap))
}

/// The plan in full, as the Details list reads it: (term, text, a prompt to keep as written).
pub(super) fn plan_items(p: &PlanDetail, summary: &str) -> Vec<(String, String, bool)> {
    let mut out = Vec::new();
    if !summary.is_empty() {
        out.push(("Plan".into(), summary.into(), false));
    }
    let hires = if p.hires.is_empty() { "Nobody new".to_string() } else { p.hires.join(", ") };
    out.push(("Hires".into(), hires, false));
    for f in &p.floors {
        out.push((format!("Cards on {}", f.floor), f.cards.join(", "), false));
    }
    out.push(("Token cap".into(), grouped(p.token_cap), false));
    for r in &p.new_roles {
        out.push((format!("New role: {} ({}, {})", r.title, r.id, r.department), r.prompt.clone(), true));
    }
    out
}

/// The Jobs still going, as the rail names them (model.ts GOING). None is a Job that is over.
pub(super) fn going(status: &JobStatus) -> Option<&'static str> {
    match status {
        JobStatus::Planning => Some("Planning"),
        JobStatus::AwaitingPlan => Some("Waiting for your approval"),
        JobStatus::Running => Some("Running"),
        JobStatus::Paused => Some("Paused"),
        _ => None,
    }
}

/// A job nobody has started on is cancelled. One that has people on it is stopped.
pub(super) fn stop_word(status: &JobStatus) -> &'static str {
    if matches!(status, JobStatus::Planning | JobStatus::AwaitingPlan) { "Cancel job" } else { "Stop job" }
}

/// The short line beside a job: its one word, and tokens used of the cap.
pub(super) fn job_state(status: &JobStatus, label: &str, used: u64, cap: u64) -> String {
    let word = if *status == JobStatus::AwaitingPlan { "Waiting" } else { label };
    format!("{word}, {} of {}", tokens_short(used), tokens_short(cap))
}

/// The same, in full, for the tooltip.
pub(super) fn job_tokens(used: u64, cap: u64) -> String {
    format!("{} of {} tokens", grouped(used), grouped(cap))
}

/// How far along a job is, from its cards: 0 to 100.
pub(super) fn done_percent(cards: Option<(u32, u32)>) -> u32 {
    match cards {
        Some((done, total)) if total > 0 => ((done as f64 / total as f64) * 100.0).round().min(100.0) as u32,
        _ => 0,
    }
}

/// Under Nothing needs you: what the team is doing.
pub(super) fn calm_line(people: u64, working: u64) -> String {
    if working > 0 {
        let who = plural(people, "person").replace("persons", "people");
        return format!("{working} of {who} {} working.", if working == 1 { "is" } else { "are" });
    }
    if people > 1 { "Everyone is resting.".into() } else { "Give the Founder a job to get started.".into() }
}

/// The box's example names a project the company has, or none: a new company has no gcare.
pub(super) fn ask_hint(first_floor: Option<&str>) -> String {
    first_floor.map_or_else(|| "Build a landing page for my app".into(), |f| format!("Add marketing to {f}"))
}

/// What a person is doing, in a line (taskstate.ts statusLine).
pub(super) fn status_line(name: &str, s: &TaskStatus) -> String {
    if s.state == TaskState::Needs
        && let Some(a) = &s.ask
    {
        let detail = a.detail.as_deref().filter(|d| !d.is_empty());
        return match (a.kind, detail) {
            (AskKind::Run, Some(d)) => format!("{name} wants to run {d}"),
            (AskKind::Run, None) => format!("{name} wants to run a command"),
            (AskKind::Edit, Some(d)) => format!("{name} wants to edit {d}"),
            (AskKind::Edit, None) => format!("{name} wants to edit a file"),
            _ => format!("{name} asks: {}", a.prompt),
        };
    }
    if s.limit.is_some() {
        return format!("{name} is out of usage");
    }
    match s.state {
        TaskState::Review => format!("{name} is done, changes ready to review"),
        TaskState::Working => format!("{name} is working"),
        TaskState::Stopped => format!("{name} stopped"),
        TaskState::Merged => format!("{name}'s work is merged"),
        _ => format!("{name} is waiting for a task"),
    }
}

pub(super) fn more_for(title: &str) -> String {
    format!("More for {title}")
}

pub(super) fn open_desk(name: &str) -> String {
    format!("Open {name}'s desk")
}

pub(super) fn desk_not_ready(name: &str) -> String {
    format!("Getting {name}'s desk ready. Try again in a moment.")
}

/// Ending a job is its own decision: the confirm says what stops and what stays.
pub(super) fn confirm_title(word: &str) -> String {
    format!("{word}?")
}

pub(super) fn ends_here(title: &str) -> String {
    format!("\u{201c}{title}\u{201d} ends here, and its people stop working on it. Its cards and branches stay.")
}

pub(super) fn below(n: usize) -> String {
    format!("{n} below")
}

/// A floor's quiet line: its people, and how many someone is waiting on.
pub(super) fn floor_meta(people: u64, needs: usize) -> String {
    let count = plural(people, "person").replace("persons", "people");
    if needs > 0 { format!("{count}, {needs} needs you") } else { count }
}

/// What a person's line says: their state, and what they are on while not stopped.
pub(super) fn doing_line(word: &str, doing: Option<&str>) -> String {
    match doing {
        Some(d) => format!("{word}: {d}"),
        None => word.to_string(),
    }
}

/// The narrow rail's foot: what waits below, or None when nothing does.
pub(super) fn jobs_below(cards: usize, jobs: bool) -> Option<String> {
    match (cards, jobs) {
        (0, false) => None,
        (0, true) => Some("Jobs below".into()),
        (n, false) => Some(below(n)),
        (n, true) => Some(format!("{n} and Jobs below")),
    }
}
