use super::*;
use crate::company::laws::{COMPANY_COMMON, FOUNDER_LAWS, LEAD_LAWS, STAFF_LAWS};
use crate::company::types::{FOUNDER_ID, Floor, Look};
use serde_json::{Map, Value, json};
use std::fs;
use std::path::PathBuf;

fn look() -> Look {
    Look { shirt: "#fff".into(), hair: "#000".into(), prop: "none".into(), ..Default::default() }
}

fn role() -> RoleDef {
    RoleDef {
        id: "marketing".into(),
        title: "Marketing".into(),
        department: "Marketing".into(),
        rank: Rank::Staff,
        cli: "claude".into(),
        deny_tools: vec![],
        look: look(),
        prompt: "You are the marketer.".into(),
        skills: vec!["campaign-brief".into()],
    }
}

fn person() -> Person {
    Person {
        id: "p1".into(),
        name: "Vy".into(),
        role: "marketing".into(),
        rank: Rank::Staff,
        floor: Some("gcare".into()),
        boss: Some(FOUNDER_ID.into()),
        cli: "claude".into(),
        look: look(),
        session: None,
        worktree: None,
        branch: None,
        extra: Map::new(),
    }
}

fn company() -> Company {
    let mut c = Company::empty();
    c.floors = vec![Floor { id: "gcare".into(), dir: "D:\\gcare".into(), name: "gcare".into(), extra: Map::new() }];
    c.people.push(person());
    c
}

fn files(plugin: Option<&str>, settings: Option<&str>) -> PersonFiles {
    PersonFiles { prompt: "f".into(), mcp: "m".into(), plugin: plugin.map(PathBuf::from), settings: settings.map(PathBuf::from) }
}

// composePrompt

#[test]
fn compose_prompt_puts_the_ranks_laws_the_role_and_the_persons_place_in_one_prompt() {
    let text = compose_prompt(&person(), &role(), &company());
    for part in [STAFF_LAWS, "You are the marketer.", "Vy", "Founder", "gcare"] {
        assert!(text.contains(part), "{part}");
    }
    assert_eq!(text, format!("{STAFF_LAWS}\n\nYour name is Vy. Your boss is Founder. You work on the gcare floor.\n\nYou are the marketer."));
}

#[test]
fn compose_prompt_picks_the_laws_by_rank() {
    let lead = Person { rank: Rank::Lead, ..person() };
    assert!(compose_prompt(&lead, &role(), &company()).contains(LEAD_LAWS));
    let founder = Person { rank: Rank::Founder, floor: None, boss: None, ..person() };
    let text = compose_prompt(&founder, &role(), &company());
    assert!(text.contains(FOUNDER_LAWS));
    assert!(text.contains("Your name is Vy. You answer only to the person who owns the company.\n\n"));
}

#[test]
fn compose_prompt_gives_each_rank_its_laws_first() {
    for (rank, laws) in [(Rank::Founder, FOUNDER_LAWS), (Rank::Lead, LEAD_LAWS), (Rank::Staff, STAFF_LAWS)] {
        let text = compose_prompt(&Person { rank, ..person() }, &role(), &company());
        assert!(text.starts_with(&format!("{laws}\n\n")));
        assert!(laws.starts_with(COMPANY_COMMON));
    }
    // A rank a newer build wrote gets the fewest rights.
    let other = Person { rank: Rank::Other("intern".into()), ..person() };
    assert!(compose_prompt(&other, &role(), &company()).starts_with(STAFF_LAWS));
}

#[test]
fn compose_prompt_does_not_tell_a_member_to_wait_for_the_users_approval() {
    assert!(!STAFF_LAWS.to_lowercase().contains("wait for the user"));
}

#[test]
fn the_founders_laws_say_a_new_role_goes_in_the_plan_for_the_owner_to_read() {
    assert!(FOUNDER_LAWS.contains("newRoles"));
    assert!(!FOUNDER_LAWS.contains("role_propose"));
}

