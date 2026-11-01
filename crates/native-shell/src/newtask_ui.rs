//! The New task sheet (Ctrl+N), painted after the approved Agent Inbox
//! design: a floating sheet with the folder crumb on top, a large borderless
//! prompt, a row of pill properties (project, agent, count, worktree,
//! permissions, flow, crew), one help line saying what Start will do, and a
//! footer with the
//! keys, "Create more" and the white Start button. Esc or a click outside
//! closes it; Ctrl+Enter starts.

use eframe::egui::{self, Color32, FontFamily, FontId, Key, Modifiers, RichText, Stroke};

use crate::newtask::{help_line, NewTask};
use crate::prefs::{Crew, Prefs};
use crate::sessions_ui::Cli;
use crate::theme::{DEL, DIM, INK3, PRIMARY, PRIMARY_INK, R_XL, SHEET, TEXT};

/// What the sheet hands back.
pub enum SheetOut {
    Start(NewTask),
    /// Start it later, at a time of day.
    Schedule(crate::schedule::Schedule),
    /// Remember the sheet's settings as a named crew.
    SaveCrew(Crew),
}

pub struct NewTaskSheet {
    pub open: bool,
    pub form: NewTask,
    /// Keep the sheet open after Start, for the next task.
    pub more: bool,
    pub error: Option<String>,
    just_opened: bool,
    /// The name typed for a crew to save.
    pub(crate) crew_name: String,
    pub(crate) out: Option<SheetOut>,
    /// Start later instead of now: at `sched_time`, once or daily.
    pub(crate) sched_on: bool,
    pub(crate) sched_time: String,
    pub(crate) sched_daily: bool,
    /// How little room the chosen CLI's plan has left, when little.
    pub room: String,
    /// The CLIs found on PATH when the sheet opened; None until looked.
    pub installed: Option<Vec<Cli>>,
}

/// The CLIs whose program is on PATH (`where`-style, with PATHEXT).
pub fn installed_clis() -> Vec<Cli> {
    let path = std::env::var_os("PATH");
    let ext = std::env::var("PATHEXT").ok();
    Cli::all().into_iter().filter(|c| agent_core::pipe_session::resolve(c.program(), path.as_deref(), ext.as_deref()).is_some()).collect()
}

impl NewTaskSheet {
    pub fn new(dir: String) -> NewTaskSheet {
        NewTaskSheet { open: false, form: NewTask::new(dir), more: false, error: None, just_opened: false, crew_name: String::new(), out: None, sched_on: false, sched_time: "09:00".to_string(), sched_daily: true, room: String::new(), installed: None }
    }

    /// Open on `dir` with the settings' defaults; `count` overrides theirs
    /// (the palette's race opens with 3).
    pub fn open_with(&mut self, dir: &str, count: Option<usize>, prefs: &Prefs) {
        if self.form.dir.trim().is_empty() || !self.open {
            self.form.dir = dir.to_string();
        }
        self.form.cli = prefs.start_cli();
        let installed = installed_clis();
        if !installed.is_empty() && !installed.contains(&self.form.cli) {
            self.form.cli = installed[0];
        }
        self.installed = Some(installed);
        self.form.worktree = prefs.worktree;
        self.form.perm = prefs.perm;
        self.form.flow = prefs.flow;
        self.form.set_count(count.unwrap_or(prefs.default_count));
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
    /// `projects`: the open project folders the Project pill offers;
    /// `crews`: the saved crews the Crew pill applies.
    pub fn show(&mut self, ctx: &egui::Context, projects: &[String], crews: &[Crew]) -> Option<SheetOut> {
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
                ui.horizontal_wrapped(|ui| self.props(ui, projects, crews));
            });
            egui::Frame::NONE.inner_margin(egui::Margin { left: 20, right: 20, top: 0, bottom: 12 }).show(ui, |ui| {
                let race_clash = self.form.count > 1 && !self.form.worktree;
                let help = help_line(self.form.count, self.form.worktree, self.form.flow);
                ui.label(RichText::new(help).size(12.5).color(if race_clash { DEL } else { INK3 }));
                if !self.room.is_empty() {
                    ui.label(RichText::new(&self.room).size(12.5).color(crate::theme::AMBER));
                }
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
                        let go_label = if self.sched_on { "Schedule task" } else { "Start task" };
                        let go = egui::Button::new(RichText::new(go_label).strong().color(PRIMARY_INK))
                            .fill(PRIMARY)
                            .corner_radius(999.0)
                            .min_size(egui::vec2(0.0, 36.0));
                        if ui.add(go).clicked() {
                            start = true;
                        }
                        ui.add_space(8.0);
                        ui.checkbox(&mut self.more, RichText::new("Create more").size(12.5).color(DIM));
                        ui.add_space(8.0);
                        self.schedule_controls(ui);
                    });
                });
            });
            close
        });
        if modal.inner || modal.should_close() {
            self.open = false;
            return None;
        }
        if start {
            if !self.sched_on {
                return Some(SheetOut::Start(self.form.clone()));
            }
            match crate::schedule::parse_time(&self.sched_time) {
                None => self.error = Some("Write the time as HH:MM, like 09:00.".to_string()),
                Some(_) => {
                    let id = format!("s{}", crate::app::now_ms());
                    let s = crate::schedule::Schedule { id, task: self.form.clone(), time: self.sched_time.trim().to_string(), daily: self.sched_daily, enabled: true, last_fired: 0 };
                    return Some(SheetOut::Schedule(s));
                }
            }
        }
        self.out.take()
    }
}

impl NewTaskSheet {
    /// "Schedule [09:00] [Daily]": start later instead of now.
    fn schedule_controls(&mut self, ui: &mut egui::Ui) {
        if self.sched_on {
            egui::ComboBox::from_id_salt("nt-repeat").selected_text(RichText::new(if self.sched_daily { "Daily" } else { "Once" }).size(12.5)).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.sched_daily, true, "Daily");
                ui.selectable_value(&mut self.sched_daily, false, "Once");
            });
            ui.add(egui::TextEdit::singleline(&mut self.sched_time).desired_width(46.0).hint_text("09:00"));
        }
        ui.checkbox(&mut self.sched_on, RichText::new("Schedule").size(12.5).color(DIM)).on_hover_text("Start it later at a time of day, once or every day");
    }
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
        let prefs = Prefs::default();
        sheet.open_with("D:\\maestro", Some(1), &prefs);
        sheet.form.prompt = "Fix the login form".to_string();
        let ctx = egui::Context::default();
        let mut got = None;
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND });
        ctx.run_ui(input, |ui| {
            got = sheet.show(ui.ctx(), &[r"D:\maestro".to_string()], &[]);
        })
        .drop_without_applying_deltas();
        let Some(SheetOut::Start(form)) = got else { panic!("Ctrl+Enter starts") };
        assert_eq!(form.prompt, "Fix the login form");
        sheet.started();
        assert!(!sheet.open, "closes unless Create more");
    }
}
