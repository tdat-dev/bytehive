//! Command palette (Ctrl/Cmd+K): one overlay that lists every agent, filtered
//! as you type, ordered the way the Queue is (needs-you first). Enter focuses
//! the picked pane in the grid; Esc closes. The matching and selection math
//! live in pure helpers so they can be tested without a window.

use eframe::egui::{self, Align2, Color32, Key, Modifiers, Order};

use crate::theme::{BORDER, DIM, SURFACE};

/// One row the palette can show: a pane, its name, and its state word.
/// Built from the already-ranked queue rows, so input order is the order.
#[derive(Clone)]
pub struct PaletteItem {
    pub pane_id: String,
    pub name: String,
    pub word: String,
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

    /// Draw the overlay. Returns the pane id to focus when a row is chosen.
    /// No-op (returns None) while closed.
    pub fn show(&mut self, ui: &mut egui::Ui, items: &[PaletteItem]) -> Option<String> {
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
            let picked = filtered.get(self.sel).map(|it| it.pane_id.clone());
            if picked.is_some() {
                self.open = false;
                return picked;
            }
        }

        let mut chosen = None;
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
                        .hint_text("Jump to agent…")
                        .desired_width(f32::INFINITY);
                    let resp = ui.add(edit);
                    if self.just_opened {
                        resp.request_focus();
                        self.just_opened = false;
                    }
                    ui.add_space(6.0);
                    if filtered.is_empty() {
                        ui.label(egui::RichText::new("No agents match").color(DIM));
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
                            chosen = Some(it.pane_id.clone());
                        }
                    }
                });
            });

        if let Some(id) = chosen {
            self.open = false;
            return Some(id);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items() -> Vec<PaletteItem> {
        vec![
            PaletteItem { pane_id: "p1".into(), name: "claude main".into(), word: "needs".into() },
            PaletteItem { pane_id: "p2".into(), name: "codex api".into(), word: "working".into() },
            PaletteItem { pane_id: "p3".into(), name: "shell 1".into(), word: "idle".into() },
        ]
    }

    #[test]
    fn empty_query_keeps_order() {
        let it = items();
        let got = palette_filter(&it, "");
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].pane_id, "p1");
        assert_eq!(got[2].pane_id, "p3");
    }

    #[test]
    fn substring_is_case_insensitive() {
        let it = items();
        let got = palette_filter(&it, "CODE");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].pane_id, "p2");
    }

    #[test]
    fn tokens_match_in_any_order() {
        let it = items();
        let got = palette_filter(&it, "main claude");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].pane_id, "p1");
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
