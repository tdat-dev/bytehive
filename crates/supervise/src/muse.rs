//! The pure pieces of the Muse parser: its questions, and how a tool
//! call reads in the chat.

use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

use crate::codex::first_line;
use crate::text::{base_name, line_diff, s_at, MAX_DIFF_LINES};
use crate::types::{AskedOption, AskedQuestion, DiffLine, DiffSign};

/// A request_user_input questions array as the chat shows and answers it.
pub fn muse_questions(input: &Value) -> Vec<AskedQuestion> {
    let mut out = Vec::new();
    let list = input.as_array().cloned().unwrap_or_default();
    for q in &list {
        let question = s_at(q, "question");
        let question = if question.is_empty() { s_at(q, "header") } else { question };
        let header = s_at(q, "header");
        let multi = q.get("selection").and_then(|sel| sel.get("mode")).and_then(|m| m.as_str()) == Some("multiple");
        let options: Vec<AskedOption> = q
            .get("options")
            .and_then(|o| o.as_array())
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|o| {
                let description = s_at(o, "description");
                AskedOption {
                    label: s_at(o, "label").to_string(),
                    description: if description.is_empty() { None } else { Some(description.to_string()) },
                }
            })
            .collect();
        out.push(AskedQuestion {
            question: question.to_string(),
            header: if header.is_empty() { None } else { Some(header.to_string()) },
            multi,
            options,
            answer: None,
        });
    }
    out
}

pub fn question_ids(input: &Value) -> Vec<String> {
    input.as_array().cloned().unwrap_or_default().iter().map(|q| s_at(q, "id").to_string()).collect()
}

/// How a Muse tool call reads in the chat: a verb, what it touched, and extras.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MuseDescribed {
    pub tool: String,
    pub verb: String,
    pub target: String,
    pub full: Option<String>,
    pub code: bool,
    pub diff: Option<Vec<DiffLine>>,
    pub added: Option<u32>,
    pub removed: Option<u32>,
    pub questions: Option<Vec<AskedQuestion>>,
}

impl MuseDescribed {
    fn plain(tool: &str, verb: &str, target: String) -> MuseDescribed {
        MuseDescribed { tool: tool.to_string(), verb: verb.to_string(), target, full: None, code: false, diff: None, added: None, removed: None, questions: None }
    }
}

fn image_ext_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\.(png|jpe?g|gif|webp|bmp)$").unwrap())
}

fn mcp_name_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^mcp__(\w+)__(\w+)$").unwrap())
}

fn word_space_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[_-]+").unwrap())
}

