//! Reads what an agent CLI is asking, from the text on its screen.
//!
//! Every CLI draws its questions the same basic way: a prompt line, then
//! numbered options with one marked as selected, then a hint about which keys
//! to press. This tells the cases apart (wants to run something, asked a
//! question, or nothing to answer) and says which keys answer each option, so
//! a decision can be made from a button instead of from inside the TUI.
//!
//! Pure: takes the plain-text screen, no DOM.

use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AskKind {
    Run,
    Edit,
    Question,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AskOption {
    /// Number shown on screen (1-based).
    pub n: u32,
    pub label: String,
    /// Bytes that pick this option in the CLI.
    pub key: String,
    /// The "No / tell it what to do differently" option.
    pub deny: bool,
    /// "Yes, and don't ask again" style options.
    pub always: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ask {
    pub kind: AskKind,
    /// The question itself ("Do you want to proceed?", or the agent's question).
    pub prompt: String,
    /// Short header when the CLI shows one ("Bash command", Claude's "[ ] Topic").
    pub title: Option<String>,
    /// The command or file the agent wants to touch, when it is on screen.
    pub detail: Option<String>,
    /// Options the user can pick, without the CLI's own "type something" / "chat" rows.
    pub options: Vec<AskOption>,
    /// Option number that lets the user type a free answer, when the CLI offers one.
    pub free_text_option: Option<u32>,
}

pub const ESC: &str = "\x1b";
const UP: &str = "\x1b[A";
const DOWN: &str = "\x1b[B";

// Box drawing and selection glyphs that wrap every line of a boxed prompt.
fn box_edge_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("^[\u{0020}\u{0009}\u{2502}\u{2503}|\u{256D}\u{256E}\u{2570}\u{256F}\u{2500}\u{2501}]+|[\u{0020}\u{0009}\u{2502}\u{2503}|\u{256D}\u{256E}\u{2570}\u{256F}]+$").unwrap())
}

// "❯ 1. Yes", "  2. No, and tell Claude … (esc)", "› 1. Yes, proceed (y)", "● 1. Yes, allow once", "> 1. Keep"
fn option_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("^[\u{276F}\u{203A}\u{25CF}>\u{25B8}]?\\s*(\\d{1,2})\\s*[.)]\\s*(.+?)\\s*$").unwrap())
}

// Hints only a live selector draws. A numbered list in the agent's prose has none of these.
fn selector_hint_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("(?i)enter to (select|confirm)|esc to cancel|\u{2191}/\u{2193} to navigate|press enter to confirm").unwrap())
}

fn prompt_hint_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("(?i)do you want to|would you like to|allow execution|allow (this|the following)|proceed\\?").unwrap())
}

fn clean(line: &str) -> String {
    box_edge_re().replace_all(line, "").trim_end().to_string()
}

/// The key that picks option `n`: an explicit single-key shortcut shown in
/// parentheses wins ("(y)", "(esc)"), otherwise the option number.
fn key_for(n: u32, label: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(?i)\((esc|[a-z])\)\s*$").unwrap());
    match re.captures(label) {
        Some(m) => {
            let k = m[1].to_lowercase();
            if k == "esc" { ESC.to_string() } else { k }
        }
        None => n.to_string(),
    }
}

fn strip_shortcut(label: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(?i)\s*\((esc|[a-z]|shift\+tab)\)\s*$").unwrap());
    re.replace(label, "").trim().to_string()
}

struct OptionBlock {
    start: usize,
    end: usize,
    opts: Vec<(u32, String)>,
}

/// The last block of consecutive-ish numbered options on screen, with the
/// line index where it starts. Description lines between options are allowed.
fn last_option_block(lines: &[String]) -> Option<OptionBlock> {
    let mut end: Option<usize> = None;
    for (i, line) in lines.iter().enumerate().rev() {
        if option_re().is_match(line) {
            end = Some(i);
            break;
        }
    }
    let end = end?;
    let mut found: Vec<(u32, String, usize)> = Vec::new();
    let mut expect: Option<u32> = None;
    let floor = end.saturating_sub(40);
    for i in (floor..=end).rev() {
        let m = match option_re().captures(&lines[i]) {
            Some(m) => m,
            None => continue,
        };
        let n: u32 = m[1].parse().unwrap_or(0);
        if let Some(e) = expect {
            if n != e {
                break;
            }
        }
        found.insert(0, (n, m[2].to_string(), i));
        expect = Some(n.saturating_sub(1));
        if n == 1 {
            break;
        }
    }
    if found.is_empty() || found[0].0 != 1 || found.len() < 2 {
        return None;
    }
    let start = found[0].2;
    Some(OptionBlock { start, end, opts: found.into_iter().map(|(n, label, _)| (n, label)).collect() })
}

fn cursor_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("^[\u{276F}\u{203A}>\u{25B8}]\\s+(.+)$").unwrap())
}

