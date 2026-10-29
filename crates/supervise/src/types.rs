//! Shared conversation types: what every CLI parser produces and every UI
//! reads. Ported from the TypeScript chat models, same shapes and caps.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffSign {
    Same,
    Added,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffLine {
    pub sign: DiffSign,
    pub text: String,
}

/// A picture in the conversation: a screenshot a tool took, an image file it
/// opened, or one you pasted. Base64, as the transcript keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatImage {
    pub media: String,
    pub data: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoState {
    Pending,
    InProgress,
    Completed,
}

impl TodoState {
    pub fn parse(s: &str) -> TodoState {
        match s {
            "in_progress" => TodoState::InProgress,
            "completed" => TodoState::Completed,
            _ => TodoState::Pending,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Todo {
    pub text: String,
    pub state: TodoState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AskedOption {
    pub label: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AskedQuestion {
    pub question: String,
    pub header: Option<String>,
    pub multi: bool,
    pub options: Vec<AskedOption>,
    /// What you picked (the option's label, or your own words); none while it waits.
    pub answer: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepItem {
    pub id: String,
    pub tool: String,
    /// "Ran", "Edited", "Read"…
    pub verb: String,
    /// What it acted on: a file name, a command, a search.
    pub target: String,
    /// Full path or command, for a tooltip.
    pub full: Option<String>,
    /// The target is code (a file, a command, a pattern), not words.
    pub code: bool,
    /// Tool output, trimmed.
    pub output: Option<String>,
    pub error: bool,
    pub done: bool,
    pub diff: Option<Vec<DiffLine>>,
    pub added: Option<u32>,
    pub removed: Option<u32>,
    pub todos: Option<Vec<Todo>>,
    /// What the step saw: screenshots and images its tool returned.
    pub images: Option<Vec<ChatImage>>,
    /// Questions it asked you, with your answers once you gave them.
    pub questions: Option<Vec<AskedQuestion>>,
    pub at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserItem {
    pub id: String,
    pub text: String,
    pub images: usize,
    pub pics: Option<Vec<ChatImage>>,
    pub at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextItem {
    pub id: String,
    pub text: String,
    pub at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteItem {
    pub id: String,
    pub text: String,
    pub at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatItem {
    User(UserItem),
    Text(TextItem),
    Note(NoteItem),
    Step(StepItem),
}

impl ChatItem {
    pub fn at(&self) -> u64 {
        match self {
            ChatItem::User(i) => i.at,
            ChatItem::Text(i) => i.at,
            ChatItem::Note(i) => i.at,
            ChatItem::Step(i) => i.at,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            ChatItem::User(_) => "user",
            ChatItem::Text(_) => "text",
            ChatItem::Note(_) => "note",
            ChatItem::Step(_) => "step",
        }
    }

    pub fn id(&self) -> &str {
        match self {
            ChatItem::User(i) => &i.id,
            ChatItem::Text(i) => &i.id,
            ChatItem::Note(i) => &i.id,
            ChatItem::Step(i) => &i.id,
        }
    }
}

/// One turn: your message and what the agent did and said until your next one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turn {
    pub start: usize,
    pub end: usize,
    pub took: u64,
}

/// Split items into turns, each starting at one of your messages.
pub fn turns_of(items: &[ChatItem]) -> Vec<Turn> {
    let mut turns: Vec<(usize, usize)> = Vec::new();
    let mut cur: Option<usize> = None;
    for (i, it) in items.iter().enumerate() {
        if it.kind() == "user" || cur.is_none() {
            cur = Some(turns.len());
            turns.push((i, i));
        }
        if let Some(c) = cur {
            turns[c].1 = i;
        }
    }
    turns
        .into_iter()
        .map(|(start, end)| {
            let first = items.get(start).map(|i| i.at()).unwrap_or(0);
            let last = items.get(end).map(|i| i.at()).unwrap_or(0);
            let took = if first > 0 && last > 0 { last.saturating_sub(first) } else { 0 };
            Turn { start, end, took }
        })
        .collect()
}

/// A file the agent changed in this conversation, summed over its edits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub name: String,
    pub added: u32,
    pub removed: u32,
    /// It did not exist until this conversation made it.
    pub is_new: bool,
    /// It was deleted.
    pub deleted: Option<bool>,
    /// The reply that last changed it (ChatMeta.turn counts your messages).
    pub turn: u32,
    /// When it last changed.
    pub at: u64,
    /// The steps that changed it, oldest first: their diffs make up its diff.
    pub steps: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BgState {
    Running,
    Done,
    Failed,
    Stopped,
}

/// A command the agent left running on its own (a dev server, a build, a watch).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BgTask {
    pub id: String,
    /// What it is: its description, else its command.
    pub label: String,
    pub command: Option<String>,
    pub state: BgState,
    /// The file its output goes to, when the CLI says.
    pub output: Option<String>,
    pub at: u64,
}

/// What the conversation says about itself, for the side panel and composer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ChatMeta {
    /// Model of the latest reply ("claude-opus-5-5").
    pub model: Option<String>,
    /// Tokens the model read for its latest reply: roughly how full the context is.
    pub context: u64,
    /// Tokens it has written in this conversation.
    pub output: u64,
    /// How many tokens its context holds, when the conversation says.
    pub window: Option<u64>,
    /// When the conversation began.
    pub started: Option<u64>,
    /// Newest change first.
    pub files: Vec<FileChange>,
    /// The plan as it stands (the latest TodoWrite), or none if it made none.
    pub todos: Option<Vec<Todo>>,
    /// How hard it thinks, as the CLI last said.
    pub effort: Option<String>,
    /// Its permission mode, in the CLI's own words.
    pub permission: Option<String>,
    /// Commands it left running on their own; running ones first.
    pub tasks: Vec<BgTask>,
    /// How many messages you have sent: the reply to the latest one is turn `turn`.
    pub turn: u32,
}

/// What every CLI parser offers: feed transcript text, read items and meta.
pub trait Chat {
    fn items(&self) -> &[ChatItem];
    fn meta(&self) -> &ChatMeta;
    fn feed(&mut self, text: &str);
    /// Keep only the newest `keep` items, forgetting the rest (long sessions).
    fn trim(&mut self, keep: usize);
}
