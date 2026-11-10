//! validate_plan and from_draft, with the helpers of org_tests.rs.

use super::*;

// validatePlan

#[test]
fn validate_plan_accepts_a_one_person_hire_on_an_empty_floor() {
    assert_eq!(validate(&company(vec![]), &JobPlan { hires: hires(&[("marketing", "a")]), ..plan() }), "");
}

#[test]
fn validate_plan_needs_a_lead_once_a_floor_reaches_two_people() {
    let e = validate(&company(vec![]), &JobPlan { hires: hires(&[("developer", "a"), ("developer", "a")]), ..plan() });
    assert!(e.contains("Floor a will have 2 people and needs a Lead: add a hire with role lead."), "{e}");
}

#[test]
fn validate_plan_rejects_an_unknown_floor_an_unknown_role_and_too_many_hires() {
    let mut h = vec![hire("developer", "a"); 6];
    h.extend(hires(&[("lead", "a"), ("ghost", "z")]));
    let e = validate(&company(vec![]), &JobPlan { hires: h, ..plan() });
    assert!(e.contains("Unknown floor z."), "{e}");
    assert!(e.contains("Unknown role ghost. See role_list, or draft it in newRoles in this plan."), "{e}");
    assert!(e.contains(&format!("A plan may hire at most {} people.", DEFAULT_LIMITS.people_per_job)), "{e}");
    assert!(e.contains("A Lead can have at most 5 people."), "{e}");
}

#[test]
fn validate_plan_rejects_a_card_on_an_unknown_floor() {
    let e = validate(&company(vec![]), &JobPlan { cards: vec![card("z", "t")], ..plan() });
    assert_eq!(e, "Card \"t\" is on unknown floor z.");
}

#[test]
fn validate_plan_holds_the_company_to_its_size() {
    let mut c = company(vec![]);
    c.limits.people_total = 2;
    let e = validate(&c, &JobPlan { hires: hires(&[("lead", "a"), ("developer", "a")]), ..plan() });
    assert_eq!(e, "The company is limited to 2 people.");
}

#[test]
fn validate_plan_refuses_to_hire_a_second_founder_whatever_the_role_file_says() {
    let founder_role = RoleDef { rank: Rank::Founder, ..role("founder", Rank::Staff) };
    let mut with = roles();
    with.push(founder_role.clone());
    let e = validate_plan(&company(vec![]), &JobPlan { hires: hires(&[("founder", "a")]), ..plan() }, &with).join("|");
    assert!(e.contains("A plan cannot hire a Founder: the company has one."), "{e}");
    let mut boss = roles();
    boss.push(RoleDef { id: "boss".into(), ..founder_role });
    let e = validate_plan(&company(vec![]), &JobPlan { hires: hires(&[("boss", "a")]), ..plan() }, &boss).join("|");
    assert!(e.contains("Founder"), "{e}");
}

#[test]
fn validate_plan_refuses_a_card_on_a_floor_with_nobody_on_it_and_nobody_hired_there() {
    let want = "Card \"Ship it\" is on floor a, where nobody works: hire someone there.";
    assert!(validate(&company(vec![]), &JobPlan { cards: vec![card("a", "Ship it")], ..plan() }).contains(want));
    let p = JobPlan { cards: vec![card("a", "Ship it")], hires: hires(&[("developer", "b")]), ..plan() };
    assert!(validate(&company(vec![]), &p).contains(want));
}

#[test]
fn validate_plan_takes_a_card_on_a_floor_with_someone_on_it_or_someone_hired_there() {
    let c = company(vec![person("d1", Rank::Staff, Some("a"), F)]);
    assert_eq!(validate(&c, &JobPlan { cards: vec![card("a", "Ship it")], ..plan() }), "");
    let p = JobPlan { cards: vec![card("a", "Ship it")], hires: hires(&[("developer", "a")]), ..plan() };
    assert_eq!(validate(&company(vec![]), &p), "");
}

#[test]
fn validate_plan_allows_one_lead_a_floor() {
    let c = company(vec![person("l1", Rank::Lead, Some("a"), F)]);
    let e = validate(&c, &JobPlan { hires: hires(&[("lead", "a")]), ..plan() });
    assert_eq!(e, "Floor a can only have one Lead.");
}

// validatePlan with new roles

fn analyst_hire() -> Vec<Hire> {
    hires(&[("data-analyst", "a")])
}

#[test]
fn validate_plan_with_new_roles_knows_a_role_the_plan_drafts() {
    let p = JobPlan { new_roles: Some(vec![draft("data-analyst")]), hires: analyst_hire(), ..plan() };
    assert_eq!(validate(&company(vec![]), &p), "");
}

