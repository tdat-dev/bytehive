use super::*;
use serde_json::json;

const LAWS: &str = "Work on the board.";

fn inputs<'a>(server: Option<&'a Path>, mcp_file: Option<&'a Path>) -> Inputs<'a> {
    Inputs { laws: LAWS, server, mcp_file, claude_guard: vec!["--settings".into(), "G".into()], codex_guard: vec!["--dangerously-bypass-hook-trust".into()], opencode_dir: None }
}

fn has_pair(args: &[String], a: &str, b: &str) -> bool {
    args.windows(2).any(|w| w[0] == a && w[1] == b)
}

#[test]
fn every_cli_new_agent_offers_has_its_way_written_down() {
    // A preset added without a line in coverage() lands in the "none" arm: this
    // list is what makes that a choice someone wrote, not an accident.
    let known = ["claude", "codex", "gemini", "aider", "cursor-agent", "opencode", "qwen", "copilot", "goose"];
    for p in crate::presets::PRESETS {
        assert!(known.contains(&p.program), "{} has no harness line: add it to coverage() and here", p.program);
    }
}

#[test]
fn the_clis_used_here_miss_no_piece_that_can_be_given() {
    for cli in ["claude", "codex"] {
        let c = coverage(cli);
        assert!([c.laws, c.mcp, c.guards].iter().all(|w| *w == Way::PerRun), "{cli}: {c:?}");
    }
    for cli in ["agy", "opencode"] {
        let c = coverage(cli);
        assert_ne!(c.laws, Way::None, "{cli}");
        assert_ne!(c.mcp, Way::None, "{cli}");
    }
}

#[test]
fn claude_gets_the_laws_the_guards_and_the_apps_mcp_server() {
    let mcp = Path::new(r"C:\data\harness\claude-mcp.json");
    let p = plan("claude", "D:\\p", &inputs(None, Some(mcp)));
    assert!(has_pair(&p.args, "--append-system-prompt", LAWS));
    assert!(has_pair(&p.args, "--settings", "G"));
    assert!(has_pair(&p.args, "--mcp-config", r"C:\data\harness\claude-mcp.json"));
    assert!(!p.args.iter().any(|a| a == "--strict-mcp-config"), "the owner's other servers stay");
    assert_eq!(p.file, None);
}

#[test]
fn claude_still_starts_with_the_laws_when_the_server_is_missing() {
    let p = plan("claude", "D:\\p", &inputs(None, None));
    assert_eq!(&p.args[..2], ["--append-system-prompt", LAWS]);
    assert!(!p.args.iter().any(|a| a == "--mcp-config"));
}

#[test]
fn codex_gets_the_laws_and_the_installed_server_with_who_it_is() {
    let server = Path::new(r"C:\Users\me\AppData\Local\ByteHive\mcp-server\bytehive-mcp.mjs");
    let p = plan("codex", "D:\\p", &inputs(Some(server), None));
    assert_eq!(p.args[0], "--dangerously-bypass-hook-trust");
    assert!(has_pair(&p.args, "-c", &format!("developer_instructions='{LAWS}'")));
    assert!(has_pair(&p.args, "-c", "mcp_servers.bytehive.command='node'"));
    assert!(has_pair(&p.args, "-c", "mcp_servers.bytehive.args=['C:/Users/me/AppData/Local/ByteHive/mcp-server/bytehive-mcp.mjs']"));
    let vars = p.args.iter().find(|a| a.starts_with("mcp_servers.bytehive.env_vars=")).expect("env_vars");
    assert!(vars.contains("'BYTEHIVE_AGENT'") && vars.contains("'BYTEHIVE_HUB_FILE'"));
}

#[test]
fn a_codex_value_cmd_would_cut_is_left_out_not_broken() {
    let odd = Path::new(r"C:\Users\Ann & Bo\bytehive-mcp.mjs");
    let args = codex::args("Laws with a ' quote", Some(odd));
    assert!(args.is_empty(), "{args:?}");
}

#[test]
fn every_law_set_survives_codex_quoting() {
    for role in [None, Some("conductor")] {
        let args = codex::args(crate::laws::for_role(role), None);
        assert_eq!(args.len(), 2, "{role:?}");
    }
}

#[test]
fn antigravity_gets_the_laws_as_a_rules_file_kept_out_of_git() {
    let p = plan("agy", r"D:\proj", &inputs(None, None));
    let (path, text) = p.file.expect("rules file");
    assert_eq!(path, Path::new(r"D:\proj").join(".agents/rules/bytehive.md"));
    assert!(text.contains(LAWS) && text.starts_with("---\ntrigger: always_on\n---"));
    assert_eq!(p.exclude.as_deref(), Some(".agents/rules/bytehive.md"));
    assert!(p.args.is_empty());
}

#[test]
fn opencode_runs_on_its_own_server_with_bytehives_config_folder() {
    let dir = Path::new(r"C:\Users\me\.bytehive\native\harness\opencode");
    let p = plan("opencode", "D:\\p", &Inputs { opencode_dir: Some(dir), ..inputs(None, None) });
    assert_eq!(p.args, ["--standalone"]);
    assert_eq!(p.env, [("OPENCODE_CONFIG_DIR".to_string(), dir.to_string_lossy().into_owned())]);
    // No folder: it starts as before, on the shared service.
    assert_eq!(plan("opencode", "D:\\p", &inputs(None, None)), Plan::default());
}

