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
    assert_eq!((p.default_cli.as_str(), p.default_count, p.job_delay, p.font_size, p.restore), ("last", 1, 4, 15, true));
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
