//! Queue panel: every agent in one list, what needs you first, with the
//! one line each draws. Picking a row focuses its pane in the grid.

use std::collections::HashMap;

use eframe::egui::{self};
use supervise::ask::{free_text_keys, Ask};
use supervise::clash::{clash_short, ClashNote};
use supervise::stuck::StuckEvidence;
use supervise::task_state::{state_rank, status_line, TaskState, TaskStatus};

use crate::states::{pane_cwd, pane_status, state_badge};
use crate::term_pane::Pane;
use crate::theme::{BORDER, DIM, FAINT, SURFACE};

pub enum QueueAction {
    Focus(String),
    /// Bytes to write to the pane's PTY: an option key or a free answer.
    Answer { pane_id: String, bytes: Vec<u8> },
}

/// One queue row, computed from a pane: sort rank, status, and its line.
pub struct QueueRow {
    pub pane_id: String,
    pub name: String,
    pub rank: u8,
    pub status: TaskStatus,
    pub line: String,
}

pub fn queue_rows(
    panes: &[Pane],
    now: u64,
    git: &HashMap<String, Vec<String>>,
    stuck: &HashMap<String, StuckEvidence>,
    clash: &HashMap<String, ClashNote>,
) -> Vec<QueueRow> {
    let mut rows: Vec<QueueRow> = panes
        .iter()
        .map(|p| {
            let changed = git.get(&pane_cwd(p)).map(|f| f.len() as u32);
            let mut status = pane_status(p, now, changed);
            status.stuck = stuck.get(&p.id).cloned();
            status.clash = clash.get(&p.id).cloned();
            let rank = state_rank(status.state);
            let mut line = status_line(&p.name, &status, now);
            if let Some(note) = clash.get(&p.id) {
                line = format!("{line} · {}", clash_short(note));
            }
            QueueRow { pane_id: p.id.clone(), name: p.name.clone(), rank, status, line }
        })
        .collect();
    rows.sort_by(|a, b| a.rank.cmp(&b.rank).then(a.name.cmp(&b.name)));
    rows
}

/// Rows that pull the eye: needs-you and blocked render as cards, the rest
/// stay flat.
pub fn is_attention(state: TaskState) -> bool {
    matches!(state, TaskState::Needs | TaskState::Blocked)
}

/// The bytes picking option `n` of an ask, or none when it has no such row.
pub fn answer_bytes(ask: &Ask, n: u32) -> Option<Vec<u8>> {
    ask.options.iter().find(|o| o.n == n).map(|o| o.key.as_bytes().to_vec())
}

/// The bytes answering in free text: the CLI's "type something" row when it
/// has one, then the text, then Enter.
pub fn free_answer_bytes(ask: Option<&Ask>, text: &str) -> Vec<u8> {
    free_text_keys(ask, text).concat().into_bytes()
}

#[derive(Default)]
pub struct QueuePanel {
    free_text: HashMap<String, String>,
}

