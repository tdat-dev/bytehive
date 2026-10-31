//! The app: a registry of agents, a grid of panes, sessions to resume.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use agent_core::{launch::launch_spec, Registry};
use eframe::egui::{self, FontId};
use supervise::stuck::{StuckEvidence, StuckWatch};

use crate::flow_ui::FlowPanel;
use crate::deliver::Delivery;
use crate::history_ui::HistoryPanel;
use crate::layout::clamp_zoom;
use crate::newtask_ui::NewTaskSheet;
use crate::commands::{command_catalog, SideTab};
use crate::palette::{Palette, PaletteItem, PaletteKind};
use crate::queue_ui::{is_attention, queue_rows, QueueAction, QueuePanel};
use crate::review_ui::ReviewPanel;
use crate::send_ui::SendPanel;
use crate::polls::StuckRead;
use crate::sessions_ui::{Cli, ResumeRequest, SessionsPanel, WorktreeRequest};
use crate::spawn::PendingCapture;
use crate::tasks::LaunchMsg;
use crate::term_pane::Pane;
use crate::theme::{DEL, DIM, PRIMARY, PRIMARY_INK};

pub(crate) const HEADER_H: f32 = 24.0;
/// Avatar rail width when a pane owns the stage.
pub(crate) const RAIL_W: f32 = 76.0;

pub(crate) fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}


pub struct ShellApp {
    pub(crate) registry: Registry,
    pub(crate) panes: Vec<Pane>,
    pub(crate) next_id: usize,
    pub(crate) font: FontId,
    /// Density zoom multiplier for terminal glyphs (Ctrl+= / Ctrl+- / Ctrl+0).
    pub(crate) zoom: f32,
    pub(crate) focused_once: bool,
    pub(crate) sessions: SessionsPanel,
    pub(crate) captures: Vec<PendingCapture>,
    pub(crate) last_poll: Instant,
    pub(crate) side_tab: SideTab,
    pub(crate) focus_pane: Option<String>,
    /// The pane filling the stage; the rest collapse into the right rail.
    /// None means the plain tidy grid. Toggled by the header Focus button.
    pub(crate) stage: Option<String>,
    pub(crate) queue: QueuePanel,
    pub(crate) review: ReviewPanel,
    pub(crate) history: HistoryPanel,
    pub(crate) send: SendPanel,
    pub(crate) flow: FlowPanel,
    pub(crate) palette: Palette,
    /// Changed-file paths per folder, read off the UI thread.
    pub(crate) git_changed: HashMap<String, Vec<String>>,
    pub(crate) git_pending: HashSet<String>,
    pub(crate) git_tx: Sender<(String, Vec<String>)>,
    pub(crate) git_rx: Receiver<(String, Vec<String>)>,
    pub(crate) git_last: Instant,
    pub(crate) stuck_watch: StuckWatch,
    pub(crate) stuck_evidence: HashMap<String, StuckEvidence>,
    pub(crate) stuck_pending: HashSet<String>,
    pub(crate) stuck_tx: Sender<StuckRead>,
    pub(crate) stuck_rx: Receiver<StuckRead>,
    pub(crate) stuck_last: Instant,
    pub(crate) new_task: NewTaskSheet,
    /// Agents whose worktree is still being made, by name.
    pub(crate) launching: Vec<String>,
    pub(crate) launch_tx: Sender<LaunchMsg>,
    pub(crate) launch_rx: Receiver<LaunchMsg>,
    /// Tasks waiting to be typed into their new agent.
    pub(crate) deliveries: Vec<Delivery>,
    /// A launch that failed after the sheet closed; shown on the top bar.
    pub(crate) notice: Option<String>,
}

