//! Pane states for the shell: what each pane is doing, read from its own
//! screen and process, plus the one-line badge the grid header shows.

use eframe::egui::Color32;
use supervise::task_state::{derive_task_state, TaskFacts, TaskState, TaskStatus};

use crate::term_pane::Pane;
use crate::theme::{state_dot, state_word};

/// The folder a pane works in: its spawn dir, else the shell's own.
pub fn pane_cwd(pane: &Pane) -> String {
    pane.spec.cwd.clone().unwrap_or_else(|| std::env::current_dir().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| ".".to_string()))
}

/// The facts a pane already knows: process, last output, screen text, and
/// how many files git says changed (`changed`, none when not yet polled).
pub fn pane_facts(pane: &Pane, changed: Option<u32>) -> TaskFacts {
    TaskFacts {
        running: pane.exit_code.is_none(),
        last_output_at: pane.last_output_ms,
        screen: pane.screen_text(),
        // The folder's changes are an agent's work: a plain shell has none to review.
        changed_files: if pane.cli.is_some() { changed } else { None },
        merged: pane.merged,
        worked: false,
    }
}

pub fn pane_status(pane: &Pane, now: u64, changed: Option<u32>) -> TaskStatus {
    derive_task_state(&pane_facts(pane, changed), now)
}

/// Header badge: a dot color and a short word per state.
pub fn state_badge(state: TaskState) -> (Color32, &'static str) {
    (state_dot(state), state_word(state))
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::launch::launch_spec;

    fn pane() -> Pane {
        let spec = launch_spec("powershell.exe", &[String::from("-NoLogo")]);
        Pane::new("p1".to_string(), "shell 1".to_string(), spec, 80, 24)
    }

    #[test]
    fn fresh_quiet_pane_is_idle() {
        let mut p = pane();
        p.last_output_ms = 1_000;
        let s = pane_status(&p, 1_000_000, None);
        assert_eq!(s.state, TaskState::Idle);
    }

    #[test]
    fn recent_output_is_working() {
        let mut p = pane();
        p.feed_bytes(b"hello\r\n");
        let s = pane_status(&p, p.last_output_ms + 100, None);
        assert_eq!(s.state, TaskState::Working);
    }

    #[test]
    fn prompt_on_screen_needs_you() {
        let mut p = pane();
        p.feed_bytes(b" Do you want to proceed?\r\n \xE2\x9D\xAF 1. Yes\r\n   2. No (esc)\r\n");
        p.last_output_ms = 1_000;
        let s = pane_status(&p, 1_000_000, None);
        assert_eq!(s.state, TaskState::Needs);
        assert_eq!(s.ask.map(|a| a.options.len()), Some(2));
    }

    #[test]
    fn quiet_with_changes_is_review() {
        let mut p = pane();
        p.cli = Some(crate::sessions_ui::Cli::Claude);
        p.last_output_ms = 1_000;
        let s = pane_status(&p, 1_000_000, Some(3));
        assert_eq!(s.state, TaskState::Review);
    }

    #[test]
    fn plain_shell_never_waits_for_review() {
        let mut p = pane();
        p.last_output_ms = 1_000;
        let s = pane_status(&p, 1_000_000, Some(3));
        assert_eq!(s.state, TaskState::Idle);
    }

    #[test]
    fn exited_pane_is_stopped() {
        let mut p = pane();
        p.exit_code = Some(0);
        let s = pane_status(&p, 1_000_000, None);
        assert_eq!(s.state, TaskState::Stopped);
    }

    #[test]
    fn every_state_has_a_badge() {
        for state in [TaskState::Needs, TaskState::Blocked, TaskState::Review, TaskState::Working, TaskState::Idle, TaskState::Stopped, TaskState::Merged] {
            let (_, word) = state_badge(state);
            assert!(!word.is_empty());
        }
    }
}
