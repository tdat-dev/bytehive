use super::*;

fn target(file: PathBuf) -> Target {
    targets(Path::new("C:/home"), "C:/app/mcp-server/bytehive-mcp.mjs").into_iter().next().map(|t| Target { file, ..t }).unwrap()
}

#[test]
fn an_old_entry_is_pointed_at_the_app_and_the_old_file_kept() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("opencode.json");
    std::fs::write(&file, "{\n  \"mcp\": {\n    \"bytehive\": { \"type\": \"local\", \"command\": [\"node\", \"D:/bytehive/mcp/dist/index.js\"], \"enabled\": true }\n  }\n}\n").unwrap();
    assert_eq!(keep(&target(file.clone()), true), Ok(true));
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(v["mcp"]["bytehive"]["command"], json!(["node", "C:/app/mcp-server/bytehive-mcp.mjs"]));
    assert!(std::fs::read_to_string(dir.path().join("opencode.json.bytehive-bak")).unwrap().contains("D:/bytehive/mcp/dist"));
    // The second start finds it right and writes nothing.
    assert_eq!(keep(&target(file), true), Ok(false));
}

#[test]
fn a_cli_that_is_not_here_gets_no_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("nope").join("opencode.json");
    assert_eq!(keep(&target(file.clone()), false), Ok(false));
    assert!(!file.exists());
}

#[test]
fn an_installed_cli_with_no_file_yet_gets_one() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("cfg").join("opencode.json");
    assert_eq!(keep(&target(file.clone()), true), Ok(true));
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(v["$schema"], json!("https://opencode.ai/config.json"));
    assert_eq!(v["mcp"]["bytehive"]["type"], json!("local"));
}

#[test]
fn a_file_it_cannot_read_safely_is_left_as_it_is() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("opencode.json");
    let text = "{ // the owner's comment\n  \"mcp\": {}\n}";
    std::fs::write(&file, text).unwrap();
    assert!(keep(&target(file.clone()), true).is_err());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
}

/// Points this machine's real OpenCode and Antigravity configs at the installed
/// app, as a start of the app does: `cargo test globals -- --ignored`.
#[test]
#[ignore]
fn on_this_machine() {
    let home = std::env::var("USERPROFILE").unwrap();
    let server = Path::new(&home).join("AppData/Local/ByteHive/mcp-server/bytehive-mcp.mjs");
    assert!(server.is_file());
    for t in targets(Path::new(&home), &server.to_string_lossy().replace('\\', "/")) {
        println!("{}: {:?}", t.cli, keep(&t, crate::presets::installed(t.cli)));
    }
}
