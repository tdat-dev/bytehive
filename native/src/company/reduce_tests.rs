//! reduce.test.ts: newJob and applyRequest, and the helpers the other reduce tests share.

use super::*;
use crate::company::types::{Floor, Hire, Look, NewRole, PlanCard};
use serde_json::Map;

pub(super) fn look() -> Look {
    Look { shirt: "#fff".into(), hair: "#000".into(), prop: "none".into(), ..Default::default() }
}

pub(super) fn role() -> RoleDef {
    RoleDef {
        id: "marketing".into(),
        title: "M".into(),
        department: "M".into(),
        rank: Rank::Staff,
        cli: "claude".into(),
        deny_tools: vec![],
        look: look(),
        prompt: "p".into(),
        skills: vec![],
    }
}

pub(super) fn person(id: &str, name: &str, role: &str, rank: Rank, floor: &str, boss: &str) -> Person {
    Person {
        id: id.into(),
        name: name.into(),
        role: role.into(),
        rank,
        floor: Some(floor.into()),
        boss: Some(boss.into()),
        cli: "claude".into(),
        look: look(),
        session: None,
        worktree: None,
        branch: None,
        extra: Map::new(),
    }
}

pub(super) fn lead() -> Person {
    person("l1", "Hai", "lead", Rank::Lead, "a", FOUNDER_ID)
}

pub(super) fn dev() -> Person {
    person("d1", "Linh", "developer", Rank::Staff, "a", "l1")
}

fn floor(id: &str) -> Floor {
    Floor { id: id.into(), dir: format!("D:\\{id}"), name: id.into(), extra: Map::new() }
}

pub(super) fn state() -> State {
    let mut company = Company::empty();
    company.floors = vec![floor("a"), floor("b")];
    company.people.extend([lead(), dev()]);
    State { company, jobs: vec![], roles: vec![role()] }
}

pub(super) fn hire(role: &str, floor: &str) -> Hire {
    Hire { role: role.into(), floor: floor.into() }
}

pub(super) fn plan() -> JobPlan {
    JobPlan {
        summary: "Add marketing".into(),
        // A hire comes with work: a plan that hires with no cards is refused.
        cards: vec![card("b", None)],
        hires: vec![hire("marketing", "b")],
        token_cap: None,
        new_roles: None,
        reuse: None,
        extra: Map::new(),
    }
}

pub(super) fn analyst() -> NewRole {
    NewRole { id: "analyst".into(), title: "Analyst".into(), department: "Data".into(), prompt: "Reads numbers and says what they mean.".into() }
}

pub(super) fn drafting() -> JobPlan {
    JobPlan { summary: "Add an analyst".into(), hires: vec![hire("analyst", "b")], new_roles: Some(vec![analyst()]), ..plan() }
}

pub(super) fn card(floor: &str, assignee: Option<&str>) -> PlanCard {
    PlanCard { floor: floor.into(), title: "t".into(), desc: "d".into(), checklist: vec![], assignee: assignee.map(Into::into), extra: Map::new() }
}

/// The pick of the TS tests: every hire is Vy, p9.
pub(super) fn approve(s: &State, id: &str) -> Approved {
    approve_plan(s, id, |_| "Vy".into(), || "p9".into())
}

pub(super) fn message(from: &str, to: &str, text: &str) -> CompanyRequest {
    CompanyRequest::Message { ts: 1, from: from.into(), to: Some(to.into()), message: text.into() }
}

pub(super) fn plan_request(from: &str, job: &str, plan: JobPlan) -> CompanyRequest {
    CompanyRequest::Plan { ts: 2, from: from.into(), job: job.into(), plan }
}

pub(super) fn done_request(from: &str, job: &str, summary: &str) -> CompanyRequest {
    CompanyRequest::JobDone { ts: 2, from: from.into(), job: job.into(), summary: summary.into() }
}

pub(super) fn deliver(person_id: &str, text: &str) -> Effect {
    Effect::Deliver { person_id: person_id.into(), text: text.into() }
}

