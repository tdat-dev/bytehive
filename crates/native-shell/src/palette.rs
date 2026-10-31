//! Command palette (Ctrl/Cmd+K): one overlay that lists every agent AND every
//! command (spawn a shell or agent, jump to a side tab), filtered as you type.
//! Agents come first, ordered the way the Queue is (needs-you first), then the
//! commands. Enter runs the highlighted row; Esc closes. The matching and
//! selection math live in pure helpers so they can be tested without a window.

use eframe::egui::{self, Align2, Color32, Key, Modifiers, Order};

use crate::theme::{BORDER, DIM, SURFACE};

/// What a palette row does when chosen: focus an existing agent pane, or run a
/// command. The row's `key` is read against this: a pane id for `Agent`, a
/// command id (`shell`, `spawn:<program>`, `tab:<name>`) for `Command`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PaletteKind {
    Agent,
    Command,
}

/// One row the palette can show: its action key, a name to match and show, and
/// a trailing word (the agent's state, or a verb like "run" for a command).
#[derive(Clone)]
pub struct PaletteItem {
    pub key: String,
    pub name: String,
    pub word: String,
    pub kind: PaletteKind,
}

/// What the palette hands back when a row is chosen: the row's action key and
/// which kind of action it is, so the caller knows whether to focus a pane or
/// run a command.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PalettePick {
    pub key: String,
    pub kind: PaletteKind,
}

impl std::fmt::Debug for PaletteKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            PaletteKind::Agent => "Agent",
            PaletteKind::Command => "Command",
        })
    }
}

/// Rows whose name matches `query`: case-insensitive, every whitespace token
/// must appear as a substring. Input order is preserved, so a ranked list
/// stays ranked. An empty query keeps everything.
pub fn palette_filter<'a>(items: &'a [PaletteItem], query: &str) -> Vec<&'a PaletteItem> {
    let needle = query.to_lowercase();
    let tokens: Vec<&str> = needle.split_whitespace().collect();
    items
        .iter()
        .filter(|it| {
            let hay = it.name.to_lowercase();
            tokens.iter().all(|t| hay.contains(t))
        })
        .collect()
}

/// The 0-based pane index a `Ctrl+<digit>` quick-jump points at, or none for
/// a key that is not 1..=9. `Num1` jumps to the first row, `Num9` to the ninth.
pub fn quickjump_index(key: Key) -> Option<usize> {
    match key {
        Key::Num1 => Some(0),
        Key::Num2 => Some(1),
        Key::Num3 => Some(2),
        Key::Num4 => Some(3),
        Key::Num5 => Some(4),
        Key::Num6 => Some(5),
        Key::Num7 => Some(6),
        Key::Num8 => Some(7),
        Key::Num9 => Some(8),
        _ => None,
    }
}

/// Keep a selection index inside `[0, len)`; an empty list pins it at 0.
pub fn clamp_sel(len: usize, sel: usize) -> usize {
    if len == 0 {
        0
    } else {
        sel.min(len - 1)
    }
}

/// The palette's own state: whether it is open, the query, and which row is
/// highlighted. A fresh toggle re-focuses the search field.
#[derive(Default)]
pub struct Palette {
    pub open: bool,
    query: String,
    sel: usize,
    just_opened: bool,
}

impl Palette {
    /// Flip open/closed. Opening clears the query and arms field focus.
    pub fn toggle(&mut self) {
        self.open = !self.open;
        if self.open {
            self.query.clear();
            self.sel = 0;
            self.just_opened = true;
        }
    }

