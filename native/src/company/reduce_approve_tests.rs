//! reduce.test.ts: approvePlan and rejectPlan, a plan with a new role, reportFailures,
//! notePerson and joinJob, with the helpers of reduce_tests.rs.

use super::*;

fn role_ids(s: &State) -> Vec<&str> {
    s.roles.iter().map(|r| r.id.as_str()).collect()
}

// approvePlan and rejectPlan

#[test]
fn approve_plan_hires_starts_the_job_and_tells_the_founder() {
    let r = approve(&waiting(), "J-0001");
    assert_eq!(r.hired.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Vy"]);
    assert_eq!(r.state.jobs[0].status, JobStatus::Running);
    assert!(r.state.jobs[0].people.contains(&"p9".to_string()));
    assert_eq!(
        told(&r.effects[0]),
        (FOUNDER_ID, "Plan for J-0001 approved. Hired: Vy (marketing). The cards are on the boards. Brief the Lead of each floor, or the person working alone there.")
    );
}

#[test]
fn approve_plan_counts_someone_already_here_who_a_card_names_as_working_the_job() {
    let mut s = waiting();
    s.jobs[0].plan = Some(JobPlan { cards: vec![card("a", Some("Linh"))], ..plan() });
    assert_eq!(approve(&s, "J-0001").state.jobs[0].people, ["p9", "d1", "l1"]);
}

#[test]
fn approve_plan_counts_the_lead_of_every_floor_with_a_card_they_run_the_floor_whoever_does_the_card() {
    let mut s = waiting();
    s.jobs[0].plan = Some(JobPlan { hires: vec![], cards: vec![card("a", None)], ..plan() });
    let r = approve(&s, "J-0001");
    assert_eq!(r.state.jobs[0].people, ["l1"]);
    assert_eq!(
        told(&r.effects[0]),
        (FOUNDER_ID, "Plan for J-0001 approved. The cards are on the boards. Brief the Lead of each floor, or the person working alone there.")
    );
    let mut named = waiting();
    named.jobs[0].plan = Some(JobPlan { hires: vec![], cards: vec![card("a", Some("Linh"))], ..plan() });
    assert_eq!(approve(&named, "J-0001").state.jobs[0].people, ["d1", "l1"]);
}

#[test]
fn approve_plan_tells_staff_who_get_a_new_boss_from_a_new_lead_and_says_who_was_moved() {
    let mut s = waiting();
    s.company.people.push(Person { id: "d2".into(), name: "Mai".into(), floor: Some("b".into()), boss: Some(FOUNDER_ID.into()), ..dev() });
    s.roles.push(RoleDef { id: "lead".into(), rank: Rank::Lead, ..role() });
    s.jobs[0].plan = Some(JobPlan { hires: vec![hire("lead", "b")], ..plan() });
    let r = approve(&s, "J-0001");
    let moved: Vec<(&str, Option<&str>)> = r.rewired.iter().map(|p| (p.name.as_str(), p.boss.as_deref())).collect();
    assert_eq!(moved, [("Mai", Some("p9"))]);
    assert_eq!(
        told(&r.effects[0]),
        ("d2", "[From Founder] You have a new boss: Vy. Take your cards from them and report to them from now on.")
    );
    assert_eq!(told(&r.effects[1]).0, FOUNDER_ID);
}

#[test]
fn approve_plan_keeps_a_plans_token_cap_inside_the_owners_limit() {
    let mut s = waiting();
    let limit = s.jobs[0].token_cap;
    s.jobs[0].plan = Some(JobPlan { token_cap: Some(limit * 10), ..plan() });
    assert_eq!(approve(&s, "J-0001").state.jobs[0].token_cap, limit);
}

#[test]
fn approve_plan_uses_the_plans_token_cap_when_it_has_one() {
    let mut s = waiting();
    s.jobs[0].plan = Some(JobPlan { token_cap: Some(900), ..plan() });
    assert_eq!(approve(&s, "J-0001").state.jobs[0].token_cap, 900);
}

#[test]
fn reject_plan_sends_a_rejection_with_its_reason_back_to_the_founder_and_replans() {
    let r = reject_plan(&waiting(), "J-0001", "too many people");
    assert_eq!(r.state.jobs[0].status, JobStatus::Planning);
    assert_eq!(told(&r.effects[0]), (FOUNDER_ID, "Plan for J-0001 sent back: too many people Make a new plan with job_plan."));
}

#[test]
fn reject_plan_closes_the_job_when_rejected_with_no_reason() {
    let r = reject_plan(&waiting(), "J-0001", "  ");
    assert_eq!((&r.state.jobs[0].status, &r.state.jobs[0].plan), (&JobStatus::Rejected, &None));
    assert!(r.effects.is_empty());
}

#[test]
fn reject_plan_leaves_a_job_alone_unless_it_is_waiting_for_approval() {
    let s = running();
    assert_eq!(reject_plan(&s, "J-0001", "changed my mind"), Reduced { state: s.clone(), effects: vec![] });
    assert_eq!(reject_plan(&s, "J-0001", ""), Reduced { state: s, effects: vec![] });
}

#[test]
fn approve_plan_sends_a_plan_back_when_the_second_of_two_would_give_a_floor_two_leads() {
    let two_leads = JobPlan { summary: "Lead for b".into(), hires: vec![hire("lead", "b")], ..plan() };
    let mut s = State { roles: vec![role(), RoleDef { id: "lead".into(), rank: Rank::Lead, ..role() }], ..state() };
    s = new_job(&s, "one", 1).0.state;
    s = new_job(&s, "two", 2).0.state;
    s = apply_request(&s, &plan_request(FOUNDER_ID, "J-0001", two_leads.clone())).state;
    s = apply_request(&s, &plan_request(FOUNDER_ID, "J-0002", two_leads)).state;
    s = approve(&s, "J-0001").state;
    let r = approve(&s, "J-0002");
    assert!(r.hired.is_empty());
    assert_eq!(r.state.company.people.iter().filter(|p| p.rank == Rank::Lead && p.floor.as_deref() == Some("b")).count(), 1);
    assert_eq!((&r.state.jobs[1].status, &r.state.jobs[1].plan), (&JobStatus::Planning, &None));
    assert_eq!(r.effects, vec![deliver(FOUNDER_ID, "Plan for J-0002 no longer holds: Floor b can only have one Lead. Make a new plan with job_plan.")]);
}

#[test]
fn approve_plan_sends_a_plan_back_when_the_company_filled_up_while_it_waited() {
    let mut s = waiting();
    s.company.limits.people_total = 3;
    let r = approve(&s, "J-0001");
    assert!(r.hired.is_empty());
    assert_eq!(r.state.jobs[0].status, JobStatus::Planning);
    let (to, text) = told(&r.effects[0]);
    assert_eq!(to, FOUNDER_ID);
    assert!(text.contains("limited to 3"), "{text}");
}

#[test]
fn approve_plan_sends_a_plan_back_when_its_role_was_removed_instead_of_crashing() {
    let r = approve(&State { roles: vec![], ..waiting() }, "J-0001");
    assert!(r.hired.is_empty());
    assert_eq!(r.state.jobs[0].status, JobStatus::Planning);
    let (to, text) = told(&r.effects[0]);
    assert_eq!(to, FOUNDER_ID);
    assert!(text.contains("marketing"), "{text}");
}

#[test]
fn approve_plan_leaves_a_job_alone_unless_it_waits_for_approval() {
    for s in [fresh(), running(), closed()] {
        let r = approve(&s, "J-0001");
        assert_eq!(r.state, s);
        assert!(r.hired.is_empty() && r.effects.is_empty());
    }
    let s = waiting();
    assert_eq!(approve(&s, "J-9999").state, s);
}

// a plan with a new role

fn drafted() -> State {
    apply_request(&fresh(), &plan_request(FOUNDER_ID, "J-0001", drafting())).state
}

#[test]
fn a_plan_with_a_new_role_saves_the_role_on_approve_with_the_plain_rights_and_hires_into_it() {
    let r = approve(&drafted(), "J-0001");
    assert_eq!(r.saved.len(), 1);
    let saved = &r.saved[0];
    assert_eq!((saved.id.as_str(), saved.prompt.as_str(), &saved.rank, saved.cli.as_str()), ("analyst", analyst().prompt.as_str(), &Rank::Staff, "claude"));
    assert!(saved.deny_tools.is_empty());
    assert_eq!(role_ids(&r.state), ["marketing", "analyst"]);
    assert_eq!(r.hired.len(), 1);
    assert_eq!((r.hired[0].role.as_str(), &r.hired[0].rank, r.hired[0].cli.as_str()), ("analyst", &Rank::Staff, "claude"));
}

#[test]
fn a_plan_with_a_new_role_saves_nothing_when_the_plan_is_sent_back_or_rejected() {
    assert_eq!(role_ids(&reject_plan(&drafted(), "J-0001", "no").state), ["marketing"]);
    assert_eq!(role_ids(&reject_plan(&drafted(), "J-0001", "").state), ["marketing"]);
}

#[test]
fn a_plan_with_a_new_role_sends_the_plan_back_when_the_id_was_taken_while_it_waited() {
    let mut s = drafted();
    s.roles.push(RoleDef { id: "analyst".into(), ..role() });
    let r = approve(&s, "J-0001");
    assert!(r.saved.is_empty() && r.hired.is_empty());
    assert_eq!(r.state.jobs[0].status, JobStatus::Planning);
}

#[test]
fn a_plan_with_a_new_role_saves_nothing_for_a_plan_without_one() {
    assert!(approve(&waiting(), "J-0001").saved.is_empty());
}

// reportFailures

fn approved() -> Vec<Effect> {
    vec![deliver(FOUNDER_ID, "Plan approved.")]
}

#[test]
fn report_failures_leaves_the_message_as_it_is_when_nothing_failed() {
    assert_eq!(report_failures(approved(), &[]), approved());
}

#[test]
fn report_failures_adds_what_failed_to_the_message() {
    let failed = ["Vy did not start.".to_string(), "No cards on b.".to_string()];
    assert_eq!(told(&report_failures(approved(), &failed)[0]).1, "Plan approved. But this went wrong: Vy did not start. No cards on b.");
}

#[test]
fn report_failures_tells_only_the_founder() {
    let staff = deliver("d1", "New boss.");
    let effects = [vec![staff.clone()], approved()].concat();
    assert_eq!(report_failures(effects, &["Vy did not start.".to_string()])[0], staff);
}

// notePerson

fn seen() -> Seen {
    Seen { session: Some("s1".into()), worktree: Some("D:\\wt".into()), branch: Some("b1".into()) }
}

fn d1(s: &State) -> &Person {
    s.company.people.iter().find(|p| p.id == "d1").unwrap()
}

#[test]
fn note_person_writes_down_what_a_persons_pane_runs_on() {
    let s = note_person(&state(), "d1", &seen());
    let p = d1(&s);
    assert_eq!((p.session.as_deref(), p.worktree.as_deref(), p.branch.as_deref()), (Some("s1"), Some("D:\\wt"), Some("b1")));
}

#[test]
fn note_person_gives_the_same_state_back_when_nothing_moved_so_an_idle_company_writes_nothing() {
    let s = note_person(&state(), "d1", &seen());
    assert_eq!(note_person(&s, "d1", &seen()), s);
    let fresh = state();
    assert_eq!(note_person(&fresh, "d1", &Seen::default()), fresh);
    assert_eq!(note_person(&fresh, "ghost", &seen()), fresh);
}

#[test]
fn note_person_drops_what_the_pane_no_longer_has() {
    let s = note_person(&note_person(&state(), "d1", &seen()), "d1", &Seen { session: Some("s2".into()), ..Seen::default() });
    assert_eq!(d1(&s).session.as_deref(), Some("s2"));
    assert_eq!(d1(&s).worktree, None);
}

// joinJob

#[test]
fn join_job_adds_someone_who_picked_up_a_card_of_a_running_job() {
    assert_eq!(join_job(&running(), "J-0001", "d1").jobs[0].people, ["p9", "d1"]);
}

#[test]
fn join_job_gives_the_same_state_back_when_nothing_changes() {
    let s = running();
    assert_eq!(join_job(&s, "J-0001", "p9"), s);
    assert_eq!(join_job(&s, "J-9999", "d1"), s);
    let done = apply_request(&s, &done_request(FOUNDER_ID, "J-0001", "ok")).state;
    assert_eq!(join_job(&done, "J-0001", "d1"), done);
}
