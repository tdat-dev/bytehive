//! What a conversation comes down to for another agent: what it was asked,
//! its plan, the files it changed and its last words. Pure, for handoff
//! briefs: a new agent starts with one to pick up where the first stopped.

use std::sync::OnceLock;

use regex::Regex;

use crate::types::{ChatItem, ChatMeta, FileChange, Todo, TodoState};

/// Most of each of your messages carried over; the digest stays readable.
const ASK_CHARS: usize = 700;
const LAST_WORDS_CHARS: usize = 1500;
const MAX_ASKS: usize = 6;
const MAX_FILES: usize = 30;

pub fn clip(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        format!("{}\u{2026}", s.chars().take(n - 1).collect::<String>().trim_end())
    } else {
        s.to_string()
    }
}

fn blank_runs_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\n{3,}").unwrap())
}

pub fn one_block(s: &str) -> String {
    blank_runs_re().replace_all(&s.replace("\r\n", "\n"), "\n\n").trim().to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Digest {
    /// Everything it was asked, in order.
    pub asks: Vec<String>,
    /// The plan as it stands.
    pub todos: Vec<Todo>,
    /// Newest change first.
    pub files: Vec<FileChange>,
    /// Its latest reply.
    pub last_words: String,
}

/// `first_asks`: the start of a long conversation, when `items` holds only its end.
pub fn digest_of(items: &[ChatItem], meta: Option<&ChatMeta>, first_asks: &[String]) -> Digest {
    let later: Vec<String> = items
        .iter()
        .filter_map(|i| match i {
            ChatItem::User(u) => {
                let t = one_block(&u.text);
                if t.is_empty() { None } else { Some(t) }
            }
            _ => None,
        })
        .collect();
    let first: Vec<String> = first_asks.iter().map(|a| one_block(a)).filter(|a| !a.is_empty() && !later.contains(a)).collect();
    let last = items.iter().rev().find_map(|i| match i {
        ChatItem::Text(t) => Some(t.text.trim().to_string()),
        _ => None,
    });
    Digest {
        asks: [first, later].concat(),
        todos: meta.and_then(|m| m.todos.clone()).unwrap_or_default(),
        files: meta.map(|m| m.files.clone()).unwrap_or_default(),
        last_words: last.unwrap_or_default(),
    }
}

/// The first ask says what the task is; the latest ones where it went.
pub fn asks_section(asks: &[String], intro: &str) -> String {
    if asks.is_empty() {
        return String::new();
    }
    let shown: Vec<&String> = if asks.len() > MAX_ASKS {
        let mut v = vec![&asks[0]];
        v.extend(asks[asks.len() - (MAX_ASKS - 1)..].iter());
        v
    } else {
        asks.iter().collect()
    };
    let note = if asks.len() > shown.len() { format!(" (the first, then the latest {} of {})", shown.len() - 1, asks.len()) } else { String::new() };
    let body: Vec<String> = shown.iter().enumerate().map(|(k, a)| format!("{}. {}", k + 1, clip(a, ASK_CHARS).replace('\n', "\n   "))).collect();
    format!("{intro}{note}:\n{}", body.join("\n"))
}

fn mark(state: TodoState) -> &'static str {
    match state {
        TodoState::Completed => "done",
        TodoState::InProgress => "doing",
        TodoState::Pending => "to do",
    }
}

pub fn plan_section(todos: &[Todo], intro: &str) -> String {
    if todos.is_empty() {
        return String::new();
    }
    format!("{intro}\n{}", todos.iter().map(|t| format!("- [{}] {}", mark(t.state), t.text)).collect::<Vec<_>>().join("\n"))
}

/// "3/7 done", or "" when it made no plan.
pub fn plan_progress(todos: &[Todo]) -> String {
    if todos.is_empty() {
        return String::new();
    }
    format!("{}/{} done", todos.iter().filter(|t| t.state == TodoState::Completed).count(), todos.len())
}

pub fn files_section(files: &[FileChange], intro: &str) -> String {
    if files.is_empty() {
        return String::new();
    }
    let shown: Vec<String> = files.iter().take(MAX_FILES).map(|f| format!("- {}{}", f.path, if f.deleted.unwrap_or(false) { " (deleted)" } else if f.is_new { " (new)" } else { "" })).collect();
    let note = if files.len() > MAX_FILES { format!(" (the latest {MAX_FILES} of {})", files.len()) } else { String::new() };
    format!("{intro}{note}:\n{}", shown.join("\n"))
}

pub fn last_words_section(text: &str, intro: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    format!("{intro}\n{}", clip(&one_block(text), LAST_WORDS_CHARS).split('\n').map(|l| format!("> {l}")).collect::<Vec<_>>().join("\n"))
}

#[derive(Debug, Clone)]
pub struct HandoffFrom {
    /// The agent handing over ("Ana").
    pub name: String,
    /// Its CLI ("Claude Code").
    pub cli: String,
    /// Why it is handing over ("it hit its usage limit").
    pub why: String,
    /// What is known it was asked, when there is no conversation to read.
    pub title: Option<String>,
    /// Its first asks, when the chat holds only the end of a long conversation.
    pub first_asks: Vec<String>,
    /// Files git says changed in its worktree: ground truth beside the chat's claims.
    pub git_files: Vec<String>,
}

/// The brief a new agent starts with. Without a readable conversation the
/// brief says what is known and asks the new agent to look at the folder.
pub fn handoff_brief(from: &HandoffFrom, chat: Option<(&[ChatItem], &ChatMeta)>) -> String {
    let (items, meta) = chat.map(|(i, m)| (i, Some(m))).unwrap_or((&[], None));
    let d = digest_of(items, meta, &from.first_asks);
    let mut parts: Vec<String> = Vec::new();
    parts.push(format!(
        "You're taking over a task from {} ({}), another coding agent working in this same folder. It stopped because {}. Its work so far is already in the folder.",
        from.name, from.cli, from.why
    ));
    let asks = asks_section(&d.asks, "What it was asked, in order");
    if !asks.is_empty() {
        parts.push(asks);
    } else if let Some(title) = from.title.as_deref().filter(|t| !t.is_empty()) {
        parts.push(format!("Its task: {title}"));
    }
    parts.push(plan_section(&d.todos, "Its plan when it stopped:"));
    parts.push(files_section(&d.files, "Files it changed"));
    if !from.git_files.is_empty() {
        let shown: Vec<&String> = from.git_files.iter().take(30).collect();
        let more = if from.git_files.len() > shown.len() { format!("\nand {} more", from.git_files.len() - shown.len()) } else { String::new() };
        parts.push(format!(
            "Changed on disk (git, {} file{}):\n{}{more}",
            from.git_files.len(),
            if from.git_files.len() == 1 { "" } else { "s" },
            shown.iter().map(|f| format!("- {f}")).collect::<Vec<_>>().join("\n")
        ));
    }
    parts.push(last_words_section(&d.last_words, "Its last words:"));
    parts.push(if chat.is_some() && (!d.asks.is_empty() || !d.files.is_empty()) {
        "Look at the current state of the files first (git status and the files above), then carry on and finish what's left. Don't redo what's already done, and say briefly what you picked up from before you start.".to_string()
    } else {
        "Its conversation couldn't be read, so look at the folder first (git status, recent changes) to see where it got to, then carry on and finish the task.".to_string()
    });
    parts.iter().filter(|p| !p.is_empty()).cloned().collect::<Vec<_>>().join("\n\n")
}
