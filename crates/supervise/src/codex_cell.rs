//! What a Codex JS cell does: the tools it calls, the commands it runs, a patch it applies.

use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

use crate::text::s;

/// Tool arguments: an object as-is, else the JSON it holds, else nothing.
pub fn parse_args(v: &Value) -> Value {
    if v.is_object() {
        return v.clone();
    }
    match s(v) {
        "" => Value::Null,
        text => serde_json::from_str(text).ok().filter(|o: &Value| o.is_object()).unwrap_or(Value::Null),
    }
}

fn unescape_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"\\(["'`\\])"#).unwrap())
}

fn newline_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\\n").unwrap())
}

/// A JS string literal's value ("…", '…' or `…`).
pub fn literal(text: &str) -> String {
    if text.starts_with('"') {
        if let Ok(val) = serde_json::from_str::<String>(text) {
            return val;
        }
    }
    let inner = text.get(1..text.len().saturating_sub(1)).unwrap_or("");
    let step = unescape_re().replace_all(inner, "$1");
    newline_re().replace_all(&step, "\n").to_string()
}

fn cell_tools_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\btools\.(\w+)\s*\(").unwrap())
}

fn cell_cmd_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?s)\b(?:cmd|command)["']?\s*:\s*("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|`(?:[^`\\]|\\.)*`)"#).unwrap()
    })
}

fn view_path_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"(?s)path\s*:\s*("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*')"#).unwrap())
}

/// What a JS cell does: the tools it calls, the commands it runs, a patch it applies.
pub struct CellInfo {
    pub tools: Vec<String>,
    pub commands: Vec<String>,
    pub patch: Option<String>,
}

pub fn cell_info(code: &str) -> CellInfo {
    let mut tools: Vec<String> = Vec::new();
    for m in cell_tools_re().captures_iter(code) {
        let t = m[1].to_string();
        if !tools.contains(&t) {
            tools.push(t);
        }
    }
    let commands: Vec<String> = cell_cmd_re().captures_iter(code).map(|m| literal(&m[1])).collect();
    let mut patch = None;
    if let Some(a) = code.find("*** Begin Patch") {
        if let Some(b) = code[a..].find("*** End Patch") {
            patch = Some(code[a..a + b + "*** End Patch".len()].to_string());
        }
    }
    // A patch inside a JSON string keeps its line breaks escaped.
    if let Some(p) = patch {
        if !p.contains('\n') && p.contains("\\n") {
            patch = Some(p.replace("\\n", "\n").replace("\\\"", "\"").replace("\\\\", "\\"));
        } else {
            patch = Some(p);
        }
    }
    CellInfo { tools, commands, patch }
}

/// The file a JS cell looks at with view_image, if the cell names one.
pub fn view_image_path(code: &str) -> Option<String> {
    view_path_re().captures(code).map(|m| literal(&m[1]))
}
