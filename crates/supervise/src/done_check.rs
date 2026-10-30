//! The light Done gate for review: what the diff itself admits is
//! unfinished (TODO markers, crude stubs), the test runs the conversation
//! shows, and hardcoded secrets or auth code to flag. Crude on purpose: a
//! warning plus one confirm, never a claim the code was verified. Pure.

use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::diff_file::{DiffFile, HunkKind};
use crate::types::ChatItem;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DoneKind {
    Todo,
    Stub,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoneFinding {
    pub path: String,
    pub kind: DoneKind,
    /// The added line, trimmed.
    pub text: String,
}

/// Generated files markers are never reviewed line by line.
fn skip_path_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"-lock\.json$|\.lock$|-lock\.yaml$|\.min\.js$").unwrap())
}

fn todo_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b(TODO|FIXME|XXX|HACK)\b").unwrap())
}

fn stub_res() -> &'static [Regex] {
    static RES: OnceLock<Vec<Regex>> = OnceLock::new();
    RES.get_or_init(|| {
        [
            r"(?i)not implemented",
            r"(?i)unimplemented",
            r"\btodo!\s*\(",
            r"\.todo\(\)",
            r"raise NotImplementedError",
            r"^\s*pass\s*(#|$)",
            r"^\s*\.\.\.\s*(#|$)",
        ]
        .iter()
        .map(|p| Regex::new(p).unwrap())
        .collect()
    })
}

fn cap(text: &str, n: usize) -> String {
    let t = text.trim();
    if t.chars().count() > n {
        t.chars().take(n).collect()
    } else {
        t.to_string()
    }
}

/// Unfinished markers on lines the diff adds. Removed lines and old context
/// don't count: they were already there. One finding per line, stub first.
pub fn scan_done_diff(files: &[DiffFile]) -> Vec<DoneFinding> {
    let mut out = Vec::new();
    for f in files {
        if f.binary || skip_path_re().is_match(&f.path) {
            continue;
        }
        for h in &f.hunks {
            for l in &h.lines {
                if l.kind != HunkKind::Add {
                    continue;
                }
                let text = cap(&l.text, 120);
                if text.is_empty() {
                    continue;
                }
                if stub_res().iter().any(|re| re.is_match(&l.text)) {
                    out.push(DoneFinding { path: f.path.clone(), kind: DoneKind::Stub, text });
                } else if todo_re().is_match(&l.text) {
                    out.push(DoneFinding { path: f.path.clone(), kind: DoneKind::Todo, text });
                }
            }
        }
    }
    out
}

/// "2 TODOs and 1 stub in 2 files", "1 TODO in src/a.ts", "" when clean.
pub fn describe_done(findings: &[DoneFinding]) -> String {
    if findings.is_empty() {
        return String::new();
    }
    let todos = findings.iter().filter(|f| f.kind == DoneKind::Todo).count();
    let stubs = findings.len() - todos;
    let mut parts: Vec<String> = Vec::new();
    if todos > 0 {
        parts.push(format!("{todos} TODO{}", if todos == 1 { "" } else { "s" }));
    }
    if stubs > 0 {
        parts.push(format!("{stubs} stub{}", if stubs == 1 { "" } else { "s" }));
    }
    let mut files: Vec<&str> = findings.iter().map(|f| f.path.as_str()).collect();
    files.sort();
    files.dedup();
    let where_ = if files.len() == 1 { format!("in {}", files[0]) } else { format!("in {} files", files.len()) };
    format!("{} {where_}", parts.join(" and "))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestRun {
    pub cmd: String,
    pub ok: bool,
}

fn shell_tool_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)bash|shell|powershell|exec|command").unwrap())
}

fn test_cmd_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\b(npm (run )?(test|t)|npx (vitest|jest)|yarn (test|vitest)|pnpm (test|vitest)|pytest|cargo test|go test|dotnet test|ctest|jest|vitest|mocha|rspec)\b").unwrap()
    })
}

