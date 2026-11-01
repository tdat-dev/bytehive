//! History panel: an agent's conversation, read from its CLI transcript and
//! parsed into the same items every chat view shares. Reads run off the UI
//! thread; the panel re-parses only when the text changed.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};

use agent_core::{claude, codex, muse, opencode, TranscriptChunk};
use eframe::egui;
use supervise::types::{Chat, ChatItem, ChatMeta};

use crate::history_task::{cards, commands, first_prompt, timeline, timeline_ui, CardFacts, HistAction};

use crate::sessions_ui::Cli;
use crate::states::pane_cwd;
use crate::term_pane::Pane;
use crate::theme::FAINT;

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

/// Whether the selected agent needs a (re)read right now: a forced Refresh
/// always does; picking an agent reads it once; an empty read retries (the
/// log shows up seconds after spawn); new output since the last read
/// re-reads, throttled so an active agent does not spam threads.
pub fn want_refresh(
    cached_text: Option<&str>,
    last_output_ms: u64,
    now_ms: u64,
    last_req_ms: u64,
    changed: bool,
    force: bool,
) -> bool {
    if force {
        return true;
    }
    if changed && cached_text.is_none() {
        return true;
    }
    if now_ms.saturating_sub(last_req_ms) < 3000 {
        return false;
    }
    match cached_text {
        None => true,
        Some(t) => t.is_empty() || last_output_ms > last_req_ms,
    }
}

enum HistoryMsg {
    Chunk(String, TranscriptChunk),
}

pub struct HistoryPanel {
    selected: Option<String>,
    /// The stage at the last look: picking another agent there follows it.
    staged: Option<String>,
    /// This agent's recordings: (pane id, how many, when counted).
    recs: Option<(String, usize, std::time::Instant)>,
    cache: HashMap<String, HistoryCache>,
    last_req: HashMap<String, u64>,
    loading: bool,
    tx: Sender<HistoryMsg>,
    rx: Receiver<HistoryMsg>,
}

impl HistoryPanel {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        HistoryPanel { selected: None, staged: None, recs: None, cache: HashMap::new(), last_req: HashMap::new(), loading: false, tx, rx }
    }

    fn now_ms() -> u64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
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

    /// One task's History: cost, session, run again, and what happened.
    /// `stage` is the agent on stage (followed when it changes), `home` the
    /// project of a pane.
    pub fn show(&mut self, ui: &mut egui::Ui, panes: &[Pane], stage: Option<&str>, schedules: &[crate::schedule::Schedule], home: impl Fn(&Pane) -> String) -> Option<HistAction> {
        ui.label(egui::RichText::new("History").strong().size(15.0));
        let agents: Vec<&Pane> = panes.iter().filter(|p| p.cli.is_some()).collect();
        if agents.is_empty() {
            ui.label(egui::RichText::new("No agent panes yet. Plain shells have no transcript.").color(FAINT));
            return None;
        }
        if stage != self.staged.as_deref() {
            self.staged = stage.map(str::to_string);
            if stage.is_some_and(|s| agents.iter().any(|p| p.id == s)) {
                self.selected = self.staged.clone();
            }
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
        if !self.loading
            && want_refresh(
                self.cache.get(&pane_id).map(|c| c.text.as_str()),
                pane.map(|p| p.last_output_ms).unwrap_or(0),
                Self::now_ms(),
                self.last_req.get(&pane_id).copied().unwrap_or(0),
                changed,
                force,
            )
        {
            if let Some(p) = pane {
                self.last_req.insert(pane_id.clone(), Self::now_ms());
                self.request(p);
            }
        }
        ui.separator();
        let (Some(entry), Some(pane)) = (self.cache.get(&pane_id), pane) else {
            if !self.loading {
                ui.label(egui::RichText::new("Nothing read yet. Press Refresh.").color(FAINT));
            }
            return None;
        };
        let now = Self::now_ms();
        if self.recs.as_ref().is_none_or(|(id, _, t)| *id != pane_id || t.elapsed().as_secs() >= 3) {
            let safe = crate::replay::safe_name(&pane.name);
            let n = crate::replay::list(&home(pane)).iter().filter(|r| r.agent == safe).count();
            self.recs = Some((pane_id.clone(), n, std::time::Instant::now()));
        }
        let first = entry.items.iter().map(|i| i.at()).find(|a| *a > 0).unwrap_or(pane.started_ms);
        let last = entry.items.iter().rev().map(|i| i.at()).find(|a| *a > 0).unwrap_or(now);
        let prompt = first_prompt(&entry.items);
        let repeats = prompt.as_ref().and_then(|p| schedules.iter().find(|s| s.daily && s.enabled && s.task.prompt == *p).map(|s| s.time.clone()));
        let usage = pane.usage.as_ref();
        let facts = CardFacts {
            pane_id: &pane_id,
            usd: usage.and_then(|u| u.usd),
            tokens: usage.and_then(|u| u.tokens),
            minutes: last.saturating_sub(first) / 60_000,
            commands: commands(&entry.items),
            recordings: self.recs.as_ref().map_or(0, |r| r.1),
            recording: pane.recording.is_some(),
            prompt,
            repeats,
        };
        let action = cards(ui, &facts);
        if !entry.meta.todos.clone().unwrap_or_default().is_empty() {
            let todos = entry.meta.todos.clone().unwrap_or_default();
            let done = todos.iter().filter(|t| t.state == supervise::types::TodoState::Completed).count();
            ui.label(egui::RichText::new(format!("Plan: {done}/{} done", todos.len())).small().strong());
        }
        let evs = timeline(&entry.items, &pane.name);
        egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| timeline_ui(ui, &evs, now));
        action
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
    fn refresh_fires_on_pick_retry_stale_and_force() {
        // first pick, nothing cached
        assert!(want_refresh(None, 0, 10_000, 0, true, false));
        // same agent again, cache hit, quiet: no re-read
        assert!(!want_refresh(Some("hi"), 5_000, 10_000, 9_000, false, false));
        // empty read retries once the throttle passes (log shows up late)
        assert!(!want_refresh(Some(""), 0, 10_000, 9_500, false, false));
        assert!(want_refresh(Some(""), 0, 10_000, 6_000, false, false));
        // new output since the last read re-reads, throttled
        assert!(want_refresh(Some("hi"), 9_500, 10_000, 6_000, false, false));
        assert!(!want_refresh(Some("hi"), 9_500, 10_000, 9_500, false, false));
        // Refresh button always forces
        assert!(want_refresh(Some("hi"), 0, 10_000, 9_999, false, true));
    }
}
