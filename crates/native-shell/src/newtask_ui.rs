//! The New task sheet (Ctrl+N), painted after the approved Agent Inbox
//! design: a floating sheet with the folder crumb on top, a large borderless
//! prompt, a row of pill properties (project, agent, count, worktree, full
//! access), one help line saying what Start will do, and a footer with the
//! keys, "Create more" and the white Start button. Esc or a click outside
//! closes it; Ctrl+Enter starts.

use eframe::egui::{self, Color32, FontFamily, FontId, Key, Modifiers, RichText, Stroke};

use crate::newtask::{help_line, NewTask, MAX_AGENTS};
use crate::sessions_ui::Cli;
use crate::theme::{DEL, DIM, INK3, PILL, PILL_HOVER, PRIMARY, PRIMARY_INK, R_XL, SHEET, TEXT};

pub struct NewTaskSheet {
    pub open: bool,
    pub form: NewTask,
    /// Keep the sheet open after Start, for the next task.
    pub more: bool,
    pub error: Option<String>,
    just_opened: bool,
}

impl NewTaskSheet {
    pub fn new(dir: String) -> NewTaskSheet {
        NewTaskSheet { open: false, form: NewTask::new(dir), more: false, error: None, just_opened: false }
    }

    /// Open on `dir` with `count` agents (the palette's race opens with 3).
    pub fn open_with(&mut self, dir: &str, count: usize) {
        if self.form.dir.trim().is_empty() || !self.open {
            self.form.dir = dir.to_string();
        }
        self.form.set_count(count);
        self.error = None;
        self.open = true;
        self.just_opened = true;
    }

    /// After a successful Start: clear for the next task, or close.
    pub fn started(&mut self) {
        self.form.prompt.clear();
        self.error = None;
        if self.more {
            self.form.set_count(1);
            self.just_opened = true;
        } else {
            self.open = false;
        }
    }

