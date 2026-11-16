use super::*;

fn line(payload: serde_json::Value) -> String {
    serde_json::json!({ "type": "response_item", "payload": payload }).to_string()
}

fn user(text: &str) -> String {
    line(serde_json::json!({ "type": "message", "role": "user", "content": [{ "type": "input_text", "text": text }] }))
}

fn steps(items: &[Item]) -> Vec<(String, String, bool, bool)> {
    items
        .iter()
        .filter_map(|i| match i {
            Item::Step { verb, target, error, done, .. } => Some((verb.clone(), target.clone(), *error, *done)),
            _ => None,
        })
        .collect()
}

#[test]
fn keeps_what_you_typed_and_drops_what_codex_injected() {
    let text = [
        user("<environment_context>cwd</environment_context>"),
        user("# AGENTS.md instructions for D:\\p"),
        user("Fix the login"),
        user("# Context from my IDE setup:\n...\n## My request for Codex:\nAnd the logout"),
        line(serde_json::json!({ "type": "message", "role": "developer", "content": [{ "type": "input_text", "text": "rules" }] })),
    ]
    .join("\n");
    let said: Vec<String> = items(&text).into_iter().filter_map(|i| if let Item::User { text, .. } = i { Some(text) } else { None }).collect();
    assert_eq!(said, ["Fix the login", "And the logout"]);
}

#[test]
fn reads_replies_and_steps_with_their_outcome() {
    let text = [
        user("go"),
        line(serde_json::json!({ "type": "function_call", "name": "shell_command", "call_id": "c1", "arguments": "{\"command\":\"cargo test\\n--all\"}" })),
        line(serde_json::json!({ "type": "function_call_output", "call_id": "c1", "output": "Exit code: 101\nfailed" })),
        line(serde_json::json!({ "type": "custom_tool_call", "name": "apply_patch", "call_id": "c2", "input": "*** Begin Patch\n*** Update File: src/a.rs\n@@\n-a\n+b\n*** Add File: src/new.rs\n+x\n*** End Patch" })),
        line(serde_json::json!({ "type": "custom_tool_call_output", "call_id": "c2", "output": "{\"output\":\"Success\",\"metadata\":{\"exit_code\":0}}" })),
        line(serde_json::json!({ "type": "message", "role": "assistant", "content": [{ "type": "output_text", "text": "Done." }] })),
    ]
    .join("\n");
    let all = items(&text);
    assert_eq!(steps(&all), vec![
        ("Ran".into(), "cargo test".into(), true, true),
        ("Edited".into(), "a.rs".into(), false, true),
        ("Wrote".into(), "new.rs".into(), false, true),
    ]);
    assert!(matches!(all.last(), Some(Item::Text { text, .. }) if text == "Done."));
}

#[test]
fn reads_the_commands_out_of_a_code_mode_cell() {
    let cell = "const r = await tools.exec_command({ cmd: \"npm test\", workdir: \".\" });\nawait tools.exec_command({cmd: 'git status'});";
    let text = [line(serde_json::json!({ "type": "custom_tool_call", "name": "exec", "call_id": "c1", "input": cell })), line(serde_json::json!({ "type": "custom_tool_call_output", "call_id": "c1", "output": "Process exited with code 0" }))].join("\n");
    assert_eq!(steps(&items(&text)), vec![("Ran".into(), "npm test (+1 more)".into(), false, true)]);
}

#[test]
fn a_step_with_no_output_yet_is_still_running() {
    let text = line(serde_json::json!({ "type": "function_call", "name": "shell", "call_id": "c1", "arguments": "{\"command\":[\"powershell.exe\",\"-Command\",\"Get-ChildItem\"]}" }));
    assert_eq!(steps(&items(&text)), vec![("Ran".into(), "Get-ChildItem".into(), false, false)]);
}

/// The newest rollout on this machine, when there is one: the reader must not choke on the real thing.
#[test]
fn reads_a_real_rollout_without_losing_the_conversation() {
    let Some(root) = agent_core::codex::store_root().map(|r| r.join("sessions")) else { return };
    let newest = agent_core::codex::sorted_dirs(&root)
        .into_iter()
        .flat_map(|y| agent_core::codex::sorted_dirs(&y))
        .flat_map(|m| agent_core::codex::sorted_dirs(&m))
        .flat_map(|d| std::fs::read_dir(d).into_iter().flatten().flatten().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "jsonl"))
        .max();
    let Some(path) = newest else { return };
    let all = items(&std::fs::read_to_string(&path).unwrap());
    assert!(all.iter().any(|i| matches!(i, Item::User { .. })), "no user message read from {}", path.display());
}
