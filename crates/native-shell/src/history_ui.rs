//! History panel: an agent's conversation, read from its CLI transcript and
//! parsed into the same items every chat view shares. Reads run off the UI
//! thread; the panel re-parses only when the text changed.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};

use agent_core::{claude, codex, muse, opencode, TranscriptChunk};
use eframe::egui;
use supervise::types::{Chat, ChatItem, ChatMeta};

use crate::sessions_ui::Cli;
use crate::states::pane_cwd;
use crate::term_pane::Pane;
use crate::theme::{DEL, DIM, FAINT, YOU};

/// Only the end of a long conversation is read: enough to see where it got to.
const TAIL_BYTES: u64 = 256 * 1024;

pub fn read_history(cli: Cli, dir: &str, session: Option<&str>) -> TranscriptChunk {
    match cli {
        Cli::Claude => claude::transcript(dir, session, None, 0, Some(TAIL_BYTES)),
        Cli::Codex => codex::sessions_root().map_or_else(TranscriptChunk::default, |r| codex::codex_transcript_in(&r, dir, None, None, 0, Some(TAIL_BYTES))),
        Cli::Muse => muse::transcript(session, 0, Some(TAIL_BYTES)),
        Cli::Opencode => opencode::db_path().map_or_else(TranscriptChunk::default, |db| opencode::opencode_transcript_in(&db, dir, None, session, 0)),
    }
}

/// Transcript text to conversation items, with the CLI's own parser.
pub fn parse_history(cli: Cli, text: &str) -> (Vec<ChatItem>, ChatMeta) {
    let mut chat: Box<dyn Chat> = match cli {
        Cli::Claude => Box::new(supervise::claude::ClaudeChat::new()),
        Cli::Codex => Box::new(supervise::codex_chat::CodexChat::new()),
        Cli::Muse => Box::new(supervise::muse_chat::MuseChat::new()),
        Cli::Opencode => Box::new(supervise::opencode_chat::OpencodeChat::new()),
    };
    chat.feed(text);
    (chat.items().to_vec(), chat.meta().clone())
}

struct HistoryCache {
    text: String,
    items: Vec<ChatItem>,
    meta: ChatMeta,
}

/// Whether picking this agent should kick off a transcript read: a forced
/// Refresh always does; otherwise only when nothing is cached yet.
pub fn needs_load(has_cache: bool, changed: bool, force: bool) -> bool {
    force || (changed && !has_cache)
}

enum HistoryMsg {
    Chunk(String, TranscriptChunk),
}

pub struct HistoryPanel {
    selected: Option<String>,
    cache: HashMap<String, HistoryCache>,
    loading: bool,
    tx: Sender<HistoryMsg>,
    rx: Receiver<HistoryMsg>,
}