#[test]
fn validate_plan_with_new_roles_tells_the_founder_how_to_add_a_role_that_does_not_exist() {
    assert!(validate(&company(vec![]), &JobPlan { hires: analyst_hire(), ..plan() }).contains("newRoles"));
}

#[test]
fn validate_plan_with_new_roles_refuses_the_id() {
    let long = format!("a{}", "b".repeat(31));
    for id in ["a", "Analyst", "9lives", "..\\evil", "a b", long.as_str(), "con", "lpt1"] {
        let e = validate(&company(vec![]), &JobPlan { new_roles: Some(vec![draft(id)]), ..plan() });
        assert_eq!(e, format!("Role {id} refused: an id is 2 to 31 lowercase letters, digits or dashes, starting with a letter, and not a Windows device name."));
    }
}

#[test]
fn validate_plan_with_new_roles_refuses_a_draft_with_no_id_at_all() {
    // A plan whose draft has no id does not read at all; an empty one is refused.
    let json = r#"{"summary":"s","cards":[],"hires":[],"newRoles":[{"title":"T","department":"D","prompt":"P"}]}"#;
    assert!(serde_json::from_str::<JobPlan>(json).is_err());
    assert_ne!(validate(&company(vec![]), &JobPlan { new_roles: Some(vec![draft("")]), ..plan() }), "");
}

#[test]
fn a_plan_line_with_a_nameless_new_role_does_not_read() {
    // The MCP server refuses such a plan before it writes the line; one written by hand reads as no request.
    let line = r#"{"type":"plan","ts":1,"from":"founder","job":"J-0001","plan":{"summary":"s","cards":[],"hires":[],"newRoles":[{"title":"T","department":"D","prompt":"P"}]}}"#;
    assert!(serde_json::from_str::<crate::company::types::CompanyRequest>(line).is_err());
    let named = line.replace(r#"{"title""#, r#"{"id":"writer","title""#);
    assert!(serde_json::from_str::<crate::company::types::CompanyRequest>(&named).is_ok());
}

#[test]
fn validate_plan_with_new_roles_refuses_an_id_that_exists_so_a_plan_cant_rewrite_a_role() {
    let e = validate(&company(vec![]), &JobPlan { new_roles: Some(vec![draft("developer")]), ..plan() });
    assert_eq!(e, "Role developer refused: that role exists already. Use it, or pick another id.");
}

#[test]
fn validate_plan_with_new_roles_refuses_the_id_founder_even_when_no_such_role_is_loaded() {
    let e = validate(&company(vec![]), &JobPlan { new_roles: Some(vec![draft(FOUNDER_ID)]), ..plan() });
    assert_eq!(e, "Role founder refused: the company has one.");
}

#[test]
fn validate_plan_with_new_roles_refuses_the_same_id_twice_in_one_plan() {
    let e = validate(&company(vec![]), &JobPlan { new_roles: Some(vec![draft("data-analyst"), draft("data-analyst")]), ..plan() });
    assert_eq!(e, "Role data-analyst is in the plan twice.");
}

#[test]
fn validate_plan_with_new_roles_refuses_a_draft_with_an_empty_field_the_owner_has_to_read_it() {
    for field in ["title", "department", "prompt"] {
        let mut d = draft("data-analyst");
        match field {
            "title" => d.title = "  ".into(),
            "department" => d.department = "  ".into(),
            _ => d.prompt = "  ".into(),
        }
        let e = validate(&company(vec![]), &JobPlan { new_roles: Some(vec![d]), ..plan() });
        assert_eq!(e, format!("Role data-analyst has no {field}."));
    }
}

#[test]
fn validate_plan_with_new_roles_counts_a_drafted_role_as_staff_whatever_its_json_says() {
    let forged: NewRole = serde_json::from_str(r#"{"id":"data-analyst","title":"Analyst","department":"Data","prompt":"p","rank":"lead"}"#).unwrap();
    let p = JobPlan { new_roles: Some(vec![forged]), hires: hires(&[("data-analyst", "a"), ("data-analyst", "a")]), ..plan() };
    assert!(validate(&company(vec![]), &p).contains("needs a Lead"));
}

// fromDraft

#[test]
fn from_draft_gives_a_drafted_role_the_plain_rights_whatever_its_json_asks_for() {
    let forged: NewRole = serde_json::from_str(r#"{"id":"x1","title":"T","department":"D","prompt":"P","rank":"founder","cli":"powershell","denyTools":["Read"],"skills":["evil"]}"#).unwrap();
    let r = from_draft(&forged);
    assert_eq!((r.id.as_str(), r.title.as_str(), r.department.as_str(), r.prompt.as_str()), ("x1", "T", "D", "P"));
    assert_eq!((&r.rank, r.cli.as_str()), (&Rank::Staff, "claude"));
    assert!(r.deny_tools.is_empty() && r.skills.is_empty());
}
