//! A seat that is a Company person: its launch from the files on disk.

use super::*;
use crate::company::launch::mcp_config;
use crate::company::laws::STAFF_LAWS;
use crate::company::types::{Company, FOUNDER_ID, Floor, Look, Person, Rank};
use serde_json::Map;
use std::fs;

fn company() -> Company {
    let mut c = Company::empty();
    c.floors = vec![Floor { id: "gcare".into(), dir: "D:\\gcare".into(), name: "gcare".into(), extra: Map::new() }];
    c.people.push(Person {
        id: "p1".into(),
        name: "Vy".into(),
        role: "marketing".into(),
        rank: Rank::Staff,
        floor: Some("gcare".into()),
        boss: Some(FOUNDER_ID.into()),
        cli: "claude".into(),
        look: Look::default(),
        session: None,
        worktree: None,
        branch: None,
        extra: Map::new(),
    });
    c
}

/// A company on disk with no roles folder yet: the launch seeds the shipped ones.
fn seated(root: &Path) {
    store::save_company(root, &company()).unwrap();
}

#[test]
fn a_persons_seat_builds_their_args_and_env_from_their_files() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("company");
    seated(&root);
    let server = Path::new("C:\\app\\mcp-server\\bytehive-mcp.mjs");
    let (args, env) = seat_launch(&root, "C:\\Users\\o", "p1", server).unwrap();
    let dir = root.join("people").join("p1");
    let at = |flag: &str| args.iter().position(|a| a == flag).map(|i| args[i + 1].clone());
    assert_eq!(at("--append-system-prompt-file"), Some(dir.join("prompt.md").to_string_lossy().into_owned()));
    assert_eq!(at("--mcp-config"), Some(dir.join("mcp.json").to_string_lossy().into_owned()));
    assert!(fs::read_to_string(dir.join("prompt.md")).unwrap().starts_with(STAFF_LAWS));
    assert_eq!(fs::read_to_string(dir.join("mcp.json")).unwrap(), mcp_config(&server.to_string_lossy()));
    assert_eq!(at("--settings"), None);
    assert_eq!(
        env,
        [("BYTEHIVE_AGENT_ID".to_string(), "p1".to_string()), ("BYTEHIVE_COMPANY_ROOT".to_string(), root.to_string_lossy().into_owned())]
    );
}

#[test]
fn the_founders_seat_gets_the_founders_settings() {
    let t = tempfile::tempdir().unwrap();
    seated(t.path());
    let (args, _) = seat_launch(t.path(), "C:\\Users\\o", FOUNDER_ID, Path::new("s.mjs")).unwrap();
    let settings = t.path().join("people").join(FOUNDER_ID).join("settings.json");
    assert!(args.windows(2).any(|w| w[0] == "--settings" && w[1] == settings.to_string_lossy()), "{args:?}");
}

#[test]
fn a_seat_for_someone_not_on_the_roster_or_with_no_role_has_no_launch() {
    let t = tempfile::tempdir().unwrap();
    seated(t.path());
    assert!(seat_launch(t.path(), "C:\\Users\\o", "ghost", Path::new("s.mjs")).unwrap_err().contains("ghost"));
    let mut c = company();
    c.people[1].role = "gone".into();
    store::save_company(t.path(), &c).unwrap();
    assert!(seat_launch(t.path(), "C:\\Users\\o", "p1", Path::new("s.mjs")).unwrap_err().contains("gone"));
}

#[test]
fn takes_a_launch_made_ready_off_the_ui_thread_once() {
    let ready: Launch = Ok((vec!["--prepared".into()], vec![("K".into(), "V".into())]));
    PREPARED.lock().unwrap().push(("seat-prepared-1".into(), ready.clone()));
    assert_eq!(take_prepared("seat-prepared-1"), Some(ready));
    assert_eq!(take_prepared("seat-prepared-1"), None);
}
