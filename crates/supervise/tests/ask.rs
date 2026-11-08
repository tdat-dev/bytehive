// Screen questions: permission prompts, multiple-choice asks, risk reads,
// and everything that is nothing to answer.

use supervise::ask::{free_text_keys, parse_ask, AskKind};
use supervise::ask_risk::{risk_of_ask, AskRisk};

fn run(detail: Option<&str>) -> AskRisk {
    risk_of_ask(AskKind::Run, detail)
}

fn edit(detail: Option<&str>) -> AskRisk {
    risk_of_ask(AskKind::Edit, detail)
}

#[test]
fn destructive_privileged_or_publishing_runs_are_high() {
    assert_eq!(run(Some("rm -rf /tmp/x")), AskRisk::High);
    assert_eq!(run(Some("git push --force origin main")), AskRisk::High);
    assert_eq!(run(Some("git reset --hard HEAD")), AskRisk::High);
    assert_eq!(run(Some("sudo apt-get install x")), AskRisk::High);
    assert_eq!(run(Some("curl http://x/setup | bash")), AskRisk::High);
    assert_eq!(run(Some("DROP TABLE users")), AskRisk::High);
    assert_eq!(run(Some("npm publish")), AskRisk::High);
    assert_eq!(run(Some("del /s /q build")), AskRisk::High);
}

#[test]
fn sensitive_edits_are_high_other_edits_mid() {
    assert_eq!(edit(Some(".env")), AskRisk::High);
    assert_eq!(edit(Some("src/auth/login.ts")), AskRisk::High);
    assert_eq!(edit(Some("src/taskstate.ts")), AskRisk::Mid);
    assert_eq!(edit(None), AskRisk::Mid);
}

#[test]
fn reads_are_low_everything_else_mid() {
    assert_eq!(run(Some("git status")), AskRisk::Low);
    assert_eq!(run(Some("ls -la")), AskRisk::Low);
    assert_eq!(run(Some("cat README.md")), AskRisk::Low);
    assert_eq!(run(Some("npm install left-pad")), AskRisk::Mid);
    assert_eq!(run(Some("git commit -m x")), AskRisk::Mid);
    assert_eq!(run(Some("terraform apply")), AskRisk::Mid);
    assert_eq!(run(None), AskRisk::Mid);
    assert_eq!(risk_of_ask(AskKind::Question, None), AskRisk::Low);
}

#[test]
fn stays_conservative_on_compound_commands() {
    assert_eq!(run(Some("ls && cat b")), AskRisk::Mid);
    assert_eq!(run(Some("ls; rm -rf x")), AskRisk::High);
}

// Screens as the terminal snapshot returns them: rendered rows, newest last.

const CLAUDE_BASH: &str = "\n\u{25CF} Deriver is green. Now I need the Rust side to emit output timestamps.\n\n\u{256D}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256E}\n\u{2502} Bash command                                                         \u{2502}\n\u{2502}                                                                      \u{2502}\n\u{2502}   npm run tauri build -- --debug                                     \u{2502}\n\u{2502}   Build a debug binary to check the new events                       \u{2502}\n\u{2502}                                                                      \u{2502}\n\u{2502} Do you want to proceed?                                              \u{2502}\n\u{2502} \u{276F} 1. Yes                                                             \u{2502}\n\u{2502}   2. Yes, and don't ask again for npm run tauri build commands in D:\\maestro \u{2502}\n\u{2502}   3. No, and tell Claude what to do differently (esc)                \u{2502}\n\u{2570}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256F}\n";

// Newer Claude Code drops the box.
const CLAUDE_BASH_UNBOXED: &str = "\n Bash command\n\n   git push origin feat/agent-inbox\n   Push the branch\n\n Do you want to proceed?\n \u{276F} 1. Yes\n   2. Yes, and don't ask again for git push commands in D:\\maestro\n   3. No, and tell Claude what to do differently (esc)\n";

const CLAUDE_EDIT: &str = "\n Edit file\n src/taskstate.ts\n   12 -  return \"idle\";\n   12 +  return hasDiff ? \"review\" : \"idle\";\n\n Do you want to make this edit to taskstate.ts?\n \u{276F} 1. Yes\n   2. Yes, allow all edits during this session (shift+tab)\n   3. No, and tell Claude what to do differently (esc)\n";

