//! How agent CLIs are launched and resumed, shared by every UI.
//!
//! - [`launch_command`]: npm/script CLIs install as shims CreateProcessW
//!   rejects; only real binaries run directly, the rest go through cmd /c.
//! - Session args: each CLI's "carry on this conversation" flags, plus what
//!   counts as the preset already picking a session itself.

use crate::command_spec::CommandSpec;

/// Resolve a program/args into something Windows CreateProcessW can actually
/// launch. Only real `.exe`/`.com` binaries run directly; everything else is
/// launched through `cmd.exe /c`, which resolves the right shim via PATHEXT.
pub fn launch_command(program: &str, args: &[String]) -> (String, Vec<String>) {
    let lower = program.trim().to_ascii_lowercase();
    if lower.ends_with(".exe") || lower.ends_with(".com") {
        return (program.to_string(), args.to_vec());
    }
    let mut full = Vec::with_capacity(args.len() + 2);
    full.push("/c".to_string());
    full.push(program.to_string());
    full.extend(args.iter().cloned());
    ("cmd.exe".to_string(), full)
}

/// Build a [`CommandSpec`] through [`launch_command`].
pub fn launch_spec(program: &str, args: &[String]) -> CommandSpec {
    let (program, args) = launch_command(program, args);
    CommandSpec { program, args, cwd: None, env: Vec::new() }
}

/// True when the agent's own args already choose a Muse session.
pub fn muse_picks_session(args: &[String]) -> bool {
    args.iter().any(|a| a == "resume" || a == "--last" || a == "exec")
}

/// True when the agent's own args already choose a Codex session.
pub fn codex_picks_session(args: &[String]) -> bool {
    args.iter().any(|a| a == "resume" || a == "--last" || a == "exec" || a == "fork")
}

/// True when the agent's own args already choose an opencode session.
pub fn opencode_picks_session(args: &[String]) -> bool {
    args.iter().any(|a| {
        a == "--continue"
            || a == "-c"
            || a == "--session"
            || a == "-s"
            || a.starts_with("--session=")
            || a == "--fork"
            || a == "run"
            || a == "session"
            || a == "export"
            || a == "import"
    })
}

/// True when the agent's own args already choose a Claude session.
pub fn claude_picks_session(args: &[String]) -> bool {
    let flag = |a: &str, f: &str| a == f || a.starts_with(&format!("{f}="));
    args.iter().any(|a| ["--session-id", "--resume", "--continue", "-r", "-c"].iter().any(|f| flag(a, f)))
}

/// Claude's session args for this run, and the session id to remember: its
/// own one again (`--resume`) when it exists, else a new one (`--session-id`).
/// Empty when the preset already picks a session itself.
pub fn claude_session_args(
    args: &[String],
    session_id: Option<String>,
    dir: &str,
    exists: impl Fn(&str, &str) -> bool,
    fresh_id: impl Fn() -> String,
) -> (Vec<String>, Option<String>) {
    if claude_picks_session(args) {
        return (Vec::new(), None);
    }
    if let Some(id) = session_id {
        if exists(dir, &id) {
            return (vec!["--resume".to_string(), id.clone()], Some(id));
        }
    }
    let id = fresh_id();
    (vec!["--session-id".to_string(), id.clone()], Some(id))
}

/// Muse's session args (`muse resume <id>`), or empty for a fresh boot whose
/// id the capture after spawning learns.
pub fn muse_session_args(args: &[String], session_id: Option<String>, exists: impl Fn(&str) -> bool) -> (Vec<String>, Option<String>) {
    if muse_picks_session(args) {
        return (Vec::new(), None);
    }
    if let Some(id) = session_id {
        if exists(&id) {
            return (vec!["resume".to_string(), id.clone()], Some(id));
        }
    }
    (Vec::new(), None)
}

/// Codex's session args (`codex resume <id>`), or empty for a fresh boot.
pub fn codex_session_args(args: &[String], session_id: Option<String>, exists: impl Fn(&str) -> bool) -> (Vec<String>, Option<String>) {
    if codex_picks_session(args) {
        return (Vec::new(), None);
    }
    if let Some(id) = session_id {
        if exists(&id) {
            return (vec!["resume".to_string(), id.clone()], Some(id));
        }
    }
    (Vec::new(), None)
}

/// opencode's session args (`--session <id>`), or empty for a fresh boot.
pub fn opencode_session_args(args: &[String], session_id: Option<String>, exists: impl Fn(&str) -> bool) -> (Vec<String>, Option<String>) {
    if opencode_picks_session(args) {
        return (Vec::new(), None);
    }
    if let Some(id) = session_id {
        if exists(&id) {
            return (vec!["--session".to_string(), id.clone()], Some(id));
        }
    }
    (Vec::new(), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn real_binaries_run_directly_case_insensitive() {
        assert_eq!(launch_command("powershell.exe", &s(&["-NoLogo"])), ("powershell.exe".to_string(), s(&["-NoLogo"])));
        assert_eq!(launch_command("CMD.EXE", &[]), ("CMD.EXE".to_string(), Vec::new()));
        assert_eq!(launch_command("tool.COM", &[]), ("tool.COM".to_string(), Vec::new()));
    }

    #[test]
    fn npm_clis_wrap_through_cmd_for_shim_resolution() {
        assert_eq!(launch_command("codex", &[]), ("cmd.exe".to_string(), s(&["/c", "codex"])));
        assert_eq!(launch_command("claude", &s(&["--foo"])), ("cmd.exe".to_string(), s(&["/c", "claude", "--foo"])));
    }

    #[test]
    fn claude_carries_its_conversation_or_starts_a_named_one() {
        let (args, kept) = claude_session_args(&[], Some("111".into()), "D:/app", |_, _| true, || "new".into());
        assert_eq!(args, s(&["--resume", "111"]));
        assert_eq!(kept, Some("111".to_string()));
        let (args, kept) = claude_session_args(&[], Some("111".into()), "D:/app", |_, _| false, || "new".into());
        assert_eq!(args, s(&["--session-id", "new"]));
        assert_eq!(kept, Some("new".to_string()));
        let (args, kept) = claude_session_args(&s(&["--continue"]), Some("x".into()), "D:/app", |_, _| true, || "new".into());
        assert!(args.is_empty() && kept.is_none());
    }

    #[test]
    fn muse_and_codex_resume_or_boot_fresh() {
        let (args, _) = muse_session_args(&[], Some("m1".into()), |_| true);
        assert_eq!(args, s(&["resume", "m1"]));
        let (args, kept) = muse_session_args(&s(&["resume", "--last"]), Some("x".into()), |_| true);
        assert!(args.is_empty() && kept.is_none());
        let (args, _) = codex_session_args(&[], Some("c1".into()), |_| true);
        assert_eq!(args, s(&["resume", "c1"]));
        assert!(codex_picks_session(&s(&["fork"])));
        let (args, kept) = codex_session_args(&[], None, |_| true);
        assert!(args.is_empty() && kept.is_none());
    }

    #[test]
    fn opencode_resumes_by_session_flag() {
        let (args, _) = opencode_session_args(&[], Some("ses_x".into()), |_| true);
        assert_eq!(args, s(&["--session", "ses_x"]));
        assert!(opencode_picks_session(&s(&["--continue"])));
        assert!(opencode_picks_session(&s(&["-s", "ses_x"])));
        assert!(opencode_picks_session(&s(&["--session=ses_x"])));
        assert!(!opencode_picks_session(&s(&["--no-replay"])));
        let (args, kept) = opencode_session_args(&[], None, |_| true);
        assert!(args.is_empty() && kept.is_none());
    }
}
