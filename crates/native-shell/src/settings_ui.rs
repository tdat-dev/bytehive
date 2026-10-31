//! Settings (Ctrl+,), painted after the approved Agent Inbox design: a
//! floating sheet with a section nav on the left (Appearance, Agents,
//! System) and one rounded row per setting, its name and a one-line why on
//! the left, the control on the right. Every change saves at once.

use eframe::egui::{self, Color32, RichText, Sense, Stroke};

use crate::newtask::MAX_AGENTS;
use crate::prefs::{Backdrop, Prefs};
use crate::sessions_ui::Cli;
use crate::taskopts::{Flow, Perm};
use crate::theme::{ADD, DEL, INK3, R3, R_XL, SHEET, SURFACE, TEXT};

const SECTIONS: [&str; 3] = ["Appearance", "Agents", "System"];

#[derive(Default)]
pub struct SettingsSheet {
    pub open: bool,
    section: usize,
    scroll_to: Option<usize>,
}

/// A pill switch like the design's `.switch`: green when on.
fn switch(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let size = egui::vec2(30.0, 18.0);
    let (rect, mut r) = ui.allocate_exact_size(size, Sense::click());
    if r.clicked() {
        *on = !*on;
        r.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(r.id, *on);
    let bg = if *on { ADD } else { Color32::from_white_alpha(36) };
    ui.painter().rect_filled(rect, 999.0, bg);
    let x = egui::lerp(rect.left() + 9.0..=rect.right() - 9.0, t);
    let knob = if *on { Color32::from_rgb(5, 5, 6) } else { TEXT };
    ui.painter().circle_filled(egui::pos2(x, rect.center().y), 7.0, knob);
    r
}

/// One setting row: name and why on the left, `control` on the right.
fn row(ui: &mut egui::Ui, name: &str, why: &str, control: impl FnOnce(&mut egui::Ui) -> bool) -> bool {
    let mut changed = false;
    egui::Frame::new().fill(SURFACE).corner_radius(R3).inner_margin(egui::Margin::symmetric(14, 12)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 220.0).max(160.0);
            ui.vertical(|ui| {
                ui.set_width(w);
                ui.label(RichText::new(name).color(TEXT).family(egui::FontFamily::Name("semibold".into())));
                ui.label(RichText::new(why).size(12.5).color(INK3));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                changed = control(ui);
            });
        });
    });
    ui.add_space(6.0);
    changed
}

fn combo<T: PartialEq + Copy>(ui: &mut egui::Ui, id: &str, value: &mut T, options: &[(T, String)]) -> bool {
    let shown = options.iter().find(|(v, _)| v == value).map(|(_, l)| l.clone()).unwrap_or_default();
    let mut changed = false;
    egui::ComboBox::from_id_salt(id).selected_text(RichText::new(shown).size(12.5)).width(190.0).show_ui(ui, |ui| {
        for (v, label) in options {
            if ui.selectable_value(value, *v, label).changed() {
                changed = true;
            }
        }
    });
    changed
}

