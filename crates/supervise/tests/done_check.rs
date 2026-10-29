// Done gate: unfinished markers, test runs, secrets, and their one-line reads.

use supervise::diff_file::parse_diff;
use supervise::done_check::{
    describe_done, describe_secrets, scan_done_diff, scan_secrets, test_runs, test_summary, DoneFinding,
    DoneKind, SecretFinding, SecretKind, TestRun,
};
use supervise::types::{ChatItem, StepItem};

fn diff_of(path: &str, body: &[&str]) -> Vec<supervise::diff_file::DiffFile> {
    let mut raw = vec![format!("diff --git a/{path} b/{path}"), format!("--- a/{path}"), format!("+++ b/{path}")];
    raw.extend(body.iter().map(|l| l.to_string()));
    parse_diff(&raw.join("\n"))
}

fn step(n: u32, over: StepItem) -> ChatItem {
    let _ = n;
    ChatItem::Step(over)
}

fn base_step(id: &str) -> StepItem {
    StepItem {
        id: id.to_string(),
        tool: "Bash".to_string(),
        verb: "Ran".to_string(),
        target: "npm test".to_string(),
        full: Some("npm test".to_string()),
        code: true,
        output: None,
        error: false,
        done: true,
        diff: None,
        added: None,
        removed: None,
        todos: None,
        images: None,
        questions: None,
        at: 1,
    }
}

#[test]
fn flags_todo_markers_added_not_removed_or_old() {
    let files = diff_of("src/a.ts", &["@@ -1,2 +1,3 @@", " keep // TODO: old", "-// TODO: gone", "+// TODO: handle empty", "+// FIXME: retry"]);
    let found = scan_done_diff(&files);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].path, "src/a.ts");
    assert_eq!(found[0].kind, DoneKind::Todo);
    assert!(found[0].text.contains("handle empty"));
}

#[test]
fn flags_crude_stubs() {
    let files = diff_of(
        "src/b.py",
        &["@@ -0,0 +1,5 @@", "+    raise NotImplementedError", "+    pass", "+    ...", "+    throw new Error('not implemented')", "+    unimplemented!()"],
    );
    let kinds: Vec<DoneKind> = scan_done_diff(&files).iter().map(|f| f.kind).collect();
    assert_eq!(kinds, vec![DoneKind::Stub, DoneKind::Stub, DoneKind::Stub, DoneKind::Stub, DoneKind::Stub]);
}

#[test]
fn reports_one_finding_per_line_stub_first() {
    let files = diff_of("src/a.ts", &["@@ -1 +1 @@", "+// TODO: not implemented yet"]);
    let found = scan_done_diff(&files);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].kind, DoneKind::Stub);
}

#[test]
fn skips_binary_lockfiles_and_minified() {
    let bin = parse_diff("diff --git a/img.png b/img.png\nBinary files differ\n");
    let lock = diff_of("package-lock.json", &["@@ -1 +1 @@", "+// TODO: whatever"]);
    let min = diff_of("app.min.js", &["@@ -1 +1 @@", "+// TODO: whatever"]);
    let mut all = bin;
    all.extend(lock);
    all.extend(min);
    assert!(scan_done_diff(&all).is_empty());
}

#[test]
fn counts_by_kind_with_files() {
    let files = diff_of("src/a.ts", &["@@ -1 +1,2 @@", "+// TODO: one", "+// TODO: two"]);
    let more = diff_of("src/b.ts", &["@@ -1 +1 @@", "+    pass"]);
    let mut all = files;
    all.extend(more);
    assert_eq!(describe_done(&scan_done_diff(&all)), "2 TODOs and 1 stub in 2 files");
}

#[test]
fn names_the_file_when_there_is_one() {
    let files = diff_of("src/a.ts", &["@@ -1 +1 @@", "+// TODO: one"]);
    assert_eq!(describe_done(&scan_done_diff(&files)), "1 TODO in src/a.ts");
}

#[test]
fn describe_done_says_nothing_when_clean() {
    assert_eq!(describe_done(&[]), "");
}

#[test]
fn keeps_test_commands_with_outcome() {
    let mut build = base_step("s2");
    build.target = "npm run build".to_string();
    build.full = Some("npm run build".to_string());
    let mut failed = base_step("s3");
    failed.target = "pytest".to_string();
    failed.full = Some("pytest -q".to_string());
    failed.error = true;
    failed.output = Some("FAILED".to_string());
    let items = vec![step(1, base_step("s1")), step(2, build), step(3, failed)];
    assert_eq!(test_runs(&items), vec![TestRun { cmd: "npm test".to_string(), ok: true }, TestRun { cmd: "pytest -q".to_string(), ok: false }]);
}

#[test]
fn ignores_non_shell_steps_naming_tests() {
    let mut read = base_step("s1");
    read.tool = "Read".to_string();
    read.verb = "Read".to_string();
    read.target = "pytest.ini".to_string();
    read.full = None;
    assert!(test_runs(&[step(1, read)]).is_empty());
}

