//! The Company's pure parts, from rail.test.ts and model.test.ts.

use super::model::*;
use super::rows::*;
use super::words::*;
use crate::company::alerts::{Alert, Level};
use crate::company::needs::{FloorCards, NeedKind, NeedRow, PlanDetail};
use crate::company::types::{Company, Floor, Job, JobPlan, JobStatus, Look, NewRole, Person, Rank};
use serde_json::Map;
use std::collections::BTreeMap;
use supervise::ask::{Ask, AskKind};
use supervise::task_state::{TaskState, TaskStatus};

fn status(state: TaskState) -> TaskStatus {
    TaskStatus { state, ask: None, limit: None, blocked: None, spent: false, stuck: None, clash: None }
}

fn person(id: &str, rank: Rank, floor: &str) -> Person {
    Person {
        id: id.into(),
        name: id.to_uppercase(),
        role: "developer".into(),
        rank,
        floor: Some(floor.into()),
        boss: Some("founder".into()),
        cli: "claude".into(),
        look: Look::default(),
        session: None,
        worktree: None,
        branch: None,
        extra: Map::new(),
    }
}

fn company(people: Vec<Person>) -> Company {
    let mut c = Company::empty();
    let floor = |id: &str, name: &str| Floor { id: id.into(), dir: format!("D:\\{id}"), name: name.into(), extra: Map::new() };
    c.floors = vec![floor("a", "alpha"), floor("b", "beta")];
    c.people.extend(people);
    c
}

