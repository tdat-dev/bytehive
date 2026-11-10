// Not a glob: gpui_kit::* in fleet.rs brings a `test` macro of its own.
use super::{Extras, Spec, run_extras, run_extras_with};

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
    let (args, env) = run_extras(&spec("claude", None), "claude").unwrap();
    assert_eq!(args, ["--append-system-prompt".to_string(), crate::laws::for_role(None).to_string()]);
    assert!(env.is_empty());
}

#[test]
fn the_director_gets_the_dispatch_laws() {
    let (args, _) = run_extras(&spec("claude", Some("conductor")), "claude").unwrap();
    assert_eq!(args[1], crate::laws::for_role(Some("conductor")));
    assert_ne!(args[1], crate::laws::for_role(None));
}

#[test]
fn a_cli_with_no_system_prompt_flag_gets_nothing_added() {
    assert_eq!(run_extras(&spec("codex", None), "codex"), Ok((vec![], vec![])));
}

#[test]
fn a_spec_saved_before_people_reads_as_no_person() {
    let json = r#"{"id":"a1","name":"Ana","cli":"claude","cwd":"D:\\p","started_ms":0}"#;
    assert_eq!(serde_json::from_str::<Spec>(json).unwrap().person, None);
}

fn person(cli: &str) -> Spec {
    Spec { person: Some("p1".into()), ..spec(cli, None) }
}

fn company_launch() -> Extras {
    (vec!["--append-system-prompt-file".into(), r"C:\c\people\p1\prompt.md".into()], vec![("BYTEHIVE_AGENT_ID".into(), "p1".into())])
}

#[test]
fn a_person_seat_gets_exactly_the_company_launch_and_no_laws() {
    let got = run_extras_with(&person("claude"), "claude", |_| Ok(company_launch())).unwrap();
    assert_eq!(got, company_launch());
    assert!(!got.0.iter().any(|a| a == "--append-system-prompt"));
}

#[test]
fn a_person_seat_whose_launch_fails_does_not_start_on_the_laws() {
    let got = run_extras_with(&person("claude"), "claude", |_| Err("the app's MCP server is missing".into()));
    assert_eq!(got, Err("the app's MCP server is missing".to_string()));
}

#[test]
fn a_person_on_a_cli_other_than_claude_starts_as_a_plain_seat() {
    let got = run_extras_with(&person("codex"), "codex", |_| panic!("only claude runs a Company launch"));
    assert_eq!(got, Ok((vec![], vec![])));
}
