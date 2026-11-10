use super::harness::*;
use crate::company::lock::{LockRead, read};

#[test]
fn tells_the_founder_when_a_hire_fails_to_start_after_it_began() {
    let mut t = T::new();
    t.waiting_job(plan_for("a", 1));
    t.host.late_fails.insert("Ana".into(), "no terminal".into());
    t.host.hold = true;
    t.approve(&t.job(0).id);
    assert!(!t.said_all().contains("did not start"));
    t.host.hold = false;
    t.advance(STEP);
    assert!(t.said_all().contains("Ana did not start: no terminal."), "{:?}", t.said());
    assert_eq!(t.job(0).status, JobStatus::Running);
}

#[test]
fn tells_the_owner_when_the_founder_fails_to_start() {
    let mut t = T::new();
    t.host.late_fails.insert("Founder".into(), "claude is not installed".into());
    t.give("do it").unwrap();
    t.advance(STEP);
    let a = t.e.alerts().into_iter().find(|a| a.key == "founder-start").unwrap();
    assert_eq!(a.text, "The Founder did not start: claude is not installed.");
}

#[test]
fn tells_the_owner_once_when_slow_work_runs_out_of_time() {
    let mut t = T::new();
    t.host.hold = true;
    t.give("do it").unwrap();
    t.advance(299_000);
    assert!(!t.alert_keys().iter().any(|k| k.starts_with("late-")));
    t.advance(3000);
    let late: Vec<_> = t.e.alerts().into_iter().filter(|a| a.key.starts_with("late-")).collect();
    assert!(late.iter().any(|a| a.text == "Starting Founder took over 300 s, so ByteHive moved on. It may still finish."), "{late:?}");
    assert!(late.iter().any(|a| a.text.starts_with("Keeping .bytehive/ out of git in D:\\a took over 300 s")), "{late:?}");
    t.advance(60_000);
    assert_eq!(t.e.alerts().into_iter().filter(|a| a.key == "late-Starting Founder").count(), 1, "told once");
}

#[test]
fn keeps_the_heartbeat_going_while_slow_work_runs() {
    let mut t = T::new();
    t.host.hold = true;
    t.waiting_job(plan_for("a", 1));
    t.approve(&t.job(0).id);
    t.advance(20_000);
    let LockRead::Held(l) = read(t.root()) else { panic!("no lock") };
    assert!(t.host.now - l.at < 3000);
}

#[test]
fn counts_a_late_result_when_it_comes() {
    let mut t = T::new();
    t.host.hold = true;
    t.host.trust_fails = Some("locked".into());
    t.give("do it").unwrap();
    t.advance(310_000);
    t.host.hold = false;
    t.advance(STEP);
    assert!(t.alert_keys().contains(&"trust-founder".to_string()));
}
