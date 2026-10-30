// Task states from facts: stopped, merged, needs, blocked, review, working,
// idle, and the one line each draws.

use supervise::stuck::{StuckEvidence, StuckKind};
use supervise::task_state::{derive_task_state, state_rank, status_line, TaskFacts, TaskState, TaskStatus, WORKING_MS};

const NOW: u64 = 1_000_000;
const ASKING: &str = "\n Do you want to proceed?\n \u{276F} 1. Yes\n   2. No, and tell Claude what to do differently (esc)\n";

fn facts() -> TaskFacts {
    TaskFacts { running: true, last_output_at: NOW - 60_000, screen: String::new(), changed_files: Some(0), merged: false, worked: false }
}

fn status(state: TaskState) -> TaskStatus {
    TaskStatus { state, ask: None, limit: None, blocked: None, spent: false, stuck: None, clash: None }
}

#[test]
fn stopped_when_the_process_is_gone() {
    let mut f = facts();
    f.running = false;
    f.screen = ASKING.to_string();
    assert_eq!(derive_task_state(&f, NOW).state, TaskState::Stopped);
}

#[test]
fn merged_once_its_branch_landed() {
    let mut f = facts();
    f.running = false;
    f.merged = true;
    f.changed_files = None;
    assert_eq!(derive_task_state(&f, NOW).state, TaskState::Merged);
    let mut f = facts();
    f.merged = true;
    f.last_output_at = NOW;
    assert_eq!(derive_task_state(&f, NOW).state, TaskState::Working);
    assert!(state_rank(TaskState::Stopped) < state_rank(TaskState::Merged));
    assert_eq!(status_line("Ivy", &status(TaskState::Merged), NOW), "Ivy's work is merged");
}

#[test]
fn needs_you_when_a_prompt_is_on_screen() {
    let mut f = facts();
    f.screen = ASKING.to_string();
    f.last_output_at = NOW - 100;
    let s = derive_task_state(&f, NOW);
    assert_eq!(s.state, TaskState::Needs);
    assert_eq!(s.ask.map(|a| a.options.len()), Some(2));
}

#[test]
fn working_while_output_is_recent() {
    let mut f = facts();
    f.last_output_at = NOW - (WORKING_MS - 1);
    assert_eq!(derive_task_state(&f, NOW).state, TaskState::Working);
}

#[test]
fn ready_to_review_when_quiet_with_changes() {
    let mut f = facts();
    f.changed_files = Some(3);
    assert_eq!(derive_task_state(&f, NOW).state, TaskState::Review);
}

#[test]
fn idle_when_quiet_with_no_or_unknown_changes() {
    let mut f = facts();
    f.changed_files = Some(0);
    assert_eq!(derive_task_state(&f, NOW).state, TaskState::Idle);
    let mut f = facts();
    f.changed_files = None;
    assert_eq!(derive_task_state(&f, NOW).state, TaskState::Idle);
}

#[test]
fn ranks_needs_above_review_above_working() {
    assert!(state_rank(TaskState::Needs) < state_rank(TaskState::Review));
    assert!(state_rank(TaskState::Review) < state_rank(TaskState::Working));
    assert!(state_rank(TaskState::Idle) < state_rank(TaskState::Stopped));
}

#[test]
fn blocked_on_a_sign_in_screen() {
    let mut f = facts();
    f.screen = "Please sign in to continue".to_string();
    let s = derive_task_state(&f, NOW);
    assert_eq!(s.state, TaskState::Blocked);
    assert_eq!(s.blocked.map(|b| b.kind), Some(supervise::screens::BlockedKind::Auth));
}

#[test]
fn blocked_even_while_redrawing() {
    let mut f = facts();
    f.screen = "Please complete the CAPTCHA".to_string();
    f.last_output_at = NOW - 100;
    let s = derive_task_state(&f, NOW);
    assert_eq!(s.state, TaskState::Blocked);
    assert_eq!(s.blocked.map(|b| b.kind), Some(supervise::screens::BlockedKind::Captcha));
}

#[test]
fn an_answerable_prompt_wins_over_a_block() {
    let mut f = facts();
    f.screen = format!("Please sign in to continue\n{ASKING}");
    assert_eq!(derive_task_state(&f, NOW).state, TaskState::Needs);
}

#[test]
fn ranks_blocked_with_needs_you() {
    assert!(state_rank(TaskState::Needs) < state_rank(TaskState::Blocked));
    assert!(state_rank(TaskState::Blocked) < state_rank(TaskState::Review));
}

#[test]
fn says_what_the_agent_wants() {
    let mut f = facts();
    f.screen = format!(" Bash command\n\n   npm test\n\n{ASKING}");
    assert_eq!(status_line("Ana", &derive_task_state(&f, NOW), NOW), "Ana wants to run npm test");
    let mut f = facts();
    f.changed_files = Some(2);
    assert_eq!(status_line("Cy", &derive_task_state(&f, NOW), NOW), "Cy is done, changes ready to review");
    assert_eq!(status_line("Hal", &derive_task_state(&facts(), NOW), NOW), "Hal is waiting for a task");
}

#[test]
fn says_what_unblocks_a_blocked_agent() {
    let mut f = facts();
    f.screen = "Please sign in to continue".to_string();
    assert_eq!(status_line("Ana", &derive_task_state(&f, NOW), NOW), "Ana is waiting on you to sign in");
    let mut f = facts();
    f.screen = "Complete the CAPTCHA".to_string();
    assert_eq!(status_line("Bo", &derive_task_state(&f, NOW), NOW), "Bo needs you to solve a CAPTCHA");
    let mut f = facts();
    f.screen = "Invalid API key".to_string();
    assert_eq!(status_line("Cy", &derive_task_state(&f, NOW), NOW), "Cy needs a valid API key");
}

#[test]
fn idle_after_working_is_spent() {
    let mut f = facts();
    f.worked = true;
    let s = derive_task_state(&f, NOW);
    assert_eq!(s.state, TaskState::Idle);
    assert_eq!(s.spent, true);
    assert_eq!(status_line("Hal", &s, NOW), "Hal is done, with no changes to review");
}

#[test]
fn a_stuck_working_agent_says_so() {
    let mut s = status(TaskState::Working);
    s.stuck = Some(StuckEvidence {
        kind: StuckKind::Stall,
        text: "no file changes in 11m across 8 steps".to_string(),
        count: 8,
        sample: "stall".to_string(),
        since: 1,
    });
    assert_eq!(status_line("Cy", &s, NOW), "Cy looks stuck: no file changes in 11m across 8 steps");
}

#[test]
fn flags_a_high_risk_approval() {
    let mut f = facts();
    f.screen = format!(" Bash command\n\n   rm -rf /tmp/x\n\n{ASKING}");
    assert_eq!(status_line("Ana", &derive_task_state(&f, NOW), NOW), "Ana wants to run rm -rf /tmp/x (high risk)");
}
