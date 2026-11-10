use super::*;
use crate::company::types::{DEFAULT_LIMITS, Floor, Look, PlanCard};
use serde_json::Map;
use std::cell::Cell;

fn role(id: &str, rank: Rank) -> RoleDef {
    RoleDef {
        id: id.into(),
        title: id.into(),
        department: "x".into(),
        rank,
        cli: "claude".into(),
        deny_tools: vec![],
        look: Look { shirt: "#fff".into(), hair: "#000".into(), prop: "none".into(), edited: None, ..Default::default() },
        prompt: "p".into(),
        skills: vec![],
    }
}

fn roles() -> Vec<RoleDef> {
    vec![role("lead", Rank::Lead), role("developer", Rank::Staff), role("marketing", Rank::Staff)]
}

fn person(id: &str, rank: Rank, floor: Option<&str>, boss: Option<&str>) -> Person {
    Person {
        id: id.into(),
        name: id.to_uppercase(),
        role: "developer".into(),
        rank,
        floor: floor.map(Into::into),
        boss: boss.map(Into::into),
        cli: "claude".into(),
        look: role("x", Rank::Staff).look,
        session: None,
        worktree: None,
        branch: None,
        extra: Map::new(),
    }
}

fn floor(id: &str) -> Floor {
    Floor { id: id.into(), dir: format!("D:\\{id}"), name: id.into(), extra: Map::new() }
}

fn company(people: Vec<Person>) -> Company {
    let mut c = Company::empty();
    c.floors = vec![floor("a"), floor("b")];
    c.people.extend(people);
    c
}

fn plan() -> JobPlan {
    JobPlan { summary: "s".into(), cards: vec![], hires: vec![], token_cap: None, new_roles: None, reuse: None, extra: Map::new() }
}

fn hire(role: &str, floor: &str) -> Hire {
    Hire { role: role.into(), floor: floor.into() }
}

fn hires(list: &[(&str, &str)]) -> Vec<Hire> {
    list.iter().map(|(r, f)| hire(r, f)).collect()
}

fn card(floor: &str, title: &str) -> PlanCard {
    PlanCard { floor: floor.into(), title: title.into(), desc: String::new(), checklist: vec![], assignee: None, extra: Map::new() }
}

fn draft(id: &str) -> NewRole {
    NewRole { id: id.into(), title: "Analyst".into(), department: "Data".into(), prompt: "Reads numbers and says what they mean.".into() }
}

const F: Option<&str> = Some(FOUNDER_ID);

fn validate(c: &Company, p: &JobPlan) -> String {
    validate_plan(c, p, &roles()).join("|")
}

fn targets(r: Result<Vec<Person>, String>) -> Vec<String> {
    r.unwrap().into_iter().map(|p| p.id).collect()
}

// canMessage

#[test]
fn can_message_lets_a_person_reach_their_boss_and_their_reports() {
    let c = company(vec![person("l1", Rank::Lead, Some("a"), F), person("d1", Rank::Staff, Some("a"), Some("l1")), person("d2", Rank::Staff, Some("a"), Some("l1")), person("l2", Rank::Lead, Some("b"), F)]);
    assert!(can_message(&c, "d1", "l1"));
    assert!(can_message(&c, "l1", "d1"));
    assert!(can_message(&c, FOUNDER_ID, "l1"));
}

#[test]
fn can_message_stops_peers_skips_and_other_floors() {
    let c = company(vec![person("l1", Rank::Lead, Some("a"), F), person("d1", Rank::Staff, Some("a"), Some("l1")), person("d2", Rank::Staff, Some("a"), Some("l1")), person("l2", Rank::Lead, Some("b"), F)]);
    assert!(!can_message(&c, "d1", "d2"));
    assert!(!can_message(&c, "d1", FOUNDER_ID));
    assert!(!can_message(&c, "l1", "l2"));
}

// resolveTargets

fn two() -> Company {
    company(vec![person("l1", Rank::Lead, Some("a"), F), person("d1", Rank::Staff, Some("a"), Some("l1"))])
}

#[test]
fn resolve_targets_says_so_when_a_person_has_nobody_to_message() {
    assert!(resolve_targets(&Company::empty(), FOUNDER_ID, None).unwrap_err().contains("nobody"));
}

#[test]
fn resolve_targets_sends_a_message_with_no_target_to_the_boss_or_the_reports_of_a_person_with_none_above() {
    let c = two();
    assert_eq!(targets(resolve_targets(&c, "d1", None)), vec!["l1"]);
    let reports: Vec<String> = reports_of(&c, FOUNDER_ID).into_iter().map(|p| p.id.clone()).collect();
    assert_eq!(targets(resolve_targets(&c, FOUNDER_ID, None)), reports);
}

