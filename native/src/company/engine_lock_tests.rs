use super::harness::*;
use crate::company::lock::{EngineLock, LockRead, read, write};
use serde_json::json;

const MIN: i64 = 60_000;

fn lock(t: &T) -> EngineLock {
    match read(t.root()) {
        LockRead::Held(l) => l,
        other => panic!("no lock: {other:?}"),
    }
}

fn other_app(t: &T, at: i64) {
    write(t.root(), &EngineLock { pid: 99, at }).unwrap();
}

/// A second app opens while another one runs the company.
fn second_app(t: &mut T) {
    other_app(t, t.host.now);
    t.relaunch();
}

fn takeovers(t: &T) -> usize {
    t.alert_keys().iter().filter(|k| *k == "lock-takeover").count()
}

// two running apps

#[test]
fn keeps_the_lock_fresh_while_it_runs_the_company() {
    let mut t = T::new();
    assert_eq!(lock(&t).pid, 7);
    let first = lock(&t).at;
    t.advance(4500);
    assert!(lock(&t).at > first);
}

#[test]
fn says_why_it_could_not_start_and_keeps_the_heartbeat_going() {
    let mut host = Fake::new();
    host.roles_fail = Some("disk full".into());
    let mut t = T::with(host);
    assert!(t.e.error().unwrap().contains("disk full"));
    assert!(t.e.state().is_none());
    let first = lock(&t).at;
    t.advance(4500);
    assert!(lock(&t).at > first);
}

#[test]
fn stays_read_only_beside_another_app_no_tick_no_watch_no_writes() {
    let mut t = T::new();
    for e in std::fs::read_dir(t.root()).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() { std::fs::remove_dir_all(&p).unwrap() } else { std::fs::remove_file(&p).unwrap() }
    }
    std::fs::write(t.root().join("requests.jsonl"), "").unwrap();
    second_app(&mut t);
    assert_eq!(t.give("do it"), Ok(()));
    for _ in 0..6 {
        other_app(&t, t.host.now);
        t.advance(STEP);
    }
    t.approve("J-0001");
    assert!(t.e.state().is_none());
    assert!(t.e.view(|_| false).locked_by_other);
    assert!(t.host.spawned.is_empty());
    assert_eq!(lock(&t).pid, 99);
    let mut names: Vec<String> = std::fs::read_dir(t.root()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    assert_eq!(names, ["engine.lock", "requests.jsonl"]);
}

#[test]
fn takes_over_the_lock_of_a_dead_app_after_15_seconds_with_no_heartbeat() {
    let mut t = T::new();
    second_app(&mut t);
    assert!(t.e.state().is_none());
    t.advance(16_500);
    assert!(t.e.state().is_some());
    assert!(!t.e.view(|_| false).locked_by_other);
    assert_eq!(lock(&t).pid, 7);
}

#[test]
fn starts_at_once_on_a_lock_that_is_stale_already() {
    let mut t = T::new();
    other_app(&t, t.host.now - 20_000);
    t.relaunch();
    assert!(t.e.state().is_some());
    assert_eq!(lock(&t).pid, 7);
}

#[test]
fn carries_on_after_a_reload_the_fresh_lock_is_its_own() {
    let mut t = T::new();
    t.relaunch();
    assert!(t.e.state().is_some());
}

#[test]
fn steps_down_when_another_app_holds_the_lock_and_picks_up_from_the_files_later() {
    let mut t = T::new();
    t.give("do it").unwrap();
    other_app(&t, t.host.now);
    let before = t.e.changes;
    t.advance(STEP);
    assert!(t.e.state().is_none());
    assert!(t.e.view(|_| false).locked_by_other);
    assert!(t.e.changes > before, "the screen hears of it");
    t.advance(16_500);
    assert_eq!(t.jobs().iter().map(|j| j.id.as_str()).collect::<Vec<_>>(), ["J-0001"]);
}

// beside an app that is alive but silent

#[test]
fn stays_read_only_after_a_minute_and_starts_nobody() {
    let mut t = T::new();
    second_app(&mut t);
    t.host.alive.insert(99);
    t.advance(MIN);
    assert!(t.e.state().is_none());
    assert_eq!(lock(&t).pid, 99);
}

