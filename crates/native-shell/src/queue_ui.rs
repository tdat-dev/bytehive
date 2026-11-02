//! The queue's rows: every agent with its status and the one line it draws,
//! what needs you first (inbox.rs paints them), and how an answer to an
//! agent's question is typed.

use std::collections::HashMap;

use supervise::clash::{clash_short, ClashNote};
use supervise::stuck::StuckEvidence;
use supervise::task_state::{state_rank, status_line, TaskState, TaskStatus};

use crate::states::pane_status;
use crate::term_pane::Pane;

/// One queue row, computed from a pane: sort rank, status, and its line.
pub struct QueueRow {
    pub pane_id: String,
    pub name: String,
    pub rank: u8,
    pub status: TaskStatus,
    pub line: String,
}

pub fn queue_rows(
    panes: &[Pane],
    now: u64,
    changed_by: &HashMap<String, u32>,
    stuck: &HashMap<String, StuckEvidence>,
    clash: &HashMap<String, ClashNote>,
) -> Vec<QueueRow> {
    let mut rows: Vec<QueueRow> = panes
        .iter()
        .map(|p| {
            let changed = changed_by.get(&p.id).copied();
            let mut status = pane_status(p, now, changed);
            status.stuck = stuck.get(&p.id).cloned();
            status.clash = clash.get(&p.id).cloned();
            let rank = state_rank(status.state);
            let mut line = status_line(&p.name, &status, now);
            if let Some(note) = clash.get(&p.id) {
                line = format!("{line} · {}", clash_short(note));
            }
            if let Some(l) = &p.limit {
                let back = l.resets_at.map(|at| format!(", back at {}", supervise::limits::reset_label(at, chrono::Local::now()))).unwrap_or_default();
                line = format!("{} hit its usage limit{back}", p.name);
            }
            QueueRow { pane_id: p.id.clone(), name: p.name.clone(), rank, status, line }
        })
        .collect();
    rows.sort_by(|a, b| a.rank.cmp(&b.rank).then(a.name.cmp(&b.name)));
    rows
}

/// Rows that pull the eye: needs-you and blocked render as cards, the rest
/// stay flat.
pub fn is_attention(state: TaskState) -> bool {
    matches!(state, TaskState::Needs | TaskState::Blocked)
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::launch::launch_spec;
    use supervise::task_state::TaskState;

    fn pane(name: &str) -> Pane {
        let spec = launch_spec("powershell.exe", &[String::from("-NoLogo")]);
        Pane::new(format!("{name}-id"), name.to_string(), spec, 80, 24)
    }

    #[test]
    fn sorts_needs_first_then_alphabetical() {
        let mut asking = pane("zulu");
        asking.feed_bytes(" Do you want to proceed?\r\n \u{276F} 1. Yes\r\n   2. No (esc)\r\n".as_bytes());
        asking.last_output_ms = 1_000;
        let mut idle = pane("alpha");
        idle.last_output_ms = 1_000;
        let mut idle2 = pane("mike");
        idle2.last_output_ms = 1_000;
        let rows = queue_rows(&[idle, asking, idle2], 1_000_000, &HashMap::new(), &HashMap::new(), &HashMap::new());
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["zulu", "alpha", "mike"]);
        assert_eq!(rows[0].status.state, TaskState::Needs);
        assert!(rows[0].line.contains("zulu"));
    }

    #[test]
    fn stuck_and_clash_show_on_the_row() {
        use supervise::clash::{ClashKind, ClashNote};
        use supervise::stuck::{StuckEvidence, StuckKind};
        let mut working = pane("bea");
        working.feed_bytes(b"hello\r\n");
        let stuck = HashMap::from([(
            "bea-id".to_string(),
            StuckEvidence { kind: StuckKind::Loop, text: "same failure 3 times: FAIL x".to_string(), count: 3, sample: "FAIL x".to_string(), since: 1 },
        )]);
        let clash = HashMap::from([(
            "bea-id".to_string(),
            ClashNote { kind: ClashKind::File, key: "k".to_string(), with: vec!["Cy".to_string()], files: vec!["a.ts".to_string()] },
        )]);
        let rows = queue_rows(&[working], 1_000_000, &HashMap::new(), &stuck, &clash);
        assert!(rows[0].line.contains("looks stuck"), "{}", rows[0].line);
        assert!(rows[0].line.contains("overlaps Cy in a.ts"), "{}", rows[0].line);
    }

    #[test]
    fn stopped_sorts_last() {
        let mut dead = pane("gone");
        dead.exit_code = Some(0);
        let mut idle = pane("awake");
        idle.last_output_ms = 1_000;
        let rows = queue_rows(&[dead, idle], 1_000_000, &HashMap::new(), &HashMap::new(), &HashMap::new());
        assert_eq!(rows[1].name, "gone");
        assert_eq!(rows[1].status.state, TaskState::Stopped);
    }

    #[test]
    fn attention_is_only_needs_and_blocked() {
        use supervise::task_state::TaskState;
        assert!(is_attention(TaskState::Needs));
        assert!(is_attention(TaskState::Blocked));
        for s in [TaskState::Review, TaskState::Working, TaskState::Idle, TaskState::Stopped, TaskState::Merged] {
            assert!(!is_attention(s));
        }
    }
}