    /// Draw the overlay. Returns the chosen row's pick when a row is run.
    /// No-op (returns None) while closed.
    pub fn show(&mut self, ui: &mut egui::Ui, items: &[PaletteItem]) -> Option<PalettePick> {
        if !self.open {
            return None;
        }

        // Navigation keys are consumed here so the focused terminal never sees
        // them while the palette is up.
        let mut enter = false;
        ui.input_mut(|i| {
            if i.consume_key(Modifiers::NONE, Key::Escape) {
                self.open = false;
            }
            if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
                self.sel = self.sel.saturating_add(1);
            }
            if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
                self.sel = self.sel.saturating_sub(1);
            }
            if i.consume_key(Modifiers::NONE, Key::Enter) {
                enter = true;
            }
        });
        if !self.open {
            return None;
        }

        let filtered = palette_filter(items, &self.query);
        self.sel = clamp_sel(filtered.len(), self.sel);

        if enter {
            if let Some(it) = filtered.get(self.sel) {
                self.open = false;
                return Some(PalettePick { key: it.key.clone(), kind: it.kind });
            }
        }

        let mut chosen: Option<PalettePick> = None;
        let frame = egui::Frame::NONE
            .fill(SURFACE)
            .stroke(egui::Stroke::new(1.0, BORDER))
            .inner_margin(10.0)
            .corner_radius(8.0);
        egui::Area::new(egui::Id::unique("command-palette"))
            .order(Order::Foreground)
            .anchor(Align2::CENTER_TOP, egui::vec2(0.0, 80.0))
            .show(ui.ctx(), |ui| {
                frame.show(ui, |ui| {
                    ui.set_width(460.0);
                    let edit = egui::TextEdit::singleline(&mut self.query)
                        .hint_text("Jump to agent or run a command…")
                        .desired_width(f32::INFINITY);
                    let resp = ui.add(edit);
                    if self.just_opened {
                        resp.request_focus();
                        self.just_opened = false;
                    }
                    ui.add_space(6.0);
                    if filtered.is_empty() {
                        ui.label(egui::RichText::new("Nothing matches").color(DIM));
                    }
                    for (row, it) in filtered.iter().enumerate() {
                        let on = row == self.sel;
                        let bg = if on { Color32::from_rgb(40, 44, 54) } else { Color32::TRANSPARENT };
                        let inner = egui::Frame::NONE.fill(bg).inner_margin(egui::vec2(8.0, 5.0)).corner_radius(5.0);
                        let r = inner
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(&it.name).strong());
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label(egui::RichText::new(&it.word).small().color(DIM));
                                    });
                                })
                                .response
                            })
                            .response
                            .interact(egui::Sense::click());
                        if r.clicked() {
                            chosen = Some(PalettePick { key: it.key.clone(), kind: it.kind });
                        }
                    }
                });
            });

        if let Some(pick) = chosen {
            self.open = false;
            return Some(pick);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(key: &str, name: &str, word: &str) -> PaletteItem {
        PaletteItem { key: key.into(), name: name.into(), word: word.into(), kind: PaletteKind::Agent }
    }

    fn items() -> Vec<PaletteItem> {
        vec![
            agent("p1", "claude main", "needs"),
            agent("p2", "codex api", "working"),
            agent("p3", "shell 1", "idle"),
        ]
    }

    #[test]
    fn empty_query_keeps_order() {
        let it = items();
        let got = palette_filter(&it, "");
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].key, "p1");
        assert_eq!(got[2].key, "p3");
    }

    #[test]
    fn substring_is_case_insensitive() {
        let it = items();
        let got = palette_filter(&it, "CODE");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].key, "p2");
    }

    #[test]
    fn tokens_match_in_any_order() {
        let it = items();
        let got = palette_filter(&it, "main claude");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].key, "p1");
    }

    #[test]
    fn commands_filter_alongside_agents() {
        let mut it = items();
        it.push(PaletteItem {
            key: "spawn:claude".into(),
            name: "Spawn Claude".into(),
            word: "run".into(),
            kind: PaletteKind::Command,
        });
        // "spawn" only matches the command; "claude" matches both.
        let spawn = palette_filter(&it, "spawn");
        assert_eq!(spawn.len(), 1);
        assert_eq!(spawn[0].kind, PaletteKind::Command);
        assert_eq!(palette_filter(&it, "claude").len(), 2);
    }

    #[test]
    fn no_match_returns_empty() {
        let it = items();
        assert!(palette_filter(&it, "zzz").is_empty());
    }

    #[test]
    fn quickjump_maps_digits_to_zero_based() {
        assert_eq!(quickjump_index(Key::Num1), Some(0));
        assert_eq!(quickjump_index(Key::Num9), Some(8));
        assert_eq!(quickjump_index(Key::Num0), None);
        assert_eq!(quickjump_index(Key::A), None);
    }

    #[test]
    fn clamp_keeps_index_in_range() {
        assert_eq!(clamp_sel(3, 5), 2);
        assert_eq!(clamp_sel(3, 1), 1);
        assert_eq!(clamp_sel(0, 4), 0);
    }

    #[test]
    fn toggle_opens_then_closes_and_resets() {
        let mut p = Palette::default();
        p.query = "stale".into();
        p.sel = 7;
        p.toggle();
        assert!(p.open);
        assert_eq!(p.query, "");
        assert_eq!(p.sel, 0);
        assert!(p.just_opened);
        p.toggle();
        assert!(!p.open);
    }
}
