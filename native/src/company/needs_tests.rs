use super::*;
use crate::company::alerts::{Alert, Level};
use crate::company::types::{Floor, Hire, Job, JobPlan, JobStatus, Look, NewRole, PlanCard, Rank, RoleDef};
use serde_json::Map;

fn job(status: JobStatus) -> Job {
    Job {
        id: "J-1".into(),
        title: "Add login".into(),
        ask: "a".into(),
        status,
        plan: None,
        token_cap: 1,
        tokens_used: 0,
        token_base: Default::default(),
        people: Vec::new(),
        summary: None,
        created_at: 1,
        extra: Map::new(),
    }
}

fn plan(summary: &str, cards: Vec<PlanCard>, hires: Vec<Hire>) -> JobPlan {
    JobPlan { summary: summary.into(), cards, hires, token_cap: None, new_roles: None, reuse: None, extra: Map::new() }
}

fn card(floor: &str, title: &str) -> PlanCard {
    PlanCard { floor: floor.into(), title: title.into(), desc: String::new(), checklist: Vec::new(), assignee: None, extra: Map::new() }
}

fn hire(role: &str, floor: &str) -> Hire {
    Hire { role: role.into(), floor: floor.into() }
}

fn alert(key: &str, job: Option<&str>, person: Option<&str>, text: &str, standing: bool) -> Alert {
    Alert { key: key.into(), job: job.map(Into::into), person_id: person.map(Into::into), text: text.into(), level: Level::Warn, standing }
}

fn none_merged(_: &str) -> bool {
    false
}

fn ids(rows: &[NeedRow]) -> Vec<NeedKind> {
    rows.iter().map(|r| r.kind).collect()
}

const NO_NAMES: Names = Names { floors: &[], roles: &[] };

#[test]
fn says_which_alerts_stand_until_you_dismiss_them() {
    let alerts = [alert("a", None, None, "file", true), alert("b", Some("J-1"), None, "stuck", false)];
    let rows = needs_you(&[], &alerts, &NO_NAMES, none_merged);
    assert_eq!(rows.iter().map(|r| r.standing).collect::<Vec<_>>(), [true, false]);
}

#[test]
fn is_empty_while_work_just_runs() {
    assert!(needs_you(&[job(JobStatus::Running)], &[], &NO_NAMES, none_merged).is_empty());
}

#[test]
fn shows_a_plan_with_what_it_will_cost_you_in_people() {
    let j = Job { plan: Some(plan("Build it", vec![], vec![hire("qa", "a")])), ..job(JobStatus::AwaitingPlan) };
    let rows = needs_you(&[j], &[], &NO_NAMES, none_merged);
    assert_eq!((rows[0].kind, rows[0].job_id.as_deref(), rows[0].title.as_str()), (NeedKind::Plan, Some("J-1"), "Approve plan: Add login"));
    assert!(rows[0].detail.contains("Build it"));
    assert!(rows[0].detail.contains("1 hire"));
    assert_eq!(rows[0].detail, "Build it 0 cards, 1 hire.");
}

#[test]
fn shows_what_you_are_approving_hires_cards_per_floor_the_cap_and_every_new_role_in_full() {
    let new_roles = vec![NewRole { id: "analyst".into(), title: "Analyst".into(), department: "Data".into(), prompt: "Line one.\nLine two.".into() }];
    let p = JobPlan {
        token_cap: Some(900),
        new_roles: Some(new_roles.clone()),
        ..plan("Build it", vec![card("a", "Login page"), card("b", "Copy"), card("a", "Tests")], vec![hire("qa", "a"), hire("analyst", "z")])
    };
    let j = Job { plan: Some(p), token_cap: 5000, ..job(JobStatus::AwaitingPlan) };
    let floor = |id: &str, name: &str| Floor { id: id.into(), dir: format!("D:\\{id}"), name: name.into(), extra: Map::new() };
    let floors = [floor("a", "gcare"), floor("b", "site")];
    let qa = RoleDef {
        id: "qa".into(),
        title: "QA".into(),
        department: String::new(),
        rank: Rank::Staff,
        cli: "claude".into(),
        deny_tools: Vec::new(),
        look: Look::default(),
        prompt: String::new(),
        skills: Vec::new(),
    };
    let roles = [qa];
    let rows = needs_you(&[j], &[], &Names { floors: &floors, roles: &roles }, none_merged);
    assert_eq!(
        rows[0].plan,
        Some(PlanDetail {
            hires: vec!["QA on gcare".into(), "Analyst on z".into()],
            floors: vec![
                FloorCards { floor: "gcare".into(), cards: vec!["Login page".into(), "Tests".into()] },
                FloorCards { floor: "site".into(), cards: vec!["Copy".into()] },
            ],
            token_cap: 900,
            new_roles,
        })
    );
}