fn job(id: &str, status: JobStatus) -> Job {
    let mut j: Job = serde_json::from_str(&format!(r#"{{"id":"{id}","status":"running","people":[]}}"#)).unwrap();
    j.title = "Add login".into();
    j.status = status;
    j.token_cap = 100;
    j.tokens_used = 5;
    j
}

fn pane(id: &str, state: TaskState) -> Pane {
    Pane { person_id: id.into(), status: status(state), doing: None, nothing_to_merge: false }
}

fn model_of(c: &Company, jobs: &[Job], alerts: &[Alert], panes: &[Pane], open: &[&str]) -> OfficeModel {
    let open: Vec<String> = open.iter().map(|s| s.to_string()).collect();
    office_model(c, jobs, alerts, panes, &[], &BTreeMap::new(), &open)
}

fn row(key: &str, kind: NeedKind, job: Option<&str>, person: Option<&str>, title: &str) -> NeedRow {
    NeedRow { key: key.into(), kind, job_id: job.map(Into::into), person_id: person.map(Into::into), title: title.into(), detail: String::new(), standing: false, plan: None }
}

fn plan_row() -> NeedRow {
    NeedRow { plan: Some(PlanDetail { hires: vec![], floors: vec![], token_cap: 100, new_roles: vec![] }), ..row("plan-J-1", NeedKind::Plan, Some("J-1"), None, "Approve plan: Add login") }
}

/// A model with the Founder and Hai, a Lead on floor a.
fn with_people() -> OfficeModel {
    model_of(&company(vec![person("hai", Rank::Lead, "a")]), &[], &[], &[], &[])
}

fn acts(b: &[Button]) -> Vec<(Act, &str, Kind)> {
    b.iter().map(|b| (b.act, b.label.as_str(), b.kind)).collect()
}

const PRIMARY: RowLook = RowLook { goes: true, primary: true, asking: false, short: false };

// the words of a plan

#[test]
fn plan_sentence_says_an_before_a_vowel() {
    let p = PlanDetail { hires: vec!["Analyst on gcare".into()], floors: vec![], token_cap: 1000, new_roles: vec![] };
    assert_eq!(plan_sentence(&p), "The Founder wants to hire an Analyst on gcare, 0 cards, up to 1K tokens.");
}

#[test]
fn plan_sentence_says_so_when_nobody_new_is_needed() {
    let p = PlanDetail { hires: vec![], floors: vec![FloorCards { floor: "a".into(), cards: vec!["x".into()] }], token_cap: 1_200_000, new_roles: vec![] };
    assert_eq!(plan_sentence(&p), "The Founder wants to do this with the people you have, 1 card, up to 1.2M tokens.");
}

#[test]
fn plan_sentence_reads_the_plan_as_a_sentence() {
    let p = PlanDetail {
        hires: vec!["Developer on bytehive".into(), "Copywriter on gcare".into()],
        floors: vec![FloorCards { floor: "bytehive".into(), cards: vec!["a".into(), "b".into()] }, FloorCards { floor: "gcare".into(), cards: vec!["c".into()] }],
        token_cap: 5_000_000,
        new_roles: vec![],
    };
    assert_eq!(plan_sentence(&p), "The Founder wants to hire a Developer on bytehive and a Copywriter on gcare, 3 cards, up to 5M tokens.");
}

#[test]
fn tokens_short_shortens_tokens_the_way_people_say_them() {
    assert_eq!(tokens_short(5_000_000), "5M");
    assert_eq!(tokens_short(1_250_000), "1.3M");
    assert_eq!(tokens_short(1_200_000), "1.2M");
    assert_eq!(tokens_short(40_100), "40.1K");
    assert_eq!(tokens_short(900), "900");
    assert_eq!(grouped(5_000_000), "5,000,000");
    assert_eq!(grouped(1500), "1,500");
    assert_eq!(grouped(15), "15");
}

#[test]
fn plan_items_list_the_plan_in_full_new_roles_word_for_word() {
    let p = PlanDetail {
        hires: vec![],
        floors: vec![FloorCards { floor: "a".into(), cards: vec!["x".into(), "y".into()] }],
        token_cap: 300_000,
        new_roles: vec![NewRole { id: "analyst".into(), title: "Analyst".into(), department: "Data".into(), prompt: "Reads numbers.".into() }],
    };
    let items = plan_items(&p, "Do it");
    let terms: Vec<&str> = items.iter().map(|(t, _, _)| t.as_str()).collect();
    assert_eq!(terms, ["Plan", "Hires", "Cards on a", "Token cap", "New role: Analyst (analyst, Data)"]);
    assert_eq!((items[1].1.as_str(), items[2].1.as_str(), items[3].1.as_str()), ("Nobody new", "x, y", "300,000"));
    assert!(items[4].2 && items[4].1 == "Reads numbers.");
    assert_eq!(plan_items(&p, "")[0].0, "Hires");
}

// Needs you rows

#[test]
fn the_first_thing_that_needs_you_gets_the_one_primary_button() {
    let rows = vec![plan_row(), row("paused-J-2", NeedKind::Paused, Some("J-2"), None, "Paused: B"), row("merge-J-3", NeedKind::Merge, Some("J-3"), None, "Merge C")];
    assert_eq!(lead_row(&rows), Some(0));
    let m = with_people();
    let all: Vec<Button> = rows.iter().enumerate().flat_map(|(i, r)| row_buttons(r, RowLook { goes: true, primary: Some(i) == lead_row(&rows), ..Default::default() }, &m)).collect();
    let primary: Vec<Act> = all.iter().filter(|b| b.kind == Kind::Primary).map(|b| b.act).collect();
    assert_eq!(primary, [Act::Approve]);
}

#[test]
fn a_standing_alert_never_takes_the_primary() {
    let standing = NeedRow { standing: true, ..row("file", NeedKind::Alert, None, None, "J-1.json could not be read.") };
    assert_eq!(lead_row(&[standing.clone(), row("stuck", NeedKind::Alert, None, Some("hai"), "Stuck")]), Some(1));
    assert_eq!(lead_row(&[standing]), None);
    assert_eq!(lead_row(&[]), None);
}

#[test]
fn send_back_takes_the_primary_while_a_note_is_written() {
    let b = row_buttons(&plan_row(), RowLook { asking: true, ..PRIMARY }, &with_people());
    assert_eq!(acts(&b), [(Act::Approve, "Approve", Kind::Secondary), (Act::SendBack, "Send back", Kind::Primary)]);
    assert!(!b[0].held);
}

#[test]
fn approve_is_held_while_the_note_box_says_a_note_is_missing() {
    let b = row_buttons(&plan_row(), RowLook { asking: true, short: true, ..PRIMARY }, &with_people());
    assert!(b[0].held && !b[1].held);
}

#[test]
fn a_merge_row_gives_review_and_mark_done() {
    let r = row("merge-J-1", NeedKind::Merge, Some("J-1"), Some("hai"), "Review and merge: Add login");
    assert_eq!(acts(&row_buttons(&r, PRIMARY, &with_people())), [(Act::Open, "Review", Kind::Primary), (Act::Finish, "Mark done", Kind::Secondary)]);
}

#[test]
fn a_merge_row_with_everyone_merged_has_only_mark_done() {
    let r = row("merge-J-1", NeedKind::Merge, Some("J-1"), None, "Review and merge: Add login");
    assert_eq!(acts(&row_buttons(&r, PRIMARY, &with_people())), [(Act::Finish, "Mark done", Kind::Primary)]);
}

#[test]
fn dismiss_is_on_an_alert_that_stands_and_only_there() {
    let m = with_people();
    let standing = NeedRow { standing: true, ..row("file", NeedKind::Alert, None, None, "J-1.json could not be read.") };
    assert_eq!(acts(&row_buttons(&standing, PRIMARY, &m)), [(Act::Dismiss, "Dismiss", Kind::Secondary)]);
    let stuck = row("stuck", NeedKind::Alert, None, Some("hai"), "It is stuck.");
    assert_eq!(acts(&row_buttons(&stuck, PRIMARY, &m)), [(Act::Open, "Go to HAI", Kind::Primary)]);
}

#[test]
fn a_paused_job_adds_to_its_cap() {
    let r = row("paused-J-1", NeedKind::Paused, Some("J-1"), None, "Paused: Add login");
    assert_eq!(acts(&row_buttons(&r, PRIMARY, &with_people())), [(Act::Raise, "Add 1M tokens", Kind::Primary)]);
    assert_eq!(RAISE_BY, 1_000_000);
}

#[test]
fn a_permission_question_goes_to_the_desk_where_it_is_answered_and_says_so() {
    let m = with_people();
    let r = row("ask-hai", NeedKind::Alert, None, Some("hai"), "HAI wants to run npm publish");
    assert_eq!(acts(&row_buttons(&r, PRIMARY, &m)), [(Act::Open, "Answer at HAI's desk", Kind::Primary)]);
    assert_eq!(target_of(&r, &m), Target::Desk("hai".into()));
}

#[test]
fn a_card_with_nobody_says_open_and_goes_to_the_company() {
    let r = row("k", NeedKind::Alert, None, Some("ghost"), "Gone");
    let m = with_people();
    assert_eq!(acts(&row_buttons(&r, PRIMARY, &m)), [(Act::Open, "Open", Kind::Primary)]);
    assert_eq!(target_of(&r, &m), Target::Company);
}

#[test]
fn a_card_that_goes_nowhere_has_no_open_button() {
    let r = row("k", NeedKind::Alert, None, Some("hai"), "Stuck");
    assert!(row_buttons(&r, RowLook { goes: false, ..PRIMARY }, &with_people()).is_empty());
}

#[test]
fn a_plan_card_goes_to_the_founders_desk_as_its_face_says() {
    let m = with_people();
    assert_eq!(target_of(&plan_row(), &m), Target::Desk("founder".into()));
    assert_eq!(who(&plan_row(), &m).map(|p| p.name.as_str()), Some("Founder"));
    let merge = row("merge-J-1", NeedKind::Merge, Some("J-1"), Some("hai"), "Merge");
    assert_eq!(target_of(&merge, &m), Target::Changes("hai".into()));
}

#[test]
fn a_card_says_who_and_what() {
    let m = with_people();
    let plan = NeedRow { plan: Some(PlanDetail { hires: vec![], floors: vec![], token_cap: 300_000, new_roles: vec![] }), ..plan_row() };
    let w = row_words(&plan, &m);
    assert_eq!((w.name.as_str(), w.sub.as_deref()), ("Founder", Some("Approve plan: Add login")));
    assert_eq!(w.what, "The Founder wants to do this with the people you have, 0 cards, up to 300K tokens.");
    let stuck = NeedRow { detail: "No output for 10 minutes.".into(), ..row("k", NeedKind::Alert, None, Some("hai"), "HAI is stuck") };
    let w = row_words(&stuck, &m);
    assert_eq!((w.name.as_str(), w.sub.as_deref(), w.what.as_str(), w.more.as_str()), ("HAI", Some("Developer, alpha"), "HAI is stuck", "No output for 10 minutes."));
    let merge = NeedRow { detail: "Shipped".into(), ..row("merge-J-1", NeedKind::Merge, Some("J-1"), None, "Review and merge: Add login") };
    let w = row_words(&merge, &m);
    assert_eq!((w.name.as_str(), w.sub, w.what.as_str(), w.more.as_str()), ("Review and merge: Add login", None, "Shipped", ""));
}

// Jobs

#[test]
fn a_job_is_cancelled_before_anyone_started_and_stopped_after() {
    assert_eq!(stop_word(&JobStatus::Planning), "Cancel job");
    assert_eq!(stop_word(&JobStatus::AwaitingPlan), "Cancel job");
    assert_eq!(stop_word(&JobStatus::Running), "Stop job");
    assert_eq!(stop_word(&JobStatus::Paused), "Stop job");
}

#[test]
fn a_job_reads_as_one_line_with_tokens_for_whoever_reads_them() {
    assert_eq!(job_state(&JobStatus::Running, "Running", 1_200_000, 5_000_000), "Running, 1.2M of 5M");
    assert_eq!(job_state(&JobStatus::AwaitingPlan, "Waiting for your approval", 0, 5_000_000), "Waiting, 0 of 5M");
    assert_eq!(job_tokens(1_200_000, 5_000_000), "1,200,000 of 5,000,000 tokens");
    assert_eq!(done_percent(Some((1, 4))), 25);
    assert_eq!(done_percent(Some((1, 3))), 33);
    assert_eq!(done_percent(Some((0, 0))), 0);
    assert_eq!(done_percent(None), 0);
}

#[test]
fn says_what_the_team_is_doing_when_nothing_needs_you() {
    assert_eq!(calm_line(4, 3), "3 of 4 people are working.");
    assert_eq!(calm_line(2, 1), "1 of 2 people is working.");
    assert_eq!(calm_line(1, 1), "1 of 1 person is working.");
    assert_eq!(calm_line(3, 0), "Everyone is resting.");
    assert_eq!(calm_line(1, 0), "Give the Founder a job to get started.");
}

#[test]
fn the_box_gives_an_example_from_a_project_the_company_has() {
    assert_eq!(ask_hint(Some("bakery-site")), "Add marketing to bakery-site");
    assert_eq!(ask_hint(None), "Build a landing page for my app");
}

// the office model

#[test]
fn office_state_draws_each_state() {
    for (from, to) in [(TaskState::Needs, OfficeState::Needs), (TaskState::Working, OfficeState::Working), (TaskState::Review, OfficeState::Review), (TaskState::Idle, OfficeState::Idle), (TaskState::Stopped, OfficeState::Stopped), (TaskState::Merged, OfficeState::Stopped)] {
        assert_eq!(office_state(Some(&status(from))), to);
    }
    assert_eq!(office_state(None), OfficeState::Stopped);
}

#[test]
fn the_model_puts_the_founder_on_top_whatever_project_its_seat_sits_in() {
    let m = model_of(&company(vec![]), &[], &[], &[pane("founder", TaskState::Working)], &["D:\\a"]);
    assert_eq!(m.founder.as_ref().map(|f| f.state), Some(OfficeState::Working));
    assert!(m.floors.iter().all(|f| f.people.is_empty()));
    let a = &m.floors[0];
    assert!(a.open && !a.lit);
}

#[test]
fn the_model_shows_floors_with_people_and_open_projects_with_nobody_and_no_others() {
    let m = model_of(&company(vec![person("x", Rank::Staff, "b")]), &[], &[], &[], &["d:/a/"]);
    assert_eq!(m.floors.iter().map(|f| f.id.as_str()).collect::<Vec<_>>(), ["a", "b"]);
    assert!(model_of(&company(vec![]), &[], &[], &[], &[]).floors.is_empty());
}

#[test]
fn the_model_seats_the_lead_first_then_people_in_the_order_they_joined() {
    let m = model_of(&company(vec![person("s1", Rank::Staff, "a"), person("l", Rank::Lead, "a"), person("s2", Rank::Staff, "a")]), &[], &[], &[], &[]);
    assert_eq!(m.floors[0].people.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["l", "s1", "s2"]);
}

#[test]
fn the_model_darkens_a_closed_project_and_keeps_its_people() {
    let m = model_of(&company(vec![person("x", Rank::Staff, "a")]), &[], &[], &[], &[]);
    assert!(!m.floors[0].open && !m.floors[0].lit);
    assert_eq!((m.floors[0].people[0].state, m.floors[0].people[0].seated), (OfficeState::Stopped, false));
    let lit = model_of(&company(vec![person("x", Rank::Staff, "a")]), &[], &[], &[pane("x", TaskState::Idle)], &["D:\\a"]);
    assert!(lit.floors[0].lit);
}

#[test]
fn the_model_puts_what_a_person_asks_in_needs_you_before_merges_after_plans() {
    let mut asking = status(TaskState::Needs);
    asking.ask = Some(Ask { kind: AskKind::Run, prompt: String::new(), title: None, detail: Some("rm x".into()), options: vec![], free_text_option: None });
    let mut planned = job("J-1", JobStatus::AwaitingPlan);
    planned.plan = Some(JobPlan { summary: "s".into(), cards: vec![], hires: vec![], token_cap: None, new_roles: None, reuse: None, extra: Map::new() });
    let mut merging = job("J-2", JobStatus::AwaitingMerge);
    merging.people = vec!["x".into()];
    let panes = [Pane { person_id: "x".into(), status: asking, doing: None, nothing_to_merge: false }];
    let m = model_of(&company(vec![person("x", Rank::Staff, "a")]), &[planned, merging], &[], &panes, &[]);
    assert_eq!(m.needs.iter().map(|r| r.key.as_str()).collect::<Vec<_>>(), ["plan-J-1", "ask-x", "merge-J-2"]);
    assert_eq!((m.needs[1].kind, m.needs[1].title.as_str(), m.needs[1].standing), (NeedKind::Alert, "X wants to run rm x", false));
}

#[test]
fn the_model_counts_a_floors_needs_once_per_person() {
    let alert = Alert { key: "k".into(), job: None, person_id: Some("x".into()), text: "stuck".into(), level: Level::Warn, standing: false };
    let m = model_of(&company(vec![person("x", Rank::Staff, "a")]), &[], &[alert], &[pane("x", TaskState::Needs)], &[]);
    assert_eq!(m.floors[0].needs, 1);
}

#[test]
fn the_model_lists_the_jobs_still_going_in_words_with_their_cards() {
    let cards = BTreeMap::from([("J-1".to_string(), (2, 6))]);
    let m = office_model(&company(vec![]), &[job("J-1", JobStatus::Paused), job("J-2", JobStatus::Done)], &[], &[], &[], &cards, &[]);
    assert_eq!(m.jobs, [OfficeJob { id: "J-1".into(), title: "Add login".into(), status: JobStatus::Paused, label: "Paused", tokens_used: 5, token_cap: 100, cards: Some((2, 6)) }]);
}

#[test]
fn the_model_makes_a_readable_title_from_the_role_id_with_no_role_file() {
    let mut x = person("x", Rank::Staff, "a");
    x.role = "data-analyst".into();
    assert_eq!(model_of(&company(vec![x]), &[], &[], &[], &[]).floors[0].people[0].title, "Data Analyst");
}

#[test]
fn the_model_has_no_founder_with_an_empty_list_of_people() {
    let mut c = company(vec![]);
    c.people.clear();
    assert!(model_of(&c, &[], &[], &[], &[]).founder.is_none());
}

#[test]
fn says_what_a_person_is_doing_in_a_line() {
    assert_eq!(status_line("Ana", &status(TaskState::Working)), "Ana is working");
    assert_eq!(status_line("Ana", &status(TaskState::Review)), "Ana is done, changes ready to review");
    assert_eq!(status_line("Ana", &status(TaskState::Merged)), "Ana's work is merged");
    assert_eq!(status_line("Ana", &status(TaskState::Idle)), "Ana is waiting for a task");
    let mut q = status(TaskState::Needs);
    q.ask = Some(Ask { kind: AskKind::Question, prompt: "Which one?".into(), title: None, detail: None, options: vec![], free_text_option: None });
    assert_eq!(status_line("Ana", &q), "Ana asks: Which one?");
}

#[test]
fn the_app_lands_on_the_company_only_for_a_job_still_going() {
    use super::source::still_going;
    for s in [JobStatus::Planning, JobStatus::AwaitingPlan, JobStatus::Running, JobStatus::Paused, JobStatus::AwaitingMerge] {
        assert!(still_going(&s), "{s:?}");
    }
    for s in [JobStatus::Done, JobStatus::Stopped, JobStatus::Rejected] {
        assert!(!still_going(&s), "{s:?}");
    }
}

#[test]
fn the_narrow_rail_says_what_waits_below_it() {
    assert_eq!(jobs_below(0, false), None);
    assert_eq!(jobs_below(0, true).as_deref(), Some("Jobs below"));
    assert_eq!(jobs_below(2, false).as_deref(), Some("2 below"));
    assert_eq!(jobs_below(1, true).as_deref(), Some("1 and Jobs below"));
}

#[test]
fn a_job_merge_row_skips_people_with_nothing_to_merge() {
    let mut merging = job("J-1", JobStatus::AwaitingMerge);
    merging.people = vec!["lead".into(), "dev".into(), "qa".into()];
    let empty = |id: &str| Pane { nothing_to_merge: true, ..pane(id, TaskState::Idle) };
    let people = vec![person("lead", Rank::Lead, "a"), person("dev", Rank::Staff, "a"), person("qa", Rank::Staff, "a")];
    let m = model_of(&company(people.clone()), &[merging.clone()], &[], &[empty("lead"), pane("dev", TaskState::Review), empty("qa")], &[]);
    assert_eq!(m.needs[0].person_id.as_deref(), Some("dev"));
    let m = model_of(&company(people), &[merging], &[], &[empty("lead"), pane("dev", TaskState::Merged), empty("qa")], &[]);
    assert_eq!(m.needs[0].person_id, None, "with everyone merged or empty, only Mark done is left");
}

#[test]
fn a_running_job_with_everyone_stopped_asks_to_resume_them() {
    let mut running = job("J-1", JobStatus::Running);
    running.people = vec!["x".into(), "y".into()];
    let c = company(vec![person("x", Rank::Staff, "a"), person("y", Rank::Staff, "a")]);
    let m = model_of(&c, &[running.clone()], &[], &[pane("x", TaskState::Stopped)], &[]);
    let r = m.needs.iter().find(|r| r.key == "resume-J-1").expect("a resume row");
    assert_eq!((r.kind, r.title.as_str()), (NeedKind::Alert, "Nobody is at work on: Add login"));
    assert_eq!(acts(&row_buttons(r, PRIMARY, &m)), [(Act::Resume, "Resume", Kind::Primary)]);
    let m = model_of(&c, &[running.clone()], &[], &[pane("x", TaskState::Working)], &[]);
    assert!(m.needs.iter().all(|r| r.key != "resume-J-1"), "someone is at work");
    running.status = JobStatus::Paused;
    let m = model_of(&c, &[running], &[], &[], &[]);
    assert!(m.needs.iter().all(|r| !r.key.starts_with("resume-")), "a paused job has its own row");
}
