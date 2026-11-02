//! The New agent sheet (Ctrl+N), after the web app's dialog: the title and
//! where it starts, what it should do, the agent, how many and how it
//! works, one line saying what Start will do, then crews, start later,
//! Cancel and the white Start. Esc or a click outside closes it;
//! Ctrl+Enter starts.

use eframe::egui::{self, Color32, FontId, Key, Modifiers, RichText, Stroke};

use crate::newtask::{help_line, NewTask};
use crate::prefs::{Crew, Prefs};
use crate::sessions_ui::Cli;
use crate::newtask_props::{im_btn, label};
use crate::style::PANEL;
use crate::theme::{DEL, INK3, R_XL, TEXT};

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
    /// Custom commands started lately, offered under Custom.
    pub(crate) recent: Vec<String>,
}

/// The CLIs whose program is on PATH (`where`-style, with PATHEXT).
pub fn installed_clis() -> Vec<Cli> {
    let path = std::env::var_os("PATH");
    let ext = std::env::var("PATHEXT").ok();
    Cli::all().into_iter().filter(|c| agent_core::pipe_session::resolve(c.program(), path.as_deref(), ext.as_deref()).is_some()).collect()
}

impl NewTaskSheet {
    pub fn new(dir: String) -> NewTaskSheet {
        NewTaskSheet { open: false, form: NewTask::new(dir), more: false, error: None, just_opened: false, crew_name: String::new(), out: None, sched_on: false, sched_time: "09:00".to_string(), sched_daily: true, room: String::new(), installed: None, recent: Vec::new() }
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
        self.recent = prefs.custom_cmds.clone();
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
        // The web app's dialog (.inbox-modal): 560 wide, panel ground, a
        // faint ring, a deep shadow, over a 55% dark backdrop.
        let frame = egui::Frame::new()
            .fill(PANEL)
            .corner_radius(R_XL)
            .stroke(Stroke::new(1.0, Color32::from_white_alpha(23)))
            .shadow(egui::Shadow { offset: [0, 40], blur: 100, spread: 0, color: Color32::from_black_alpha(190) })
            .inner_margin(egui::Margin { left: 22, right: 22, top: 22, bottom: 18 });
        let modal = egui::Modal::new(egui::Id::unique("new-task")).frame(frame).backdrop_color(Color32::from_black_alpha(140)).show(ctx, |ui| {
            ui.set_width(516.0_f32.min(ctx.content_rect().width() - 76.0));
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);
            let close = self.head(ui, projects);
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                label(ui, "What should it do?");
                ui.label(RichText::new("optional").size(12.0).color(INK3));
            });
            ui.add_space(8.0);
            egui::Frame::new()
                .fill(Color32::from_black_alpha(89))
                .corner_radius(12.0)
                .stroke(Stroke::new(1.0, Color32::from_white_alpha(23)))
                .inner_margin(egui::Margin::symmetric(14, 12))
                .show(ui, |ui| {
                    let prompt = egui::TextEdit::multiline(&mut self.form.prompt)
                        .frame(egui::Frame::NONE)
                        .font(FontId::proportional(14.0))
                        .text_color(TEXT)
                        .hint_text(RichText::new("Fix the flaky upload test and add a regression test.").size(14.0).color(INK3))
                        .desired_rows(4)
                        .desired_width(f32::INFINITY);
                    let resp = ui.add(prompt);
                    if self.just_opened {
                        resp.request_focus();
                        self.just_opened = false;
                    }
                });
            ui.add_space(14.0);
            self.agents(ui);
            if !self.room.is_empty() {
                ui.add_space(8.0);
                ui.label(RichText::new(&self.room).size(12.5).color(crate::theme::AMBER));
            }
            ui.add_space(14.0);
            self.options(ui);
            ui.add_space(8.0);
            let race_clash = self.form.count > 1 && !self.form.worktree;
            let help = help_line(self.form.count, self.form.worktree, self.form.flow);
            ui.label(RichText::new(help).size(12.5).color(if race_clash { DEL } else { INK3 }));
            if let Some(e) = &self.error {
                ui.add_space(4.0);
                ui.label(RichText::new(e).size(12.5).color(DEL));
            }
            ui.add_space(12.0);
            self.schedule_controls(ui);
            ui.add_space(14.0);
            let mut cancel = false;
            ui.horizontal(|ui| {
                self.crews(ui, crews);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let go_label = if self.sched_on { "Schedule" } else if self.form.count > 1 { "Start agents" } else { "Start agent" };
                    if im_btn(ui, go_label, true).on_hover_text("Start (Ctrl+Enter)").clicked() {
                        start = true;
                    }
                    cancel = im_btn(ui, "Cancel", false).on_hover_text("Esc").clicked();
                });
            });
            close || cancel
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn a_custom_command_starts_from_the_sheet() {
        let mut sheet = NewTaskSheet::new("D:\\maestro".to_string());
        let mut prefs = Prefs::default();
        prefs.remember_custom("grok");
        sheet.open_with("D:\\maestro", Some(1), &prefs);
        assert_eq!(sheet.recent, ["grok"]);
        sheet.form.custom = Some("gemini".to_string());
        let ctx = egui::Context::default();
        let mut got = None;
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND });
        ctx.run_ui(input, |ui| {
            got = sheet.show(ui.ctx(), &[r"D:\maestro".to_string()], &[]);
        })
        .drop_without_applying_deltas();
        let Some(SheetOut::Start(form)) = got else { panic!("Ctrl+Enter starts") };
        assert_eq!(form.custom.as_deref(), Some("gemini"));
    }
}