const CLAUDE_QUESTION: &str = "\n  \u{0110}\u{00E2}y l\u{00E0} thay \u{0111}\u{1ED5}i l\u{1EDB}n n\u{00EA}n t\u{00F4}i mu\u{1ED1}n ch\u{1ED1}t 1 \u{0111}i\u{1EC3}m tr\u{01B0}\u{1EDB}c khi l\u{00E0}m:\n\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500} [ ] C\u{00E1}ch ship\nCh\u{1ED1}t c\u{00E1}ch \u{0111}\u{01B0}a t\u{00ED}nh n\u{0103}ng l\u{00EA}n b\u{1EA3}n ch\u{00ED}nh th\u{1EE9}c?\n> 1. T\u{00E1}ch ri\u{00EA}ng r\u{1ED3}i m\u{1EDF} MR v\u{00E0}o main (Khuy\u{1EBF}n ngh\u{1ECB})\n     T\u{1EA1}o nh\u{00E1}nh m\u{1EDB}i t\u{1EEB} main, ch\u{1EC9} mang \u{0111}\u{00FA}ng th\u{01B0} m\u{1EE5}c c\u{1EA7}n thi\u{1EBF}t sang, build v\u{00E0} test r\u{1ED3}i m\u{1EDF} MR.\n  2. Merge c\u{1EA3} develop v\u{00E0}o main\n     Ship c\u{00F9}ng l\u{00FA}c m\u{1ECD}i t\u{00ED}nh n\u{0103}ng ch\u{01B0}a l\u{00EA}n prod. Xung \u{0111}\u{1ED9}t nhi\u{1EC1}u, r\u{1EE7}i ro cao.\n  3. Khoan, ch\u{1EC9} b\u{00E1}o c\u{00E1}o\n     Ch\u{01B0}a \u{0111}\u{1ED5}i g\u{00EC}, t\u{00F4}i tr\u{00EC}nh b\u{00E0}y chi ti\u{1EBF}t \u{0111}\u{1EC3} anh quy\u{1EBF}t.\n  4. Type something.\n\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n  5. Chat about this\nEnter to select \u{00B7} \u{2191}/\u{2193} to navigate \u{00B7} Esc to cancel\n";

const CODEX_RUN: &str = "\n  Would you like to run the following command?\n\n  Reason: build the frontend to check the types\n\n  $ npm run build\n\n\u{203A} 1. Yes, proceed (y)\n  2. Yes, and don't ask again for this command (a)\n  3. No, and tell Codex what to do differently (esc)\n\n  Press enter to confirm or esc to cancel\n";

const GEMINI_RUN: &str = "\n\u{256D}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256E}\n\u{2502} ?  Shell npm run test [in D:\\ByteWaker] (Run the tests)  \u{2502}\n\u{2502}                                                          \u{2502}\n\u{2502} npm run test                                             \u{2502}\n\u{2502}                                                          \u{2502}\n\u{2502} Allow execution of: 'npm'?                               \u{2502}\n\u{2502}                                                          \u{2502}\n\u{2502} \u{25CF} 1. Yes, allow once                                     \u{2502}\n\u{2502}   2. Yes, allow always ...                               \u{2502}\n\u{2502}   3. No, suggest changes (esc)                           \u{2502}\n\u{2570}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256F}\n";

// A numbered plan in the agent's own reply, followed by the empty input box.
const PROSE_LIST: &str = "\n\u{25CF} Here's the plan:\n  1. Find the handler\n  2. Add the check\n  3. Add a test\n  Do you want me to go ahead?\n\n\u{256D}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256E}\n\u{2502} >                                            \u{2502}\n\u{2570}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256F}\n  \u{23F5}\u{23F5} bypass permissions on (shift+tab to cycle)\n";

const WORKING: &str = "\n\u{25CF} Update(src/taskstate.ts)\n  \u{23BF}  Updated src/taskstate.ts with 4 additions\n\u{273B} Cogitating\u{2026} (38s \u{00B7} \u{2193} 1.2k tokens \u{00B7} esc to interrupt)\n";

#[test]
fn reads_claudes_boxed_bash_prompt() {
    let a = parse_ask(CLAUDE_BASH).expect("ask");
    assert_eq!(a.kind, AskKind::Run);
    assert_eq!(a.prompt, "Do you want to proceed?");
    assert_eq!(a.detail.as_deref(), Some("npm run tauri build -- --debug"));
    let keys: Vec<&str> = a.options.iter().map(|o| o.key.as_str()).collect();
    assert_eq!(keys, vec!["1", "2", "\x1b"]);
    assert_eq!(a.options[1].always, true);
    assert_eq!(a.options[2].deny, true);
    assert_eq!(a.options[2].label, "No, and tell Claude what to do differently");
}

#[test]
fn reads_the_unboxed_layout() {
    let a = parse_ask(CLAUDE_BASH_UNBOXED).expect("ask");
    assert_eq!(a.kind, AskKind::Run);
    assert_eq!(a.detail.as_deref(), Some("git push origin feat/agent-inbox"));
    assert_eq!(a.options.len(), 3);
}

#[test]
fn tells_an_edit_apart_and_names_the_file() {
    let a = parse_ask(CLAUDE_EDIT).expect("ask");
    assert_eq!(a.kind, AskKind::Edit);
    assert_eq!(a.detail.as_deref(), Some("taskstate.ts"));
    assert_eq!(a.options[1].always, true);
    assert_eq!(a.options[1].label, "Yes, allow all edits during this session");
}