#[test]
fn what_the_founder_is_told_about_open_jobs_says_company_status_lists_them() {
    assert!(FOUNDER_LAWS.contains("company_status lists every open job"));
}

#[test]
fn what_the_founder_is_told_about_a_job_with_nothing_to_do_says_to_tell_the_owner_with_job_finish() {
    let at = FOUNDER_LAWS.find("If a job needs no work").unwrap();
    assert!(FOUNDER_LAWS[at..].contains("job_finish"));
}

#[test]
fn what_a_lead_is_told_about_their_own_cards_says_to_pass_the_job_to_card_add() {
    let at = LEAD_LAWS.find("card_add").unwrap();
    assert!(LEAD_LAWS[at..].contains("job"));
}

// founderFolder

#[test]
fn founder_folder_is_the_active_project() {
    assert_eq!(founder_folder(Some(Some("D:\\b")), &[Some("D:\\a"), Some("D:\\b")]).as_deref(), Some("D:\\b"));
}

#[test]
fn founder_folder_is_the_first_open_project_with_a_folder_when_the_active_one_has_none() {
    assert_eq!(founder_folder(Some(None), &[None, Some("D:\\a"), Some("D:\\b")]).as_deref(), Some("D:\\a"));
}

#[test]
fn founder_folder_is_the_first_open_project_with_a_folder_when_none_is_active() {
    assert_eq!(founder_folder(None, &[Some("D:\\a"), Some("D:\\b")]).as_deref(), Some("D:\\a"));
}

#[test]
fn founder_folder_is_nothing_when_no_open_project_has_a_folder() {
    assert_eq!(founder_folder(Some(None), &[None]), None);
    assert_eq!(founder_folder(None, &[]), None);
}

// nodeVerdict

const NEEDS: &str = "ByteHive needs Node.js 22.13 or newer to run the Company.";

fn printed(text: &str, code: Option<i32>) -> NodeVerdict {
    node_verdict(&NodeRun::Printed { printed: text.into(), code })
}

#[test]
fn node_verdict_lets_a_new_enough_node_through() {
    for v in ["v22.13.0", "v22.13.0\r\n", "v22.14.1\n", "v23.0.0", "v24.6.0\r\n", "v100.1.0"] {
        assert_eq!(printed(v, Some(0)), NodeVerdict::Ok, "{v:?}");
    }
}

#[test]
fn node_verdict_refuses_an_old_node_naming_the_version() {
    for (v, found) in [("v22.12.9", "v22.12.9"), ("v22.9.0\r\n", "v22.9.0"), ("v20.19.0\n", "v20.19.0"), ("v18.20.0", "v18.20.0"), ("v9.99.0", "v9.99.0")] {
        assert_eq!(printed(v, Some(0)), NodeVerdict::Refuse(format!("{NEEDS} Found {found}.")), "{v:?}");
    }
}

#[test]
fn node_verdict_says_node_was_not_found_when_the_command_printed_nothing_and_failed() {
    for (v, code) in [("", 1), ("\r\n", 1), ("  ", 9009)] {
        let want = format!("{NEEDS} Node.js was not found. If you just installed it, restart ByteHive.");
        assert_eq!(printed(v, Some(code)), NodeVerdict::Refuse(want), "{v:?} {code}");
    }
}

#[test]
fn node_verdict_does_not_call_node_missing_on_empty_output_without_a_failed_exit() {
    for code in [Some(0), None] {
        let NodeVerdict::Log(text) = printed("", code) else { panic!("{code:?}") };
        assert!(text.contains("printed nothing"), "{text}");
    }
}

#[test]
fn node_verdict_does_not_block_when_the_check_itself_failed_or_timed_out_and_gives_the_reason_to_log() {
    let NodeVerdict::Log(text) = node_verdict(&NodeRun::Failed("node took longer than 30 s".into())) else { panic!() };
    assert!(text.contains("node took longer than 30 s"));
}

#[test]
fn node_verdict_does_not_block_on_an_answer_it_cannot_read() {
    for v in ["'node' is not recognized", "22", "Welcome to nvm"] {
        let NodeVerdict::Log(text) = printed(v, Some(0)) else { panic!("{v}") };
        assert!(text.contains(v), "{text}");
    }
}