/// Who a delivery goes to, and what it says.
pub(super) fn told(e: &Effect) -> (&str, &str) {
    match e {
        Effect::Deliver { person_id, text } => (person_id, text),
        Effect::Owner { key, text } => (key, text),
    }
}

pub(super) fn fresh() -> State {
    new_job(&state(), "x", 1).0.state
}

pub(super) fn waiting() -> State {
    apply_request(&fresh(), &plan_request(FOUNDER_ID, "J-0001", plan())).state
}

pub(super) fn running() -> State {
    approve(&waiting(), "J-0001").state
}

pub(super) fn closed() -> State {
    reject_plan(&waiting(), "J-0001", "").state
}

// newJob

#[test]
fn new_job_titles_a_job_with_its_whole_first_line_the_panel_wraps_it_nothing_cuts_a_word() {
    let line = "Write the launch post for the new pricing page and check every number in it against the billing code";
    assert_eq!(new_job(&state(), &format!("  {line}\nmore detail"), 100).1.title, line);
}

#[test]
fn new_job_numbers_jobs_and_wakes_the_founder() {
    let (Reduced { state: s, effects }, job) = new_job(&state(), "add marketing to b", 100);
    assert_eq!(job.id, "J-0001");
    assert_eq!(s.company.next_job, 2);
    assert_eq!(job.status, JobStatus::Planning);
    assert_eq!(job.token_cap, 5_000_000);
    assert_eq!(s.jobs, vec![job]);
    assert_eq!(effects, vec![deliver(FOUNDER_ID, "JOB J-0001: add marketing to b")]);
}

// applyRequest

#[test]
fn apply_request_delivers_a_message_between_boss_and_report_with_who_sent_it() {
    assert_eq!(apply_request(&state(), &message("d1", "Hai", "done")).effects, vec![deliver("l1", "[From Linh] done")]);
}

#[test]
fn apply_request_takes_control_characters_out_of_a_message_it_is_typed_into_a_terminal() {
    let text = "done\u{1b}[201~\r\nrm\u{7} -rf\u{0}\u{7f}\u{85}\n\tnext";
    assert_eq!(apply_request(&state(), &message("d1", "Hai", text)).effects, vec![deliver("l1", "[From Linh] done[201~\nrm -rf\n\tnext")]);
}

#[test]
fn apply_request_does_not_pass_on_messages_from_people_on_a_paused_job_and_tells_the_sender() {
    let mut s = fresh();
    s.jobs[0].status = JobStatus::Paused;
    s.jobs[0].people = vec!["d1".into()];
    let effects = apply_request(&s, &message("d1", "Hai", "done")).effects;
    assert_eq!(effects.len(), 1);
    let (to, text) = told(&effects[0]);
    assert_eq!(to, "d1");
    assert!(text.starts_with("Held: your job J-0001 is paused") && text.contains("Hai"), "{text}");
}

#[test]
fn apply_request_still_delivers_a_message_from_someone_who_is_not_on_the_paused_job() {
    let mut s = fresh();
    s.jobs[0].status = JobStatus::Paused;
    s.jobs[0].people = vec!["l1".into()];
    assert_eq!(apply_request(&s, &message("d1", "Hai", "done")).effects.len(), 1);
}

#[test]
fn apply_request_drops_a_forged_message_that_breaks_the_hierarchy() {
    assert!(apply_request(&state(), &message("d1", "Founder", "hi")).effects.is_empty());
}

#[test]
fn apply_request_puts_a_valid_plan_up_for_approval() {
    let r = apply_request(&fresh(), &plan_request(FOUNDER_ID, "J-0001", plan()));
    assert_eq!(r.state.jobs[0].status, JobStatus::AwaitingPlan);
    assert_eq!(r.state.jobs[0].plan, Some(plan()));
}

#[test]
fn apply_request_refuses_a_plan_from_anyone_but_the_founder() {
    assert_eq!(apply_request(&fresh(), &plan_request("l1", "J-0001", plan())).state.jobs[0].status, JobStatus::Planning);
}

