use super::harness::*;
use super::watching::measure;
use serde_json::json;

const MIN: i64 = 60_000;

/// What Needs you lists, from the view, as the surface builds it.
fn needs(t: &T) -> Vec<crate::company::needs::NeedRow> {
    let v = t.e.view();
    let Some(c) = v.company else { return Vec::new() };
    crate::company::needs::needs_you(&v.jobs, &v.alerts, &crate::company::needs::Names { floors: &c.floors, roles: &v.roles }, |_| false)
}

/// Job J-0001 running, with Ana on floor a holding its one card in Doing.
fn running(t: &mut T, token_cap: Option<u64>) -> Person {
    let mut plan = plan_for("a", 1);
    plan.token_cap = token_cap;
    plan.cards[0].assignee = Some("Ana".into());
    t.waiting_job(plan);
    t.approve(&t.job(0).id);
    let card = t.list_mut(A, "To do").pop().unwrap();
    t.list_mut(A, "Doing").push(card);
    t.clear_said();
    t.person(|p| p.name == "Ana")
}

fn usage(t: &mut T, id: &str, fresh: u64) {
    t.host.seat_mut(id).fresh_tokens = Some(Some(fresh));
}

/// Ana reads `before`, then `after`, 30 seconds apart.
fn spend(t: &mut T, id: &str, before: u64, after: u64) {
    usage(t, id, before);
    t.advance(30_000);
    usage(t, id, after);
    t.advance(30_000);
}

// the watch and To do

#[test]
fn nudges_the_boss_about_a_card_nobody_picked_up() {
    let mut t = T::new();
    t.waiting_job(plan_for("a", 1));
    t.approve(&t.job(0).id);
    t.clear_said();
    t.advance(11 * MIN);
    assert!(t.said_all().contains("has waited in To do"), "{:?}", t.said());
}

#[test]
fn does_not_count_someone_named_on_a_to_do_card_as_working_the_job() {
    let mut t = T::new();
    t.waiting_job(plan_for("a", 1));
    t.approve(&t.job(0).id);
    let todo = t.list_mut(A, "To do");
    todo[0].assignee = Some("Founder".into());
    todo[0].assignee_id = Some("founder".into());
    t.advance(30_000);
    assert!(!t.job(0).people.contains(&"founder".to_string()));
}

// Needs you

#[test]
fn needs_you_is_nothing_for_someone_with_no_jobs_whatever_the_alerts_say() {
    let mut t = T::new();
    t.e.dismiss("none");
    t.host.close("founder");
    t.host.projects.clear();
    let v = t.e.view();
    assert!(v.jobs.is_empty() && needs(&t).is_empty());
}

#[test]
fn needs_you_is_the_rows_waiting_on_the_owner() {
    let mut t = T::new();
    assert!(needs(&t).is_empty());
    t.waiting_job(plan_for("a", 1));
    assert_eq!(needs(&t).len(), 1);
    t.approve(&t.job(0).id);
    assert!(needs(&t).is_empty());
}

#[test]
fn tells_the_screen_of_a_change() {
    let mut t = T::new();
    let before = t.e.changes;
    t.give("one").unwrap();
    assert!(t.e.changes > before);
}

// watching a running job

#[test]
fn only_reads_the_boards_of_floors_with_people_on_them() {
    let mut t = T::new();
    running(&mut t, None);
    t.host.board_reads.clear();
    t.advance(30_000);
    assert_eq!(t.host.board_reads, [A]);
}

#[test]
fn watches_a_card_a_lead_added_with_its_job_and_its_assignee_joins_the_job() {
    let mut t = T::new();
    running(&mut t, None);
    let mut card = crate::board::mk_card("Added later");
    card.job = Some("J-0001".into());
    card.assignee = Some("Founder".into());
    card.assignee_id = Some("founder".into());
    t.list_mut(A, "Doing").push(card);
    t.advance(10 * MIN + 30_000);
    assert!(t.job(0).people.contains(&"founder".to_string()));
    // Nobody above the Founder to nudge.
    assert!(t.e.alerts().iter().any(|a| a.text.contains("Added later")));
}

#[test]
fn counts_the_fresh_tokens_a_person_adds_from_when_they_were_first_seen() {
    let mut t = T::new();
    let ana = running(&mut t, None);
    usage(&mut t, &ana.id, 400);
    t.advance(30_000);
    assert_eq!(t.job(0).token_base.get(&ana.id), Some(&400));
    usage(&mut t, &ana.id, 650);
    t.advance(30_000);
    assert_eq!(t.job(0).tokens_used, 250);
    assert_eq!(t.stored("J-0001")["tokensUsed"], 250);
}

