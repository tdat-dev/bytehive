use super::*;

fn ask(dir: &str, worktree: bool) -> Prepare {
    Prepare { seat: "s1".into(), person: "p1".into(), name: "Ana".into(), dir: dir.into(), claude: false, worktree }
}

#[test]
fn makes_a_seat_ready_with_nothing_to_settle() {
    let dir = tempfile::tempdir().unwrap();
    let r = prepare(&ask(dir.path().to_str().unwrap(), false));
    assert_eq!(r, Ready { seat: "s1".into(), person: "p1".into(), trust: Ok(()), worktree: None });
}

#[test]
fn says_why_a_seat_outside_git_has_no_worktree() {
    let dir = tempfile::tempdir().unwrap();
    let r = prepare(&ask(dir.path().to_str().unwrap(), true));
    assert_eq!(r.worktree, Some(Err("the project is not a git repo".into())));
}

#[test]
fn excludes_nothing_outside_git_and_calls_it_done() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_str().unwrap().to_string();
    assert_eq!(run_op(Op::Exclude { dir: d.clone() }), Done::Excluded { dir: d, result: Ok(()) });
}