#[test]
fn apply_request_tells_the_founder_when_a_plan_fails_on_arrival() {
    let bad = JobPlan { hires: vec![hire("ghost", "b")], ..plan() };
    let r = apply_request(&fresh(), &plan_request(FOUNDER_ID, "J-0001", bad));
    assert_eq!(r.state.jobs[0].status, JobStatus::Planning);
    let (to, text) = told(&r.effects[0]);
    assert_eq!(to, FOUNDER_ID);
    assert!(text.contains("ghost"), "{text}");
}

#[test]
fn apply_request_refuses_job_done_from_anyone_but_the_founder() {
    let s = running();
    assert_eq!(apply_request(&s, &done_request("l1", "J-0001", "ok")), Reduced { state: s, effects: vec![] });
}

#[test]
fn apply_request_marks_a_finished_job_for_merge() {
    let job = &apply_request(&running(), &done_request(FOUNDER_ID, "J-0001", "ok")).state.jobs[0];
    assert_eq!((&job.status, job.summary.as_deref()), (&JobStatus::AwaitingMerge, Some("ok")));
}

/// A request is a closed enum here: a type this build does not know never parses,
/// so the engine skips its line and nothing changes.
#[test]
fn apply_request_ignores_a_request_type_it_does_not_know_even_from_the_founder() {
    let line = r#"{"type":"mystery","ts":2,"from":"founder","job":"J-0001","summary":"ok"}"#;
    assert!(serde_json::from_str::<CompanyRequest>(line).is_err());
}

#[test]
fn apply_request_refuses_a_second_plan_on_a_running_job_so_nobody_is_hired_twice() {
    let s = running();
    let r = apply_request(&s, &plan_request(FOUNDER_ID, "J-0001", plan()));
    assert_eq!(r.state, s);
    let (to, text) = told(&r.effects[0]);
    assert_eq!(to, FOUNDER_ID);
    assert!(text.contains("running"), "{text}");
}

#[test]
fn apply_request_takes_a_plan_that_hires_a_role_a_floor_already_has_and_lets_the_person_there_do_it() {
    let p = JobPlan { hires: vec![hire("developer", "a"), hire("marketing", "b")], ..plan() };
    let r = apply_request(&fresh(), &plan_request(FOUNDER_ID, "J-0001", p));
    assert_eq!(r.state.jobs[0].status, JobStatus::AwaitingPlan);
    assert_eq!(r.state.jobs[0].plan.as_ref().unwrap().hires, vec![hire("marketing", "b")]);
    assert_eq!(r.effects.len(), 1);
    let (to, text) = told(&r.effects[0]);
    assert_eq!(to, FOUNDER_ID);
    assert_eq!(text, "Plan for J-0001 taken without 1 of its hires: Linh (developer on a) is already there and does that work.");
}

#[test]
fn apply_request_tells_the_founder_only_who_takes_the_dropped_hires_and_gives_them_the_job_on_approval() {
    let mut s = fresh();
    s.company.people.push(Person { id: "d2".into(), name: "Minh".into(), ..dev() });
    let p = JobPlan { hires: vec![hire("developer", "a"), hire("marketing", "b")], ..plan() };
    let r = apply_request(&s, &plan_request(FOUNDER_ID, "J-0001", p));
    let (_, text) = told(&r.effects[0]);
    assert!(text.contains("Linh") && !text.contains("Minh"), "{text}");
    let a = approve(&r.state, "J-0001");
    assert_eq!(a.reused.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["d1"]);
    assert!(a.state.jobs[0].people.contains(&"d1".to_string()));
    assert_eq!(a.hired.iter().map(|p| p.role.as_str()).collect::<Vec<_>>(), ["marketing"]);
}

#[test]
fn apply_request_drops_a_reuse_list_the_founder_sends_who_takes_a_hire_is_the_apps_to_say() {
    let r = apply_request(&fresh(), &plan_request(FOUNDER_ID, "J-0001", JobPlan { reuse: Some(vec!["d1".into()]), ..plan() }));
    assert_eq!(r.state.jobs[0].plan.as_ref().unwrap().reuse, None);
    assert!(approve(&r.state, "J-0001").reused.is_empty());
}