#[test]
fn resolve_targets_explains_a_refusal_in_words_the_agent_can_act_on() {
    let e = resolve_targets(&two(), "d1", Some("Founder")).unwrap_err();
    assert_eq!(e, "You cannot message Founder directly. Send it to L1 and they will pass it on.");
}

#[test]
fn resolve_targets_reports_an_unknown_name() {
    let e = resolve_targets(&two(), "d1", Some("Ghost")).unwrap_err();
    assert_eq!(e, "Nobody named Ghost works here. See company_status.");
}

#[test]
fn resolve_targets_refuses_someone_off_the_roster() {
    assert_eq!(resolve_targets(&two(), "x9", None).unwrap_err(), "You are not on the company roster (x9).");
}

#[test]
fn resolve_targets_finds_a_name_whatever_its_case_and_spaces() {
    assert_eq!(targets(resolve_targets(&two(), "d1", Some("  l1 "))), vec!["l1"]);
}

// reuseStaff

#[test]
fn reuse_staff_drops_a_hire_whose_role_already_works_on_that_floor_so_the_person_there_takes_the_work() {
    let c = company(vec![person("l1", Rank::Lead, Some("a"), F), person("d1", Rank::Staff, Some("a"), Some("l1"))]);
    let p = reuse_staff(&c, &JobPlan { hires: hires(&[("developer", "a"), ("developer", "b"), ("marketing", "a")]), ..plan() });
    assert_eq!(p.hires, hires(&[("developer", "b"), ("marketing", "a")]));
}

#[test]
fn reuse_staff_keeps_a_second_hire_of_the_same_role_when_the_floor_has_only_one_of_it() {
    let c = company(vec![person("d1", Rank::Staff, Some("a"), F)]);
    let p = reuse_staff(&c, &JobPlan { hires: hires(&[("developer", "a"), ("developer", "a")]), ..plan() });
    assert_eq!(p.hires, hires(&[("developer", "a")]));
}

#[test]
fn reuse_staff_names_the_one_person_each_dropped_hire_falls_to_not_everyone_with_the_role() {
    let c = company(vec![person("d1", Rank::Staff, Some("a"), F), person("d2", Rank::Staff, Some("a"), F)]);
    let p = reuse_staff(&c, &JobPlan { hires: hires(&[("developer", "a")]), ..plan() });
    assert_eq!(p.reuse, Some(vec!["d1".to_string()]));
}

#[test]
fn reuse_staff_drops_a_drafted_role_whose_only_hire_went_to_someone_already_there() {
    let c = company(vec![Person { role: "analyst".into(), ..person("x1", Rank::Staff, Some("a"), F) }]);
    let analyst = NewRole { id: "analyst".into(), ..draft("analyst") };
    let kept = NewRole { id: "writer".into(), title: "Writer".into(), ..analyst.clone() };
    let p = reuse_staff(&c, &JobPlan { hires: hires(&[("analyst", "a"), ("writer", "b")]), new_roles: Some(vec![analyst, kept.clone()]), ..plan() });
    assert_eq!(p.new_roles, Some(vec![kept]));
    assert_eq!(p.reuse, Some(vec!["x1".to_string()]));
}

#[test]
fn reuse_staff_gives_back_the_same_plan_when_nothing_repeats() {
    let p = JobPlan { hires: hires(&[("developer", "a")]), ..plan() };
    assert_eq!(reuse_staff(&company(vec![]), &p), p);
}

#[test]
fn reuse_staff_leaves_new_roles_out_when_the_plan_had_none() {
    let c = company(vec![person("d1", Rank::Staff, Some("a"), F)]);
    assert_eq!(reuse_staff(&c, &JobPlan { hires: hires(&[("developer", "a")]), ..plan() }).new_roles, None);
}

// applyHires

/// The oracle's pick: a name from a counter, an id from where the counter is.
fn picker() -> (impl FnMut(&[String]) -> String, impl FnMut() -> String) {
    let n = std::rc::Rc::new(Cell::new(0));
    let m = n.clone();
    (move |_: &[String]| { n.set(n.get() + 1); format!("N{}", n.get()) }, move || format!("id{}", m.get()))
}

#[test]
fn apply_hires_reports_a_lone_hire_to_the_founder() {
    let (mut name, mut id) = picker();
    let (_, hired) = apply_hires(&company(vec![]), &JobPlan { hires: hires(&[("marketing", "a")]), ..plan() }, &roles(), &mut name, &mut id);
    assert_eq!(hired[0].boss.as_deref(), Some(FOUNDER_ID));
    assert_eq!(hired[0].rank, Rank::Staff);
}

