//! Replays: record what an agent's terminal shows (Ctrl+K "Record this
//! agent"), and the sheet that lists a project's recordings and plays one
//! back with pause, seek and speed. The files are the web app's, so either
//! app plays what the other recorded.

use std::path::PathBuf;
use std::time::Instant;

use eframe::egui::{self, Color32, FontId, RichText, Stroke};

use crate::app::{now_ms, ShellApp};
use crate::projects::{pane_home, project_name};
use crate::replay::{clock, list, parse, rec_path, size_text, Player, RecMeta};
use crate::term_pane::paint_grid;
use crate::theme::{DEL, INK3, R_XL, SHEET, TEXT};

const SPEEDS: [f32; 5] = [0.5, 1.0, 2.0, 4.0, 8.0];

#[derive(Default)]
pub struct ReplaySheet {
    pub open: bool,
    pub project: String,
    pub recs: Vec<RecMeta>,
    pub player: Option<Player>,
    last_tick: Option<Instant>,
    pub error: Option<String>,
}

fn when(at: u64) -> String {
    chrono::DateTime::from_timestamp_millis(at as i64).map(|t| t.with_timezone(&chrono::Local).format("%b %-d, %H:%M").to_string()).unwrap_or_default()
}

impl ReplaySheet {
    pub fn open_on(&mut self, project: &str, play: Option<PathBuf>) {
        self.open = true;
        self.project = project.to_string();
        self.recs = list(project);
        self.player = None;
        self.error = None;
        if let Some(path) = play {
            if let Some(r) = self.recs.iter().find(|r| r.path == path).cloned() {
                self.play(&r);
            }
        }
    }

    fn play(&mut self, r: &RecMeta) {
        match std::fs::read_to_string(&r.path) {
            Ok(text) => {
                self.player = Some(Player::new(format!("{} \u{00B7} {}", r.agent, when(r.at)), parse(&text), 100, 30));
                self.last_tick = None;
            }
            Err(e) => self.error = Some(format!("Couldn't open that recording: {e}")),
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, font: &FontId) {
        if !self.open {
            return;
        }
        if let Some(p) = self.player.as_mut() {
            let now = Instant::now();
            let dt = self.last_tick.map_or(0, |t| now.duration_since(t).as_millis() as u64);
            self.last_tick = Some(now);
            p.tick(dt);
            if p.playing {
                ctx.request_repaint();
            }
        }
        let frame = egui::Frame::new().fill(SHEET).corner_radius(R_XL).stroke(Stroke::new(1.0, Color32::from_white_alpha(20))).inner_margin(egui::Margin::same(20));
        let screen = ctx.content_rect();
        let modal = egui::Modal::new(egui::Id::unique("replay-sheet")).frame(frame).show(ctx, |ui| {
            ui.set_width(920.0_f32.min(screen.width() - 72.0));
            let mut close = false;
            ui.horizontal(|ui| {
                ui.label(RichText::new("Replays").size(16.0).color(TEXT));
                if let Some(p) = &self.player {
                    ui.label(RichText::new(&p.label).size(12.5).color(INK3));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    close = ui.add(egui::Button::new(RichText::new("Close  Esc").size(12.0).color(INK3)).frame(false)).clicked();
                    if self.player.is_some() && ui.add(egui::Button::new(RichText::new("All recordings").size(12.0).color(INK3)).frame(false)).clicked() {
                        self.player = None;
                        self.recs = list(&self.project);
                    }
                });
            });
            if let Some(e) = &self.error {
                ui.label(RichText::new(e).size(12.5).color(DEL));
            }
            if self.player.is_some() {
                self.player_ui(ui, font, screen.height());
            } else {
                self.list_ui(ui);
            }
            close
        });
        if modal.inner || modal.should_close() {
            self.open = false;
            self.player = None;
        }
    }