/// Test commands the conversation ran to completion, oldest first.
pub fn test_runs(items: &[ChatItem]) -> Vec<TestRun> {
    let mut out = Vec::new();
    for it in items {
        let ChatItem::Step(s) = it else { continue };
        if !s.done || !shell_tool_re().is_match(&s.tool) {
            continue;
        }
        let cmd = s.full.as_deref().unwrap_or(s.target.as_str()).trim();
        if cmd.is_empty() || !test_cmd_re().is_match(cmd) {
            continue;
        }
        out.push(TestRun { cmd: cap(cmd, 60), ok: !s.error });
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestSummary {
    pub ok: bool,
    pub text: String,
}

/// The latest run for the review header, or none when none ran.
pub fn test_summary(runs: &[TestRun]) -> Option<TestSummary> {
    let last = runs.last()?;
    Some(if last.ok {
        TestSummary { ok: true, text: format!("Tests ran green ({})", last.cmd) }
    } else {
        TestSummary { ok: false, text: format!("Last test run failed ({})", last.cmd) }
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SecretKind {
    Secret,
    Auth,
    Dep,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretFinding {
    pub path: String,
    pub kind: SecretKind,
    /// The added line, or the path for file-level findings.
    pub text: String,
}

fn secret_res() -> &'static [Regex] {
    static RES: OnceLock<Vec<Regex>> = OnceLock::new();
    RES.get_or_init(|| {
        [
            r"AKIA[0-9A-Z]{16}",
            r"-----BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY-----",
            r"gh[pousr]_[A-Za-z0-9]{36,}",
            r"\bsk-(live|test|proj)-[A-Za-z0-9_-]{8,}",
            r"xox[baprs]-[A-Za-z0-9-]{10,}",
            r#"(?i)(mongodb(\+srv)?|postgres(ql)?|mysql|redis)://[^\s'"]+:[^\s'"]+@"#,
            r"eyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}",
        ]
        .iter()
        .map(|p| Regex::new(p).unwrap())
        .collect()
    })
}

fn secret_assign_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"(?i)(api[_-]?key|apikey|secret|passwd|password|pwd|token)\s*[:=]\s*['"]?[A-Za-z0-9_\-./+]{8,}['"]?"#).unwrap())
}

/// Values that name where the secret lives instead of being one.
fn safe_value_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)process\.env|os\.environ|getenv\(|getsecret|readsecret").unwrap())
}

fn auth_path_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(auth|login|session|password|passwd|permission|acl)\b").unwrap())
}

fn auth_line_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)verify\s*(password|token|signature)|authenticate|authorize|checkPermission|hasRole|isAdmin").unwrap())
}

fn dep_path_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"package\.json$|package-lock\.json$|yarn\.lock$|pnpm-lock\.yaml$|requirements[^/]*\.txt$|pyproject\.toml$|Cargo\.toml$|Cargo\.lock$|go\.mod$|go\.sum$|\.csproj$|Gemfile$|composer\.json$").unwrap()
    })
}

/// Hardcoded secrets on added lines, auth code, and dependency manifests:
/// flag for review, never auto-judge. One finding per line for secrets, one
/// per file for auth paths and manifests.
pub fn scan_secrets(files: &[DiffFile]) -> Vec<SecretFinding> {
    let mut out = Vec::new();
    for f in files {
        if f.binary {
            continue;
        }
        if dep_path_re().is_match(&f.path) {
            out.push(SecretFinding { path: f.path.clone(), kind: SecretKind::Dep, text: f.path.clone() });
        }
        let auth_file = auth_path_re().is_match(&f.path);
        if auth_file {
            out.push(SecretFinding { path: f.path.clone(), kind: SecretKind::Auth, text: f.path.clone() });
        }
        for h in &f.hunks {
            for l in &h.lines {
                if l.kind != HunkKind::Add {
                    continue;
                }
                if secret_res().iter().any(|re| re.is_match(&l.text)) || (!safe_value_re().is_match(&l.text) && secret_assign_re().is_match(&l.text)) {
                    out.push(SecretFinding { path: f.path.clone(), kind: SecretKind::Secret, text: cap(&l.text, 120) });
                    continue;
                }
                if !auth_file && auth_line_re().is_match(&l.text) {
                    out.push(SecretFinding { path: f.path.clone(), kind: SecretKind::Auth, text: cap(&l.text, 120) });
                }
            }
        }
    }
    out
}

/// "1 secret in .env and auth in 2 files", "" when clean. File names stay:
/// a flag without a where is a hunt.
pub fn describe_secrets(findings: &[SecretFinding]) -> String {
    if findings.is_empty() {
        return String::new();
    }
    let mut parts: Vec<String> = Vec::new();
    let secrets: Vec<&SecretFinding> = findings.iter().filter(|f| f.kind == SecretKind::Secret).collect();
    let mut auth: Vec<&str> = findings.iter().filter(|f| f.kind == SecretKind::Auth).map(|f| f.path.as_str()).collect();
    auth.sort();
    auth.dedup();
    let mut deps: Vec<&str> = findings.iter().filter(|f| f.kind == SecretKind::Dep).map(|f| f.path.as_str()).collect();
    deps.sort();
    deps.dedup();
    if !secrets.is_empty() {
        let mut files: Vec<&str> = secrets.iter().map(|f| f.path.as_str()).collect();
        files.sort();
        files.dedup();
        let where_ = if files.len() == 1 { format!(" in {}", files[0]) } else { format!(" in {} files", files.len()) };
        parts.push(format!("{} secret{}{where_}", secrets.len(), if secrets.len() == 1 { "" } else { "s" }));
    }
    if !auth.is_empty() {
        parts.push(if auth.len() == 1 { format!("auth in {}", auth[0]) } else { format!("auth in {} files", auth.len()) });
    }
    if !deps.is_empty() {
        parts.push(if deps.len() == 1 { format!("deps in {}", deps[0]) } else { format!("deps in {} files", deps.len()) });
    }
    parts.join(" and ")
}