#[test]
fn shows_the_cap_that_will_apply_not_what_the_plan_asked_for() {
    let p = JobPlan { token_cap: Some(9000), ..plan("s", vec![], vec![]) };
    let j = Job { plan: Some(p), token_cap: 5000, ..job(JobStatus::AwaitingPlan) };
    assert_eq!(needs_you(&[j], &[], &NO_NAMES, none_merged)[0].plan.as_ref().unwrap().token_cap, 5000);
}

#[test]
fn shows_merge_and_paused_jobs() {
    let jobs = [Job { summary: Some("Done".into()), ..job(JobStatus::AwaitingMerge) }, Job { id: "J-2".into(), ..job(JobStatus::Paused) }];
    let rows = needs_you(&jobs, &[], &NO_NAMES, none_merged);
    assert_eq!(ids(&rows), [NeedKind::Paused, NeedKind::Merge]);
    assert_eq!((rows[0].title.as_str(), rows[0].detail.as_str()), ("Paused: Add login", "It reached its token cap."));
    assert_eq!((rows[1].key.as_str(), rows[1].title.as_str()), ("merge-J-1", "Review and merge: Add login"));
}

#[test]
fn lets_a_merge_row_open_the_first_person_of_the_job() {
    let jobs = [Job { people: vec!["p1".into(), "p2".into()], ..job(JobStatus::AwaitingMerge) }, Job { id: "J-2".into(), ..job(JobStatus::AwaitingMerge) }];
    let rows = needs_you(&jobs, &[], &NO_NAMES, none_merged);
    assert_eq!(rows.iter().map(|r| r.person_id.as_deref()).collect::<Vec<_>>(), [Some("p1"), None]);
}

#[test]
fn moves_a_merge_row_on_to_the_next_person_whose_work_is_not_merged_yet_and_counts() {
    let jobs = [Job { summary: Some("Done.".into()), people: vec!["p1".into(), "p2".into(), "p3".into()], ..job(JobStatus::AwaitingMerge) }];
    let at = |done: &[&str]| {
        let row = needs_you(&jobs, &[], &NO_NAMES, |id| done.contains(&id)).remove(0);
        (row.person_id, row.detail)
    };
    assert_eq!(at(&[]), (Some("p1".into()), "Done. 3 people to merge.".into()));
    assert_eq!(at(&["p1"]), (Some("p2".into()), "Done. 1 of 3 merged.".into()));
    assert_eq!(at(&["p1", "p3"]), (Some("p2".into()), "Done. 2 of 3 merged.".into()));
    assert_eq!(at(&["p1", "p2", "p3"]), (None, "Done. All 3 merged.".into()));
}

#[test]
fn counts_a_merge_with_no_summary_without_a_leading_space() {
    let jobs = [Job { people: vec!["p1".into(), "p2".into()], ..job(JobStatus::AwaitingMerge) }];
    assert_eq!(needs_you(&jobs, &[], &NO_NAMES, none_merged)[0].detail, "2 people to merge.");
}

#[test]
fn keeps_a_one_person_merge_rows_words_as_the_jobs_own() {
    let jobs = [Job { summary: Some("Done.".into()), people: vec!["p1".into()], ..job(JobStatus::AwaitingMerge) }];
    let row = needs_you(&jobs, &[], &NO_NAMES, |_| true).remove(0);
    assert_eq!((row.person_id.as_deref(), row.detail.as_str()), (Some("p1"), "Done."));
}

#[test]
fn puts_plans_first_then_stops_then_the_rest() {
    let jobs = [job(JobStatus::AwaitingMerge), Job { id: "J-2".into(), plan: Some(plan("s", vec![], vec![])), ..job(JobStatus::AwaitingPlan) }];
    let rows = needs_you(&jobs, &[alert("k", Some("J-1"), Some("d1"), "stuck", false)], &NO_NAMES, none_merged);
    assert_eq!(ids(&rows), [NeedKind::Plan, NeedKind::Alert, NeedKind::Merge]);
    assert_eq!((rows[1].key.as_str(), rows[1].person_id.as_deref(), rows[1].title.as_str(), rows[1].detail.as_str()), ("k", Some("d1"), "stuck", ""));
}

#[test]
fn leaves_out_a_stop_alert_a_paused_job_already_has_a_row_for() {
    let stop = Alert { level: Level::Stop, ..alert("cap-J-1", Some("J-1"), None, "paused", false) };
    assert!(needs_you(&[], &[stop], &NO_NAMES, none_merged).is_empty());
}

#[test]
fn shows_no_plan_row_for_a_job_awaiting_a_plan_it_does_not_have() {
    assert!(needs_you(&[job(JobStatus::AwaitingPlan)], &[], &NO_NAMES, none_merged).is_empty());
}
