//! How a tool call reads in the chat: a verb and what it touched, worded as
//! `describeTool` in src/chatmodel.ts words it.

use serde_json::Value;

/// A string field of a JSON object, or "".
pub fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

/// The last part of a path.
pub fn base_name(p: &str) -> String {
    let t = p.trim_end_matches(['\\', '/']);
    match t.rsplit(['\\', '/']).next() {
        Some(last) if !last.is_empty() => last.to_string(),
        _ => p.to_string(),
    }
}

/// The first line of `s`, cut to `max` characters.
pub fn first_line(s: &str, max: usize) -> String {
    let line = s.trim().lines().next().unwrap_or("").trim_end_matches('\r');
    if line.chars().count() > max {
        let mut out: String = line.chars().take(max - 1).collect();
        out.push('…');
        out
    } else {
        line.to_string()
    }
}

/// The host of a URL ("docs.rs"), or the URL as given when it has none.
fn host_of(url: &str) -> String {
    let Some((_, rest)) = url.split_once("://") else { return url.to_string() };
    let auth = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = auth.rsplit('@').next().unwrap_or(auth);
    if host.is_empty() { url.to_string() } else { host.to_lowercase() }
}

/// `mcp__<server>__<tool>` split into server and tool, as /^mcp__(.+?)__(.+)$/ does.
fn mcp_parts(name: &str) -> Option<(&str, &str)> {
    let rest = name.strip_prefix("mcp__")?;
    let mut from = 1;
    while from < rest.len() {
        let i = from + rest.get(from..)?.find("__")?;
        if i + 2 < rest.len() {
            return Some((&rest[..i], &rest[i + 2..]));
        }
        from = i + 1;
    }
    None
}

/// Runs of `_` and `-` as one space.
fn spaced(s: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in s.chars() {
        if c == '_' || c == '-' {
            if !gap {
                out.push(' ');
            }
            gap = true;
        } else {
            out.push(c);
            gap = false;
        }
    }
    out
}

fn is_image(file: &str) -> bool {
    let f = file.to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp"].iter().any(|e| f.ends_with(e))
}

/// The verb and target of one tool call: ("Read", "main.rs"), ("Ran", "npm test").
pub fn describe(name: &str, input: &Value) -> (String, String) {
    let file = [s(input, "file_path"), s(input, "notebook_path"), s(input, "path")]
        .into_iter()
        .find(|f| !f.is_empty())
        .unwrap_or("");
    let or = |a: &str, b: String| if a.is_empty() { b } else { a.to_string() };
    let (verb, target) = match name {
        "Bash" | "PowerShell" => ("Ran", or(s(input, "description"), first_line(s(input, "command"), 120))),
        "AskUserQuestion" => {
            let first = input.get("questions").and_then(Value::as_array).and_then(|q| q.first());
            let target = match first {
                Some(q) => or(s(q, "header"), first_line(s(q, "question"), 120)),
                None => "a question".to_string(),
            };
            ("Asked you", target)
        }
        "Monitor" => ("Watched", or(s(input, "description"), first_line(s(input, "command"), 120))),
        "Read" => (if is_image(file) { "Looked at" } else { "Read" }, base_name(file)),
        "Edit" | "MultiEdit" | "NotebookEdit" => ("Edited", base_name(file)),
        "Write" => ("Wrote", base_name(file)),
        "Grep" => ("Searched for", s(input, "pattern").to_string()),
        "Glob" => ("Looked for files", s(input, "pattern").to_string()),
        "LS" => ("Listed", base_name(file)),
        "WebSearch" => ("Searched the web for", s(input, "query").to_string()),
        "WebFetch" => ("Read", host_of(s(input, "url"))),
        "Task" | "Agent" => ("Asked a helper agent to", or(s(input, "description"), first_line(s(input, "prompt"), 120))),
        "TodoWrite" => {
            let todos = input.get("todos").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
            let done = todos.iter().filter(|t| s(t, "status") == "completed").count();
            ("Updated the plan", format!("{done} of {} done", todos.len()))
        }
        "Skill" => ("Used the skill", or(s(input, "skill"), s(input, "command").to_string())),
        "ToolSearch" => ("Loaded tools", first_line(s(input, "query"), 60)),
        "ExitPlanMode" => ("Proposed a plan", String::new()),
        _ => match mcp_parts(name) {
            // A browser or desktop screenshot, whichever server took it.
            Some((server, tool)) if s(input, "action") == "screenshot" || {
                let t = tool.to_ascii_lowercase();
                t.contains("screenshot") || t.contains("snapshot")
            } => {
                let server = match server.strip_prefix("plugin_").and_then(|r| r.split_once('_')) {
                    Some((plugin, rest)) if !plugin.is_empty() => rest,
                    _ => server,
                };
                ("Took a screenshot", spaced(server))
            }
            Some((server, tool)) => ("Used", format!("{} · {}", spaced(server), tool.replace('_', " "))),
            None => ("Used", name.to_string()),
        },
    };
    (verb.to_string(), target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn verbs_and_targets() {
        let d = |n: &str, v: Value| describe(n, &v);
        assert_eq!(d("Read", json!({"file_path": "D:\\x\\src\\main.rs"})), ("Read".into(), "main.rs".into()));
        assert_eq!(d("Read", json!({"file_path": "/a/shot.PNG"})), ("Looked at".into(), "shot.PNG".into()));
        assert_eq!(d("Bash", json!({"command": "npm test\nmore"})), ("Ran".into(), "npm test".into()));
        assert_eq!(d("Bash", json!({"command": "x", "description": "Run tests"})), ("Ran".into(), "Run tests".into()));
        assert_eq!(d("WebFetch", json!({"url": "https://Docs.rs/serde?x=1"})), ("Read".into(), "docs.rs".into()));
        assert_eq!(d("TodoWrite", json!({"todos": [{"status": "completed"}, {"status": "pending"}]})), ("Updated the plan".into(), "1 of 2 done".into()));
        assert_eq!(d("mcp__playwright__browser_take_screenshot", json!({})), ("Took a screenshot".into(), "playwright".into()));
        assert_eq!(d("mcp__plugin_figma_figma__get_screenshot", json!({})), ("Took a screenshot".into(), "figma".into()));
        assert_eq!(d("mcp__bytehive__card_add", json!({})), ("Used".into(), "bytehive · card add".into()));
        assert_eq!(d("Mystery", json!({})), ("Used".into(), "Mystery".into()));
    }

    #[test]
    fn long_first_line_is_cut() {
        let long = "a".repeat(130);
        let got = first_line(&long, 120);
        assert_eq!(got.chars().count(), 120);
        assert!(got.ends_with('…'));
    }
}