#[test]
fn opencodes_folder_has_the_laws_and_the_owners_config_but_not_their_agents_md() {
    let tmp = tempfile::tempdir().unwrap();
    let (owner, lean) = (tmp.path().join("owner"), tmp.path().join("lean"));
    std::fs::create_dir_all(owner.join("skills/one")).unwrap();
    std::fs::write(owner.join("skills/one/SKILL.md"), "skill").unwrap();
    std::fs::write(owner.join("AGENTS.md"), "121 KB of the owner's").unwrap();
    std::fs::write(owner.join("opencode.json"), "{\"model\":\"a\"}").unwrap();
    std::fs::write(owner.join("service.json"), "{\"password\":\"x\"}").unwrap();
    std::fs::write(owner.join("opencode.json.bak"), "old").unwrap();
    opencode::prepare(&lean, &owner, LAWS).unwrap();
    assert_eq!(std::fs::read_to_string(lean.join("AGENTS.md")).unwrap(), opencode::agents_md(LAWS));
    assert_eq!(std::fs::read_to_string(lean.join("opencode.json")).unwrap(), "{\"model\":\"a\"}");
    assert_eq!(std::fs::read_to_string(lean.join("skills/one/SKILL.md")).unwrap(), "skill", "a folder is joined, not copied");
    assert!(!lean.join("service.json").exists() && !lean.join("opencode.json.bak").exists());
    // The owner changes their config: the next launch takes it.
    std::fs::write(owner.join("opencode.json"), "{\"model\":\"b\"}").unwrap();
    opencode::prepare(&lean, &owner, LAWS).unwrap();
    assert_eq!(std::fs::read_to_string(lean.join("opencode.json")).unwrap(), "{\"model\":\"b\"}");
    assert_eq!(std::fs::read_to_string(owner.join("AGENTS.md")).unwrap(), "121 KB of the owner's", "the owner's file is untouched");
}

#[test]
fn a_cli_with_no_way_starts_unchanged() {
    assert_eq!(plan("aider", "D:\\p", &inputs(None, None)), Plan::default());
}

mod json_edit {
    use super::super::jsonedit::set_member;
    use super::*;

    const FILE: &str = "{\n  \"$schema\": \"x\",\n  \"mcp\": {\n    \"bytehive\": {\n      \"command\": [\"node\", \"D:/old.js\"]\n    },\n    \"pencil\": { \"type\": \"local\" }\n  },\n  \"zeta\": 1\n}\n";

    #[test]
    fn replaces_one_entry_and_keeps_the_rest_of_the_text() {
        let entry = json!({ "command": ["node", "C:/new.mjs"] });
        let out = set_member(FILE, "mcp", "bytehive", &entry).unwrap().unwrap();
        assert!(out.contains("C:/new.mjs") && !out.contains("D:/old.js"));
        assert!(out.starts_with("{\n  \"$schema\": \"x\",\n  \"mcp\": {\n    \"bytehive\": {"), "{out}");
        assert!(out.ends_with("    \"pencil\": { \"type\": \"local\" }\n  },\n  \"zeta\": 1\n}\n"), "{out}");
    }

    #[test]
    fn does_nothing_when_the_entry_is_right() {
        let entry = json!({ "command": ["node", "D:/old.js"] });
        assert_eq!(set_member(FILE, "mcp", "bytehive", &entry), Ok(None));
    }

    #[test]
    fn adds_the_entry_and_the_section_when_missing() {
        let entry = json!({ "command": "node" });
        let out = set_member("{\n  \"a\": 1\n}", "mcpServers", "bytehive", &entry).unwrap().unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v, json!({ "a": 1, "mcpServers": { "bytehive": { "command": "node" } } }));
        let out = set_member("{}", "mcpServers", "bytehive", &entry).unwrap().unwrap();
        assert_eq!(serde_json::from_str::<serde_json::Value>(&out).unwrap(), json!({ "mcpServers": { "bytehive": { "command": "node" } } }));
        let out = set_member("{ \"mcp\": { \"x\": 1 } }", "mcp", "bytehive", &entry).unwrap().unwrap();
        assert_eq!(serde_json::from_str::<serde_json::Value>(&out).unwrap(), json!({ "mcp": { "bytehive": { "command": "node" }, "x": 1 } }));
    }

    #[test]
    fn leaves_a_file_with_comments_or_odd_shapes_alone() {
        let entry = json!(1);
        assert!(set_member("{ // c\n \"a\": 1 }", "mcp", "bytehive", &entry).is_err());
        assert!(set_member("[1]", "mcp", "bytehive", &entry).is_err());
        assert!(set_member("{ \"mcp\": [] }", "mcp", "bytehive", &entry).is_err());
    }

    #[test]
    fn keeps_a_bom_and_strings_with_braces() {
        let text = "\u{feff}{ \"note\": \"a } and \\\" {\", \"mcp\": { \"bytehive\": 0 } }";
        let out = set_member(text, "mcp", "bytehive", &json!(2)).unwrap().unwrap();
        assert!(out.starts_with('\u{feff}'));
        assert_eq!(serde_json::from_str::<serde_json::Value>(&out[3..]).unwrap(), json!({ "note": "a } and \" {", "mcp": { "bytehive": 2 } }));
    }
}
