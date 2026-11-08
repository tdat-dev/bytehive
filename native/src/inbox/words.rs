//! What the Inbox says: state names, a row's second line, the headline, ages (inbox.ts).

use super::*;

/// The CLI's name as the header says it: its preset's label, else the program.
pub(super) fn cli_label(cli: &str) -> String {
    let key = cli.to_ascii_lowercase();
    if let Some(p) = PRESETS.iter().find(|p| p.program == key) {
        return p.label.to_string();
    }
    match key.trim_end_matches(".exe") {
        "powershell" | "pwsh" => "PowerShell".to_string(),
        "cmd" => "CMD".to_string(),
        other => other.to_string(),
    }
}

/// GROUP_LABEL in inbox.ts.
pub(super) fn label_of(state: State) -> &'static str {
    match state {
        State::Needs | State::Blocked => "Needs you",
        State::Review => "Ready to review",
        State::Working => "Working",
        State::Idle => "Idle",
        State::Stopped => "Stopped",
        State::Merged => "Merged",
    }
}

/// The colour a state's words and dot take; None keeps the quiet ink.
pub(super) fn hue_of(state: State) -> Option<u32> {
    match state {
        State::Needs | State::Blocked => Some(NEEDS),
        State::Review => Some(REVIEW),
        State::Working => Some(WORK),
        _ => None,
    }
}

pub(super) fn waits(state: State) -> bool {
    matches!(state, State::Needs | State::Blocked)
}

pub(super) fn ended(state: State) -> bool {
    matches!(state, State::Stopped | State::Merged)
}

/// rowLine in inbox.ts: the second line of a queue row, from the agent's side.
pub(super) fn row_line(status: &TaskStatus, worked: bool) -> String {
    if let Some(limit) = &status.limit {
        return match limit.resets_at {
            Some(at) => format!("Out of usage · back {}", supervise::task_state::clock_of(at, now_ms())),
            None => "Out of usage".to_string(),
        };
    }
    let said = match status.state {
        State::Needs | State::Blocked => match &status.ask {
            None => match &status.blocked {
                Some(b) => b.what.clone(),
                None => "Needs you".to_string(),
            },
            Some(a) => match (a.kind, &a.detail) {
                (AskKind::Run, Some(d)) => format!("Wants to run {d}"),
                (AskKind::Run, None) => "Wants to run a command".to_string(),
                (AskKind::Edit, Some(d)) => format!("Wants to edit {d}"),
                (AskKind::Edit, None) => "Wants to edit a file".to_string(),
                _ => format!("Asks: {}", a.prompt),
            },
        },
        State::Review => "Changes to review".to_string(),
        State::Working => "Working".to_string(),
        State::Stopped => "Stopped".to_string(),
        State::Merged => "Merged into the base branch".to_string(),
        State::Idle => if worked { "Done" } else { "Waiting for a task" }.to_string(),
    };
    let state = label_of(status.state);
    if said == state || status.state == State::Merged { said } else { format!("{state} · {said}") }
}

/// The top bar's sentence, worded as today's app words it (inbox.ts headline).
pub(super) fn headline(statuses: &[TaskStatus], worked: &[bool]) -> (String, String) {
    let n = |s: State| statuses.iter().filter(|x| x.state == s).count();
    let are = |k: usize| if k == 1 { "is" } else { "are" };
    if statuses.is_empty() {
        return ("No agents yet.".to_string(), " Start one with New agent.".to_string());
    }
    let needs = n(State::Needs) + n(State::Blocked);
    let lead = match needs {
        0 => "Nothing needs you.".to_string(),
        1 => "1 agent needs you.".to_string(),
        k => format!("{k} agents need you."),
    };
    let (review, working) = (n(State::Review), n(State::Working));
    let mut parts: Vec<String> = Vec::new();
    if review > 0 {
        parts.push(format!("{review} {} ready to review", are(review)));
    }
    if working > 0 {
        parts.push(format!("{working} {} working", are(working)));
    }
    if parts.is_empty() {
        let idle = n(State::Idle);
        let done = statuses.iter().zip(worked).filter(|(s, w)| s.state == State::Idle && **w).count();
        let (stopped, merged) = (n(State::Stopped), n(State::Merged));
        if done > 0 {
            parts.push(format!("{done} {} done", are(done)));
        }
        if idle > done {
            parts.push(format!("{} {} waiting for a task", idle - done, are(idle - done)));
        }
        if stopped > 0 {
            parts.push(format!("{stopped} {} stopped", are(stopped)));
        }
        if merged > 0 {
            parts.push(format!("{merged} {} merged", are(merged)));
        }
    }
    let rest = if parts.is_empty() { String::new() } else { format!(" {}.", parts.join(", ")) };
    (lead, rest)
}

/// ago() in inbox.ts: "12s", "4m", "2h", "3d".
pub(super) fn ago(ms: u64) -> String {
    let s = ms / 1000;
    match s {
        0..=59 => format!("{s}s"),
        60..=3599 => format!("{}m", s / 60),
        3600..=86_399 => format!("{}h", s / 3600),
        _ => format!("{}d", s / 86_400),
    }
}
