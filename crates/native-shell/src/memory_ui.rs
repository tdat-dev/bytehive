//! The Memory sheet: every fact a project's agents share (memory.rs), to
//! read, add, edit and delete by hand, and to send to a running agent of the
//! project to check against the code and fix. Agents change the same file
//! over MCP; while the sheet is open it follows them every 1.5 s, unless you
//! are editing.

use std::time::{Duration, Instant, SystemTime};

use eframe::egui::{self, Color32, RichText, Stroke};

use crate::memory::{add_fact, change, delete_fact, fix_prompt, load, memory_path, stamp, update_fact, MemoryEntry};
use crate::projects::project_name;
use crate::theme::{DEL, DIM, INK3, PRIMARY, PRIMARY_INK, R_XL, SHEET, TEXT};

const FOLLOW: Duration = Duration::from_millis(1500);

#[derive(Default)]
pub struct MemorySheet {
    pub open: bool,
    /// The project shown.
    pub project: String,
    pub entries: Vec<MemoryEntry>,
    seen: Option<SystemTime>,
    last_look: Option<Instant>,
    pub error: Option<String>,
    pub status: String,
    undo: Option<MemoryEntry>,
    new_text: String,
    /// The fact being edited, and its text so far.
    editing: Option<(String, String)>,
    /// The fact being sent to an agent ("*" for all of it), the note, and
    /// the agent picked.
    asking: Option<String>,
    note: String,
    to: Option<String>,
}

fn button(ui: &mut egui::Ui, text: &str, color: Color32) -> bool {
    ui.add(egui::Button::new(RichText::new(text).size(12.0).color(color)).frame(false)).clicked()
}

fn primary(ui: &mut egui::Ui, text: &str) -> bool {
    ui.add(egui::Button::new(RichText::new(text).strong().color(PRIMARY_INK)).fill(PRIMARY).corner_radius(999.0).min_size(egui::vec2(0.0, 30.0))).clicked()
}

impl MemorySheet {
    /// Open on `project`, read fresh.
    pub fn open_on(&mut self, project: &str) {
        *self = MemorySheet { open: true, project: project.to_string(), ..Default::default() };
        self.reload();
    }

