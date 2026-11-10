use super::harness::*;
use super::watch_tests::{MIN, running, spend, usage};
use serde_json::json;

// stopping, raising the cap and closing a running job

#[test]
fn stops_a_running_job_for_good_and_tells_the_founder() {
    let mut t = T::new();
    let ana = running(&mut t, None);
    t.founder_up(A);
    t.e.stop_job(&mut t.host, "J-0001").unwrap();
    assert_eq!(t.job(0).status, JobStatus::Stopped);
    assert_eq!(t.stored("J-0001")["status"], "stopped");
    assert!(t.said().iter().any(|s| s.contains("J-0001") && s.contains("stopped")));
    t.clear_said();
    t.message(&ana.id, "Founder", "done");
    assert_eq!(t.said().len(), 1);
    assert!(t.said()[0].starts_with("Held"), "{:?}", t.said());
}

#[test]
fn stops_a_paused_job_and_its_stop_alert_goes_with_it() {
    let mut t = T::new();
    let ana = running(&mut t, Some(1000));
    spend(&mut t, &ana.id, 100, 1200);
    t.e.stop_job(&mut t.host, "J-0001").unwrap();
    t.advance(30_000);
    assert_eq!(t.job(0).status, JobStatus::Stopped);
    assert!(t.e.alerts().is_empty());
}

#[test]
fn takes_a_stopped_jobs_warning_off_needs_you_at_once() {
    let mut t = T::new();
    let ana = running(&mut t, Some(1000));
    spend(&mut t, &ana.id, 100, 950);
    assert_eq!(t.alert_keys(), ["cap-J-0001"]);
    t.e.stop_job(&mut t.host, "J-0001").unwrap();
    assert!(t.e.alerts().is_empty());
}

#[test]
fn leaves_the_cards_of_a_stopped_job_alone_and_counts_nothing_more() {
    let mut t = T::new();
    let ana = running(&mut t, None);
    spend(&mut t, &ana.id, 100, 200);
    t.e.stop_job(&mut t.host, "J-0001").unwrap();
    t.clear_said();
    usage(&mut t, &ana.id, 900);
    t.advance(30 * MIN);
    assert!(t.said().is_empty());
    assert_eq!(t.job(0).tokens_used, 100);
}

#[test]
fn stops_a_job_still_being_planned_and_tells_the_founder() {
    let mut t = T::new();
    t.give("do it").unwrap();
    t.clear_said();
    t.e.stop_job(&mut t.host, "J-0001").unwrap();
    assert_eq!(t.job(0).status, JobStatus::Stopped);
    assert_eq!(t.stored("J-0001")["status"], "stopped");
    assert!(t.said().iter().any(|s| s.contains("J-0001") && s.contains("stopped")));
}

#[test]
fn does_not_stop_a_job_that_is_over_or_one_that_isnt_there() {
    let mut t = T::new();
    t.give("do it").unwrap();
    t.e.stop_job(&mut t.host, "J-0001").unwrap();
    t.clear_said();
    t.e.stop_job(&mut t.host, "J-0001").unwrap();
    t.e.stop_job(&mut t.host, "J-9999").unwrap();
    assert!(t.said().is_empty());
}

#[test]
fn closes_a_job_that_needs_no_work_and_shows_the_owner_why() {
    let mut t = T::new();
    t.give("is it done?").unwrap();
    t.request(json!({ "type": "job-done", "ts": 5, "from": "founder", "job": "J-0001", "summary": "Yes, last week." }));
    assert_eq!(t.job(0).status, JobStatus::Done);
    assert_eq!(t.stored("J-0001")["summary"], "Yes, last week.");
    let a = t.e.alerts().into_iter().find(|a| a.key == "no-work-J-0001").unwrap();
    assert!(a.standing && a.text.contains("Yes, last week."));
}

#[test]
fn raises_the_cap_and_carries_on_a_paused_job_but_never_by_nothing() {
    let mut t = T::new();
    let ana = running(&mut t, Some(1000));
    spend(&mut t, &ana.id, 100, 1200);
    t.e.raise_cap(&mut t.host, "J-0001", 0).unwrap();
    assert_eq!((t.job(0).status, t.job(0).token_cap), (JobStatus::Paused, 1000));
    t.e.raise_cap(&mut t.host, "J-0001", 5000).unwrap();
    assert_eq!((t.job(0).status, t.job(0).token_cap), (JobStatus::Running, 6000));
    assert_eq!(t.stored("J-0001")["tokenCap"], 6000);
}

#[test]
fn does_not_raise_the_cap_of_a_job_that_is_not_paused() {
    let mut t = T::new();
    running(&mut t, Some(1000));
    t.e.raise_cap(&mut t.host, "J-0001", 5000).unwrap();
    assert_eq!(t.job(0).token_cap, 1000);
}

#[test]
fn closes_a_job_that_waits_for_merge_and_only_that() {
    let mut t = T::new();
    running(&mut t, Some(1000));
    t.e.finish_job(&mut t.host, "J-0001").unwrap();
    assert_eq!(t.job(0).status, JobStatus::Running);
    t.request(json!({ "type": "job-done", "ts": 5, "from": "founder", "job": "J-0001", "summary": "ok" }));
    assert_eq!(t.job(0).status, JobStatus::AwaitingMerge);
    t.e.finish_job(&mut t.host, "J-0001").unwrap();
    assert_eq!(t.job(0).status, JobStatus::Done);
    assert_eq!(t.stored("J-0001")["status"], "done");
    t.e.finish_job(&mut t.host, "J-9999").unwrap();
    assert_eq!(t.job(0).status, JobStatus::Done);
}
