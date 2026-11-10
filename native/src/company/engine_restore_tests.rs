use super::harness::*;

// a person whose seat closes

const SESSION: &str = "11111111-2222-3333-4444-555555555555";
const TREE: &str = "D:\\wt\\ana";

fn hired(t: &mut T) -> Person {
    t.host.on_boot.insert("Ana".into(), Restore { session: Some(SESSION.into()), worktree: Some(TREE.into()), branch: Some("bytehive/ana".into()) });
    t.waiting_job(hiring("developer", "a"));
    t.approve(&t.job(0).id);
    t.host.on_boot.clear();
    t.person(|p| p.name == "Ana")
}

#[test]
fn has_their_session_worktree_and_branch_written_down_at_boot() {
    let mut t = T::new();
    let ana = hired(&mut t);
    assert_eq!((ana.session.as_deref(), ana.worktree.as_deref(), ana.branch.as_deref()), (Some(SESSION), Some(TREE), Some("bytehive/ana")));
    assert_eq!(t.on_disk(&ana.id)["session"], SESSION);
}

#[test]
fn follows_a_session_that_moves_on_like_after_clear() {
    let mut t = T::new();
    let ana = hired(&mut t);
    t.host.seat_mut(&ana.id).session = Some("22222222-2222-3333-4444-555555555555".into());
    t.advance(30_000);
    assert_eq!(t.on_disk(&ana.id)["session"], "22222222-2222-3333-4444-555555555555");
}

#[test]
fn comes_back_on_them_with_nothing_to_tell_the_owner() {
    let mut t = T::new();
    let ana = hired(&mut t);
    t.host.close(&ana.id);
    t.host.sessions.insert((TREE.into(), SESSION.into()));
    t.message("founder", "Ana", "carry on");
    let s = t.host.seat(&ana.id).unwrap();
    assert_eq!((s.session.as_deref(), s.worktree.as_deref(), s.branch.as_deref()), (Some(SESSION), Some(TREE), Some("bytehive/ana")));
    assert!(t.e.alerts().is_empty());
}

#[test]
fn raises_an_alert_when_the_session_is_gone_and_keeps_the_worktree() {
    let mut t = T::new();
    let ana = hired(&mut t);
    t.host.close(&ana.id);
    t.message("founder", "Ana", "carry on");
    let s = t.host.seat(&ana.id).unwrap();
    assert_eq!((s.session, s.worktree.as_deref()), (None, Some(TREE)));
    let a = &t.e.alerts()[0];
    assert_eq!((a.person_id.as_deref(), a.standing), (Some(ana.id.as_str()), true));
    assert_eq!(a.text, "Ana's conversation could not be found, so they start blank.");
}

#[test]
fn raises_an_alert_when_the_worktree_is_gone_naming_its_branch_and_the_session_goes_with_it() {
    let mut t = T::new();
    let ana = hired(&mut t);
    t.host.close(&ana.id);
    t.host.gone.insert(TREE.into());
    t.message("founder", "Ana", "carry on");
    let s = t.host.seat(&ana.id).unwrap();
    assert_eq!((s.session, s.worktree, s.branch), (None, None, None));
    assert_eq!(t.e.alerts()[0].text, "Ana's worktree and conversation could not be found, so they start blank. Anything they committed is on branch bytehive/ana.");
}

#[test]
fn writes_down_what_they_start_on_instead_so_the_next_close_finds_it() {
    let mut t = T::new();
    let ana = hired(&mut t);
    t.host.close(&ana.id);
    t.host.gone.insert(TREE.into());
    t.host.on_boot.insert("Ana".into(), Restore { session: Some("33".into()), worktree: Some("D:\\wt\\new".into()), branch: Some("bytehive/ana-2".into()) });
    t.message("founder", "Ana", "carry on");
    let d = t.on_disk(&ana.id);
    assert_eq!((d["session"].as_str(), d["worktree"].as_str(), d["branch"].as_str()), (Some("33"), Some("D:\\wt\\new"), Some("bytehive/ana-2")));
}

#[test]
fn says_nothing_for_someone_who_never_had_any() {
    let mut t = T::new();
    t.give("do it").unwrap();
    assert!(t.e.alerts().is_empty());
}