fn question_mark_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(.*?\?)(\s|$)").unwrap())
}

/// A menu you move through with the arrow keys and no numbers, like Claude's
/// "Yes, I trust this folder" screen: the lines right above a selector hint,
/// one of them marked with the cursor. Each option's key walks the cursor to it
/// and presses Enter.
fn arrow_menu(lines: &[String]) -> Option<Ask> {
    // The hint must be the last thing on screen; anything below it means the
    // agent has moved on and the menu is history.
    let mut hint = lines.len() as isize - 1;
    while hint >= 0 && lines[hint as usize].trim().is_empty() {
        hint -= 1;
    }
    if hint < 0 || !selector_hint_re().is_match(&lines[hint as usize]) {
        return None;
    }
    let mut end = hint - 1;
    while end >= 0 && lines[end as usize].trim().is_empty() {
        end -= 1;
    }
    let mut start = end;
    while start > 0 && !lines[(start - 1) as usize].trim().is_empty() {
        start -= 1;
    }
    if end < 0 || start < 0 {
        return None;
    }
    let rows: Vec<&String> = lines[start as usize..=end as usize].iter().collect();
    let cursors: Vec<usize> = rows.iter().enumerate().filter(|(_, l)| cursor_re().is_match(l)).map(|(i, _)| i).collect();
    if rows.len() < 2 || rows.len() > 8 || cursors.len() != 1 || rows.iter().any(|l| l.chars().count() > 90) {
        return None;
    }
    let cursor = cursors[0];
    let options: Vec<AskOption> = rows
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let label = strip_shortcut(cursor_re().replace(l, "$1").trim());
            let step = i as isize - cursor as isize;
            let walk = if step < 0 { UP } else { DOWN }.repeat(step.unsigned_abs());
            AskOption {
                n: (i + 1) as u32,
                label: label.clone(),
                key: format!("{walk}\r"),
                deny: deny_re().is_match(&label),
                always: false,
            }
        })
        .collect();
    // The question: the nearest line above that asks something, cut at the "?".
    let mut prompt = String::new();
    let floor = (start - 14).max(0);
    for i in (floor..start).rev() {
        if let Some(m) = question_mark_re().captures(&lines[i as usize]) {
            prompt = m[1].trim().to_string();
            break;
        }
    }
    if prompt.is_empty() {
        prompt = lines[..start as usize].iter().rev().find(|l| !l.trim().is_empty()).map(|l| l.trim().to_string()).unwrap_or_default();
    }
    Some(Ask { kind: AskKind::Question, prompt, title: None, detail: None, options, free_text_option: None })
}

fn deny_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^no\b").unwrap())
}

fn always_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^yes,? .*(don't|do not) ask again|^yes,? allow (always|all)").unwrap())
}

fn chip_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("\\[\\s?[ x\u{2714}]?\\s?\\]\\s*(.+?)\\s*$").unwrap())
}

fn rule_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("^[\u{2500}\u{2501}\\s]+$").unwrap())
}

fn indented_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\s{2,}\S").unwrap())
}

fn bare_cursor_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("^[\u{003E}\u{203A}\u{276F}]\\s*$").unwrap())
}

fn esc_suffix_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?mi)\(esc\)\s*$").unwrap())
}

fn is_edit_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)do you want to (make this edit|create|write|delete|overwrite)").unwrap())
}

fn is_run_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)do you want to proceed|would you like to run|allow execution|run the following").unwrap())
}

fn shell_head_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?mi)^(bash|shell|powershell) command$").unwrap())
}

fn dollar_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\s*\$\s+\S").unwrap())
}

fn dollar_strip_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\s*\$\s+").unwrap())
}

fn gemini_shell_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\s*\?\s+Shell\s+(.+?)(\s+\[.*\])?(\s+\(.*\))?\s*$").unwrap())
}

fn edit_of_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(?:edit to|create|write to|delete|overwrite)\s+([^\s?]+)\??").unwrap())
}

fn shell_head_line_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^\s*(bash|shell|powershell) command\s*$").unwrap())
}

fn type_something_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^type something\.?$").unwrap())
}

fn chat_about_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^chat about this\.?$").unwrap())
}