#[test]
fn takes_over_after_10_minutes_without_a_heartbeat_and_says_what_happened() {
    let mut t = T::new();
    other_app(&t, t.host.now);
    t.host.alive.insert(99);
    t.advance(9 * MIN);
    assert!(t.e.state().is_none());
    t.advance(MIN + 1500);
    assert!(t.e.state().is_some());
    assert_eq!(lock(&t).pid, 7);
    let a = t.e.alerts().into_iter().find(|a| a.key == "lock-takeover").unwrap();
    assert!(a.text.contains("process 99") && a.text.contains("took over the Company"), "{}", a.text);
}

#[test]
fn takes_over_at_once_from_a_lock_silent_that_long_already_and_says_so() {
    let mut t = T::new();
    other_app(&t, t.host.now - 11 * MIN);
    t.host.alive.insert(99);
    t.relaunch();
    assert!(t.e.state().is_some());
    assert_eq!(takeovers(&t), 1);
}

#[test]
fn leaves_a_live_app_be_after_the_machine_slept() {
    let mut t = T::new();
    second_app(&mut t);
    t.host.alive.insert(99);
    t.host.now += 8 * 60 * MIN; // no tick runs in a sleep
    t.advance(STEP);
    assert!(t.e.state().is_none());
    assert_eq!(lock(&t).pid, 99);
    assert_eq!(takeovers(&t), 0);
}

#[test]
fn leaves_a_live_app_be_when_the_clock_is_moved_back() {
    let mut t = T::new();
    other_app(&t, t.host.now);
    t.host.alive.insert(99);
    t.advance(STEP);
    t.host.now -= 11 * MIN;
    t.advance(3000);
    assert!(t.e.state().is_none());
    assert_eq!(lock(&t).pid, 99);
    assert_eq!(takeovers(&t), 0);
}

#[test]
fn leaves_a_live_app_be_after_a_gap_just_under_10_minutes() {
    let mut t = T::new();
    other_app(&t, t.host.now);
    t.host.alive.insert(99);
    t.advance(STEP);
    t.host.now += 10 * MIN - 2000;
    t.advance(4500);
    assert!(t.e.state().is_none());
    assert_eq!(takeovers(&t), 0);
}

#[test]
fn still_takes_over_from_a_stuck_app_once_its_heartbeat_is_10_minutes_old_by_the_moved_clock() {
    let mut t = T::new();
    other_app(&t, t.host.now);
    t.host.alive.insert(99);
    t.advance(STEP);
    t.host.now -= 11 * MIN;
    t.advance(20 * MIN);
    assert!(t.e.state().is_none());
    t.advance(MIN + 3000);
    assert!(t.e.state().is_some());
    assert_eq!(takeovers(&t), 1);
}

#[test]
fn raises_no_alert_when_the_holder_is_simply_dead() {
    let mut t = T::new();
    second_app(&mut t);
    t.advance(16_500);
    assert!(t.e.state().is_some());
    assert_eq!(takeovers(&t), 0);
}

// on a lock it cannot read

#[test]
fn never_takes_over_on_a_lock_it_cannot_read() {
    let mut t = T::new();
    second_app(&mut t);
    t.advance(STEP);
    // A folder where the lock should be: every read fails, and says nothing about who holds it.
    std::fs::remove_file(t.root().join("engine.lock")).unwrap();
    std::fs::create_dir(t.root().join("engine.lock")).unwrap();
    t.advance(20_000);
    assert!(t.e.state().is_none());
}

#[test]
fn keeps_running_what_it_runs_and_writes_no_heartbeat_until_it_can_read_again() {
    let mut t = T::new();
    t.give("do it").unwrap();
    let before = lock(&t).at;
    let at = t.root().join("engine.lock");
    std::fs::remove_file(&at).unwrap();
    std::fs::create_dir(&at).unwrap();
    t.advance(4500);
    assert!(t.e.state().is_some());
    std::fs::remove_dir(&at).unwrap();
    write(t.root(), &EngineLock { pid: 7, at: before }).unwrap();
    t.advance(STEP);
    assert!(lock(&t).at > before);
}

// a job file that can't be read, a company that won't load