impl HistoryPanel {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        HistoryPanel { selected: None, cache: HashMap::new(), loading: false, tx, rx }
    }

    fn request(&mut self, pane: &Pane) {
        let (Some(cli), session) = (pane.cli, pane.session_id.clone()) else { return };
        self.loading = true;
        let tx = self.tx.clone();
        let id = pane.id.clone();
        let dir = pane_cwd(pane);
        std::thread::spawn(move || {
            let chunk = read_history(cli, &dir, session.as_deref());
            let _ = tx.send(HistoryMsg::Chunk(id, chunk));
        });
    }

    pub fn show(&mut self, ui: &mut egui::Ui, panes: &[Pane]) {
        ui.label(egui::RichText::new("History").strong().size(15.0));
        let agents: Vec<&Pane> = panes.iter().filter(|p| p.cli.is_some()).collect();
        if agents.is_empty() {
            ui.label(egui::RichText::new("No agent panes yet. Plain shells have no transcript.").color(FAINT));
            return;
        }
        let before = self.selected.clone();
        if self.selected.as_ref().is_none_or(|id| agents.iter().all(|p| &p.id != id)) {
            self.selected = Some(agents[0].id.clone());
        }
        ui.horizontal_wrapped(|ui| {
            for p in &agents {
                let sel = self.selected.as_deref() == Some(p.id.as_str());
                if ui.selectable_label(sel, &p.name).clicked() {
                    self.selected = Some(p.id.clone());
                }
            }
        });
        let changed = self.selected != before;
        while let Ok(HistoryMsg::Chunk(id, chunk)) = self.rx.try_recv() {
            self.loading = false;
            let same = self.cache.get(&id).map(|c| c.text == chunk.text).unwrap_or(false);
            if same {
                continue;
            }
            if let Some(pane) = panes.iter().find(|p| p.id == id) {
                if let Some(cli) = pane.cli {
                    let (items, meta) = parse_history(cli, &chunk.text);
                    self.cache.insert(id, HistoryCache { text: chunk.text, items, meta });
                }
            }
        }
        let pane_id = self.selected.clone().unwrap_or_default();
        let pane = panes.iter().find(|p| p.id == pane_id);
        let mut force = false;
        ui.horizontal(|ui| {
            if ui.small_button("Refresh").clicked() {
                if pane.is_some() {
                    force = true;
                }
            }
            if self.loading {
                ui.label(egui::RichText::new("reading…").small().color(FAINT));
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
            }
        });
        if needs_load(self.cache.contains_key(&pane_id), changed, force) {
            if let Some(p) = pane {
                self.request(p);
            }
        }
        ui.separator();
        let Some(entry) = self.cache.get(&pane_id) else {
            if !self.loading {
                ui.label(egui::RichText::new("Nothing read yet. Press Refresh.").color(FAINT));
            }
            return;
        };
        if !entry.meta.todos.clone().unwrap_or_default().is_empty() {
            let todos = entry.meta.todos.clone().unwrap_or_default();
            let done = todos.iter().filter(|t| t.state == supervise::types::TodoState::Completed).count();
            ui.label(egui::RichText::new(format!("Plan: {done}/{} done", todos.len())).small().strong());
        }
        egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
            let items = entry.items.iter().rev().take(300).collect::<Vec<_>>();
            for it in items.into_iter().rev() {
                match it {
                    ChatItem::User(u) => {
                        ui.label(egui::RichText::new("You").small().strong().color(YOU));
                        ui.label(egui::RichText::new(&u.text).small());
                    }
                    ChatItem::Text(t) => {
                        ui.label(egui::RichText::new(&t.text).small());
                    }
                    ChatItem::Step(s) => {
                        let mark = if s.error { "✕" } else if s.done { "✓" } else { "…" };
                        let color = if s.error { DEL } else { DIM };
                        ui.label(egui::RichText::new(format!("{mark} {} {}", s.verb, s.target)).small().color(color));
                        if let Some(out) = s.output.as_deref() {
                            let head: String = out.lines().take(6).collect::<Vec<_>>().join("\n");
                            ui.label(egui::RichText::new(head).small().monospace().color(FAINT));
                        }
                    }
                    ChatItem::Note(n) => {
                        ui.label(egui::RichText::new(&n.text).small().italics().color(FAINT));
                    }
                }
                ui.separator();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_claude_lines_to_items() {
        let text = "{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"hi\"}]}}\n{\"type\":\"assistant\",\"message\":{\"id\":\"m1\",\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"hello\"}]}}\n";
        let (items, meta) = parse_history(Cli::Claude, text);
        assert_eq!(items.len(), 2);
        assert_eq!(meta.turn, 1);
    }

    #[test]
    fn parses_codex_and_muse_lines() {
        let codex = "{\"timestamp\":\"2026-09-25T10:00:00.000Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":[{\"type\":\"input_text\",\"text\":\"go\"}]}}\n";
        let (items, _) = parse_history(Cli::Codex, codex);
        assert_eq!(items.len(), 1);
        let muse = "{\"id\":\"e1\",\"recorded_at\":1000000,\"payload\":{\"event\":{\"kind\":\"started\",\"prompt\":\"fix it\"}}}\n";
        let (items, meta) = parse_history(Cli::Muse, muse);
        assert_eq!(items.len(), 1);
        assert_eq!(meta.turn, 1);
    }

    #[test]
    fn unknown_text_yields_nothing() {
        let (items, _) = parse_history(Cli::Opencode, "not json\n");
        assert!(items.is_empty());
    }

    #[test]
    fn load_fires_on_first_pick_or_forced_refresh() {
        assert!(needs_load(false, true, false)); // first open, nothing cached
        assert!(!needs_load(true, true, false)); // switching back, cache hit
        assert!(needs_load(true, false, true)); // Refresh forces re-read
        assert!(!needs_load(false, false, false));
    }
}