#[test]
fn ignores_steps_still_running() {
    let mut open = base_step("s1");
    open.done = false;
    assert!(test_runs(&[step(1, open)]).is_empty());
}

#[test]
fn summarizes_the_latest_run() {
    assert_eq!(test_summary(&[]), None);
    let s = test_summary(&[TestRun { cmd: "npm test".to_string(), ok: true }]).unwrap();
    assert_eq!((s.ok, s.text.as_str()), (true, "Tests ran green (npm test)"));
    let s = test_summary(&[TestRun { cmd: "npm test".to_string(), ok: true }, TestRun { cmd: "cargo test".to_string(), ok: false }]).unwrap();
    assert_eq!((s.ok, s.text.as_str()), (false, "Last test run failed (cargo test)"));
}

#[test]
fn flags_hardcoded_credentials_on_added_lines() {
    // Built in pieces: the fixtures must match the secret patterns at runtime
    // without ever spelling a realistic credential in this source.
    let akia = ["+AWS_KEY=", "AKIA", "EXAMPLEKEY123456"].concat();
    let tok = ["+token", ": abcdefgh"].concat();
    let lines: Vec<&str> = vec!["@@ -0,0 +1,4 @@", akia.as_str(), "+api_key = \"hunter2-secret-value\"", tok.as_str(), "+postgres://bob:hunter2x@db:5432/app"];
    let files = diff_of(".env", &lines);
    let found = scan_secrets(&files);
    assert_eq!(found.iter().filter(|f| f.kind == SecretKind::Secret).count(), 4);
}

#[test]
fn flags_private_keys_and_vendor_tokens() {
    let files = diff_of("key.pem", &["@@ -0,0 +1,2 @@", "+-----BEGIN RSA PRIVATE KEY-----", "+ghp_abcdefghij0123456789abcdefghij0123456789"]);
    let kinds: Vec<SecretKind> = scan_secrets(&files).iter().map(|f| f.kind).collect();
    assert_eq!(kinds, vec![SecretKind::Secret, SecretKind::Secret]);
}

#[test]
fn ignores_removed_lines_and_safe_patterns() {
    let files = diff_of(
        "src/a.ts",
        &[
            "@@ -1,3 +1,4 @@",
            "-api_key = \"old-hardcoded-secret\"",
            "+const key = process.env.API_KEY;",
            "+apiKey = process.env.API_KEY;",
            "+const t = getSecret(\"api-key\");",
            "+password = \"\"",
        ],
    );
    assert!(scan_secrets(&files).iter().all(|f| f.kind != SecretKind::Secret));
}

#[test]
fn flags_auth_files_once_per_file() {
    let files = diff_of("src/auth/login.ts", &["@@ -1 +1,2 @@", "+verifyPassword(u, p)", "+normalize(u)"]);
    assert_eq!(
        scan_secrets(&files),
        vec![SecretFinding { path: "src/auth/login.ts".to_string(), kind: SecretKind::Auth, text: "src/auth/login.ts".to_string() }]
    );
}

#[test]
fn flags_auth_checks_in_plain_files() {
    let files = diff_of("src/app.ts", &["@@ -1 +1 @@", "+if (!isAdmin(user)) throw 0;"]);
    assert_eq!(
        scan_secrets(&files),
        vec![SecretFinding { path: "src/app.ts".to_string(), kind: SecretKind::Auth, text: "if (!isAdmin(user)) throw 0;".to_string() }]
    );
}

#[test]
fn flags_dependency_manifests_once_per_file() {
    let files = diff_of("package.json", &["@@ -1 +1,2 @@", "+\"left-pad\": \"9.9.9\",", "+// bump"]);
    assert_eq!(
        scan_secrets(&files),
        vec![SecretFinding { path: "package.json".to_string(), kind: SecretKind::Dep, text: "package.json".to_string() }]
    );
}

#[test]
fn says_nothing_on_ordinary_code() {
    let files = diff_of("src/a.ts", &["@@ -1 +1,2 @@", "+const secretary = roster[0];", "+token: string;"]);
    assert!(scan_secrets(&files).is_empty());
}

#[test]
fn counts_secrets_with_auth_and_dep_files() {
    let findings = vec![
        SecretFinding { path: ".env".to_string(), kind: SecretKind::Secret, text: "x".to_string() },
        SecretFinding { path: "src/auth/a.ts".to_string(), kind: SecretKind::Auth, text: "src/auth/a.ts".to_string() },
        SecretFinding { path: "src/auth/b.ts".to_string(), kind: SecretKind::Auth, text: "src/auth/b.ts".to_string() },
        SecretFinding { path: "package.json".to_string(), kind: SecretKind::Dep, text: "package.json".to_string() },
    ];
    assert_eq!(describe_secrets(&findings), "1 secret in .env and auth in 2 files and deps in package.json");
    assert_eq!(describe_secrets(&[]), "");
    assert_eq!(describe_done(&[DoneFinding { path: "a".to_string(), kind: DoneKind::Todo, text: "t".to_string() }]), "1 TODO in a");
}
