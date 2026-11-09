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
