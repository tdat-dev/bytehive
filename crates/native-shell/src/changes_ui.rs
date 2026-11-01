//! The Changes panel, after the approved design's review panel: a right
//! panel with "N files changed +A -D" and branch into base on top, a viewed
//! progress bar, the file list with a Viewed box each, and the picked file's
//! diff with old/new line numbers and tinted rows. V marks the file viewed
//! and jumps to the next one not yet viewed. Reads the Review tab's snapshot,
//! so both always show the same diff.

use eframe::egui::{self, Color32, RichText, Stroke};
use supervise::diff_file::{DiffFile, HunkKind};

use crate::diffview::{rows, DiffRow, Viewed};
use crate::theme::{ADD, DEL, DIM, INK3, SURFACE, TEXT};

const ROW_H: f32 = 19.0;
const ADD_BG: Color32 = Color32::from_rgba_premultiplied(9, 21, 15, 26);
const DEL_BG: Color32 = Color32::from_rgba_premultiplied(24, 11, 10, 26);

#[derive(Default)]
pub struct ChangesPanel {
    pub open: bool,
    pub cur: usize,
    pub viewed: Viewed,
}

/// What the panel is showing: whose diff, which branch into which base.
pub struct ChangesView<'a> {
    pub pane_id: &'a str,
    pub pane_name: &'a str,
    pub branch: Option<&'a str>,
    pub base: Option<&'a str>,
    /// None while the diff is still being read.
    pub files: Option<&'a [DiffFile]>,
}

impl ChangesPanel {
    pub fn open_at(&mut self, file: usize) {
        self.open = true;
        self.cur = file;
    }

    pub fn show(&mut self, ui: &mut egui::Ui, v: ChangesView) {
        if !self.open {
            return;
        }
        egui::Panel::right("changes").default_size(600.0).size_range(380.0..=1000.0).show(ui, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Changes on this branch").size(12.5).color(INK3));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(RichText::new("Close").size(12.5).color(INK3)).frame(false)).clicked() {
                        self.open = false;
                    }
                });
            });
            let Some(files) = v.files else {
                empty(ui, "Reading the diff\u{2026}", "");
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
                return;
            };
            if files.is_empty() {
                empty(ui, "Nothing to review", &format!("No changes in {}'s folder yet.", v.pane_name));
                return;
            }
            self.cur = self.cur.min(files.len() - 1);
            self.keys(ui, v.pane_id, files);
            self.head(ui, &v, files);
            self.file_list(ui, v.pane_id, files);
            ui.add_space(4.0);
            diff_body(ui, &files[self.cur]);
        });
    }

    /// V: mark the file viewed, go to the next one not viewed yet.
    fn keys(&mut self, ui: &egui::Ui, pane: &str, files: &[DiffFile]) {
        let typing = ui.ctx().memory(|m| m.focused().is_some());
        if typing || !ui.input(|i| i.key_pressed(egui::Key::V) && i.modifiers.is_none()) {
            return;
        }
        self.viewed.set(pane, &files[self.cur], true);
        if let Some(next) = self.viewed.next_unviewed(pane, files, self.cur) {
            self.cur = next;
        }
    }

    fn head(&self, ui: &mut egui::Ui, v: &ChangesView, files: &[DiffFile]) {
        let adds: u32 = files.iter().map(|f| f.additions).sum();
        let dels: u32 = files.iter().map(|f| f.deletions).sum();
        ui.horizontal(|ui| {
            let n = files.len();
            ui.label(RichText::new(format!("{n} file{} changed", if n == 1 { "" } else { "s" })).size(17.0).color(TEXT));
            ui.label(RichText::new(format!("+{adds}")).monospace().color(ADD));
            ui.label(RichText::new(format!("\u{2212}{dels}")).monospace().color(DEL));
        });
        let route = match (v.branch, v.base) {
            (Some(b), Some(base)) => format!("{b} \u{2192} {base}"),
            (Some(b), None) => format!("{b} (uncommitted)"),
            _ => "uncommitted changes".to_string(),
        };
        ui.label(RichText::new(route).size(12.0).monospace().color(INK3));
        ui.add_space(6.0);
        let seen = self.viewed.count(v.pane_id, files);
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{seen} / {} files viewed", files.len())).size(12.0).color(DIM));
            let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width().min(220.0), 4.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 2.0, Color32::from_white_alpha(20));
            let mut done = rect;
            done.set_width(rect.width() * seen as f32 / files.len() as f32);
            ui.painter().rect_filled(done, 2.0, crate::theme::FOCUS);
        });
        ui.add_space(4.0);
    }

    fn file_list(&mut self, ui: &mut egui::Ui, pane: &str, files: &[DiffFile]) {
        let h = (files.len() as f32 * 28.0).min(ui.available_height() * 0.3).max(28.0);
        egui::ScrollArea::vertical().id_salt("changes-files").max_height(h).auto_shrink([false, true]).show(ui, |ui| {
            for (i, f) in files.iter().enumerate() {
                let cur = i == self.cur;
                let seen = self.viewed.is_viewed(pane, f);
                let fill = if cur { SURFACE } else { Color32::TRANSPARENT };
                egui::Frame::new().fill(fill).corner_radius(8.0).inner_margin(egui::Margin::symmetric(8, 3)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let mut on = seen;
                        if ui.checkbox(&mut on, "").on_hover_text("Viewed (V)").changed() {
                            self.viewed.set(pane, f, on);
                        }
                        let mut path = RichText::new(&f.path).monospace().size(12.5).color(if cur { TEXT } else if seen { INK3 } else { DIM });
                        if seen {
                            path = path.strikethrough();
                        }
                        let w = (ui.available_width() - 90.0).max(60.0);
                        let left = egui::Layout::left_to_right(egui::Align::Center);
                        let r = ui.allocate_ui_with_layout(egui::vec2(w, 18.0), left, |ui| {
                            ui.set_width(w);
                            ui.add(egui::Label::new(path).truncate().sense(egui::Sense::click()))
                        });
                        if r.inner.on_hover_text(&f.path).clicked() {
                            self.cur = i;
                        }
                        ui.label(RichText::new(format!("+{}", f.additions)).monospace().size(12.0).color(ADD));
                        ui.label(RichText::new(format!("\u{2212}{}", f.deletions)).monospace().size(12.0).color(DEL));
                    });
                });
            }
        });
    }
}