#[test]
fn apply_hires_puts_staff_under_the_floors_lead_and_moves_a_lone_worker_under_a_new_lead() {
    let c = company(vec![person("d1", Rank::Staff, Some("a"), F)]);
    let (mut name, mut id) = picker();
    let (next, hired) = apply_hires(&c, &JobPlan { hires: hires(&[("lead", "a"), ("developer", "a")]), ..plan() }, &roles(), &mut name, &mut id);
    let lead = hired.iter().find(|p| p.rank == Rank::Lead).unwrap();
    assert_eq!(lead.boss.as_deref(), Some(FOUNDER_ID));
    assert_eq!(next.people.iter().find(|p| p.id == "d1").unwrap().boss, Some(lead.id.clone()));
    assert_eq!(hired.iter().find(|p| p.rank == Rank::Staff).unwrap().boss, Some(lead.id.clone()));
}

#[test]
fn apply_hires_takes_the_role_s_cli_and_look_and_a_name_nobody_has() {
    let (mut name, mut id) = picker();
    let mut seen: Vec<Vec<String>> = vec![];
    let mut names = |taken: &[String]| { seen.push(taken.to_vec()); name(taken) };
    let (next, hired) = apply_hires(&company(vec![]), &JobPlan { hires: hires(&[("marketing", "b")]), ..plan() }, &roles(), &mut names, &mut id);
    assert_eq!(seen, vec![vec!["Founder".to_string()]]);
    assert_eq!((hired[0].id.as_str(), hired[0].name.as_str(), hired[0].role.as_str()), ("id0", "N1", "marketing"));
    assert_eq!((hired[0].cli.as_str(), hired[0].floor.as_deref()), ("claude", Some("b")));
    assert_eq!(hired[0].look, role("marketing", Rank::Staff).look);
    assert_eq!(next.people.len(), 2);
}

// planCap

#[test]
fn plan_cap_is_the_plans_cap_inside_the_owners_limit() {
    assert_eq!(plan_cap(&JobPlan { token_cap: Some(900), ..plan() }, 1000), 900);
    assert_eq!(plan_cap(&JobPlan { token_cap: Some(9000), ..plan() }, 1000), 1000);
    assert_eq!(plan_cap(&plan(), 1000), 1000);
}

// floorId

#[test]
fn floor_id_makes_a_short_id_from_the_project_name() {
    assert_eq!(floor_id("My App", &[]), "my-app");
}

#[test]
fn floor_id_adds_a_number_until_the_id_is_free() {
    assert_eq!(floor_id("app", &["app".into()]), "app-2");
    assert_eq!(floor_id("app", &["app".into(), "app-2".into()]), "app-3");
}

#[test]
fn floor_id_falls_back_when_the_name_has_no_latin_letters_or_digits() {
    assert_eq!(floor_id("日本語", &[]), "floor");
    assert_eq!(floor_id("日本語", &["floor".into()]), "floor-2");
}

#[test]
fn floor_id_leaves_no_dash_at_either_end() {
    assert_eq!(floor_id("--app (old)--", &[]), "app-old");
}

// validRoleId

#[test]
fn valid_role_id_takes_a_short_lowercase_id() {
    assert!(valid_role_id("data-analyst"));
    assert!(valid_role_id(&format!("a{}", "b".repeat(30))));
}

#[test]
fn valid_role_id_refuses_anything_that_could_not_be_a_folder_name_or_is_too_long() {
    for id in ["a", "Analyst", "9lives", "a b", "a/b", &format!("a{}", "b".repeat(31)), "", "é-role"] {
        assert!(!valid_role_id(id), "{id}");
    }
}

#[test]
fn valid_role_id_refuses_a_name_windows_keeps_for_a_device() {
    for id in ["con", "prn", "aux", "nul", "com1", "com9", "lpt1", "lpt9"] {
        assert!(!valid_role_id(id), "{id}");
    }
}

#[test]
fn valid_role_id_takes_a_name_that_only_starts_like_a_device() {
    for id in ["console", "com10", "lpt0", "aux-writer", "nullable"] {
        assert!(valid_role_id(id), "{id}");
    }
}

// floorContact

#[test]
fn floor_contact_is_the_lead_when_there_is_one_else_the_first_person_else_nobody() {
    let staff = person("s", Rank::Staff, Some("a"), F);
    let lead = person("l", Rank::Lead, Some("a"), F);
    let mut c = Company::empty();
    c.people = vec![staff.clone(), lead];
    assert_eq!(floor_contact(&c, "a").map(|p| p.id.as_str()), Some("l"));
    c.people = vec![staff];
    assert_eq!(floor_contact(&c, "a").map(|p| p.id.as_str()), Some("s"));
    assert!(floor_contact(&c, "b").is_none());
}

// personByName

#[test]
fn person_by_name_ignores_case_and_spaces() {
    let c = two();
    assert_eq!(person_by_name(&c, " founder ").map(|p| p.id.as_str()), Some(FOUNDER_ID));
    assert!(person_by_id(&c, "nobody").is_none());
}

#[path = "org_plan_tests.rs"]
mod plan;
