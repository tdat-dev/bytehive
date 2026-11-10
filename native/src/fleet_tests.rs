// Not a glob: gpui_kit::* in fleet.rs brings a `test` macro of its own.
use super::{Spec, run_extras};

fn spec(cli: &str, role: Option<&str>) -> Spec {
    Spec {
        id: "a1".into(),
        name: "Ana".into(),
        cli: cli.into(),
        args: vec![],
        cwd: "D:\\p".into(),
        session_id: None,
        started_ms: 0,
        title: None,
        race: None,
        worktree: None,
        branch: None,
        merged_into: None,
        role: role.map(Into::into),
        person: None,
    }
}

#[test]
fn a_plain_claude_seat_still_gets_the_laws() {
    let (args, env) = run_extras(&spec("claude", None), "claude");
    assert_eq!(args, ["--append-system-prompt".to_string(), crate::laws::for_role(None).to_string()]);
    assert!(env.is_empty());
}

#[test]
fn the_director_gets_the_dispatch_laws() {
    let (args, _) = run_extras(&spec("claude", Some("conductor")), "claude");
    assert_eq!(args[1], crate::laws::for_role(Some("conductor")));
    assert_ne!(args[1], crate::laws::for_role(None));
}

#[test]
fn a_cli_with_no_system_prompt_flag_gets_nothing_added() {
    assert_eq!(run_extras(&spec("codex", None), "codex"), (vec![], vec![]));
}

#[test]
fn a_spec_saved_before_people_reads_as_no_person() {
    let json = r#"{"id":"a1","name":"Ana","cli":"claude","cwd":"D:\\p","started_ms":0}"#;
    assert_eq!(serde_json::from_str::<Spec>(json).unwrap().person, None);
}