#[test]
fn counts_someone_who_picked_up_a_card_of_the_job_from_when_they_took_it() {
    let mut t = T::new();
    running(&mut t, None);
    t.list_mut(A, "Doing")[0].assignee_id = Some("founder".into());
    let now = t.host.now;
    t.host.add_seat("founder", "Founder", A, SeatView { running: true, at_prompt: true, spawned_at: Some(now), ..SeatView::default() });
    spend(&mut t, "founder", 500, 800);
    assert!(t.job(0).people.contains(&"founder".to_string()));
    assert_eq!(t.job(0).tokens_used, 300);
}

#[test]
fn does_not_take_a_persons_unread_usage_for_zero() {
    let mut t = T::new();
    let ana = running(&mut t, None);
    t.advance(30_000);
    assert!(t.job(0).token_base.is_empty());
    usage(&mut t, &ana.id, 90_000);
    t.advance(30_000);
    assert_eq!(t.job(0).token_base.get(&ana.id), Some(&90_000));
    assert_eq!(t.job(0).tokens_used, 0);
}

#[test]
fn pauses_the_job_at_its_cap_raises_an_alert_and_tells_the_founder() {
    let mut t = T::new();
    let ana = running(&mut t, Some(1000));
    t.founder_up(A);
    spend(&mut t, &ana.id, 100, 1200);
    assert_eq!(t.job(0).status, JobStatus::Paused);
    assert_eq!(t.stored("J-0001")["status"], "paused");
    let alerts = t.e.alerts();
    assert_eq!(alerts.len(), 1);
    assert_eq!((alerts[0].key.as_str(), alerts[0].job.as_deref()), ("cap-J-0001", Some("J-0001")));
    assert!(t.said().iter().any(|s| s.contains("J-0001") && s.contains("token cap")));
    t.clear_said();
    t.e.raise_cap(&mut t.host, "J-0001", 1000).unwrap();
    assert_eq!(t.said().iter().filter(|s| s.contains("J-0001")).count(), 2);
}

#[test]
fn leaves_someone_stopped_asleep_when_the_job_runs_again() {
    let mut t = T::new();
    let ana = running(&mut t, Some(1000));
    t.founder_up(A);
    spend(&mut t, &ana.id, 100, 1200);
    t.host.seat_mut(&ana.id).running = false;
    t.clear_said();
    t.e.raise_cap(&mut t.host, "J-0001", 1000).unwrap();
    assert!(t.host.restarts.is_empty());
    assert_eq!(t.said().iter().filter(|s| s.contains("running again")).count(), 1);
}

#[test]
fn keeps_the_stop_alert_for_as_long_as_the_job_is_paused_and_drops_it_on_resume() {
    let mut t = T::new();
    let ana = running(&mut t, Some(1000));
    spend(&mut t, &ana.id, 100, 1200);
    t.advance(3 * 30_000);
    assert_eq!(t.alert_keys(), ["cap-J-0001"]);
    t.e.raise_cap(&mut t.host, "J-0001", 5000).unwrap();
    t.advance(30_000);
    assert!(t.e.alerts().is_empty());
}

#[test]
fn keeps_counting_when_a_persons_seat_is_gone_and_another_person_spends() {
    let mut t = T::new();
    let plan = JobPlan { token_cap: Some(10_000_000), hires: vec![Hire { role: "developer".into(), floor: "a".into() }, Hire { role: "developer".into(), floor: "b".into() }], ..plan_for("a", 0) };
    t.waiting_job(plan);
    t.approve(&t.job(0).id);
    let devs: Vec<Person> = t.people().into_iter().filter(|p| p.role == "developer" && !p.id.starts_with("r-")).collect();
    usage(&mut t, &devs[0].id, 0);
    usage(&mut t, &devs[1].id, 0);
    t.advance(30_000);
    usage(&mut t, &devs[0].id, 2_000_000);
    t.advance(30_000);
    assert_eq!(t.job(0).tokens_used, 2_000_000);
    t.host.close(&devs[0].id);
    usage(&mut t, &devs[1].id, 2_000_000);
    t.advance(30_000);
    assert_eq!(t.job(0).tokens_used, 4_000_000);
}

#[test]
fn rebases_when_a_session_restarts_without_losing_what_was_counted() {
    let mut t = T::new();
    let ana = running(&mut t, None);
    spend(&mut t, &ana.id, 100, 1100);
    assert_eq!(t.job(0).tokens_used, 1000);
    usage(&mut t, &ana.id, 50); // /clear: a new session starts from nothing
    t.advance(30_000);
    assert_eq!(t.job(0).tokens_used, 1000);
    usage(&mut t, &ana.id, 350);
    t.advance(30_000);
    assert_eq!(t.job(0).tokens_used, 1300);
    assert_eq!(t.stored("J-0001")["tokensUsed"], 1300);
}