    fn reload(&mut self) {
        let path = memory_path(&self.project);
        self.seen = stamp(&path);
        self.last_look = Some(Instant::now());
        match load(&path) {
            Ok(m) => {
                self.entries = m;
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Change the file; the sheet shows what it holds afterwards.
    fn run<T>(&mut self, f: impl FnOnce(&mut Vec<MemoryEntry>) -> Result<T, String>) -> Option<T> {
        let path = memory_path(&self.project);
        match change(&path, f) {
            Ok((out, m)) => {
                self.entries = m;
                self.seen = stamp(&path);
                self.error = None;
                Some(out)
            }
            Err(e) => {
                self.error = Some(e);
                None
            }
        }
    }

    /// An agent's save shows up while the sheet is open.
    fn follow(&mut self, ctx: &egui::Context) {
        ctx.request_repaint_after(FOLLOW);
        if self.editing.is_some() || self.last_look.is_some_and(|t| t.elapsed() < FOLLOW) {
            return;
        }
        self.last_look = Some(Instant::now());
        if stamp(&memory_path(&self.project)) != self.seen {
            self.reload();
        }
    }

    /// Paint the sheet. `projects`: the open projects, to switch between;
    /// `agents`: (pane id, name) of the project's running agents. Hands back
    /// a message to type into an agent.
    pub fn show(&mut self, ctx: &egui::Context, projects: &[String], agents: &[(String, String)]) -> Option<(String, String)> {
        if !self.open {
            return None;
        }
        self.follow(ctx);
        let mut send = None;
        let frame = egui::Frame::new()
            .fill(SHEET)
            .corner_radius(R_XL)
            .stroke(Stroke::new(1.0, Color32::from_white_alpha(20)))
            .shadow(egui::Shadow { offset: [0, 18], blur: 40, spread: 0, color: Color32::from_black_alpha(140) })
            .inner_margin(egui::Margin::same(20));
        let screen = ctx.content_rect();
        let modal = egui::Modal::new(egui::Id::unique("memory-sheet")).frame(frame).show(ctx, |ui| {
            ui.set_width(720.0_f32.min(screen.width() - 72.0));
            let mut close = false;
            ui.horizontal(|ui| {
                let mut job = egui::text::LayoutJob::default();
                job.append("Project memory", 0.0, crate::style::tight(crate::style::medium(20.0), TEXT, -0.02));
                ui.label(job);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| close = crate::newtask_props::round_x(ui).on_hover_text("Close (Esc)").clicked());
            });
            ui.add_space(2.0);
            ui.label(RichText::new(format!("What every agent in {} knows when it starts. Agents add to it as they learn.", project_name(&self.project))).size(12.5).color(INK3));
            if projects.len() > 1 {
                ui.horizontal_wrapped(|ui| {
                    for p in projects {
                        let on = *p == self.project;
                        if ui.selectable_label(on, RichText::new(project_name(p)).size(12.5)).clicked() && !on {
                            self.open_on(p);
                        }
                    }
                });
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let hint = "Add a fact every agent here should know: a convention, a decision, where something lives";
                let r = ui.add(egui::TextEdit::multiline(&mut self.new_text).desired_rows(2).desired_width(ui.available_width() - 70.0).hint_text(hint));
                let enter = r.has_focus() && ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter));
                if (primary(ui, "Add") || enter) && !self.new_text.trim().is_empty() {
                    let text = self.new_text.clone();
                    if let Some(id) = self.run(|m| add_fact(m, &text, "you", crate::app::now_ms())) {
                        self.new_text.clear();
                        self.status = format!("Added {id}.");
                        self.undo = None;
                    }
                }
            });
            if let Some(e) = &self.error {
                ui.label(RichText::new(e).size(12.5).color(DEL));
            }
            ui.add_space(6.0);
            let max_h = (screen.height() - 300.0).max(160.0);
            egui::ScrollArea::vertical().max_height(max_h).auto_shrink([false, true]).show(ui, |ui| {
                if self.entries.is_empty() {
                    ui.label(RichText::new(format!("Nothing yet. Agents save what they learn about {} here; add your own above.", project_name(&self.project))).size(13.0).color(INK3));
                }
                for e in self.entries.clone() {
                    send = send.take().or(self.entry(ui, &e, agents));
                }
            });
            ui.add_space(6.0);
            if self.asking.as_deref() == Some("*") {
                send = send.take().or(self.ask_form(ui, None, agents));
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new(&self.status).size(12.5).color(DIM));
                if self.undo.is_some() && button(ui, "Undo", TEXT) {
                    let back = self.undo.take().unwrap();
                    let by = if back.by.is_empty() { "you".to_string() } else { back.by.clone() };
                    if self.run(|m| add_fact(m, &back.text, &by, crate::app::now_ms())).is_some() {
                        self.status = "Put back.".into();
                    }
                }
                if !self.entries.is_empty() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if button(ui, "Ask an agent to check all of it", TEXT) {
                            self.asking = if self.asking.as_deref() == Some("*") { None } else { Some("*".into()) };
                            self.editing = None;
                        }
                    });
                }
            });
            close
        });
        if modal.inner || modal.should_close() {
            if self.editing.is_some() || self.asking.is_some() {
                self.editing = None;
                self.asking = None;
            } else {
                self.open = false;
            }
        }
        send
    }

    fn entry(&mut self, ui: &mut egui::Ui, e: &MemoryEntry, agents: &[(String, String)]) -> Option<(String, String)> {
        let mut send = None;
        egui::Frame::new().fill(Color32::from_white_alpha(6)).corner_radius(10.0).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            if let Some((id, text)) = self.editing.as_mut().filter(|(id, _)| *id == e.id) {
                ui.add(egui::TextEdit::multiline(text).desired_rows(3).desired_width(f32::INFINITY));
                let (id, text) = (id.clone(), text.clone());
                ui.horizontal(|ui| {
                    if primary(ui, "Save") && self.run(|m| update_fact(m, &id, &text, "you", crate::app::now_ms())).is_some() {
                        self.editing = None;
                        self.status = format!("Saved {id}.");
                        self.undo = None;
                    }
                    if button(ui, "Cancel", INK3) {
                        self.editing = None;
                    }
                });
                return;
            }
            ui.label(RichText::new(&e.text).size(13.5).color(TEXT));
            ui.horizontal(|ui| {
                let who = if e.by == "you" { "You" } else { e.by.as_str() };
                let when = if e.at > 0 { chrono::DateTime::from_timestamp_millis(e.at as i64).map(|t| t.with_timezone(&chrono::Local).format("%b %-d").to_string()).unwrap_or_default() } else { String::new() };
                let meta: Vec<&str> = [e.id.as_str(), who, when.as_str()].into_iter().filter(|s| !s.is_empty()).collect();
                ui.label(RichText::new(meta.join("  \u{00B7}  ")).size(11.5).color(INK3));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if button(ui, "Delete", DEL) {
                        if let Some(gone) = self.run(|m| delete_fact(m, &e.id)) {
                            self.status = format!("Deleted {}.", gone.id);
                            self.undo = Some(gone);
                        }
                    }
                    if button(ui, "Edit", DIM) {
                        self.editing = Some((e.id.clone(), e.text.clone()));
                        self.asking = None;
                    }
                    if button(ui, "Ask an agent", DIM) {
                        self.asking = if self.asking.as_deref() == Some(&e.id) { None } else { Some(e.id.clone()) };
                        self.editing = None;
                    }
                });
            });
            if self.asking.as_deref() == Some(&e.id) {
                send = self.ask_form(ui, Some(e), agents);
            }
        });
        ui.add_space(4.0);
        send
    }

    /// "What should change [...] Send to [Ana] Send".
    fn ask_form(&mut self, ui: &mut egui::Ui, e: Option<&MemoryEntry>, agents: &[(String, String)]) -> Option<(String, String)> {
        if agents.is_empty() {
            let what = if e.is_some() { "this fact" } else { "the memory" };
            ui.label(RichText::new(format!("Start an agent in {} first: it checks the code and fixes {what} for you.", project_name(&self.project))).size(12.5).color(INK3));
            return None;
        }
        let hint = if e.is_some() { "What is wrong with it? (optional)" } else { "Anything to look at? (optional)" };
        ui.add(egui::TextEdit::multiline(&mut self.note).desired_rows(2).desired_width(f32::INFINITY).hint_text(hint));
        if !self.to.as_ref().is_some_and(|t| agents.iter().any(|(id, _)| id == t)) {
            self.to = Some(agents[0].0.clone());
        }
        let mut send = None;
        ui.horizontal(|ui| {
            let to = self.to.clone().unwrap_or_default();
            let name = agents.iter().find(|(id, _)| *id == to).map(|(_, n)| n.clone()).unwrap_or_default();
            ui.label(RichText::new("Send to").size(12.5).color(DIM));
            egui::ComboBox::from_id_salt("mem-to").selected_text(&name).show_ui(ui, |ui| {
                for (id, n) in agents {
                    ui.selectable_value(&mut self.to, Some(id.clone()), n);
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if primary(ui, "Send") {
                    send = Some((to.clone(), fix_prompt(e, &self.note)));
                    self.status = format!("Sent to {name}. Its changes show up here as it makes them.");
                    self.asking = None;
                    self.note.clear();
                    self.undo = None;
                }
                if button(ui, "Cancel", INK3) {
                    self.asking = None;
                }
            });
        });
        send
    }
}
