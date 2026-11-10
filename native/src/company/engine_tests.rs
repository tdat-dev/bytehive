use super::harness::*;
use super::super::types::NewRole;
use serde_json::json;

// giving a job without a Node that can run the MCP server

#[test]
fn says_what_it_found_and_starts_nothing_when_node_is_refused() {
    let mut host = Fake::new();
    host.node = Err("ByteHive needs Node.js 22.13 or newer to run the Company. Found v18.20.0.".into());
    let mut t = T::with(host);
    assert_eq!(t.give("do it").unwrap_err(), "ByteHive needs Node.js 22.13 or newer to run the Company. Found v18.20.0.");
    assert!(t.jobs().is_empty());
    assert!(t.host.spawned.is_empty());
}

#[test]
fn looks_again_next_time_node_may_have_been_installed_since() {
    let mut host = Fake::new();
    host.node = Err("no node".into());
    let mut t = T::with(host);
    t.host.node = Ok(());
    assert!(t.give("do it").is_err());
    t.advance(STEP);
    t.give("do it").unwrap();
    assert_eq!(t.jobs().len(), 1);
    assert_eq!(t.host.node_checks, 2);
}

#[test]
fn asks_node_once_off_the_ui_thread_not_on_every_job() {
    let mut t = T::new();
    t.give("one").unwrap();
    t.give("two").unwrap();
    assert_eq!(t.host.node_checks, 1);
}

// starting the app

#[test]
fn brings_everyones_files_up_to_date_a_session_restarts_on_the_files_it_was_given() {
    let mut t = T::new();
    t.waiting_job(hiring("developer", "a"));
    t.approve(&t.job(0).id);
    let staff = t.person(|p| p.rank == Rank::Staff);
    t.host.files.clear();
    t.relaunch();
    assert!(t.host.files.contains(&staff.id));
    assert!(t.host.files.contains(&"founder".to_string()));
}

// approve

#[test]
fn rewrites_the_files_of_someone_a_new_lead_becomes_boss_of() {
    let mut t = T::new();
    t.waiting_job(hiring("developer", "a"));
    t.approve(&t.job(0).id);
    let staff = t.person(|p| p.rank == Rank::Staff);
    t.give("a lead").unwrap();
    let id = t.job(1).id;
    t.plan(&id, &hiring("lead", "a"));
    t.host.files.clear();
    t.approve(&id);
    let lead = t.person(|p| p.rank == Rank::Lead);
    assert_eq!(t.person(|p| p.id == staff.id).boss.as_deref(), Some(lead.id.as_str()));
    assert!(t.host.files.contains(&staff.id));
}

#[test]
fn starts_the_person_a_plan_reuses_when_their_floor_was_closed_and_hires_nobody_twice() {
    let mut t = T::new();
    t.waiting_job(hiring("developer", "a"));
    t.approve(&t.job(0).id);
    let dev = t.person(|p| p.rank == Rank::Staff);
    t.host.close(&dev.id);
    t.host.spawned.clear();
    t.give("more").unwrap();
    let id = t.job(1).id;
    t.plan(&id, &hiring("developer", "a"));
    assert!(t.job(1).plan.unwrap().hires.is_empty());
    t.approve(&id);
    assert_eq!(t.people().iter().filter(|p| p.rank == Rank::Staff).count(), 1);
    assert!(t.host.spawned.contains(&dev.name));
}

#[test]
fn does_not_blame_a_reused_person_for_an_old_error_on_a_seat_that_was_already_running() {
    let mut t = T::new();
    t.waiting_job(hiring("developer", "a"));
    t.approve(&t.job(0).id);
    let dev = t.person(|p| p.rank == Rank::Staff);
    t.host.seat_mut(&dev.id).error = Some("an old crash".into());
    t.give("more").unwrap();
    let id = t.job(1).id;
    t.plan(&id, &hiring("developer", "a"));
    t.clear_said();
    t.approve(&id);
    assert!(!t.said_all().contains("did not start"));
}

#[test]
fn places_the_cards_once_however_many_clicks() {
    let mut t = T::new();
    t.waiting_job(plan_for("a", 0));
    let id = t.job(0).id;
    t.approve(&id);
    t.approve(&id);
    assert_eq!(t.cards(A).len(), 1);
    assert_eq!(t.job(0).status, JobStatus::Running);
}

#[test]
fn does_nothing_to_a_job_that_isnt_waiting() {
    let mut t = T::new();
    t.give("do it").unwrap();
    t.approve(&t.job(0).id);
    assert_eq!(t.job(0).status, JobStatus::Planning);
    assert!(t.host.boards.is_empty());
}

#[test]
fn tries_the_board_again_on_a_conflict_on_a_fresh_copy() {
    let mut t = T::new();
    t.waiting_job(plan_for("a", 0));
    t.host.write_fails.extend([BoardWrite::Conflict, BoardWrite::Conflict]);
    t.approve(&t.job(0).id);
    assert_eq!(t.cards(A).len(), 1);
    assert!(!t.said_all().contains("were not placed"));
}

