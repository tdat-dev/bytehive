use super::*;

const NOW: Duration = Duration::ZERO;
const LONG: Duration = Duration::from_secs(600);

fn names(dir: &Path) -> Vec<std::ffi::OsString> {
    fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name()).collect()
}

#[test]
fn a_worktree_counts_as_its_main_checkout() {
    assert_eq!(project_key("D:\\.bytehive-worktrees\\shop-x\\cid", Some("D:/work/shop/.git")), "D:/work/shop");
}

#[test]
fn a_folder_outside_git_is_its_own_key() {
    assert_eq!(project_key("C:\\Users\\me\\notes\\", None), "C:/Users/me/notes");
}

#[test]
fn a_submodule_keeps_its_own_folder() {
    assert_eq!(project_key("D:\\work\\shop\\sub", Some("D:/work/shop/.git/modules/sub")), "D:/work/shop/sub");
}

#[test]
fn trust_keeps_everything_else_in_the_config() {
    let text = r#"{"oauthAccount":{"id":1},"projects":{"D:/a":{"allowedTools":["x"]},"D:/b":{"hasTrustDialogAccepted":false}}}"#;
    let out: Value = serde_json::from_str(&with_trust(text, "D:/b").unwrap().unwrap()).unwrap();
    assert_eq!(out["oauthAccount"]["id"], 1);
    assert_eq!(out["projects"]["D:/a"]["allowedTools"][0], "x");
    assert_eq!(out["projects"]["D:/b"]["hasTrustDialogAccepted"], true);
}

#[test]
fn a_new_folder_gets_an_entry() {
    let out: Value = serde_json::from_str(&with_trust("{}", "D:/a").unwrap().unwrap()).unwrap();
    assert_eq!(out["projects"]["D:/a"]["hasTrustDialogAccepted"], true);
}

#[test]
fn a_trusted_folder_writes_nothing() {
    assert_eq!(with_trust(r#"{"projects":{"D:/a":{"hasTrustDialogAccepted":true}}}"#, "D:/a").unwrap(), None);
}

#[test]
fn a_config_it_cannot_read_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(".claude.json");
    fs::write(&file, "{ half written").unwrap();
    assert!(trust_in(&file, "D:/a", NOW, LONG).is_err());
    assert_eq!(fs::read_to_string(&file).unwrap(), "{ half written");
}

#[test]
fn no_config_stays_no_config() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(".claude.json");
    trust_in(&file, "D:/a", NOW, LONG).unwrap();
    assert!(!file.exists());
}

#[test]
fn writes_the_file_and_gives_the_lock_back() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(".claude.json");
    fs::write(&file, "{}").unwrap();
    trust_in(&file, "D:/a", NOW, LONG).unwrap();
    let out: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(out["projects"]["D:/a"]["hasTrustDialogAccepted"], true);
    assert_eq!(names(dir.path()), [".claude.json"]);
}

#[test]
fn a_held_lock_is_waited_for_and_never_broken() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(".claude.json");
    fs::write(&file, "{}").unwrap();
    fs::create_dir(dir.path().join(".claude.json.lock")).unwrap();
    assert!(trust_in(&file, "D:/a", NOW, LONG).is_err());
    assert_eq!(fs::read_to_string(&file).unwrap(), "{}");
    assert!(dir.path().join(".claude.json.lock").exists());
}

#[test]
fn a_write_that_fails_leaves_the_config_and_no_temp_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(".claude.json");
    fs::write(&file, "{}").unwrap();
    let half_written = |tmp: &Path, _: &str| -> std::io::Result<()> {
        fs::write(tmp, "{ half")?;
        Err(std::io::Error::other("disk full"))
    };
    assert!(swap_with(&file, "{\"a\":1}", half_written).is_err());
    assert_eq!(fs::read_to_string(&file).unwrap(), "{}");
    assert_eq!(names(dir.path()), [".claude.json"]);
}

#[test]
fn the_temp_file_holds_all_of_the_text() {
    let dir = tempfile::tempdir().unwrap();
    let tmp = dir.path().join("t");
    write_temp(&tmp, "{\"a\":1}").unwrap();
    assert_eq!(fs::read_to_string(&tmp).unwrap(), "{\"a\":1}");
}

#[test]
fn a_trusted_folder_needs_no_lock() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(".claude.json");
    fs::write(&file, r#"{"projects":{"D:/a":{"hasTrustDialogAccepted":true}}}"#).unwrap();
    fs::create_dir(dir.path().join(".claude.json.lock")).unwrap();
    trust_in(&file, "D:/a", NOW, LONG).unwrap();
    assert!(dir.path().join(".claude.json.lock").exists());
}

#[test]
fn windows_refusing_the_lock_folder_counts_as_held() {
    assert!(lock_contended(ErrorKind::AlreadyExists));
    assert_eq!(lock_contended(ErrorKind::PermissionDenied), cfg!(windows));
    assert!(!lock_contended(ErrorKind::NotFound));
}

#[test]
fn a_stale_lock_that_will_not_go_is_given_up_on() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(".claude.json");
    fs::write(&file, "{}").unwrap();
    let lock = dir.path().join(".claude.json.lock");
    fs::create_dir(&lock).unwrap();
    fs::write(lock.join("in-use"), "").unwrap(); // remove_dir refuses a folder with something in it
    sleep(Duration::from_millis(30));
    assert!(trust_in(&file, "D:/a", NOW, Duration::from_millis(1)).is_err());
    assert_eq!(fs::read_to_string(&file).unwrap(), "{}");
}

#[test]
fn a_lock_left_by_a_dead_claude_is_taken() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(".claude.json");
    fs::write(&file, "{}").unwrap();
    fs::create_dir(dir.path().join(".claude.json.lock")).unwrap();
    sleep(Duration::from_millis(30));
    trust_in(&file, "D:/a", NOW, Duration::from_millis(1)).unwrap();
    assert!(fs::read_to_string(&file).unwrap().contains("hasTrustDialogAccepted"));
}