#[test]
fn every_rank_is_told_to_keep_out_of_the_owners_claude_files() {
    for laws in [FOUNDER_LAWS, LEAD_LAWS, STAFF_LAWS] {
        assert!(laws.contains("Never save memories or write files outside"), "{laws}");
        assert!(laws.contains("personal Claude memory"), "{laws}");
    }
}

// buildRoleLaunch

#[test]
fn args_pass_the_prompt_file_the_mcp_config_the_plugin_and_env_the_identity() {
    let f = PersonFiles { prompt: "C:\\p\\prompt.md".into(), mcp: "C:\\p\\mcp.json".into(), plugin: Some("C:\\p\\plugin".into()), settings: None };
    assert_eq!(args(&role(), &f), ["--append-system-prompt-file", "C:\\p\\prompt.md", "--mcp-config", "C:\\p\\mcp.json", "--strict-mcp-config", "--plugin-dir", "C:\\p\\plugin"]);
    let home = "C:\\Users\\me";
    let real = std::path::PathBuf::from(store::company_root(home));
    assert_eq!(env(&person(), &real, home), [("BYTEHIVE_AGENT_ID".to_string(), "p1".to_string())]);
}

#[test]
fn args_add_denied_tools_only_when_the_role_has_some_and_skip_an_empty_plugin() {
    let r = RoleDef { deny_tools: vec!["Edit".into(), "Write".into()], ..role() };
    assert_eq!(args(&r, &files(None, None)), ["--append-system-prompt-file", "f", "--mcp-config", "m", "--strict-mcp-config", "--disallowedTools", "Edit,Write"]);
}

#[test]
fn env_names_a_sandbox_root_so_the_mcp_server_reads_the_same_company() {
    let sandbox = std::path::Path::new("D:\\sandbox\\company");
    let e = env(&person(), sandbox, "C:\\Users\\me");
    assert_eq!(e[1], ("BYTEHIVE_COMPANY_ROOT".to_string(), "D:\\sandbox\\company".to_string()));
}

// the Founder's settings

#[test]
fn the_founders_settings_turn_every_hook_off() {
    assert_eq!(serde_json::from_str::<Value>(FOUNDER_SETTINGS).unwrap(), json!({ "disableAllHooks": true }));
}

#[test]
fn the_founders_settings_are_passed_when_given_and_only_then() {
    assert_eq!(args(&role(), &files(None, Some("C:\\p\\settings.json"))), ["--append-system-prompt-file", "f", "--mcp-config", "m", "--strict-mcp-config", "--settings", "C:\\p\\settings.json"]);
    assert!(!args(&role(), &files(None, None)).contains(&"--settings".to_string()));
}

// mcpConfig, pluginManifest

#[test]
fn mcp_config_starts_the_server_the_app_carries_with_node_under_the_name_the_laws_use() {
    let server = "C:\\Program Files\\ByteHive\\mcp-server\\bytehive-mcp.mjs";
    let v: Value = serde_json::from_str(&mcp_config(server)).unwrap();
    assert_eq!(v, json!({ "mcpServers": { "bytehive": { "command": "node", "args": [server] } } }));
}

#[test]
fn plugin_manifest_is_valid_json_with_a_safe_name() {
    let v: Value = serde_json::from_str(&plugin_manifest(&Person { id: "p 1!".into(), ..person() })).unwrap();
    assert_eq!(v["name"], "bytehive-role-p-1");
    assert_eq!(v["version"], "1.0.0");
    let v: Value = serde_json::from_str(&plugin_manifest(&Person { id: "--AB_c".into(), ..person() })).unwrap();
    assert_eq!(v["name"], "bytehive-role-ab-c");
}

// person_files (engine.ts personSpec and refreshFiles)

