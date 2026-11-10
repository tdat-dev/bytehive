use super::*;
use std::fs;
use std::path::Path;

fn shipped(id: &str) -> RoleDef {
    default_roles().into_iter().find(|r| r.id == id).unwrap()
}

fn put(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn skill(name: &str, text: &str) -> Skill {
    Skill { name: name.into(), text: text.into() }
}

// roleSkills

#[test]
fn role_skills_reads_the_owners_copy_so_an_edited_skill_counts() {
    let root = tempfile::tempdir().unwrap();
    put(root.path(), "roles/marketing/skills/campaign-brief/SKILL.md", "edited by hand");
    assert_eq!(role_skills(root.path(), &shipped("marketing")), vec![skill("campaign-brief", "edited by hand")]);
}

#[test]
fn role_skills_falls_back_on_the_apps_copy_of_a_skill_the_folder_does_not_have() {
    let root = tempfile::tempdir().unwrap();
    let marketing = shipped("marketing");
    assert_eq!(role_skills(root.path(), &marketing), skill_files(&marketing));
}

#[test]
fn role_skills_finds_a_skill_the_app_never_shipped() {
    let root = tempfile::tempdir().unwrap();
    put(root.path(), "roles/analyst/skills/charts/SKILL.md", "draw them");
    let analyst = RoleDef { id: "analyst".into(), skills: vec!["charts".into(), "ghost".into()], ..shipped("marketing") };
    assert_eq!(role_skills(root.path(), &analyst), vec![skill("charts", "draw them")]);
}

// default roles

#[test]
fn default_roles_cover_the_starting_catalog() {
    let mut ids: Vec<String> = default_roles().into_iter().map(|r| r.id).collect();
    ids.sort();
    assert_eq!(ids, ["copywriter", "designer", "developer", "founder", "lead", "marketing", "qa", "researcher"]);
}

#[test]
fn default_roles_name_the_cli_every_role_runs_on() {
    for r in default_roles() {
        assert!(!r.cli.is_empty(), "{}", r.id);
    }
}

#[test]
fn default_roles_give_every_role_a_prompt() {
    for r in default_roles() {
        assert!(r.prompt.len() > 80, "{}", r.id);
    }
}

#[test]
fn default_roles_forbid_the_founder_and_the_lead_from_editing_files() {
    for id in ["founder", "lead"] {
        assert!(shipped(id).deny_tools.contains(&"Edit".to_string()), "{id}");
    }
}

#[test]
fn default_roles_keep_the_founder_out_of_the_shell_too() {
    let deny = shipped("founder").deny_tools;
    assert!(deny.contains(&"Bash".to_string()));
    assert!(deny.contains(&"PowerShell".to_string()));
    assert!(!shipped("lead").deny_tools.contains(&"Bash".to_string()));
}

#[test]
fn default_roles_ship_a_real_skill_for_marketing() {
    let skills = skill_files(&shipped("marketing"));
    assert!(skills.iter().any(|s| s.name == "campaign-brief"));
    assert!(skills[0].text.starts_with("---\nname: "));
}

#[test]
fn default_roles_read_the_same_on_any_checkout_a_windows_one_gives_these_files_crlf() {
    for r in default_roles() {
        assert!(!r.prompt.contains('\r'), "{}", r.id);
        for s in skill_files(&r) {
            assert!(!s.text.contains('\r'), "{}", s.name);
        }
    }
    assert_eq!(lf("a\r\nb\r\n"), "a\nb\n");
}

#[test]
fn default_roles_tell_every_role_with_skills_to_use_them() {
    for r in default_roles().into_iter().filter(|r| !r.skills.is_empty()) {
        for s in &r.skills {
            assert!(r.prompt.contains(s.as_str()), "{} {s}", r.id);
        }
        assert_eq!(skill_files(&r).len(), r.skills.len());
    }
}

#[test]
fn the_shipped_files_are_the_role_folders_byte_for_byte() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/company/roles");
    assert_eq!(SHIPPED.len(), 18);
    for (rel, text) in SHIPPED {
        assert_eq!(lf(&fs::read_to_string(dir.join(rel)).unwrap()), lf(text), "{rel}");
    }
}

