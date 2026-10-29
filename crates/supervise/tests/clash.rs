// Clash warnings: shared files, same tasks, and how a row reads them.

use supervise::clash::{clash_history, clash_notes, clash_short, ClashInput, ClashKind};
use supervise::task_state::TaskState;

static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn agent() -> ClashInput {
    let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    ClashInput {
        pane_id: format!("p{n}"),
        name: format!("Agent{n}"),
        ws_id: "ws-1".to_string(),
        state: TaskState::Working,
        running: true,
        title: None,
        race: None,
        paths: vec![],
    }
}

fn named(pane: &str, name: &str) -> ClashInput {
    let mut a = agent();
    a.pane_id = pane.to_string();
    a.name = name.to_string();
    a
}

#[test]
fn warns_every_agent_sharing_a_file() {
    let mut a = named("a", "Ana");
    a.paths = vec!["src/app.ts".to_string()];
    let mut b = named("b", "Bo");
    b.paths = vec!["src/app.ts".to_string(), "src/lib.ts".to_string()];
    let notes = clash_notes(&[a, b]);
    assert_eq!(notes.get("a").map(|n| n.kind), Some(ClashKind::File));
    assert_eq!(notes.get("a").map(|n| n.with.clone()).unwrap(), vec!["Bo".to_string()]);
    assert_eq!(notes.get("a").map(|n| n.files.clone()).unwrap(), vec!["src/app.ts".to_string()]);
    assert_eq!(notes.get("b").map(|n| n.files.clone()).unwrap(), vec!["src/app.ts".to_string()]);
}

#[test]
fn compares_paths_across_case_and_separators() {
    let mut a = named("a", "Ana");
    a.paths = vec!["SRC\\App.ts".to_string()];
    let mut b = named("b", "Bo");
    b.paths = vec!["src/app.ts".to_string()];
    let notes = clash_notes(&[a, b]);
    assert_eq!(notes.get("a").map(|n| n.kind), Some(ClashKind::File));
    assert_eq!(notes.get("b").map(|n| n.kind), Some(ClashKind::File));
}

#[test]
fn ignores_agents_in_different_projects() {
    let mut a = named("a", "Ana");
    a.paths = vec!["src/app.ts".to_string()];
    let mut b = named("b", "Bo");
    b.ws_id = "ws-2".to_string();
    b.paths = vec!["src/app.ts".to_string()];
    assert!(clash_notes(&[a, b]).is_empty());
}

#[test]
fn ignores_stopped_or_merged_agents() {
    let mut a = named("a", "Ana");
    a.paths = vec!["src/app.ts".to_string()];
    let mut b = named("b", "Bo");
    b.running = false;
    b.paths = vec!["src/app.ts".to_string()];
    let mut c = named("c", "Cy");
    c.state = TaskState::Merged;
    c.paths = vec!["src/app.ts".to_string()];
    assert!(clash_notes(&[a, b, c]).is_empty());
}

#[test]
fn keeps_a_stable_key_for_the_same_overlap() {
    let inputs = || {
        let mut a = named("a", "Ana");
        a.paths = vec!["src/app.ts".to_string(), "src/b.ts".to_string()];
        let mut b = named("b", "Bo");
        b.paths = vec!["src/b.ts".to_string(), "src/app.ts".to_string()];
        vec![a, b]
    };
    assert_eq!(clash_notes(&inputs()).get("a").map(|n| n.key.clone()), clash_notes(&inputs()).get("a").map(|n| n.key.clone()));
}

#[test]
fn warns_agents_running_the_same_task() {
    let mut a = named("a", "Ana");
    a.title = Some("Fix login".to_string());
    let mut b = named("b", "Bo");
    b.title = Some("  fix   LOGIN ".to_string());
    let notes = clash_notes(&[a, b]);
    assert_eq!(notes.get("a").map(|n| n.kind), Some(ClashKind::Task));
    assert_eq!(notes.get("a").map(|n| n.with.clone()).unwrap(), vec!["Bo".to_string()]);
    assert_eq!(notes.get("b").map(|n| n.with.clone()).unwrap(), vec!["Ana".to_string()]);
}

#[test]
fn skips_empty_titles() {
    assert!(clash_notes(&[named("a", "Ana"), named("b", "Bo")]).is_empty());
}

#[test]
fn a_race_is_on_purpose_not_a_dupe() {
    let mut a = named("a", "Ana");
    a.title = Some("Fix login".to_string());
    a.race = Some("r1".to_string());
    let mut b = named("b", "Bo");
    b.title = Some("Fix login".to_string());
    b.race = Some("r1".to_string());
    let mut c = named("c", "Cy");
    c.title = Some("Fix login".to_string());
    c.race = Some("r2".to_string());
    let notes = clash_notes(&[a, b, c]);
    assert_eq!(notes.get("a").map(|n| n.with.clone()).unwrap(), vec!["Cy".to_string()]);
    assert_eq!(notes.get("b").map(|n| n.with.clone()).unwrap(), vec!["Cy".to_string()]);
    assert_eq!(notes.get("c").map(|n| n.with.clone()).unwrap(), vec!["Ana".to_string(), "Bo".to_string()]);
}

#[test]
fn a_file_clash_wins_over_a_task_dupe() {
    let mut a = named("a", "Ana");
    a.title = Some("Fix login".to_string());
    a.paths = vec!["src/app.ts".to_string()];
    let mut b = named("b", "Bo");
    b.title = Some("Fix login".to_string());
    b.paths = vec!["src/app.ts".to_string()];
    assert_eq!(clash_notes(&[a, b]).get("a").map(|n| n.kind), Some(ClashKind::File));
}

#[test]
fn names_who_and_what_capped() {
    let inputs: Vec<ClashInput> = ["a", "b", "c", "d"]
        .iter()
        .zip(["Ana", "Bo", "Cy", "Dee"])
        .map(|(pane, name)| {
            let mut a = named(pane, name);
            a.paths = vec!["a.ts".to_string(), "b.ts".to_string(), "c.ts".to_string()];
            a
        })
        .collect();
    let notes = clash_notes(&inputs);
    let note = notes.get("a").unwrap();
    assert_eq!(clash_short(note), "overlaps Bo, Cy +1 more in a.ts, b.ts +1 more");
    assert_eq!(clash_history(note), "Overlaps Bo, Cy +1 more in a.ts, b.ts +1 more");
    let mut x = named("x", "Xia");
    x.title = Some("T".to_string());
    let mut y = named("y", "Yara");
    y.title = Some("T".to_string());
    let dupe = clash_notes(&[x, y]).remove("x").unwrap();
    assert_eq!(clash_short(&dupe), "same task as Yara");
}