#[test]
fn person_files_writes_the_prompt_the_mcp_config_and_the_plugin_and_gives_back_where() {
    let root = tempfile::tempdir().unwrap();
    let server = std::path::Path::new("C:\\app\\mcp-server\\bytehive-mcp.mjs");
    let f = person_files(root.path(), &person(), &role(), &company(), server).unwrap();
    let base = root.path().join("people").join("p1");
    assert_eq!((f.prompt.clone(), f.mcp.clone(), f.settings.clone()), (base.join("prompt.md"), base.join("mcp.json"), None));
    assert_eq!(fs::read_to_string(&f.prompt).unwrap(), compose_prompt(&person(), &role(), &company()));
    assert_eq!(fs::read_to_string(&f.mcp).unwrap(), mcp_config("C:\\app\\mcp-server\\bytehive-mcp.mjs"));
    let plugin = f.plugin.clone().unwrap();
    assert_eq!(plugin, base.join("plugin"));
    assert_eq!(fs::read_to_string(plugin.join(".claude-plugin").join("plugin.json")).unwrap(), plugin_manifest(&person()));
    let brief = fs::read_to_string(plugin.join("skills").join("campaign-brief").join("SKILL.md")).unwrap();
    assert!(brief.starts_with("---\nname: "));
    assert!(!base.join("settings.json").exists());
}

#[test]
fn person_files_gives_the_founder_settings_and_no_plugin_without_skills() {
    let root = tempfile::tempdir().unwrap();
    let founder = Company::empty().people[0].clone();
    let r = RoleDef { skills: vec![], rank: Rank::Founder, ..role() };
    let f = person_files(root.path(), &founder, &r, &Company::empty(), std::path::Path::new("s.mjs")).unwrap();
    let settings = f.settings.clone().unwrap();
    assert_eq!(settings, root.path().join("people").join(FOUNDER_ID).join("settings.json"));
    assert_eq!(fs::read_to_string(settings).unwrap(), FOUNDER_SETTINGS);
    assert_eq!(f.plugin, None);
    assert!(args(&r, &f).contains(&"--settings".to_string()));
}

#[test]
fn person_files_reads_the_owners_edited_skill() {
    let root = tempfile::tempdir().unwrap();
    let mine = root.path().join("roles/marketing/skills/campaign-brief");
    fs::create_dir_all(&mine).unwrap();
    fs::write(mine.join("SKILL.md"), "mine").unwrap();
    let f = person_files(root.path(), &person(), &role(), &company(), std::path::Path::new("s.mjs")).unwrap();
    assert_eq!(fs::read_to_string(f.plugin.unwrap().join("skills/campaign-brief/SKILL.md")).unwrap(), "mine");
}

#[test]
fn refresh_files_rewrites_whoever_has_a_role_and_names_what_could_not_be_written() {
    let root = tempfile::tempdir().unwrap();
    let c = company();
    let roles = vec![role()];
    let server = std::path::Path::new("s.mjs");
    // The Founder has no role loaded here, so nothing of theirs is written.
    assert!(refresh_files(root.path(), &c.people, &roles, &c, server).is_empty());
    assert!(root.path().join("people/p1/prompt.md").is_file());
    assert!(!root.path().join("people").join(FOUNDER_ID).exists());
    // A file where Vy's folder should be.
    let blocked = tempfile::tempdir().unwrap();
    fs::write(blocked.path().join("people"), "x").unwrap();
    let failed = refresh_files(blocked.path(), &c.people, &roles, &c, server);
    assert_eq!(failed.len(), 3, "{failed:?}");
    assert!(failed[0].starts_with("Vy's prompt was not updated: "));
    assert!(failed[1].starts_with("Vy's MCP config was not updated: "));
    assert!(failed[2].starts_with("Vy's skills were not updated: "));
}

#[test]
fn compose_prompt_takes_an_empty_boss_or_floor_name_for_none() {
    let mut c = company();
    c.people[0].name = String::new();
    c.floors[0].name = String::new();
    let text = compose_prompt(&person(), &role(), &c);
    assert!(text.contains("\n\nYour name is Vy. You answer only to the person who owns the company.\n\n"), "{text}");
}