impl SettingsSheet {
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    /// Paint the sheet; true when a setting changed (save and apply it).
    pub fn show(&mut self, ctx: &egui::Context, prefs: &mut Prefs, store: &str) -> bool {
        if !self.open {
            return false;
        }
        let mut changed = false;
        let frame = egui::Frame::new()
            .fill(SHEET)
            .corner_radius(R_XL + 2.0)
            .stroke(Stroke::new(1.0, Color32::from_white_alpha(20)))
            .shadow(egui::Shadow { offset: [0, 18], blur: 40, spread: 0, color: Color32::from_black_alpha(140) })
            .inner_margin(egui::Margin::ZERO);
        let screen = ctx.content_rect();
        let size = egui::vec2(900.0_f32.min(screen.width() - 32.0), 620.0_f32.min(screen.height() - 80.0));
        let modal = egui::Modal::new(egui::Id::unique("settings")).frame(frame).show(ctx, |ui| {
            let mut close = false;
            // Fixed areas, like the design's 190px nav column and body.
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            let nav = egui::Rect::from_min_size(rect.min, egui::vec2(190.0, size.y));
            let body = egui::Rect::from_min_max(egui::pos2(nav.right(), rect.top()), rect.max);
            ui.painter().vline(nav.right(), rect.y_range(), Stroke::new(1.0, Color32::from_white_alpha(15)));
            let top_down = egui::Layout::top_down(egui::Align::Min);
            ui.scope_builder(egui::UiBuilder::new().max_rect(nav.shrink2(egui::vec2(10.0, 18.0))).layout(top_down), |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    ui.label(RichText::new("Settings").size(15.0).color(TEXT));
                });
                ui.add_space(10.0);
                for (i, name) in SECTIONS.iter().enumerate() {
                    let on = self.section == i;
                    let (r, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 32.0), Sense::click());
                    let fill = if on { Color32::from_white_alpha(20) } else if resp.hovered() { Color32::from_white_alpha(8) } else { Color32::TRANSPARENT };
                    ui.painter().rect_filled(r, 10.0, fill);
                    let color = if on || resp.hovered() { TEXT } else { INK3 };
                    ui.painter().text(egui::pos2(r.left() + 10.0, r.center().y), egui::Align2::LEFT_CENTER, *name, egui::FontId::proportional(13.0), color);
                    if resp.clicked() {
                        self.section = i;
                        self.scroll_to = Some(i);
                    }
                }
            });
            let inner = egui::Rect::from_min_max(body.min + egui::vec2(24.0, 14.0), body.max - egui::vec2(20.0, 14.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(inner).layout(top_down), |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    if ui.add(egui::Button::new(RichText::new("Close  Esc").size(12.5).color(INK3)).frame(false)).clicked() {
                        close = true;
                    }
                });
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    changed |= self.body(ui, prefs, store);
                });
            });
            close
        });
        if modal.inner || modal.should_close() {
            self.open = false;
        }
        changed
    }

    fn heading(&mut self, ui: &mut egui::Ui, i: usize) {
        let r = ui.label(RichText::new(SECTIONS[i]).size(12.5).color(INK3));
        if self.scroll_to == Some(i) {
            r.scroll_to_me(Some(egui::Align::TOP));
            self.scroll_to = None;
        }
        ui.add_space(4.0);
    }

    fn body(&mut self, ui: &mut egui::Ui, p: &mut Prefs, store: &str) -> bool {
        let mut ch = false;
        self.heading(ui, 0);
        ch |= row(ui, "Terminal font size", "Applies to every agent's terminal. Ctrl + and Ctrl - still zoom on top.", |ui| {
            let mut size = p.term_size.round() as u32;
            let sizes: Vec<(u32, String)> = (12..=18).map(|n| (n, n.to_string())).collect();
            let c = combo(ui, "set-font", &mut size, &sizes);
            p.term_size = size as f32;
            c
        });
        ch |= row(ui, "Background", "The colour behind the terminals.", |ui| {
            let mut c = false;
            for b in Backdrop::ALL.into_iter().rev() {
                let (rect, r) = ui.allocate_exact_size(egui::vec2(26.0, 26.0), Sense::click());
                let ring = if p.backdrop == b { TEXT } else { Color32::from_white_alpha(40) };
                ui.painter().circle(rect.center(), 11.0, b.color(), Stroke::new(if p.backdrop == b { 2.0 } else { 1.0 }, ring));
                if r.on_hover_text(b.label()).clicked() && p.backdrop != b {
                    p.backdrop = b;
                    c = true;
                }
            }
            c
        });
        ui.add_space(18.0);
        self.heading(ui, 1);
        ch |= row(ui, "Default agent", "The CLI a New task starts with.", |ui| {
            let mut opts: Vec<(Option<Cli>, String)> = vec![(None, "The last one used".to_string())];
            opts.extend(Cli::all().into_iter().map(|c| (Some(c), c.name().to_string())));
            combo(ui, "set-cli", &mut p.default_cli, &opts)
        });
        ch |= row(ui, "Agents per task", "More than one is a race: compare the diffs and keep one.", |ui| {
            let opts: Vec<(usize, String)> = (1..=MAX_AGENTS).map(|n| (n, n.to_string())).collect();
            combo(ui, "set-count", &mut p.default_count, &opts)
        });
        ch |= row(ui, "Own worktree for every task", "Each agent works on its own branch. Nothing touches your checkout until you merge.", |ui| switch(ui, &mut p.worktree).changed());
        ch |= row(ui, "Default permission mode", "What a new agent starts with. You can still allow commands one by one.", |ui| {
            let opts: Vec<(Perm, String)> = Perm::ALL.into_iter().map(|x| (x, x.label().to_string())).collect();
            combo(ui, "set-perm", &mut p.perm, &opts)
        });
        ch |= row(ui, "Default flow", "How a task moves: straight through, in stages, or a plan you approve first.", |ui| {
            let opts: Vec<(Flow, String)> = Flow::ALL.into_iter().map(|x| (x, x.label().to_string())).collect();
            combo(ui, "set-flow", &mut p.flow, &opts)
        });
        ch |= row(ui, "Time to reach the prompt", "How long a new CLI gets before its task is typed in.", |ui| {
            ui.add(egui::Slider::new(&mut p.job_delay_s, 1..=20).suffix(" s")).changed()
        });
        let mut drop = None;
        for (i, c) in p.crews.iter().enumerate() {
            let why = format!("{} \u{00D7} {} \u{00B7} {} \u{00B7} {}", c.count, c.cli.name(), c.perm.label(), c.flow.label());
            row(ui, &format!("Crew: {}", c.name), &why, |ui| {
                if ui.add(egui::Button::new(RichText::new("Delete").size(12.5).color(DEL)).frame(false)).clicked() {
                    drop = Some(i);
                }
                false
            });
        }
        if let Some(i) = drop {
            p.crews.remove(i);
            ch = true;
        }
        ui.add_space(18.0);
        self.heading(ui, 2);
        ch |= row(ui, "Reopen projects on start", "Opens the same project folders as last time.", |ui| switch(ui, &mut p.reopen_projects).changed());
        row(ui, "Where settings live", store, |_| false);
        ch
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sheet_paints_every_section() {
        let mut sheet = SettingsSheet::default();
        sheet.toggle();
        let mut prefs = Prefs::default();
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            sheet.show(ui.ctx(), &mut prefs, "~/.maestro/native-prefs.json");
        })
        .drop_without_applying_deltas();
        assert!(sheet.open);
        assert_eq!(prefs, Prefs::default(), "painting changes nothing");
    }
}