#[test]
fn apply_request_refuses_a_plan_while_another_waits_for_approval() {
    let s = waiting();
    let r = apply_request(&s, &plan_request(FOUNDER_ID, "J-0001", plan()));
    assert_eq!(r.state, s);
    assert_eq!(told(&r.effects[0]), (FOUNDER_ID, "Plan for J-0001 refused: the job is awaiting-plan, and a plan is only taken while it is planning."));
}

#[test]
fn apply_request_does_not_bring_a_rejected_job_back_with_a_plan() {
    let s = closed();
    let r = apply_request(&s, &plan_request(FOUNDER_ID, "J-0001", plan()));
    assert_eq!(r.state, s);
    let (to, text) = told(&r.effects[0]);
    assert_eq!(to, FOUNDER_ID);
    assert!(text.contains("rejected"), "{text}");
}

#[test]
fn apply_request_closes_a_planning_job_that_needs_no_work_straight_to_done_no_merge_and_the_owner_hears_why() {
    let r = apply_request(&fresh(), &done_request(FOUNDER_ID, "J-0001", "It was done last week."));
    let job = &r.state.jobs[0];
    assert_eq!(job.status, JobStatus::Done);
    assert_eq!(job.summary.as_deref(), Some("It was done last week."));
    assert_eq!((&job.plan, job.people.len()), (&None, 0));
    assert_eq!(r.effects, vec![Effect::Owner { key: "no-work-J-0001".into(), text: "J-0001 needed no work: It was done last week.".into() }]);
}

#[test]
fn apply_request_still_refuses_job_done_on_a_plan_the_owner_is_reading_so_a_job_cant_skip_approval() {
    let s = waiting();
    let r = apply_request(&s, &done_request(FOUNDER_ID, "J-0001", "ok"));
    assert_eq!(r.state, s);
    assert_eq!(
        told(&r.effects[0]),
        (FOUNDER_ID, "J-0001 cannot be finished: it is awaiting-plan, and only a running job, or a planning one that needs no work, can.")
    );
}

#[test]
fn apply_request_does_not_bring_a_rejected_job_back_with_job_done() {
    let s = closed();
    let r = apply_request(&s, &done_request(FOUNDER_ID, "J-0001", "ok"));
    assert_eq!(r.state, s);
    let (to, text) = told(&r.effects[0]);
    assert_eq!(to, FOUNDER_ID);
    assert!(text.contains("rejected"), "{text}");
}

/// As with an unknown type: a role request does not parse, so it changes nothing.
#[test]
fn apply_request_ignores_a_role_request_a_role_comes_with_a_plan_now_for_the_owner_to_read() {
    let line = r#"{"type":"role","ts":1,"from":"founder","job":"J-0001","role":{"id":"analyst"}}"#;
    assert!(serde_json::from_str::<CompanyRequest>(line).is_err());
}

#[test]
fn apply_request_saves_nothing_while_a_plan_with_a_new_role_waits() {
    let s = apply_request(&fresh(), &plan_request(FOUNDER_ID, "J-0001", drafting())).state;
    assert_eq!(s.jobs[0].status, JobStatus::AwaitingPlan);
    assert_eq!(s.jobs[0].plan.as_ref().unwrap().new_roles, drafting().new_roles);
    assert_eq!(s.roles.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["marketing"]);
}

#[test]
fn apply_request_tells_the_founder_when_a_new_role_is_refused() {
    let bad = JobPlan { new_roles: Some(vec![NewRole { id: "marketing".into(), ..analyst() }]), ..drafting() };
    let r = apply_request(&fresh(), &plan_request(FOUNDER_ID, "J-0001", bad));
    assert_eq!(r.state.jobs[0].status, JobStatus::Planning);
    let (to, text) = told(&r.effects[0]);
    assert_eq!(to, FOUNDER_ID);
    assert!(text.contains("exists"), "{text}");
}

#[path = "reduce_approve_tests.rs"]
mod approve;
#[path = "reduce_stop_tests.rs"]
mod stop;
