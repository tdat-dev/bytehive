//! What the Grid's cards say about the agents beside its terminal
//! (src/gridcards.ts).
//!
//! A card is plain text that wraps at any width, so it stays readable where a
//! terminal of the same size would be mush. It says what the agent last said
//! and the step it is on, read from its CLI's own conversation file (the same
//! reading History uses, transcript.rs), a little at a time: the first read
//! takes the end of the file, later ones only what was added. Only Claude's
//! conversations are read here; another CLI's card says what its row says.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex};

use regex::Regex;

use crate::transcript::{Chat, Item};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Glance {
    /// The end of its last reply, as plain text.
    pub said: String,
    /// The step it is on ("Editing billing.ts"), while one is open after that reply.
    pub doing: String,
    /// What you last asked it, while it hasn't answered yet.
    pub asked: String,
}

/// How much of a reply a card keeps: its end, where the conclusion is.
pub const SAID_MAX: usize = 360;
/// The first read of a long conversation starts this far from its end.
const FIRST_READ: u64 = 256 * 1024;

static FENCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"```[^\n]*\n?").unwrap());
static TICKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`([^`]*)`").unwrap());
static BOLD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\*\*([^*]+)\*\*|__([^_]+)__").unwrap());
static HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\s{0,3}#{1,6}\s+").unwrap());
static BULLET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\s*[-*+]\s+").unwrap());
static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\([^)]+\)").unwrap());

/// Markdown to the words it shows: no fences, stars, backticks or heading marks.
pub fn plain(md: &str) -> String {
    let s = FENCE.replace_all(md, "");
    let s = TICKS.replace_all(&s, "$1");
    let s = BOLD.replace_all(&s, "$1$2");
    let s = HEADING.replace_all(&s, "");
    let s = BULLET.replace_all(&s, "• ");
    let s = LINK.replace_all(&s, "$1");
    let lines: Vec<&str> = s.split('\n').map(str::trim_end).collect();
    // a blank line stays only after one with words: never two in a row
    let kept: Vec<&str> = lines.iter().enumerate().filter(|(i, l)| !l.is_empty() || (*i > 0 && !lines[i - 1].is_empty())).map(|(_, l)| *l).collect();
    kept.join("\n").trim().to_string()
}

/// The end of a text, cut at a word, with "…" where it was cut.
pub fn tail(text: &str, max: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max {
        return text.to_string();
    }
    let cut: String = chars[chars.len() - max..].iter().collect();
    let kept = match cut.chars().position(char::is_whitespace) {
        Some(at) if at < 40 => cut.chars().skip(at).collect::<String>(),
        _ => cut,
    };
    format!("…{}", kept.trim_start())
}

/// The chat's verbs say what a step did; a card says what it is doing.
fn now_of(verb: &str) -> Option<&'static str> {
    Some(match verb {
        "Edited" => "Editing",
        "Ran" => "Running",
        "Read" => "Reading",
        "Wrote" => "Writing",
        "Searched" => "Searching",
        "Listed" => "Listing",
        "Looked" => "Looking",
        "Watched" => "Watching",
        "Used" => "Using",
        "Asked" => "Asking",
        "Took" => "Taking",
        "Loaded" => "Loading",
        "Updated" => "Updating",
        "Proposed" => "Proposing",
        _ => return None,
    })
}

/// "Editing invoice.ts" for an open step; a command it described says what it is for.
pub fn doing_of(verb: &str, target: &str, code: bool) -> String {
    if verb == "Ran" && !code {
        return target.to_string();
    }
    let mut words = verb.split(' ');
    let first = words.next().unwrap_or("");
    let mut out: Vec<&str> = vec![now_of(first).unwrap_or(first)];
    out.extend(words);
    out.push(target);
    out.join(" ").trim().to_string()
}

/// What a card says of an agent, from its conversation.
pub fn glance_of(items: &[Item]) -> Glance {
    let mut doing = String::new();
    for it in items.iter().rev() {
        match it {
            Item::Step { verb, target, done, code, .. } => {
                if doing.is_empty() && !done {
                    doing = doing_of(verb, target, *code);
                }
            }
            Item::Text { text, .. } if !text.trim().is_empty() => return Glance { said: tail(&plain(text), SAID_MAX), doing, asked: String::new() },
            // asked and not answered yet: what it is working on is what you asked
            Item::User { text, .. } if !text.trim().is_empty() => return Glance { said: String::new(), doing, asked: tail(&plain(text), 200) },
            _ => {}
        }
    }
    Glance { doing, ..Glance::default() }
}

/// Where a Claude agent's conversation is written, when it has one.
pub fn claude_file(run_dir: &str, session_id: Option<&str>) -> Option<PathBuf> {
    let id = session_id.filter(|id| agent_core::transcript::is_session_id(id))?;
    Some(agent_core::claude::project_dir(run_dir)?.join(format!("{id}.jsonl")))
}

/// One agent to read: which run of it, and its conversation file.
pub struct Source {
    pub id: String,
    pub run: u64,
    pub path: Option<PathBuf>,
}

#[derive(Default)]
struct Feed {
    run: u64,
    chat: Chat,
    offset: u64,
    path: Option<PathBuf>,
    glance: Glance,
}

/// What every card says, shared with the reader that runs off the UI thread.
#[derive(Clone, Default)]
pub struct Glances(Arc<Mutex<HashMap<String, Feed>>>);

impl Glances {
    /// What the card of this agent says now (empty until its first read).
    pub fn get(&self, id: &str) -> Glance {
        self.0.lock().unwrap().get(id).map(|f| f.glance.clone()).unwrap_or_default()
    }

    /// Forget the agents that have no card any more.
    pub fn keep(&self, ids: &HashSet<String>) {
        self.0.lock().unwrap().retain(|id, _| ids.contains(id));
    }

    /// Read what each agent wrote since the last look. True when a card
    /// changes. Blocking: call it off the UI thread.
    pub fn refresh(&self, sources: &[Source]) -> bool {
        let mut changed = false;
        for src in sources {
            let Some(path) = &src.path else { continue };
            let mut feeds = self.0.lock().unwrap();
            let f = feeds.entry(src.id.clone()).or_default();
            if f.run != src.run {
                *f = Feed { run: src.run, ..Feed::default() };
            }
            // another conversation (a /clear, a new session): start it from its end
            if f.path.as_ref().is_some_and(|p| p != path) {
                *f = Feed { run: src.run, path: Some(path.clone()), ..Feed::default() };
                continue;
            }
            f.path = Some(path.clone());
            let Some((text, start, next)) = read_from(path, f.offset) else { continue };
            // the file was replaced or cut: read it again from scratch
            if start < f.offset {
                f.chat = Chat::new();
            }
            f.chat.feed(&text);
            f.offset = next;
            let g = glance_of(&f.chat.items);
            changed |= g != f.glance;
            f.glance = g;
        }
        changed
    }
}

/// Whole lines from `offset` (the end of the file on a first read): the
/// text, where it started and where the next read starts.
fn read_from(path: &PathBuf, offset: u64) -> Option<(String, u64, u64)> {
    use std::io::{Read as _, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let size = file.metadata().ok()?.len();
    let start = agent_core::transcript::read_start(&mut file, size, offset, (offset == 0).then_some(FIRST_READ));
    let mut buf = Vec::new();
    file.seek(SeekFrom::Start(start)).ok()?;
    file.read_to_end(&mut buf).ok()?;
    // a line being written right now waits for the next read
    buf.truncate(buf.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1));
    let next = start + buf.len() as u64;
    Some((String::from_utf8_lossy(&buf).into_owned(), start, next))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(t: &str) -> Item {
        Item::Text { id: t.into(), text: t.into() }
    }
    fn user(t: &str) -> Item {
        Item::User { id: t.into(), text: t.into() }
    }
    fn step(verb: &str, target: &str, done: bool) -> Item {
        Item::Step { id: target.into(), verb: verb.into(), target: target.into(), error: false, done, code: true }
    }
    fn glance(said: &str, doing: &str, asked: &str) -> Glance {
        Glance { said: said.into(), doing: doing.into(), asked: asked.into() }
    }

    #[test]
    fn the_last_reply_and_the_step_after_it() {
        assert_eq!(glance_of(&[user("fix it"), text("Looking at the handler."), step("Edited", "handler.ts", false)]), glance("Looking at the handler.", "Editing handler.ts", ""));
    }

    #[test]
    fn a_step_that_is_done_drops() {
        assert_eq!(glance_of(&[text("Done."), step("Ran", "npm test", true)]).doing, "");
    }

    #[test]
    fn what_you_asked_while_it_has_not_answered() {
        assert_eq!(glance_of(&[text("old reply"), user("now add tests"), step("Read", "a.ts", false)]), glance("", "Reading a.ts", "now add tests"));
    }

    #[test]
    fn a_step_reads_as_what_it_is_doing() {
        assert_eq!(doing_of("Ran", "npm test", true), "Running npm test");
        assert_eq!(doing_of("Ran", "Benchmark indexing", false), "Benchmark indexing");
        assert_eq!(doing_of("Searched for", "TODO", false), "Searching for TODO");
    }

    #[test]
    fn markdown_reads_as_words() {
        assert_eq!(plain("## Result\n\n**Fixed** the `upload` test:\n- one\n- two\n\n```ts\nx()\n```"), "Result\n\nFixed the upload test:\n• one\n• two\n\nx()");
    }

    #[test]
    fn a_long_reply_keeps_its_end_cut_at_a_word() {
        let t = tail(&format!("{}the conclusion.", "word ".repeat(200)), 60);
        assert!(t.starts_with('…'));
        assert!(t.ends_with("the conclusion."));
        assert!(t.chars().count() <= 61);
    }

    #[test]
    fn a_feed_reads_only_what_was_added() {
        let dir = std::env::temp_dir().join(format!("bytehive-gridcards-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("s.jsonl");
        let reply = |t: &str| format!(r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"{t}"}}]}}}}"#);
        std::fs::write(&path, format!("{}\n", reply("first"))).unwrap();
        let g = Glances::default();
        let src = [Source { id: "a".into(), run: 1, path: Some(path.clone()) }];
        assert!(g.refresh(&src));
        assert_eq!(g.get("a").said, "first");
        assert!(!g.refresh(&src));
        std::fs::write(&path, format!("{}\n{}\n", reply("first"), reply("second"))).unwrap();
        assert!(g.refresh(&src));
        assert_eq!(g.get("a").said, "second");
        g.keep(&HashSet::new());
        assert_eq!(g.get("a"), Glance::default());
        std::fs::remove_dir_all(&dir).ok();
    }
}