    /// Paint the sheet; hands back the form when Start is pressed.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<NewTask> {
        if !self.open {
            return None;
        }
        // Read Ctrl+Enter before the prompt can take it as a newline.
        let mut start = ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));
        let frame = egui::Frame::new()
            .fill(SHEET)
            .corner_radius(R_XL)
            .stroke(Stroke::new(1.0, Color32::from_white_alpha(20)))
            .shadow(egui::Shadow { offset: [0, 18], blur: 40, spread: 0, color: Color32::from_black_alpha(140) })
            .inner_margin(egui::Margin::ZERO);
        let modal = egui::Modal::new(egui::Id::unique("new-task")).frame(frame).show(ctx, |ui| {
            // Wide enough that the pill row fits on one line, as in the design.
            ui.set_width(820.0_f32.min(ctx.content_rect().width() - 32.0));
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            let mut close = false;
            egui::Frame::NONE.inner_margin(egui::Margin { left: 20, right: 14, top: 14, bottom: 0 }).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(crumb(&self.form.dir)).size(12.5).color(INK3));
                    ui.label(RichText::new("\u{203A}").size(12.5).color(INK3));
                    ui.label(RichText::new("New task").size(12.5).color(INK3));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let x = ui.add(egui::Button::new(RichText::new("\u{2715}").color(INK3)).frame(false).min_size(egui::vec2(28.0, 28.0)));
                        if x.on_hover_text("Close (Esc)").clicked() {
                            close = true;
                        }
                    });
                });
            });
            egui::Frame::NONE.inner_margin(egui::Margin { left: 20, right: 20, top: 6, bottom: 4 }).show(ui, |ui| {
                let prompt = egui::TextEdit::multiline(&mut self.form.prompt)
                    .frame(egui::Frame::NONE)
                    .font(FontId::new(20.0, FontFamily::Name("semibold".into())))
                    .text_color(TEXT)
                    .hint_text(RichText::new("What should get done? e.g. The login form accepts an empty password. Reject it and add a test.").size(20.0).color(INK3))
                    .desired_rows(4)
                    .desired_width(f32::INFINITY);
                let resp = ui.add(prompt);
                if self.just_opened {
                    resp.request_focus();
                    self.just_opened = false;
                }
            });
            egui::Frame::NONE.inner_margin(egui::Margin { left: 20, right: 20, top: 4, bottom: 10 }).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| self.props(ui));
            });
            egui::Frame::NONE.inner_margin(egui::Margin { left: 20, right: 20, top: 0, bottom: 12 }).show(ui, |ui| {
                let race_clash = self.form.count > 1 && !self.form.worktree;
                let help = help_line(self.form.count, self.form.worktree);
                ui.label(RichText::new(help).size(12.5).color(if race_clash { DEL } else { INK3 }));
                if let Some(e) = &self.error {
                    ui.label(RichText::new(e).size(12.5).color(DEL));
                }
            });
            let line = ui.cursor().min.y;
            ui.painter().hline(ui.min_rect().x_range(), line, Stroke::new(1.0, Color32::from_white_alpha(15)));
            egui::Frame::NONE.inner_margin(egui::Margin { left: 20, right: 20, top: 12, bottom: 12 }).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Ctrl \u{21B5} start  \u{00B7}  Esc cancel").size(12.0).color(INK3));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let go = egui::Button::new(RichText::new("Start task").strong().color(PRIMARY_INK))
                            .fill(PRIMARY)
                            .corner_radius(999.0)
                            .min_size(egui::vec2(0.0, 36.0));
                        if ui.add(go).clicked() {
                            start = true;
                        }
                        ui.add_space(8.0);
                        ui.checkbox(&mut self.more, RichText::new("Create more").size(12.5).color(DIM));
                    });
                });
            });
            close
        });
        if modal.inner || modal.should_close() {
            self.open = false;
            return None;
        }
        start.then(|| self.form.clone())
    }

    /// The pill row: project folder, agent CLI, how many, worktree, access.
    fn props(&mut self, ui: &mut egui::Ui) {
        pill(ui, |ui| {
            ui.label(RichText::new("Project").size(12.5).color(INK3));
            ui.add(egui::TextEdit::singleline(&mut self.form.dir).frame(egui::Frame::NONE).desired_width(200.0).font(FontId::proportional(12.5)));
        });
        pill(ui, |ui| {
            ui.label(RichText::new("Agent").size(12.5).color(INK3));
            egui::ComboBox::from_id_salt("nt-cli").selected_text(RichText::new(self.form.cli.name()).size(12.5)).show_ui(ui, |ui| {
                for cli in Cli::all() {
                    ui.selectable_value(&mut self.form.cli, cli, cli.name());
                }
            });
        });
        pill(ui, |ui| {
            let n = self.form.count;
            if ui.add_enabled(n > 1, egui::Button::new("\u{2212}").frame(false)).on_hover_text("Fewer agents").clicked() {
                self.form.set_count(n - 1);
            }
            ui.label(RichText::new(n.to_string()).monospace().strong().size(12.5).color(TEXT));
            if ui.add_enabled(n < MAX_AGENTS, egui::Button::new("+").frame(false)).on_hover_text("More agents: a race").clicked() {
                self.form.set_count(n + 1);
            }
            ui.label(RichText::new(if self.form.count == 1 { "agent" } else { "agents" }).size(12.5).color(INK3));
        });
        pill(ui, |ui| {
            ui.checkbox(&mut self.form.worktree, RichText::new("Own worktree").size(12.5).color(DIM));
        });
        pill(ui, |ui| {
            ui.checkbox(&mut self.form.skip_perms, RichText::new("Full access").size(12.5).color(DIM))
                .on_hover_text("Start without permission prompts (--dangerously-skip-permissions, --yolo). Opencode sets this in its own config.");
        });
    }
}

/// A rounded property pill; brighter while hovered.
fn pill(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    let id = ui.next_auto_id();
    let hovered = ui.ctx().read_response(id).is_some_and(|r| r.contains_pointer());
    let r = egui::Frame::NONE
        .fill(if hovered { PILL_HOVER } else { PILL })
        .corner_radius(999.0)
        .inner_margin(egui::Margin { left: 12, right: 12, top: 4, bottom: 4 })
        .show(ui, |ui| {
            ui.set_min_height(24.0);
            ui.horizontal_centered(add);
        })
        .response;
    ui.interact(r.rect, id, egui::Sense::hover());
}

/// The folder's last name for the crumb, or the whole path when it has none.
fn crumb(dir: &str) -> String {
    let t = dir.trim().trim_end_matches(['\\', '/']);
    t.rsplit(['\\', '/']).next().filter(|s| !s.is_empty()).unwrap_or(t).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crumb_is_the_folder_name() {
        assert_eq!(crumb("D:\\maestro"), "maestro");
        assert_eq!(crumb("D:/work/app/"), "app");
        assert_eq!(crumb(""), "");
    }

    #[test]
    fn sheet_paints_and_starts_on_ctrl_enter() {
        let mut sheet = NewTaskSheet::new("D:\\maestro".to_string());
        sheet.open_with("D:\\maestro", 1);
        sheet.form.prompt = "Fix the login form".to_string();
        let ctx = egui::Context::default();
        let mut got = None;
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND });
        ctx.run_ui(input, |ui| {
            got = sheet.show(ui.ctx());
        })
        .drop_without_applying_deltas();
        let form = got.expect("Ctrl+Enter starts");
        assert_eq!(form.prompt, "Fix the login form");
        sheet.started();
        assert!(!sheet.open, "closes unless Create more");
    }
}