#[test]
fn gives_up_after_three_conflicts_and_tells_the_founder_who_still_gets_the_rest() {
    let mut t = T::new();
    let mut plan = plan_for("a", 0);
    plan.cards.extend(plan_for("b", 0).cards);
    t.waiting_job(plan);
    t.host.write_fails.extend([BoardWrite::Conflict, BoardWrite::Conflict, BoardWrite::Conflict]);
    t.approve(&t.job(0).id);
    assert_eq!(t.job(0).status, JobStatus::Running);
    assert_eq!(t.cards(B).len(), 1);
    assert!(t.said_all().contains("approved"));
    assert!(t.said_all().contains("Cards for a"));
}

#[test]
fn gets_past_a_corrupt_board_and_a_hire_that_wont_start() {
    let mut t = T::new();
    let mut plan = plan_for("a", 1);
    plan.cards.extend(plan_for("b", 0).cards);
    t.waiting_job(plan);
    t.host.read_fails.insert(A.into(), "board.json is not valid JSON".into());
    t.host.spawn_fails.insert("Ana".into());
    t.approve(&t.job(0).id);
    assert_eq!(t.cards(B).len(), 1);
    assert!(t.said_all().contains("Cards for a"));
    assert!(t.said_all().contains("Ana did not start: no terminal"));
}

#[test]
fn matches_a_cards_assignee_whatever_the_case() {
    let mut t = T::new();
    let mut plan = plan_for("a", 0);
    plan.cards[0].assignee = Some("founder".into());
    t.waiting_job(plan);
    t.approve(&t.job(0).id);
    let c = &t.cards(A)[0];
    assert_eq!((c.assignee.as_deref(), c.assignee_id.as_deref()), (Some("Founder"), Some("founder")));
    assert_eq!(c.job.as_deref(), Some("J-0001"));
}

#[test]
fn places_nothing_when_the_plan_no_longer_holds_and_says_why() {
    let mut t = T::new();
    t.waiting_job(plan_for("a", 1));
    t.e.state_mut().company.limits.people_total = 1;
    t.approve(&t.job(0).id);
    assert_eq!(t.job(0).status, JobStatus::Planning);
    assert!(t.host.boards.is_empty());
    assert_eq!(t.host.spawned, ["Founder"]);
}

// a role the plan drafts

fn drafting() -> JobPlan {
    JobPlan {
        new_roles: Some(vec![NewRole { id: "analyst".into(), title: "A".into(), department: "A".into(), prompt: "Reads numbers and says what they mean.".into() }]),
        ..hiring("analyst", "a")
    }
}

#[test]
fn a_drafted_role_is_not_on_disk_until_the_owner_approves_and_then_the_hire_runs_on_it() {
    let mut t = T::new();
    t.waiting_job(drafting());
    let file = |t: &T, name: &str| std::fs::read_to_string(t.root().join("roles").join("analyst").join(name)).ok();
    assert_eq!(file(&t, "role.json"), None);
    t.approve(&t.job(0).id);
    let def: serde_json::Value = serde_json::from_str(&file(&t, "role.json").unwrap()).unwrap();
    assert_eq!((def["id"].as_str(), def["rank"].as_str(), def["cli"].as_str()), (Some("analyst"), Some("staff"), Some("claude")));
    assert_eq!(def["denyTools"], json!([]));
    assert!(def.get("prompt").is_none());
    assert_eq!(file(&t, "prompt.md").unwrap(), "Reads numbers and says what they mean.");
    let hire = t.person(|p| p.role == "analyst");
    assert!(t.host.spawned.contains(&hire.name));
}

#[test]
fn a_drafted_role_is_never_saved_when_the_plan_is_sent_back() {
    let mut t = T::new();
    t.waiting_job(drafting());
    let id = t.job(0).id;
    t.e.send_back(&mut t.host, &id, "no").unwrap();
    assert!(!t.root().join("roles").join("analyst").join("role.json").exists());
    assert!(!t.e.state().unwrap().roles.iter().any(|r| r.id == "analyst"));
}

// reject

#[test]
fn sending_back_leaves_a_running_job_alone() {
    let mut t = T::new();
    t.waiting_job(plan_for("a", 0));
    let id = t.job(0).id;
    t.approve(&id);
    t.clear_said();
    t.e.send_back(&mut t.host, &id, "no").unwrap();
    assert_eq!(t.job(0).status, JobStatus::Running);
    assert!(t.said().is_empty());
}

// requests

#[test]
fn starts_one_founder_when_two_jobs_arrive_together() {
    let mut t = T::new();
    t.give("one").unwrap();
    t.give("two").unwrap();
    assert_eq!(t.host.spawned, ["Founder"]);
    assert_eq!(t.jobs().iter().map(|j| j.id.as_str()).collect::<Vec<_>>(), ["J-0001", "J-0002"]);
}

#[test]
fn writes_nothing_for_a_request_that_changes_nothing() {
    let mut t = T::new();
    t.give("do it").unwrap();
    let at = t.root().join("jobs").join("J-0001.json");
    std::fs::write(&at, "untouched").unwrap();
    t.message("founder", "Nobody", "hi");
    assert_eq!(std::fs::read_to_string(&at).unwrap(), "untouched");
}

