//! The words Home says, checked against inboxstart.ts.

use super::dropdown::{count_label, folder_name};
use super::home::{greeting, status_sentence};
// Not super::*: gpui's own `test` macro would shadow #[test].
use super::State;

#[test]
fn greets_by_hour() {
    assert_eq!(greeting(3), "Working late");
    assert_eq!(greeting(9), "Good morning");
    assert_eq!(greeting(14), "Good afternoon");
    assert_eq!(greeting(21), "Good evening");
}

#[test]
fn says_who_waits_first() {
    assert_eq!(status_sentence(&["Ana", "Bob"], &[State::Needs, State::Working], false), "Ana is waiting on you.");
    assert_eq!(status_sentence(&["Ana", "Bob"], &[State::Needs, State::Blocked], false), "2 agents are waiting on you.");
    assert_eq!(status_sentence(&["Ana"], &[State::Review], false), "One agent has finished and is ready to review.");
    assert_eq!(status_sentence(&["Ana", "Bob"], &[State::Working, State::Working], false), "2 agents are working. Nothing needs you yet.");
    assert_eq!(status_sentence(&[], &[], true), "Pick a folder, say what you want done, and an agent starts on it.");
    assert_eq!(status_sentence(&["Ana"], &[State::Stopped], false), "Nothing is running. What should an agent do next?");
}

#[test]
fn names_folders_and_counts() {
    assert_eq!(folder_name(r"D:\bytehive\"), "bytehive");
    assert_eq!(folder_name("C:/Users/me/code/app"), "app");
    assert_eq!(count_label(1), "1 agent");
    assert_eq!(count_label(3), "3 agents, same job");
}

#[test]
fn settings_ship_with_todays_defaults() {
    let p = crate::presets::Prefs::default();
    assert_eq!((p.default_cli.as_str(), p.default_count, p.job_delay, p.font_size, p.restore, p.worktree), ("last", 1, 4, 15, true, false));
    let mut odd: crate::presets::Prefs = serde_json::from_str(r#"{"default_count":7,"job_delay":0,"font_size":40}"#).unwrap();
    odd.clamp();
    assert_eq!((odd.default_count, odd.job_delay, odd.font_size), (1, 1, 20));
}

#[test]
fn sessions_say_where_and_when() {
    use super::sessions::list::{day_label, place_of};
    assert_eq!(place_of("D:/app", Some(r"D:\app")), "");
    assert_eq!(place_of("D:/.bytehive-worktrees/app-1a2b3c/fix-upload-9z8y7x", None), "worktree fix-upload");
    assert_eq!(place_of("C:/Users/me/.claude/worktrees/try", None), "worktree try");
    assert_eq!(place_of("C:/Users/me/code/app", None), "code/app");
    let now = 1_794_000_000_000;
    assert_eq!(day_label(now, now), "Today");
    assert_eq!(day_label(now - 86_400_000, now), "Yesterday");
}

#[test]
fn history_says_what_happened_once() {
    use super::history::{Event, Histories, describe};
    use supervise::task_state::TaskStatus;
    let status = |state| TaskStatus { state, ask: None, limit: None, blocked: None, spent: false, stuck: None, clash: None };
    assert_eq!(describe(&status(State::Working), None).as_deref(), Some("Started working"));
    assert_eq!(describe(&status(State::Needs), None).as_deref(), Some("Waited for you"));
    assert_eq!(describe(&status(State::Idle), None), None);
    assert_eq!(describe(&status(State::Review), Some(1)).as_deref(), Some("Finished with 1 file changed"));
    let mut h = Histories::default();
    for (at, state) in [(1, State::Working), (2, State::Idle), (3, State::Working), (4, State::Stopped)] {
        h.observe("a", &status(state), None, at);
    }
    let texts: Vec<&str> = h.of("a").iter().map(|e| e.text.as_str()).collect();
    assert_eq!(texts, vec!["Started working", "Stopped"], "one line per stretch of work");
    h.log("a", Event { at: 5, state: State::Merged, text: "Merged x into main".into() });
    assert_eq!(h.of("a").len(), 3);
}

#[test]
fn evening_reads_pm() {
    use chrono::{Local, TimeZone as _};
    let at = Local.with_ymd_and_hms(2026, 11, 9, 23, 0, 0).unwrap().timestamp_millis() as u64;
    assert_eq!(supervise::task_state::clock_of(at, at), "11:00 PM");
    let morning = Local.with_ymd_and_hms(2026, 11, 9, 9, 5, 0).unwrap().timestamp_millis() as u64;
    assert_eq!(supervise::task_state::clock_of(morning, morning), "9:05 AM");
}

#[test]
fn a_company_person_is_never_done_only_waiting() {
    use super::words::{counts_done, said_line};
    assert!(counts_done(true, None));
    assert!(!counts_done(true, Some("founder")), "the Founder idles on a plan waiting for the owner");
    assert!(!counts_done(false, None));
    let idle = supervise::task_state::TaskStatus { state: State::Idle, ask: None, limit: None, blocked: None, spent: false, stuck: None, clash: None };
    assert_eq!(said_line(&idle, counts_done(true, Some("p1"))), "Waiting for a task");
}