fn empty(ui: &mut egui::Ui, title: &str, why: &str) {
    ui.add_space(20.0);
    ui.label(RichText::new(title).color(DIM));
    if !why.is_empty() {
        ui.label(RichText::new(why).size(13.0).color(INK3));
    }
}

/// The file's diff: numbered, tinted rows, painted only where visible.
fn diff_body(ui: &mut egui::Ui, file: &DiffFile) {
    let line_y = ui.cursor().min.y;
    ui.painter().hline(ui.max_rect().x_range(), line_y, Stroke::new(1.0, Color32::from_white_alpha(15)));
    if file.binary {
        empty(ui, "Binary file", "No text to show.");
        return;
    }
    let all = rows(file);
    let font = egui::FontId::monospace(12.5);
    egui::ScrollArea::both().id_salt(("changes-diff", &file.path)).auto_shrink([false, false]).show_rows(ui, ROW_H, all.len(), |ui, range| {
        for row in &all[range] {
            let width = ui.available_width().max(600.0);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(width, ROW_H), egui::Sense::hover());
            let p = ui.painter();
            let y = rect.center().y;
            match row {
                DiffRow::Hunk(h) => {
                    p.rect_filled(rect, 0.0, Color32::from_white_alpha(6));
                    p.text(egui::pos2(rect.left() + 12.0, y), egui::Align2::LEFT_CENTER, h, font.clone(), INK3);
                }
                DiffRow::Line { old, new, kind, text } => {
                    let (bg, color, mark) = match kind {
                        HunkKind::Add => (ADD_BG, Color32::from_rgb(196, 236, 212), "+"),
                        HunkKind::Del => (DEL_BG, Color32::from_rgb(244, 204, 199), "\u{2212}"),
                        HunkKind::Ctx => (Color32::TRANSPARENT, DIM, " "),
                    };
                    p.rect_filled(rect, 0.0, bg);
                    let num = |n: &Option<u32>| n.map(|v| v.to_string()).unwrap_or_default();
                    p.text(egui::pos2(rect.left() + 38.0, y), egui::Align2::RIGHT_CENTER, num(old), font.clone(), INK3);
                    p.text(egui::pos2(rect.left() + 78.0, y), egui::Align2::RIGHT_CENTER, num(new), font.clone(), INK3);
                    p.text(egui::pos2(rect.left() + 90.0, y), egui::Align2::LEFT_CENTER, format!("{mark} {}", text.replace('\t', "    ")), font.clone(), color);
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use supervise::diff_file::parse_diff;

    #[test]
    fn panel_paints_and_v_marks_viewed() {
        let files = parse_diff("diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-a\n+b\ndiff --git a/b.rs b/b.rs\n--- a/b.rs\n+++ b/b.rs\n@@ -1 +1 @@\n-c\n+d\n");
        let mut panel = ChangesPanel::default();
        panel.open_at(0);
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key { key: egui::Key::V, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
        ctx.run_ui(input, |ui| {
            let v = ChangesView { pane_id: "p", pane_name: "Ana", branch: Some("ana/fix"), base: Some("master"), files: Some(&files) };
            panel.show(ui, v);
        })
        .drop_without_applying_deltas();
        assert!(panel.viewed.is_viewed("p", &files[0]));
        assert_eq!(panel.cur, 1, "moved to the next file not viewed");
    }
}
