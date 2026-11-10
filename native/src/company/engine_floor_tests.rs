use super::harness::*;

// keeping git status clean

#[test]
fn changes_nothing_in_anyones_repo_at_launch_when_the_company_was_never_used() {
    let mut t = T::new();
    assert!(t.host.excluded.is_empty());
    t.relaunch();
    assert!(t.host.excluded.is_empty());
}

#[test]
fn excludes_bytehive_in_every_floor_a_job_can_touch_when_the_job_is_given() {
    let mut t = T::new();
    t.host.projects.push(project("D:\\c"));
    t.give("do it").unwrap();
    assert_eq!(t.host.excluded, [A, B, "D:\\c"]);
}

#[test]
fn excludes_at_launch_once_there_are_jobs() {
    let mut t = T::new();
    t.give("do it").unwrap();
    t.host.excluded.clear();
    t.relaunch();
    assert_eq!(t.host.excluded, [A, B]);
}

#[test]
fn asks_git_once_per_folder() {
    let mut t = T::new();
    t.give("one").unwrap();
    t.give("two").unwrap();
    assert_eq!(t.host.excluded.iter().filter(|d| *d == A).count(), 1);
}

#[test]
fn excludes_where_the_founder_starts_floor_or_not() {
    let mut t = T::new();
    t.give("do it").unwrap();
    t.staff_floors(&plan_for("b", 0));
    t.host.close("founder");
    t.host.projects = vec![project("D:\\c")];
    t.message("r-b", "Founder", "done");
    assert!(t.host.excluded.contains(&"D:\\c".to_string()));
    assert!(!t.e.state().unwrap().company.floors.iter().any(|f| f.dir == "D:\\c"));
}

#[test]
fn never_blocks_a_job_on_git_says_why_and_tries_again() {
    let mut t = T::new();
    t.host.projects.push(project("D:\\c"));
    t.host.exclude_fails = true;
    t.give("one").unwrap();
    assert_eq!(t.jobs().len(), 1);
    t.advance(STEP);
    assert!(t.host.logs.iter().any(|l| l.contains("access denied")));
    t.host.exclude_fails = false;
    t.give("two").unwrap();
    assert!(t.host.excluded.contains(&"D:\\c".to_string()));
}

// floors

#[test]
fn gives_two_folders_of_the_same_name_their_own_floors() {
    let mut t = T::new();
    t.host.projects.push(Project { name: "a".into(), dir: Some("D:\\x\\a".into()) });
    t.give("do it").unwrap();
    let ids: Vec<String> = t.e.state().unwrap().company.floors.iter().map(|f| f.id.clone()).collect();
    assert_eq!(ids, ["a", "b", "a-2"]);
}