impl ShellApp {
    pub fn new() -> anyhow::Result<Self> {
        let (git_tx, git_rx) = channel();
        let (stuck_tx, stuck_rx) = channel();
        let (launch_tx, launch_rx) = channel();
        let sessions = SessionsPanel::new();
        let new_task = NewTaskSheet::new(sessions.dir.clone());
        let mut app = ShellApp {
            registry: Registry::new(),
            panes: Vec::new(),
            next_id: 1,
            font: FontId::monospace(crate::theme::TERM_SIZE),
            focused_once: false,
            sessions,
            captures: Vec::new(),
            last_poll: Instant::now(),
            side_tab: SideTab::from_env(),
            focus_pane: None,
            zoom: 1.0,
            stage: None,
            queue: QueuePanel::default(),
            review: ReviewPanel::new(),
            history: HistoryPanel::new(),
            send: SendPanel::default(),
            flow: FlowPanel::default(),
            palette: Palette::default(),
            git_changed: HashMap::new(),
            git_pending: HashSet::new(),
            git_tx,
            git_rx,
            git_last: Instant::now() - Duration::from_secs(60),
            stuck_watch: StuckWatch::new(),
            stuck_evidence: HashMap::new(),
            stuck_pending: HashSet::new(),
            stuck_tx,
            stuck_rx,
            stuck_last: Instant::now() - Duration::from_secs(60),
            new_task,
            launching: Vec::new(),
            launch_tx,
            launch_rx,
            deliveries: Vec::new(),
            notice: None,
        };
        let spec = launch_spec("powershell.exe", &[String::from("-NoLogo")]);
        app.spawn_pane("shell", spec, None, None)?;
        // Debug hooks for screenshots without clicking: MAESTRO_PANES=N
        // boots with N shells, MAESTRO_STAGE=1 puts the first on stage.
        let extra: usize = std::env::var("MAESTRO_PANES").ok().and_then(|v| v.parse().ok()).unwrap_or(1).clamp(1, 6);
        for _ in 1..extra {
            let spec = launch_spec("powershell.exe", &[String::from("-NoLogo")]);
            let _ = app.spawn_pane("shell", spec, None, None);
        }
        // MAESTRO_SHEET=new (or race) boots with the New task sheet open.
        if let Ok(key) = std::env::var("MAESTRO_SHEET") {
            app.run_command(&key);
        }
        if std::env::var("MAESTRO_STAGE").is_ok() {
            if let Some(p) = app.panes.first() {
                app.stage = Some(p.id.clone());
            }
        }
        Ok(app)
    }


    /// Run a palette command by its key: `shell`, `spawn:<program>`, or
    /// `tab:<name>`. Unknown keys are ignored.
    pub(crate) fn run_command(&mut self, key: &str) {
        if key == "new" || key == "race" {
            let dir = self.sessions.dir.clone();
            self.new_task.open_with(&dir, if key == "race" { 3 } else { 1 });
        } else if key == "shell" {
            let spec = launch_spec("powershell.exe", &[String::from("-NoLogo")]);
            let _ = self.spawn_pane("shell", spec, None, None);
        } else if let Some(program) = key.strip_prefix("spawn:") {
            if let Some(cli) = Cli::all().into_iter().find(|c| c.program() == program) {
                self.spawn_fresh(cli);
            }
        } else if let Some(tab) = key.strip_prefix("tab:") {
            self.side_tab = SideTab::from_key(tab);
        }
    }

    /// Spawn presets on the top bar: a shell plus fresh agent boots.
    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("native shell").strong());
            if ui.button("+ Shell").clicked() {
                let spec = launch_spec("powershell.exe", &[String::from("-NoLogo")]);
                let _ = self.spawn_pane("shell", spec, None, None);
            }
            let new = egui::Button::new(egui::RichText::new("New task").strong().color(PRIMARY_INK)).fill(PRIMARY).corner_radius(999.0);
            if ui.add(new).on_hover_text("Start agents on a task (Ctrl+N). Spawn a bare CLI from Ctrl+K.").clicked() {
                self.run_command("new");
            }
            if !self.launching.is_empty() {
                ui.label(egui::RichText::new(format!("Making worktrees for {}\u{2026}", self.launching.join(", "))).small().color(DIM));
            }
            if let Some(n) = self.notice.clone() {
                if ui.add(egui::Label::new(egui::RichText::new(n).small().color(DEL)).sense(egui::Sense::click())).on_hover_text("Click to dismiss").clicked() {
                    self.notice = None;
                }
            }
            ui.label(
                egui::RichText::new(format!("{}%", (self.zoom * 100.0) as u32))
                    .small()
                    .color(DIM),
            )
            .on_hover_text("Density zoom: Ctrl+= bigger, Ctrl+- smaller, Ctrl+0 reset");
        });
    }
}

