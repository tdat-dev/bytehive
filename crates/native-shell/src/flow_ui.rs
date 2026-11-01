//! Flow panel: the fleet's hand-off log, tailed from
//! `.maestro/outbox.jsonl` (one JSON object per line: ts/from/to/message).
//! The web app feeds its flow buffer at delivery time; native has no
//! delivery hook, so it re-reads the outbox file, at most once every few
//! seconds, keeping only complete lines and the last 200 messages.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui;
use serde_json::Value;

use crate::theme::{DIM, FAINT};

/// Messages kept in memory. Older ones fall off the front.
pub const FLOW_MAX: usize = 200;
/// Row text is clamped so one runaway paste can't flood the panel.
pub const FLOW_TEXT_MAX: usize = 2000;
/// How far back the Queue shows hand-offs.
const RECENT_MS: u64 = 30 * 60_000;
/// Re-read the outbox at most this often; the file only grows on hand-offs.
const POLL_EVERY: Duration = Duration::from_secs(2);

pub struct FlowMsg {
    pub from: String,
    pub to: Option<String>,
    pub text: String,
    pub ts: u64,
}

/// Store `s` at a sane length.
pub fn clamp_text(s: &str) -> String {
    let t = s.trim();
    if t.chars().count() > FLOW_TEXT_MAX {
        let mut out: String = t.chars().take(FLOW_TEXT_MAX - 1).collect();
        out.push('…');
        out
    } else {
        t.to_string()
    }
}

/// Who a row says it went to: the addressed name, or everyone.
pub fn to_label(m: &FlowMsg) -> String {
    match m.to.as_deref().filter(|t| !t.trim().is_empty()) {
        Some(to) => to.to_string(),
        None => "everyone".to_string(),
    }
}

/// A clock reading as HH:MM:SS in local time.
pub fn fmt_time(ts_ms: u64) -> String {
    use chrono::{Local, TimeZone};
    match Local.timestamp_millis_opt(ts_ms as i64).single() {
        Some(d) => d.format("%H:%M:%S").to_string(),
        None => String::new(),
    }
}

/// One outbox line to a message, or nothing when it isn't one.
pub fn parse_line(line: &str) -> Option<FlowMsg> {
    let v: Value = serde_json::from_str(line).ok()?;
    let text = v.get("message")?.as_str()?;
    if text.trim().is_empty() {
        return None;
    }
    let from = v.get("from").and_then(|f| f.as_str()).unwrap_or("someone").to_string();
    let to = v.get("to").and_then(|t| t.as_str()).filter(|t| !t.trim().is_empty()).map(|t| t.to_string());
    let ts = v.get("ts").and_then(|t| t.as_u64()).unwrap_or(0);
    Some(FlowMsg { from, to, text: clamp_text(text), ts })
}

pub struct FlowPanel {
    items: Vec<FlowMsg>,
    offset: u64,
    last_poll: Option<Instant>,
}

impl Default for FlowPanel {
    fn default() -> Self {
        FlowPanel { items: Vec::new(), offset: 0, last_poll: None }
    }
}

impl FlowPanel {
    fn outbox_path() -> Option<PathBuf> {
        let mut dir = std::env::current_dir().ok()?;
        dir.push(".maestro");
        dir.push("outbox.jsonl");
        Some(dir)
    }

    /// Read any complete lines appended since the last poll, at most every
    /// few seconds so an idle frame costs nothing.
    fn poll(&mut self) {
        let now = Instant::now();
        if self.last_poll.is_some_and(|t| now.duration_since(t) < POLL_EVERY) {
            return;
        }
        self.last_poll = Some(now);
        let Some(path) = Self::outbox_path() else { return };
        let Ok(text) = std::fs::read(&path) else { return };
        if (text.len() as u64) < self.offset {
            self.offset = 0; // rotated or replaced: start over
        }
        let Some(chunk) = text.get(self.offset as usize..) else { return };
        // Hand back whole lines only; a half-written tail waits.
        let Some(end) = chunk.iter().rposition(|&b| b == b'\n').map(|i| i + 1) else { return };
        let chunk = &chunk[..end];
        self.offset += end as u64;
        if let Ok(text) = std::str::from_utf8(chunk) {
            for line in text.lines() {
                if let Some(m) = parse_line(line) {
                    self.items.push(m);
                }
            }
            if self.items.len() > FLOW_MAX {
                let drop = self.items.len() - FLOW_MAX;
                self.items.drain(..drop);
            }
        }
    }

    /// The last few hand-offs of the past half hour, on top of the Queue:
    /// the approved design shows every hand-off there.
    pub fn show_recent(&mut self, ui: &mut egui::Ui, now_ms: u64) {
        self.poll();
        let recent: Vec<&FlowMsg> = self.items.iter().rev().filter(|m| m.ts + RECENT_MS >= now_ms).take(3).collect();
        if recent.is_empty() {
            return;
        }
        ui.label(egui::RichText::new("Hand-offs").small().color(DIM));
        for m in recent.into_iter().rev() {
            let head = format!("{} \u{2192} {}  {}", m.from, to_label(m), fmt_time(m.ts));
            let first: String = m.text.lines().next().unwrap_or("").chars().take(90).collect();
            ui.label(egui::RichText::new(head).small().strong());
            ui.add(egui::Label::new(egui::RichText::new(first).small().color(DIM)).truncate()).on_hover_text(&m.text);
        }
        ui.separator();
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Flow").strong().size(15.0));
        self.poll();
        if self.items.is_empty() {
            ui.label(egui::RichText::new("No hand-offs yet. When someone sends work to an agent, it shows up here.").color(FAINT));
            return;
        }
        egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
            for m in &self.items {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("{} → {}", m.from, to_label(m))).strong().small());
                    if m.ts > 0 {
                        ui.label(egui::RichText::new(fmt_time(m.ts)).small().monospace().color(DIM));
                    }
                });
                ui.label(egui::RichText::new(&m.text).small());
                ui.separator();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINE: &str = r#"{"ts":1793326782398,"from":"Dot","to":"Cid","message":"Done, pushed 65b54d5"}"#;

    #[test]
    fn reads_a_real_outbox_line() {
        let m = parse_line(LINE).unwrap();
        assert_eq!(m.from, "Dot");
        assert_eq!(m.to.as_deref(), Some("Cid"));
        assert_eq!(to_label(&m), "Cid");
        assert_eq!(m.ts, 1793326782398);
        assert!(m.text.contains("65b54d5"));
    }

    #[test]
    fn broadcast_and_broken_lines() {
        let m = parse_line(r#"{"ts":1,"from":"Dot","message":"hi all"}"#).unwrap();
        assert_eq!(to_label(&m), "everyone");
        assert!(parse_line("not json").is_none());
        assert!(parse_line(r#"{"ts":1,"from":"Dot","message":"  "}"#).is_none());
        assert!(parse_line(r#"{"ts":1}"#).is_none());
    }

    #[test]
    fn long_text_is_clamped_and_clock_reads() {
        let long = "x".repeat(FLOW_TEXT_MAX + 10);
        assert!(clamp_text(&long).chars().count() <= FLOW_TEXT_MAX);
        assert!(clamp_text("  hi  ") == "hi");
        let clock = fmt_time(1793326782398);
        assert_eq!(clock.len(), 8);
        assert_eq!(clock.chars().filter(|&c| c == ':').count(), 2);
    }
}