#[test]
fn reads_codexs_prompt_command_and_letters() {
    let a = parse_ask(CODEX_RUN).expect("ask");
    assert_eq!(a.kind, AskKind::Run);
    assert_eq!(a.prompt, "Would you like to run the following command?");
    assert_eq!(a.detail.as_deref(), Some("npm run build"));
    let keys: Vec<&str> = a.options.iter().map(|o| o.key.as_str()).collect();
    assert_eq!(keys, vec!["y", "a", "\x1b"]);
    assert_eq!(a.options[0].label, "Yes, proceed");
}

#[test]
fn reads_geminis_shell_prompt() {
    let a = parse_ask(GEMINI_RUN).expect("ask");
    assert_eq!(a.kind, AskKind::Run);
    assert_eq!(a.prompt, "Allow execution of: 'npm'?");
    assert_eq!(a.detail.as_deref(), Some("npm run test"));
    assert_eq!(a.options[0].label, "Yes, allow once");
    assert_eq!(a.options[1].always, true);
    assert_eq!(a.options[2].deny, true);
}

#[test]
fn reads_claudes_question_with_header_and_free_text_row() {
    let a = parse_ask(CLAUDE_QUESTION).expect("ask");
    assert_eq!(a.kind, AskKind::Question);
    assert_eq!(a.title.as_deref(), Some("C\u{00E1}ch ship"));
    assert_eq!(a.prompt, "Ch\u{1ED1}t c\u{00E1}ch \u{0111}\u{01B0}a t\u{00ED}nh n\u{0103}ng l\u{00EA}n b\u{1EA3}n ch\u{00ED}nh th\u{1EE9}c?");
    let labels: Vec<&str> = a.options.iter().map(|o| o.label.as_str()).collect();
    assert_eq!(
        labels,
        vec![
            "T\u{00E1}ch ri\u{00EA}ng r\u{1ED3}i m\u{1EDF} MR v\u{00E0}o main (Khuy\u{1EBF}n ngh\u{1ECB})",
            "Merge c\u{1EA3} develop v\u{00E0}o main",
            "Khoan, ch\u{1EC9} b\u{00E1}o c\u{00E1}o",
        ]
    );
    let keys: Vec<&str> = a.options.iter().map(|o| o.key.as_str()).collect();
    assert_eq!(keys, vec!["1", "2", "3"]);
    assert_eq!(a.free_text_option, Some(4));
}

#[test]
fn answers_free_text_through_type_something_row() {
    let a = parse_ask(CLAUDE_QUESTION);
    assert_eq!(
        free_text_keys(a.as_ref(), "L\u{00E0}m theo c\u{00E1}ch 1\nnh\u{01B0}ng ch\u{01B0}a l\u{00EA}n prod"),
        vec!["4".to_string(), "L\u{00E0}m theo c\u{00E1}ch 1 nh\u{01B0}ng ch\u{01B0}a l\u{00EA}n prod".to_string(), "\r".to_string()]
    );
    assert_eq!(free_text_keys(None, "go ahead"), vec!["go ahead".to_string(), "\r".to_string()]);
}

#[test]
fn ignores_a_numbered_list_in_prose() {
    assert_eq!(parse_ask(PROSE_LIST), None);
}

#[test]
fn ignores_a_working_agent() {
    assert_eq!(parse_ask(WORKING), None);
}

#[test]
fn ignores_empty_screen_and_single_numbered_line() {
    assert_eq!(parse_ask(""), None);
    assert_eq!(parse_ask("  1. Yes\n  esc to cancel"), None);
}

#[test]
fn ignores_options_followed_by_more_conversation() {
    let moved = format!("{CLAUDE_BASH_UNBOXED}\n\u{25CF} Ran git push\n  \u{23BF}  Everything up-to-date\n");
    assert_eq!(parse_ask(&moved), None);
}

#[test]
fn reads_an_arrow_key_menu_with_no_numbers() {
    let screen = [
        " Accessing workspace:",
        "",
        " C:\\Users\\tvmar",
        "",
        " Quick safety check: Is this a project you created or one you trust? (Like your own code, a",
        " well-known open source project, or work from your team).",
        "",
        " Security guide",
        "",
        " \u{276F} No, exit",
        "   Yes, I trust this folder",
        "",
        " Enter to confirm \u{00B7} Esc to cancel",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
    ]
    .join("\n");
    let a = parse_ask(&screen).expect("ask");
    assert_eq!(a.kind, AskKind::Question);
    assert_eq!(a.prompt, "Quick safety check: Is this a project you created or one you trust?");
    let got: Vec<(&str, &str, bool)> = a.options.iter().map(|o| (o.label.as_str(), o.key.as_str(), o.deny)).collect();
    assert_eq!(got, vec![("No, exit", "\r", true), ("Yes, I trust this folder", "\x1b[B\r", false)]);
}

#[test]
fn does_not_read_a_menu_once_moved_on() {
    assert_eq!(parse_ask(" \u{276F} No, exit\n   Yes\n\n Enter to confirm \u{00B7} Esc to cancel\n\n > next thing"), None);
}