#[test]
fn tells_the_sender_of_a_request_it_could_not_read_and_keeps_reading() {
    let mut t = T::new();
    t.give("do it").unwrap();
    t.clear_said();
    t.request(json!({ "type": "plan", "ts": 1, "from": "founder", "job": "J-0001" }));
    assert!(t.said_all().contains("Your request was not understood"), "{:?}", t.said());
    assert!(t.host.logs.iter().any(|l| l.contains("not understood")));
    t.staff_floors(&plan_for("a", 0));
    t.plan("J-0001", &plan_for("a", 0));
    assert_eq!(t.job(0).status, JobStatus::AwaitingPlan);
}

#[test]
fn logs_a_line_with_no_sender_and_tells_nobody() {
    let mut t = T::new();
    t.give("do it").unwrap();
    t.clear_said();
    let at = t.root().join("requests.jsonl");
    std::fs::write(&at, "{ half\n").unwrap();
    t.advance(STEP);
    assert!(t.said().is_empty());
    assert!(t.host.logs.iter().any(|l| l.contains("{ half")));
}

#[test]
fn reads_a_request_once() {
    let mut t = T::new();
    t.give("do it").unwrap();
    t.staff_floors(&plan_for("a", 0));
    t.clear_said();
    t.message("r-a", "Founder", "only once");
    t.advance(10 * STEP);
    assert_eq!(t.said().iter().filter(|s| s.contains("only once")).count(), 1);
}

#[test]
fn types_no_control_keys_from_a_request_into_anyones_terminal() {
    let mut t = T::new();
    t.give("do it").unwrap();
    t.staff_floors(&plan_for("a", 0));
    t.clear_said();
    t.request(json!({ "type": "x\r/exit\r", "from": "r-a" }));
    let said = t.said();
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(!said[0].contains('\r') && !said[0].contains('\u{1b}'), "{said:?}");
    assert!(said[0].contains("/exit"));
}

#[test]
fn types_no_control_keys_from_a_message_either() {
    let mut t = T::new();
    t.give("do it").unwrap();
    t.staff_floors(&plan_for("a", 0));
    t.founder_up(A);
    t.clear_said();
    t.message("r-a", "Founder", "hi\r/exit\r\u{1b}[2J");
    t.advance(STEP);
    assert!(t.host.typed.iter().all(|(_, text)| !text.contains('\r') && !text.contains('\u{1b}')), "{:?}", t.host.typed);
}

#[test]
fn keeps_reading_after_a_request_that_throws() {
    let mut t = T::new();
    t.give("do it").unwrap();
    t.staff_floors(&plan_for("a", 0));
    t.founder_up(A);
    t.host.panic_on = Some("boom".into());
    let at = t.root().join("requests.jsonl");
    let lines = [
        json!({ "type": "message", "ts": 1, "from": "r-a", "to": "Founder", "message": "boom" }),
        json!({ "type": "message", "ts": 2, "from": "r-a", "to": "Founder", "message": "after it" }),
    ];
    std::fs::write(&at, lines.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
    t.advance(STEP);
    assert!(t.host.typed.iter().any(|(_, text)| text.contains("after it")), "{:?}", t.host.typed);
    assert!(t.host.logs.iter().any(|l| l.contains("went wrong")), "{:?}", t.host.logs);
}

#[test]
fn saves_only_the_jobs_a_request_changed() {
    let mut t = T::new();
    t.give("one").unwrap();
    t.give("two").unwrap();
    let first = t.root().join("jobs").join("J-0001.json");
    std::fs::write(&first, "untouched").unwrap();
    t.staff_floors(&plan_for("a", 0));
    t.plan("J-0002", &plan_for("a", 0));
    assert_eq!(t.job(1).status, JobStatus::AwaitingPlan);
    assert_eq!(std::fs::read_to_string(&first).unwrap(), "untouched");
}

#[test]
fn tells_the_founder_a_drafted_role_could_not_be_saved_and_still_runs_the_job() {
    let mut t = T::new();
    t.waiting_job(drafting());
    std::fs::create_dir_all(t.root().join("roles")).unwrap();
    std::fs::write(t.root().join("roles").join("analyst"), "a file in the way").unwrap();
    t.approve(&t.job(0).id);
    assert_eq!(t.job(0).status, JobStatus::Running);
    assert!(t.said_all().contains("Role analyst was not saved"), "{:?}", t.said());
}

#[test]
fn tells_the_founder_a_plan_that_hires_with_no_cards_is_refused() {
    let mut t = T::new();
    t.give("do it").unwrap();
    t.clear_said();
    t.plan("J-0001", &JobPlan { cards: Vec::new(), ..hiring("marketing", "a") });
    assert_eq!(t.job(0).status, JobStatus::Planning);
    assert!(t.said_all().contains("Plan for J-0001 refused: It hires people but gives them no cards. Add the cards they will work on."), "{:?}", t.said());
}
