//! What a person's line names: the Job they are on, never their seat's role title.

use super::model::*;
use crate::company::types::{Company, Floor, Job, JobStatus, Look, Person, Rank};
use serde_json::Map;
use std::collections::BTreeMap;
use supervise::task_state::{TaskState, TaskStatus};

fn person(id: &str, rank: Rank) -> Person {
    Person {
        id: id.into(),
        name: id.to_uppercase(),
        role: if rank == Rank::Founder { "founder".into() } else { "developer".into() },
        rank,
        floor: Some("a".into()),
        boss: None,
        cli: "claude".into(),
        look: Look::default(),
        session: None,
        worktree: None,
        branch: None,
        extra: Map::new(),
    }
}

fn job(id: &str, title: &str, status: JobStatus, people: &[&str]) -> Job {
    let mut j: Job = serde_json::from_str(&format!(r#"{{"id":"{id}","status":"running","people":[]}}"#)).unwrap();
    j.title = title.into();
    j.status = status;
    j.people = people.iter().map(|p| p.to_string()).collect();
    j
}

/// A seat titled after its role, as fleet_host names every seat.
fn seat(id: &str, title: &str) -> Pane {
    let status = TaskStatus { state: TaskState::Working, ask: None, limit: None, blocked: None, spent: false, stuck: None, clash: None };
    Pane { person_id: id.into(), status, doing: Some(title.into()), nothing_to_merge: false }
}

fn doing(jobs: &[Job], panes: &[Pane], id: &str) -> Option<String> {
    let mut c = Company::empty();
    c.floors = vec![Floor { id: "a".into(), dir: "D:\\a".into(), name: "alpha".into(), extra: Map::new() }];
    c.people.extend([person("boss", Rank::Founder), person("hai", Rank::Staff)]);
    let m = office_model(&c, jobs, &[], panes, &[], &BTreeMap::new(), &[]);
    m.person(id).and_then(|p| p.doing.clone())
}

#[test]
fn the_founder_names_the_job_they_plan_not_founder() {
    let jobs = [job("J-1", "Old work", JobStatus::Running, &[]), job("J-2", "Add a pricing page", JobStatus::Planning, &[])];
    assert_eq!(doing(&jobs, &[seat("boss", "Founder")], "boss").as_deref(), Some("Add a pricing page"));
}

#[test]
fn the_founder_names_the_newest_running_job_when_none_is_planned() {
    let jobs = [job("J-1", "Old work", JobStatus::Running, &[]), job("J-2", "New work", JobStatus::Running, &[])];
    assert_eq!(doing(&jobs, &[seat("boss", "Founder")], "boss").as_deref(), Some("New work"));
}

#[test]
fn staff_name_the_running_job_that_counts_them_in() {
    let jobs = [job("J-1", "Fix the login", JobStatus::Running, &["hai"]), job("J-2", "Not mine", JobStatus::Running, &[])];
    assert_eq!(doing(&jobs, &[seat("hai", "Developer")], "hai").as_deref(), Some("Fix the login"));
}

#[test]
fn a_seat_titled_after_its_role_says_nothing_more() {
    assert_eq!(doing(&[], &[seat("boss", "Founder"), seat("hai", "Developer")], "boss"), None);
    assert_eq!(doing(&[], &[seat("hai", "Developer")], "hai"), None);
}

#[test]
fn a_seat_with_its_own_title_keeps_it() {
    assert_eq!(doing(&[], &[seat("hai", "Hai, from Minh")], "hai").as_deref(), Some("Hai, from Minh"));
}

#[test]
fn a_long_job_is_clipped() {
    let long = "Add a pricing page with three plans, a yearly switch and a comparison table below it";
    let got = doing(&[job("J-1", long, JobStatus::Planning, &[])], &[], "boss").unwrap();
    assert!(got.ends_with('\u{2026}') && got.chars().count() <= 61, "{got}");
}