impl eframe::App for ShellApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // One dark theme across panels and grid.
        let vis = ui.visuals_mut();
        vis.panel_fill = crate::theme::BG;
        vis.window_fill = crate::theme::BG;
        vis.widgets.noninteractive.bg_fill = crate::theme::SURFACE;
        vis.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, crate::theme::BORDER);
        for pane in &mut self.panes {
            pane.drain(&mut self.registry);
        }
        for pane in &mut self.panes {
            if let Some(t) = pane.pending_title.take() {
                // Shells report their exe path; show the pane name instead.
                let boring = t.contains('\\') || t.contains('/') || t.ends_with(".exe");
                let title = if boring { pane.name.clone() } else { t.chars().take(48).collect() };
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Title(format!("{title} - Maestro Native")));
            }
        }
        self.poll_captures();
        self.poll_git();
        self.poll_stuck();
        self.poll_launches();
        self.poll_deliveries();
        // Density zoom: Ctrl+= / Ctrl+- step the terminal glyphs, Ctrl+0
        // resets. Keyboard only on purpose: the wheel already scrolls back.
        let mut zoom = self.zoom;
        ui.ctx().input(|i| {
            if !i.modifiers.ctrl {
                return;
            }
            if i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals) {
                zoom = clamp_zoom(zoom + 0.1);
            } else if i.key_pressed(egui::Key::Minus) {
                zoom = clamp_zoom(zoom - 0.1);
            } else if i.key_pressed(egui::Key::Num0) {
                zoom = 1.0;
            }
        });
        self.zoom = zoom;
        self.font = FontId::monospace(crate::theme::TERM_SIZE * self.zoom);
        if self.panes.iter().any(|p| p.exit_code.is_none()) {
            // Poll the PTYs at ~30fps; try_recv is cheap when idle.
            ui.ctx().request_repaint_after(Duration::from_millis(33));
        }

        egui::Panel::top("bar").show(ui, |ui| self.top_bar(ui));

        let mut resume: Option<ResumeRequest> = None;
        let mut worktree: Option<WorktreeRequest> = None;
        let mut focus: Option<String> = None;
        // Ctrl/Cmd+K toggles the command palette. Consuming the key here keeps
        // it from reaching a focused terminal pane as a raw control byte.
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::K)) {
            self.palette.toggle();
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::N)) {
            self.run_command("new");
        }

        let clashes = self.clash_map();
        let rows = queue_rows(&self.panes, now_ms(), &self.git_changed, &self.stuck_evidence, &clashes);
        let needs = rows.iter().filter(|r| is_attention(r.status.state)).count();

        // Ctrl/Cmd+1..9 jumps straight to that Queue row's pane, no palette.
        for n in 1..=9u32 {
            let key = match n {
                1 => egui::Key::Num1,
                2 => egui::Key::Num2,
                3 => egui::Key::Num3,
                4 => egui::Key::Num4,
                5 => egui::Key::Num5,
                6 => egui::Key::Num6,
                7 => egui::Key::Num7,
                8 => egui::Key::Num8,
                _ => egui::Key::Num9,
            };
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, key)) {
                if let Some(idx) = crate::palette::quickjump_index(key) {
                    if let Some(r) = rows.get(idx) {
                        self.focus_pane = Some(r.pane_id.clone());
                    }
                }
            }
        }
        let mut palette_items: Vec<PaletteItem> = rows
            .iter()
            .map(|r| PaletteItem {
                key: r.pane_id.clone(),
                name: r.name.clone(),
                word: crate::theme::state_word(r.status.state).to_string(),
                kind: PaletteKind::Agent,
            })
            .collect();
        // Agents first (needs-you ranked), then the commands.
        palette_items.extend(command_catalog());
        let queue_tab = if needs > 0 { format!("Queue ({needs})") } else { "Queue".to_string() };
        egui::Panel::left("side").default_size(280.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.selectable_label(self.side_tab == SideTab::Queue, queue_tab).clicked() {
                    self.side_tab = SideTab::Queue;
                }
                if ui.selectable_label(self.side_tab == SideTab::Review, "Review").clicked() {
                    self.side_tab = SideTab::Review;
                }
                if ui.selectable_label(self.side_tab == SideTab::History, "History").clicked() {
                    self.side_tab = SideTab::History;
                }
                if ui.selectable_label(self.side_tab == SideTab::Sessions, "Sessions").clicked() {
                    self.side_tab = SideTab::Sessions;
                }
                if ui.selectable_label(self.side_tab == SideTab::Flow, "Flow").clicked() {
                    self.side_tab = SideTab::Flow;
                }
            });
            ui.separator();
            match self.side_tab {
                SideTab::Queue => match QueuePanel::show(&mut self.queue, ui, &self.panes, now_ms(), &self.git_changed, &self.stuck_evidence, &clashes) {
                    Some(QueueAction::Focus(id)) => focus = Some(id),
                    Some(QueueAction::Answer { pane_id, bytes }) => {
                        let _ = self.registry.write_input(&pane_id, &bytes);
                    }
                    None => {}
                },
                SideTab::Review => {
                    self.review.show(ui, &mut self.panes);
                }
                SideTab::History => {
                    self.history.show(ui, &self.panes);
                }
                SideTab::Sessions => {
                    resume = self.sessions.show(ui);
                    worktree = self.sessions.show_worktree(ui);
                }
                SideTab::Flow => {
                    self.flow.show(ui);
                }
            }
            self.send.show(ui, &self.panes, &mut self.registry);
        });
        if let Some(req) = resume {
            self.spawn_resume(req);
        }
        if let Some(req) = worktree {
            if let Err(e) = self.spawn_worktree(req) {
                self.sessions.wt_error = Some(e);
            }
        }
        if focus.is_some() {
            self.focus_pane = focus;
        }

        let grid_out = crate::grid::show_grid(self, ui);
        let kill = grid_out.kill;
        let restart = grid_out.restart;

        if let Some(i) = restart {
            self.restart_pane(i);
        }
        for id in &kill {
            self.registry.kill(id);
        }
        self.panes.retain(|p| !kill.contains(&p.id));

        // The palette floats over everything; a pick focuses a pane or runs a
        // command.
        if let Some(pick) = self.palette.show(ui, &palette_items) {
            match pick.kind {
                PaletteKind::Agent => self.focus_pane = Some(pick.key),
                PaletteKind::Command => self.run_command(&pick.key),
            }
        }
        if let Some(form) = self.new_task.show(ui.ctx()) {
            match self.start_task(&form) {
                Ok(()) => {
                    if self.new_task.more {
                        // Keep typing in the sheet, not in the new pane.
                        self.focus_pane = None;
                    }
                    self.new_task.started();
                }
                Err(e) => self.new_task.error = Some(e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::CommandSpec;
    use crate::term_pane::grid_text;
    use std::time::Instant;

    fn shell_spec() -> CommandSpec {
        launch_spec("powershell.exe", &[String::from("-NoLogo")])
    }

    /// Drain until `want` shows in the pane's grid, or the deadline passes.
    fn feed_until(app: &mut ShellApp, index: usize, want: &str, secs: u64) -> Vec<String> {
        let ShellApp { panes, registry, .. } = app;
        let pane = &mut panes[index];
        let end = Instant::now() + Duration::from_secs(secs);
        let mut grid = grid_text(&pane.term);
        while Instant::now() < end {
            pane.drain(registry);
            grid = grid_text(&pane.term);
            if grid.iter().any(|r| r.contains(want)) {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        grid
    }

    #[test]
    fn pty_output_reaches_the_grid() {
        let mut app = ShellApp::new().unwrap();
        // A prompt ends in ">", on any drive line; wait for any of it.
        let grid = feed_until(&mut app, 0, ">", 20);
        assert!(grid.iter().any(|r| r.contains(">")), "prompt visible: {grid:?}");
    }

    #[test]
    fn typed_input_echoes_back_through_the_shell() {
        let mut app = ShellApp::new().unwrap();
        feed_until(&mut app, 0, ">", 20);
        let id = app.panes[0].id.clone();
        app.registry.write_input(&id, b"echo native-probe-7\r").unwrap();
        let grid = feed_until(&mut app, 0, "native-probe-7", 15);
        assert!(grid.iter().any(|r| r.contains("native-probe-7")), "echo visible: {grid:?}");
    }

    #[test]
    fn two_panes_stay_independent() {
        let mut app = ShellApp::new().unwrap();
        app.spawn_pane("shell", shell_spec(), None, None).unwrap();
        feed_until(&mut app, 0, ">", 20);
        feed_until(&mut app, 1, ">", 20);
        let (id0, id1) = (app.panes[0].id.clone(), app.panes[1].id.clone());
        app.registry.write_input(&id0, b"echo marker-aaa\r").unwrap();
        app.registry.write_input(&id1, b"echo marker-bbb\r").unwrap();
        let g0 = feed_until(&mut app, 0, "marker-aaa", 15);
        let g1 = feed_until(&mut app, 1, "marker-bbb", 15);
        assert!(g0.iter().any(|r| r.contains("marker-aaa")));
        assert!(!g0.iter().any(|r| r.contains("marker-bbb")), "pane 0 leaked: {g0:?}");
        assert!(g1.iter().any(|r| r.contains("marker-bbb")));
        assert!(!g1.iter().any(|r| r.contains("marker-aaa")), "pane 1 leaked: {g1:?}");
    }

    #[test]
    fn restart_kills_and_respawns() {
        let mut app = ShellApp::new().unwrap();
        feed_until(&mut app, 0, ">", 20);
        let id = app.panes[0].id.clone();
        assert!(app.registry.has(&id));
        app.restart_pane(0);
        assert!(app.registry.has(&id));
        feed_until(&mut app, 0, ">", 20);
        app.registry.write_input(&id, b"echo back-again\r").unwrap();
        let grid = feed_until(&mut app, 0, "back-again", 15);
        assert!(grid.iter().any(|r| r.contains("back-again")), "respawned shell answers: {grid:?}");
    }
}