// seedRoles, loadRoles

#[test]
fn seed_writes_every_shipped_role_and_load_reads_them_back() {
    let root = tempfile::tempdir().unwrap();
    seed(root.path()).unwrap();
    let mut back = load(root.path()).unwrap();
    let mut want = default_roles();
    back.sort_by(|a, b| a.id.cmp(&b.id));
    want.sort_by(|a, b| a.id.cmp(&b.id));
    assert_eq!(back, want);
    let brief = fs::read_to_string(root.path().join("roles/marketing/skills/campaign-brief/SKILL.md")).unwrap();
    assert_eq!(brief, skill_files(&shipped("marketing"))[0].text);
}

#[test]
fn seed_writes_a_role_json_with_no_prompt_in_it() {
    let root = tempfile::tempdir().unwrap();
    seed(root.path()).unwrap();
    let text = fs::read_to_string(root.path().join("roles/qa/role.json")).unwrap();
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(v.get("prompt").is_none());
    assert_eq!(v["id"], "qa");
    assert!(!text.contains('\r'));
}

#[test]
fn seed_never_writes_over_a_role_the_owner_has() {
    let root = tempfile::tempdir().unwrap();
    put(root.path(), "roles/qa/role.json", "{\"mine\":true}");
    put(root.path(), "roles/qa/prompt.md", "my prompt");
    seed(root.path()).unwrap();
    assert_eq!(fs::read_to_string(root.path().join("roles/qa/role.json")).unwrap(), "{\"mine\":true}");
    assert_eq!(fs::read_to_string(root.path().join("roles/qa/prompt.md")).unwrap(), "my prompt");
    assert!(root.path().join("roles/lead/role.json").is_file());
}

#[test]
fn load_leaves_out_a_role_folder_missing_a_file_or_with_a_broken_one() {
    let root = tempfile::tempdir().unwrap();
    seed(root.path()).unwrap();
    fs::remove_file(root.path().join("roles/qa/prompt.md")).unwrap();
    put(root.path(), "roles/designer/role.json", "{ not json");
    put(root.path(), "roles/empty/notes.txt", "x");
    let ids: Vec<String> = load(root.path()).unwrap().into_iter().map(|r| r.id).collect();
    assert_eq!(ids.len(), 6);
    assert!(!ids.contains(&"qa".to_string()) && !ids.contains(&"designer".to_string()));
}

#[test]
fn load_reads_a_role_json_saved_with_a_byte_order_mark_and_keeps_an_edited_prompt() {
    let root = tempfile::tempdir().unwrap();
    let mut json = serde_json::to_value(shipped("qa")).unwrap();
    json.as_object_mut().unwrap().remove("prompt");
    put(root.path(), "roles/qa/role.json", &format!("\u{FEFF}{json}"));
    put(root.path(), "roles/qa/prompt.md", "Check it.\r\n");
    let qa = load(root.path()).unwrap().pop().unwrap();
    assert_eq!((qa.id.as_str(), qa.prompt.as_str()), ("qa", "Check it.\r\n"));
}

#[test]
fn save_writes_an_approved_role_and_names_one_it_could_not() {
    let root = tempfile::tempdir().unwrap();
    let analyst = RoleDef { id: "analyst".into(), prompt: "Reads numbers.".into(), ..shipped("qa") };
    assert!(save(root.path(), std::slice::from_ref(&analyst)).is_empty());
    assert_eq!(load(root.path()).unwrap(), vec![analyst.clone()]);
    // A file where the role's folder should be.
    put(root.path(), "roles/blocked", "x");
    let failed = save(root.path(), &[RoleDef { id: "blocked".into(), ..analyst }]);
    assert_eq!(failed.len(), 1);
    assert!(failed[0].starts_with("Role blocked was not saved: "), "{}", failed[0]);
    assert!(failed[0].ends_with('.'));
}