#[test]
fn does_not_nudge_a_card_that_comes_back_to_doing_as_if_it_had_been_quiet_all_along() {
    let mut t = T::new();
    running(&mut t, None);
    t.advance(9 * MIN);
    let card = t.list_mut(A, "Doing").pop().unwrap();
    t.list_mut(A, "To do").push(card);
    t.advance(30_000);
    let card = t.list_mut(A, "To do").pop().unwrap();
    t.list_mut(A, "Doing").push(card);
    t.advance(5 * MIN);
    assert!(!t.said_all().contains("gone quiet"));
}

#[test]
fn writes_nothing_while_nothing_changes() {
    let mut t = T::new();
    let ana = running(&mut t, None);
    usage(&mut t, &ana.id, 100);
    t.advance(30_000);
    let job = t.root().join("jobs").join("J-0001.json");
    let company = t.root().join("company.json");
    std::fs::write(&job, "untouched").unwrap();
    std::fs::write(&company, "untouched").unwrap();
    t.advance(5 * 30_000);
    assert_eq!(std::fs::read_to_string(&job).unwrap(), "untouched");
    assert_eq!(std::fs::read_to_string(&company).unwrap(), "untouched");
}

#[test]
fn asks_the_boss_to_look_at_a_card_that_has_gone_quiet() {
    let mut t = T::new();
    running(&mut t, None);
    t.advance(30_000);
    assert!(t.said().is_empty());
    t.advance(10 * MIN);
    assert_eq!(t.said().iter().filter(|s| s.contains("gone quiet") && s.contains("Do it")).count(), 1);
}

#[test]
fn leaves_the_cards_of_a_paused_job_alone() {
    let mut t = T::new();
    let ana = running(&mut t, Some(1000));
    spend(&mut t, &ana.id, 100, 1200);
    t.clear_said();
    t.advance(30 * MIN);
    assert!(t.said().is_empty());
}

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

#[test]
fn measure_gives_back_the_same_job_when_nothing_moved() {
    let mut t = T::new();
    running(&mut t, None);
    let j = t.job(0);
    assert_eq!(measure(&j, |_| None), j);
}

#[test]
fn nudges_go_to_the_boss_as_a_message_for_them() {
    let mut t = T::new();
    let ana = running(&mut t, None);
    t.advance(10 * MIN + 30_000);
    let nudge = t.e.said.iter().find(|(_, text, _)| text.contains("gone quiet")).unwrap();
    assert_eq!(Some(nudge.0.clone()), ana.boss);
}

#[test]
fn counts_each_jobs_cards_and_those_done_from_the_boards_it_reads() {
    let mut t = T::new();
    running(&mut t, None);
    assert!(t.e.view().job_cards.is_empty());
    t.advance(30_000);
    assert_eq!(t.e.view().job_cards.get("J-0001"), Some(&(0, 1)));
    let card = t.list_mut(A, "Doing").pop().unwrap();
    t.list_mut(A, "Done").push(card);
    t.host.read_fails.insert(A.into(), "busy".into());
    t.advance(30_000);
    assert_eq!(t.e.view().job_cards.get("J-0001"), Some(&(0, 1)), "a failed read keeps the last count");
    t.host.read_fails.clear();
    t.advance(30_000);
    assert_eq!(t.e.view().job_cards.get("J-0001"), Some(&(1, 1)));
    t.host.boards.remove(A);
    t.advance(30_000);
    assert!(t.e.view().job_cards.is_empty(), "a board that is gone has no count");
}

#[test]
fn the_view_carries_the_roles_and_the_alerts() {
    let mut t = T::new();
    t.e.state_mut().jobs.clear();
    let v = t.e.view();
    assert_eq!(v.roles.len(), t.host.roles.len());
    assert!(v.company.is_some() && !v.locked_by_other && v.error.is_none());
}

#[test]
fn says_why_an_owner_action_did_nothing_beside_another_app() {
    let mut t = T::new();
    crate::company::lock::write(t.root(), &crate::company::lock::EngineLock { pid: 99, at: t.host.now }).unwrap();
    t.relaunch();
    assert_eq!(t.e.stop_job(&mut t.host, "J-0001").unwrap_err(), "The Company is running in another ByteHive window.");
    assert!(t.e.view().company.is_none());
}

#[test]
fn keeps_watching_after_the_clock_moves_back() {
    let mut t = T::new();
    running(&mut t, None);
    t.host.now -= 60 * MIN;
    t.host.board_reads.clear();
    t.advance(31_500);
    assert_eq!(t.host.board_reads, [A]);
}