    fn list_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new(format!("What agents in {} showed while you recorded them. Record one from Ctrl+K.", project_name(&self.project))).size(12.5).color(INK3));
        ui.add_space(8.0);
        if self.recs.is_empty() {
            ui.label(RichText::new("No recordings yet.").size(12.5).color(INK3));
            return;
        }
        let mut play = None;
        let mut delete = None;
        egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
            for r in &self.recs {
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(RichText::new(format!("{}   {}", r.agent, when(r.at))).size(13.0).color(TEXT)).frame(false)).clicked() {
                        play = Some(r.clone());
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new(RichText::new("Delete").size(12.0).color(DEL)).frame(false)).clicked() {
                            delete = Some(r.path.clone());
                        }
                        ui.label(RichText::new(size_text(r.size)).size(12.0).color(INK3));
                    });
                });
            }
        });
        if let Some(r) = play {
            self.play(&r);
        }
        if let Some(path) = delete {
            if let Err(e) = std::fs::remove_file(&path) {
                self.error = Some(format!("Couldn't delete it: {e}"));
            }
            self.recs = list(&self.project);
        }
    }

    fn player_ui(&mut self, ui: &mut egui::Ui, font: &FontId, screen_h: f32) {
        let Some(p) = self.player.as_mut() else { return };
        let (cell_w, cell_h) = {
            crate::term_pane::cell_metrics(ui.painter(), &font)
        };
        let size = egui::vec2(ui.available_width(), (screen_h - 260.0).clamp(200.0, 560.0));
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        ui.painter().rect_filled(rect, 8.0, Color32::from_rgb(10, 10, 12));
        let inner = rect.shrink(8.0);
        p.fit(((inner.width() / cell_w) as usize).max(20), ((inner.height() / cell_h) as usize).max(5));
        paint_grid(&ui.painter().with_clip_rect(inner), &p.term, inner, p.size, font, (cell_w, cell_h));
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let word = if p.playing { "Pause" } else if p.vt >= p.dur { "Play again" } else { "Play" };
            if ui.button(RichText::new(word).size(12.5)).clicked() {
                p.toggle();
            }
            let mut at = p.vt as f64;
            ui.spacing_mut().slider_width = (ui.available_width() - 190.0).max(80.0);
            if ui.add(egui::Slider::new(&mut at, 0.0..=p.dur.max(1) as f64).show_value(false)).changed() {
                p.seek(at as u64);
            }
            ui.label(RichText::new(format!("{} / {}", clock(p.vt), clock(p.dur))).monospace().size(12.0).color(INK3));
            egui::ComboBox::from_id_salt("replay-speed").width(56.0).selected_text(format!("{}x", p.speed)).show_ui(ui, |ui| {
                for s in SPEEDS {
                    ui.selectable_value(&mut p.speed, s, format!("{s}x"));
                }
            });
        });
    }
}

impl ShellApp {
    /// The agent a record command acts on: the one on stage, else the focused
    /// one, else the only one.
    fn record_target(&self) -> Option<usize> {
        match self.stage.clone().or_else(|| self.focus_pane.clone()) {
            Some(id) => self.panes.iter().position(|p| p.id == id),
            None => (self.panes.len() == 1).then_some(0),
        }
    }

    /// Start or stop recording the agent on stage.
    pub(crate) fn toggle_record(&mut self) {
        let Some(i) = self.record_target() else {
            self.notice = Some("Put an agent on stage first: that is the one recorded.".into());
            return;
        };
        let reg = std::sync::Arc::clone(&self.registry);
        let pane = &mut self.panes[i];
        if let Some(path) = pane.recording.take() {
            agent_core::lock(&reg).record_stop(&pane.id);
            self.notice = Some(format!("Recorded {}. Ctrl+K Replays plays it.", pane.name));
            let project = self.projects.owner(&pane_home(pane)).map(str::to_string).unwrap_or_else(|| pane_home(pane));
            self.extras.replay.open_on(&project, Some(path));
            return;
        }
        if pane.exit_code.is_some() {
            self.notice = Some(format!("{} isn't running: nothing to record.", pane.name));
            return;
        }
        let project = self.projects.owner(&pane_home(pane)).map(str::to_string).unwrap_or_else(|| pane_home(pane));
        let path = rec_path(&project, &pane.name, now_ms());
        let started = path.parent().map_or(Ok(()), std::fs::create_dir_all).map_err(anyhow::Error::from).and_then(|_| agent_core::lock(&reg).record_start(&pane.id, &path.to_string_lossy()));
        match started {
            Ok(()) => pane.recording = Some(path),
            Err(e) => self.notice = Some(format!("Couldn't start recording: {e}")),
        }
    }

    pub(crate) fn open_replays(&mut self) {
        match self.current_project() {
            Some(p) => self.extras.replay.open_on(&p, None),
            None => self.notice = Some("Open a project first: recordings belong to a project.".into()),
        }
    }
}