/// A URL's host, or the whole string when it is not a URL.
pub fn host_of(url: &str) -> &str {
    let rest = match url.split_once("://") {
        Some((scheme, rest)) if !scheme.is_empty() && !scheme.contains(['/', '?', '#']) => rest,
        _ => return url,
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    if host.is_empty() {
        return url;
    }
    let host = host.rsplit('@').next().unwrap_or(host);
    if let Some(bracketed) = host.strip_prefix('[') {
        match bracketed.split_once(']') {
            Some((inner, _)) if !inner.is_empty() => return &host[..inner.len() + 2],
            _ => return url,
        }
    }
    match host.rsplit_once(':') {
        Some((bare, port)) if !bare.is_empty() && !port.contains(':') => {
            if bare.is_empty() { url } else { bare }
        }
        _ => host,
    }
}

pub fn describe_muse_tool(name: &str, input: &Value) -> MuseDescribed {
    let file = s_at(input, "path");
    let file = if file.is_empty() { s_at(input, "file_path") } else { file };
    match name {
        "powershell" => {
            let cmd = s_at(input, "command");
            let said = s_at(input, "description");
            MuseDescribed {
                tool: name.to_string(),
                verb: "Ran".to_string(),
                target: if said.is_empty() { first_line(cmd, 120) } else { said.to_string() },
                full: Some(cmd.to_string()),
                code: said.is_empty(),
                ..MuseDescribed::plain(name, "", String::new())
            }
        }
        "monitor" => {
            let cmd = s_at(input, "command");
            let said = s_at(input, "description");
            MuseDescribed {
                tool: name.to_string(),
                verb: "Watched".to_string(),
                target: if said.is_empty() { first_line(cmd, 120) } else { said.to_string() },
                full: if cmd.is_empty() { None } else { Some(cmd.to_string()) },
                ..MuseDescribed::plain(name, "", String::new())
            }
        }
        "read_file" => MuseDescribed {
            tool: name.to_string(),
            verb: if image_ext_re().is_match(file) { "Looked at".to_string() } else { "Read".to_string() },
            target: base_name(file).to_string(),
            full: Some(file.to_string()),
            code: true,
            ..MuseDescribed::plain(name, "", String::new())
        },
        "edit_file" => {
            let diff: Vec<DiffLine> = line_diff(s_at(input, "find"), s_at(input, "replace")).into_iter().take(MAX_DIFF_LINES).collect();
            let mut added = 0;
            let mut removed = 0;
            for l in &diff {
                match l.sign {
                    DiffSign::Added => added += 1,
                    DiffSign::Removed => removed += 1,
                    DiffSign::Same => {}
                }
            }
            MuseDescribed {
                tool: name.to_string(),
                verb: "Edited".to_string(),
                target: base_name(file).to_string(),
                full: Some(file.to_string()),
                code: true,
                diff: Some(diff),
                added: Some(added),
                removed: Some(removed),
                ..MuseDescribed::plain(name, "", String::new())
            }
        }
        "write_file" => {
            let content = s_at(input, "content");
            let lines: Vec<&str> = if content.is_empty() { Vec::new() } else { content.split('\n').collect() };
            MuseDescribed {
                tool: name.to_string(),
                verb: "Wrote".to_string(),
                target: base_name(file).to_string(),
                full: Some(file.to_string()),
                code: true,
                diff: Some(lines.iter().take(MAX_DIFF_LINES).map(|text| DiffLine { sign: DiffSign::Added, text: text.to_string() }).collect()),
                added: Some(lines.len() as u32),
                removed: Some(0),
                ..MuseDescribed::plain(name, "", String::new())
            }
        }
        "web_fetch" => {
            let url = s_at(input, "url");
            MuseDescribed {
                tool: name.to_string(),
                verb: "Read".to_string(),
                target: host_of(url).to_string(),
                full: Some(url.to_string()),
                ..MuseDescribed::plain(name, "", String::new())
            }
        }
        "web_search" => MuseDescribed::plain(name, "Searched the web for", s_at(input, "query").to_string()),
        "read_skill" => MuseDescribed::plain(name, "Used the skill", s_at(input, "name").to_string()),
        "request_user_input" => {
            let questions = muse_questions(input.get("questions").unwrap_or(&Value::Null));
            let target = match questions.first() {
                Some(first) => match first.header.as_deref().filter(|h| !h.is_empty()) {
                    Some(header) => header.to_string(),
                    None => first_line(&first.question, 120),
                },
                None => "a question".to_string(),
            };
            MuseDescribed {
                tool: name.to_string(),
                verb: "Asked you".to_string(),
                target,
                questions: Some(questions),
                ..MuseDescribed::plain(name, "", String::new())
            }
        }
        "subagent_spawn" => {
            let task = s_at(input, "task_name");
            MuseDescribed::plain(name, "Asked a helper agent to", if task.is_empty() { first_line(s_at(input, "objective"), 120) } else { task.to_string() })
        }
        "image_generation" => MuseDescribed::plain(name, "Generated an image", first_line(s_at(input, "prompt"), 80)),
        _ => match mcp_name_re().captures(name) {
            Some(m) => {
                let server = word_space_re().replace_all(&m[1], " ");
                let tool = m[2].replace('_', " ");
                MuseDescribed::plain(name, "Used", format!("{server} \u{b7} {tool}"))
            }
            None => MuseDescribed::plain(name, "Used", name.to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_common_tools_like_the_chat() {
        let d = describe_muse_tool("powershell", &json!({ "command": "ls foo", "description": "List foo" }));
        assert_eq!((d.verb.as_str(), d.target.as_str()), ("Ran", "List foo"));
        let d = describe_muse_tool("powershell", &json!({ "command": "ls foo" }));
        assert_eq!((d.verb.as_str(), d.target.as_str(), d.code), ("Ran", "ls foo", true));
        let d = describe_muse_tool("write_file", &json!({ "path": "D:/a/b.md", "content": "x\ny" }));
        assert_eq!((d.verb.as_str(), d.target.as_str()), ("Wrote", "b.md"));
        assert_eq!(d.added, Some(2));
        let d = describe_muse_tool("subagent_spawn", &json!({ "task_name": "Scout" }));
        assert_eq!(d.target, "Scout");
        let d = describe_muse_tool("mcp__bytehive__card_add", &json!({}));
        assert_eq!((d.verb.as_str(), d.target.as_str()), ("Used", "bytehive \u{b7} card add"));
        let d = describe_muse_tool("something_new", &json!({}));
        assert_eq!((d.verb.as_str(), d.target.as_str()), ("Used", "something_new"));
    }

    #[test]
    fn reads_questions_with_multi_select() {
        let q = muse_questions(&json!([{ "id": "a", "header": "H", "question": "Q?", "selection": { "mode": "multiple" }, "options": [{ "label": "L" }] }]));
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].question, "Q?");
        assert_eq!(q[0].header.as_deref(), Some("H"));
        assert_eq!(q[0].multi, true);
        assert_eq!(q[0].options.len(), 1);
        assert_eq!(q[0].options[0].description, None);
        assert!(muse_questions(&Value::Null).is_empty());
    }

    #[test]
    fn host_of_reads_hosts_and_keeps_bare_strings() {
        assert_eq!(host_of("https://example.com/docs?a=1"), "example.com");
        assert_eq!(host_of("http://localhost:3000/x"), "localhost");
        assert_eq!(host_of("not a url"), "not a url");
        assert_eq!(host_of("example.com/x"), "example.com/x");
    }
}
