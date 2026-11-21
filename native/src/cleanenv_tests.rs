use super::*;

fn vars(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

#[test]
fn a_plain_launch_is_not_an_agent_one() {
    assert!(!launched_by_agent(&vars(&[("PATH", "C:\\Windows"), ("NO_COLOR", "1"), ("BYTEHIVE_RELAY", "x")])));
}

#[test]
fn any_agent_marker_is_seen_whatever_its_case() {
    for m in AGENT_MARKERS {
        assert!(launched_by_agent(&vars(&[(m, "1")])), "{m}");
    }
    assert!(launched_by_agent(&vars(&[("ClaudeCode", "1")])));
}

#[test]
fn the_session_s_variables_go_and_windows_own_come_back() {
    let current = vars(&[
        ("NO_COLOR", "1"),
        ("CLAUDECODE", "1"),
        ("CLAUDE_CODE_DISABLE_TERMINAL_TITLE", "1"),
        ("GIT_EDITOR", "true"),
        ("Path", "C:\\session\\bin;C:\\Windows"),
    ]);
    let windows = vars(&[("Path", "C:\\Windows;C:\\Users\\me\\bin"), ("USERPROFILE", "C:\\Users\\me")]);
    let next = plan(&current, &windows);
    assert_eq!(next, vars(&[("Path", "C:\\Windows;C:\\Users\\me\\bin"), ("USERPROFILE", "C:\\Users\\me")]));
}

#[test]
fn bytehive_settings_come_along_but_not_one_agent_s_identity() {
    let current = vars(&[
        ("BYTEHIVE_OFFICE3D", "off"),
        ("MAESTRO_HUB", "1"),
        ("RUST_LOG", "debug"),
        ("BYTEHIVE_AGENT", "Bob"),
        ("BYTEHIVE_WORKSPACE", "D:\\x"),
        ("BYTEHIVE_RUN_DIR", "D:\\y"),
    ]);
    let next = plan(&current, &[]);
    assert_eq!(next, vars(&[("BYTEHIVE_OFFICE3D", "off"), ("MAESTRO_HUB", "1"), ("RUST_LOG", "debug")]));
}

#[test]
fn a_user_s_own_windows_variable_stays_even_if_the_session_had_it_too() {
    // Someone who really set NO_COLOR in Windows keeps it.
    let next = plan(&vars(&[("NO_COLOR", "1"), ("CLAUDECODE", "1")]), &vars(&[("NO_COLOR", "1")]));
    assert_eq!(next, vars(&[("NO_COLOR", "1")]));
}

#[cfg(windows)]
#[test]
fn windows_gives_this_user_s_environment() {
    let env = windows_env().expect("CreateEnvironmentBlock");
    let has = |name: &str| env.iter().any(|(k, _)| k.eq_ignore_ascii_case(name));
    assert!(has("PATH") && has("USERPROFILE") && has("SYSTEMROOT"));
    assert!(!has("CLAUDECODE"), "not inherited from this process");
}

/// Run alone, from an agent's shell: `cargo test cleanenv -- --ignored`. It
/// changes this test process's environment.
#[cfg(windows)]
#[test]
#[ignore]
fn a_child_started_after_restore_has_no_session_variables() {
    assert!(std::env::var_os("CLAUDECODE").is_some(), "run this from an agent's shell");
    restore();
    let out = std::process::Command::new("cmd").args(["/c", "set"]).output().expect("cmd");
    let text = String::from_utf8_lossy(&out.stdout).to_ascii_uppercase();
    for gone in ["NO_COLOR=", "CLAUDECODE=", "CLAUDE_CODE_SESSION_ID=", "GIT_EDITOR="] {
        assert!(!text.lines().any(|l| l.starts_with(gone)), "{gone} still set");
    }
    assert!(text.lines().any(|l| l.starts_with("PATH=")) && text.lines().any(|l| l.starts_with("USERPROFILE=")));
}