/// What the agent is asking right now, or none when there is nothing to
/// answer. `screen` is the visible text, newest line last.
pub fn parse_ask(screen: &str) -> Option<Ask> {
    let lines: Vec<String> = screen.split('\n').map(|l| clean(l.strip_suffix('\r').unwrap_or(l))).collect();
    let block = match last_option_block(&lines) {
        Some(b) => b,
        None => return arrow_menu(&lines),
    };

    // Everything after the options must be selector chrome, not new conversation:
    // if the agent kept talking below the list, the list is prose, not a prompt.
    let after: Vec<&String> = lines[block.end + 1..].iter().filter(|l| !l.trim().is_empty()).collect();
    let before: Vec<&String> = lines[block.start.saturating_sub(14)..block.start].iter().collect();
    let labels = block.opts.iter().map(|(_, l)| l.as_str()).collect::<Vec<_>>().join("\n");
    let hinted = after.iter().any(|l| selector_hint_re().is_match(l)) || esc_suffix_re().is_match(&labels) || before.iter().any(|l| prompt_hint_re().is_match(l));
    if !hinted {
        return None;
    }
    let tail_is_chrome = after.iter().all(|l| {
        selector_hint_re().is_match(l) || rule_re().is_match(l) || option_re().is_match(l) || indented_re().is_match(l) || bare_cursor_re().is_match(l)
    });
    if !tail_is_chrome {
        return None;
    }

    // The prompt is the permission question when there is one ("Would you like to
    // run the following command?" sits above the command in Codex), otherwise the
    // nearest non-empty line above the options.
    let floor = block.start.saturating_sub(14);
    let mut prompt_idx: Option<usize> = None;
    for i in (floor..block.start).rev() {
        if prompt_hint_re().is_match(&lines[i]) {
            prompt_idx = Some(i);
            break;
        }
    }
    if prompt_idx.is_none() {
        for i in (floor..block.start).rev() {
            if !lines[i].trim().is_empty() && !rule_re().is_match(&lines[i]) {
                prompt_idx = Some(i);
                break;
            }
        }
    }
    let prompt = match prompt_idx {
        Some(i) => lines[i].trim().strip_prefix('?').unwrap_or(lines[i].trim()).trim_start().to_string(),
        None => String::new(),
    };
    // TS strips a leading "?" plus following spaces: /^[?]\s*/ — strip_prefix('?') then trim_start matches.

    // Claude draws a header chip above its questions: "──── [ ] Topic".
    let mut title: Option<String> = None;
    if let Some(pi) = prompt_idx {
        let chip_floor = pi.saturating_sub(3);
        for i in (chip_floor..pi).rev() {
            if let Some(m) = chip_re().captures(&lines[i]) {
                title = Some(m[1].to_string());
                break;
            }
        }
    }

    // Classify. A permission prompt asks yes/no about an action; anything else is a question.
    let context = before.iter().map(|l| l.as_str()).collect::<Vec<_>>().join("\n");
    let both = format!("{prompt}\n{context}");
    let is_edit = is_edit_re().is_match(&both);
    let is_run = !is_edit && (is_run_re().is_match(&both) || shell_head_re().is_match(&context));
    let kind = if is_edit { AskKind::Edit } else if is_run { AskKind::Run } else { AskKind::Question };

    // What it wants to touch: Codex "$ cmd", Gemini "? Shell cmd […]]", Claude's indented line under "Bash command".
    let mut detail: Option<String> = None;
    if kind != AskKind::Question {
        let ctx: Vec<&String> = lines[block.start.saturating_sub(14)..block.start].iter().collect();
        let dollar = ctx.iter().find(|l| dollar_re().is_match(l));
        let shell = ctx.iter().filter_map(|l| gemini_shell_re().captures(l)).next();
        let edit_of = edit_of_re().captures(&both);
        if let Some(d) = dollar {
            detail = Some(dollar_strip_re().replace(d, "").trim().to_string());
        } else if let Some(m) = shell {
            detail = Some(m[1].trim().to_string());
        } else if kind == AskKind::Edit {
            if let Some(m) = edit_of {
                detail = Some(m[1].to_string());
            }
        } else {
            let head = ctx.iter().position(|l| shell_head_line_re().is_match(l));
            if let Some(h) = head {
                if let Some(first) = ctx[h + 1..].iter().find(|l| !l.trim().is_empty()) {
                    detail = Some(first.trim().to_string());
                }
            }
        }
    }

    let mut free_text_option: Option<u32> = None;
    let mut options: Vec<AskOption> = Vec::new();
    for (n, original) in &block.opts {
        let label = strip_shortcut(original);
        if type_something_re().is_match(&label) {
            free_text_option = Some(*n);
            continue;
        }
        if chat_about_re().is_match(&label) {
            continue;
        }
        options.push(AskOption { n: *n, label: label.clone(), key: key_for(*n, original), deny: deny_re().is_match(&label), always: always_re().is_match(&label) });
    }
    if options.is_empty() {
        return None;
    }

    Some(Ask { kind, prompt, title, detail, options, free_text_option })
}

/// The bytes to send for a free-text answer: pick the CLI's "type something"
/// row when it has one, then the text, then Enter.
pub fn free_text_keys(ask: Option<&Ask>, text: &str) -> Vec<String> {
    static NL: OnceLock<Regex> = OnceLock::new();
    let nl = NL.get_or_init(|| Regex::new(r"\r?\n").unwrap());
    let body = nl.replace_all(text, " ").trim().to_string();
    match ask.and_then(|a| a.free_text_option) {
        Some(n) => vec![n.to_string(), body, "\r".to_string()],
        None => vec![body, "\r".to_string()],
    }
}