#[test]
fn raises_an_alert_for_a_job_file_that_cant_be_read_and_the_rest_of_the_company_starts() {
    let mut t = T::new();
    t.give("do it").unwrap();
    let bad = t.root().join("jobs").join("J-0009.json");
    std::fs::write(&bad, "{ half").unwrap();
    t.relaunch();
    assert_eq!(t.jobs().iter().map(|j| j.id.as_str()).collect::<Vec<_>>(), ["J-0001"]);
    let alerts = t.e.alerts();
    assert_eq!(alerts.len(), 1);
    assert!(alerts[0].text.contains("J-0009.json"));
    assert_eq!(std::fs::read_to_string(&bad).unwrap(), "{ half");
    let key = alerts[0].key.clone();
    t.e.dismiss(&key);
    assert!(t.e.alerts().is_empty());
}

#[test]
fn keeps_the_error_of_a_company_that_wont_load_and_never_writes_over_the_file() {
    let mut t = T::new();
    let at = t.root().join("company.json");
    std::fs::write(&at, "{oops").unwrap();
    t.relaunch();
    assert!(t.e.error().is_some());
    assert!(t.e.state().is_none());
    assert_eq!(std::fs::read_to_string(&at).unwrap(), "{oops");
    assert!(!t.host.logs.is_empty());
}

#[test]
fn has_no_error_when_it_loads() {
    assert_eq!(T::new().e.error(), None);
}

// a disk that refuses to save

/// company.json.tmp can't be written: a folder is in its place.
fn refuse_saves(t: &T) {
    std::fs::create_dir_all(t.root().join("company.json.tmp")).unwrap();
}

fn allow_saves(t: &T) {
    std::fs::remove_dir_all(t.root().join("company.json.tmp")).unwrap();
}

#[test]
fn still_places_the_cards_starts_the_hires_and_tells_the_founder_when_approve_cannot_save() {
    let mut t = T::new();
    t.waiting_job(plan_for("a", 1));
    refuse_saves(&t);
    t.approve(&t.job(0).id);
    assert_eq!(t.job(0).status, JobStatus::Running);
    assert_eq!(t.cards(A).len(), 1);
    assert!(t.host.spawned.contains(&"Ana".to_string()));
    assert!(t.said_all().contains("approved"));
    assert!(t.said_all().contains("J-0001 could not be saved"));
}

#[test]
fn does_not_let_a_failed_save_open_the_door_to_a_second_approve() {
    let mut t = T::new();
    t.waiting_job(plan_for("a", 0));
    refuse_saves(&t);
    t.approve(&t.job(0).id);
    t.approve(&t.job(0).id);
    assert_eq!(t.cards(A).len(), 1);
}

#[test]
fn takes_a_request_in_memory_when_a_tick_cannot_save() {
    let mut t = T::new();
    t.give("do it").unwrap();
    refuse_saves(&t);
    t.staff_floors(&plan_for("a", 0));
    t.plan("J-0001", &plan_for("a", 0));
    assert_eq!(t.job(0).status, JobStatus::AwaitingPlan);
    assert!(t.host.logs.iter().any(|l| l.contains("could not be saved")));
}

#[test]
fn tells_the_founder_a_plan_was_sent_back_even_when_it_cannot_save() {
    let mut t = T::new();
    t.waiting_job(plan_for("a", 0));
    refuse_saves(&t);
    let id = t.job(0).id;
    t.e.send_back(&mut t.host, &id, "too many people");
    assert_eq!(t.job(0).status, JobStatus::Planning);
    assert!(t.said_all().contains("too many people"));
}

#[test]
fn tells_the_founder_of_a_new_job_when_it_cannot_save() {
    let mut t = T::new();
    refuse_saves(&t);
    t.give("do it").unwrap();
    assert!(t.said_all().contains("JOB J-0001"));
    assert!(!t.host.logs.is_empty());
}

#[test]
fn starts_reading_requests_even_when_the_first_save_fails() {
    let mut t = T::new();
    refuse_saves(&t);
    t.relaunch();
    t.advance(STEP);
    allow_saves(&t);
    t.give("do it").unwrap();
    t.staff_floors(&plan_for("a", 0));
    t.request(json!({ "type": "plan", "ts": 1, "from": "founder", "job": "J-0001", "plan": plan_for("a", 0) }));
    assert_eq!(t.job(0).status, JobStatus::AwaitingPlan);
}