impl QueuePanel {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        panes: &[Pane],
        hidden: &std::collections::HashSet<String>,
        now: u64,
        git: &HashMap<String, Vec<String>>,
        stuck: &HashMap<String, StuckEvidence>,
        clash: &HashMap<String, ClashNote>,
    ) -> Option<QueueAction> {
        let mut out = None;
        ui.label(egui::RichText::new("Queue").strong().size(15.0));
        let mut rows = queue_rows(panes, now, git, stuck, clash);
        rows.retain(|r| !hidden.contains(&r.pane_id));
        if rows.is_empty() {
            ui.label(egui::RichText::new("No agents yet.").color(FAINT));
            return None;
        }
        // Drop typed answers for panes that are gone.
        self.free_text.retain(|id, _| rows.iter().any(|r| &r.pane_id == id));
        egui::ScrollArea::vertical().show(ui, |ui| {
            for row in &rows {
                if is_attention(row.status.state) {
                    let card = egui::Frame::new().fill(SURFACE).stroke(egui::Stroke::new(1.0, BORDER)).corner_radius(egui::CornerRadius::same(4)).inner_margin(egui::Margin::same(6));
                    let acted = card.show(ui, |ui| self.show_row(ui, row)).inner;
                    if out.is_none() {
                        out = acted;
                    }
                    ui.add_space(4.0);
                } else {
                    let acted = self.show_row(ui, row);
                    if out.is_none() {
                        out = acted;
                    }
                    ui.separator();
                }
            }
        });
        out
    }

    fn show_row(&mut self, ui: &mut egui::Ui, row: &QueueRow) -> Option<QueueAction> {
        let mut out = None;
        let (dot, word) = state_badge(row.status.state);
        let resp = ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(egui::Vec2::new(8.0, 8.0), egui::Sense::hover());
            ui.painter().circle_filled(r.center(), 4.0, dot);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&row.name).strong());
                    ui.label(egui::RichText::new(word).small().color(dot));
                });
                ui.label(egui::RichText::new(&row.line).small().color(DIM));
            });
        });
        if resp.response.interact(egui::Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            out = Some(QueueAction::Focus(row.pane_id.clone()));
        }
        if out.is_none() {
            if let Some(ask) = row.status.ask.as_ref() {
                out = self.show_ask(ui, &row.pane_id, ask);
            }
        }
        out
    }

    /// The ask inline under its row: what it wants, one button per option,
    /// and a free-text answer when the CLI offers one.
    fn show_ask(&mut self, ui: &mut egui::Ui, pane_id: &str, ask: &Ask) -> Option<QueueAction> {
        let mut out = None;
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(&ask.prompt).small().strong());
            if let Some(detail) = ask.detail.as_deref() {
                ui.label(egui::RichText::new(detail).small().monospace().color(DIM));
            }
            ui.horizontal_wrapped(|ui| {
                for o in &ask.options {
                    let label = if o.always { format!("{} (always)", o.label) } else { o.label.clone() };
                    if ui.small_button(label).on_hover_text(format!("sends {:?}", o.key)).clicked() {
                        if let Some(bytes) = answer_bytes(ask, o.n) {
                            out = Some(QueueAction::Answer { pane_id: pane_id.to_string(), bytes });
                        }
                    }
                }
            });
            if ask.free_text_option.is_some() {
                let entry = self.free_text.entry(pane_id.to_string()).or_default();
                ui.horizontal(|ui| {
                    let resp = ui.add(egui::TextEdit::singleline(entry).hint_text("Type an answer…").desired_width(160.0));
                    let send = ui.small_button("Send").clicked()
                        || (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                    if send && !entry.trim().is_empty() {
                        out = Some(QueueAction::Answer { pane_id: pane_id.to_string(), bytes: free_answer_bytes(Some(ask), entry) });
                        entry.clear();
                    }
                });
            }
        });
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::launch::launch_spec;
    use supervise::ask::{parse_ask, AskKind};
    use supervise::task_state::TaskState;

    fn pane(name: &str) -> Pane {
        let spec = launch_spec("powershell.exe", &[String::from("-NoLogo")]);
        Pane::new(format!("{name}-id"), name.to_string(), spec, 80, 24)
    }

    #[test]
    fn sorts_needs_first_then_alphabetical() {
        let mut asking = pane("zulu");
        asking.feed_bytes(" Do you want to proceed?\r\n \u{276F} 1. Yes\r\n   2. No (esc)\r\n".as_bytes());
        asking.last_output_ms = 1_000;
        let mut idle = pane("alpha");
        idle.last_output_ms = 1_000;
        let mut idle2 = pane("mike");
        idle2.last_output_ms = 1_000;
        let rows = queue_rows(&[idle, asking, idle2], 1_000_000, &HashMap::new(), &HashMap::new(), &HashMap::new());
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["zulu", "alpha", "mike"]);
        assert_eq!(rows[0].status.state, TaskState::Needs);
        assert!(rows[0].line.contains("zulu"));
    }

    #[test]
    fn answer_bytes_pick_the_option_key() {
        let ask = parse_ask("\n Would you like to run the following command?\n\n $ npm run build\n\n\u{203A} 1. Yes, proceed (y)\n  2. No (esc)\n\n Press enter to confirm or esc to cancel\n").unwrap();
        assert_eq!(ask.kind, AskKind::Run);
        assert_eq!(answer_bytes(&ask, 1), Some(b"y".to_vec()));
        assert_eq!(answer_bytes(&ask, 2), Some(b"\x1b".to_vec()));
        assert_eq!(answer_bytes(&ask, 9), None);
    }

    #[test]
    fn free_answer_routes_through_type_something() {
        let ask = parse_ask("\nPick one?\n> 1. Alpha\n  2. Type something.\nEnter to select \u{00B7} Esc to cancel\n").unwrap();
        assert_eq!(ask.free_text_option, Some(2));
        assert_eq!(free_answer_bytes(Some(&ask), "my words"), b"2my words\r".to_vec());
        assert_eq!(free_answer_bytes(None, "go"), b"go\r".to_vec());
    }

    #[test]
    fn stuck_and_clash_show_on_the_row() {
        use supervise::clash::{ClashKind, ClashNote};
        use supervise::stuck::{StuckEvidence, StuckKind};
        let mut working = pane("bea");
        working.feed_bytes(b"hello\r\n");
        let stuck = HashMap::from([(
            "bea-id".to_string(),
            StuckEvidence { kind: StuckKind::Loop, text: "same failure 3 times: FAIL x".to_string(), count: 3, sample: "FAIL x".to_string(), since: 1 },
        )]);
        let clash = HashMap::from([(
            "bea-id".to_string(),
            ClashNote { kind: ClashKind::File, key: "k".to_string(), with: vec!["Cy".to_string()], files: vec!["a.ts".to_string()] },
        )]);
        let rows = queue_rows(&[working], 1_000_000, &HashMap::new(), &stuck, &clash);
        assert!(rows[0].line.contains("looks stuck"), "{}", rows[0].line);
        assert!(rows[0].line.contains("overlaps Cy in a.ts"), "{}", rows[0].line);
    }

    #[test]
    fn stopped_sorts_last() {
        let mut dead = pane("gone");
        dead.exit_code = Some(0);
        let mut idle = pane("awake");
        idle.last_output_ms = 1_000;
        let rows = queue_rows(&[dead, idle], 1_000_000, &HashMap::new(), &HashMap::new(), &HashMap::new());
        assert_eq!(rows[1].name, "gone");
        assert_eq!(rows[1].status.state, TaskState::Stopped);
    }

    #[test]
    fn attention_is_only_needs_and_blocked() {
        use supervise::task_state::TaskState;
        assert!(is_attention(TaskState::Needs));
        assert!(is_attention(TaskState::Blocked));
        for s in [TaskState::Review, TaskState::Working, TaskState::Idle, TaskState::Stopped, TaskState::Merged] {
            assert!(!is_attention(s));
        }
    }
}
