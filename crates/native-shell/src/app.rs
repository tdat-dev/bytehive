//! The app: a registry of agents, a grid of panes, sessions to resume.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use agent_core::{launch::launch_spec, lock, Registry};
use eframe::egui::{self, FontId};
use supervise::stuck::{StuckEvidence, StuckWatch};

use crate::flow_ui::FlowPanel;
use crate::deliver::Delivery;
use crate::history_ui::HistoryPanel;
use crate::layout::clamp_zoom;
use crate::newtask_ui::NewTaskSheet;
use crate::commands::{command_catalog, SideTab};
use crate::palette::{Palette, PaletteItem, PaletteKind};
use crate::queue_ui::queue_rows;
use crate::review_ui::ReviewPanel;
use crate::polls::StuckRead;
use crate::sessions_ui::{Cli, SessionsPanel};
use crate::spawn::PendingCapture;
use crate::tasks::LaunchMsg;
use crate::term_pane::Pane;
use crate::prefs::{prefs_path, Prefs};
use crate::projects::{store_path, Projects};
use crate::settings_ui::SettingsSheet;
use crate::changes_ui::ChangesPanel;
use crate::files_ui::FilesDrawer;
use crate::board_ui::BoardView;


pub(crate) fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}


pub struct ShellApp {
    /// Shared with the remote dashboard's server threads.
    pub(crate) registry: std::sync::Arc<std::sync::Mutex<Registry>>,
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
    pub(crate) review: ReviewPanel,
    pub(crate) history: HistoryPanel,
    pub(crate) flow: FlowPanel,
    pub(crate) palette: Palette,
    /// Changed-file paths per folder, read off the UI thread.
    pub(crate) git_changed: HashMap<String, Vec<String>>,
    pub(crate) git_pending: HashSet<String>,
    pub(crate) git_tx: Sender<(String, crate::touched::FolderRead)>,
    pub(crate) git_rx: Receiver<(String, crate::touched::FolderRead)>,
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
    /// Open project folders and which one is shown.
    pub(crate) projects: Projects,
    pub(crate) projects_path: Option<std::path::PathBuf>,
    pub(crate) prefs: Prefs,
    pub(crate) prefs_path: Option<std::path::PathBuf>,
    pub(crate) settings: SettingsSheet,
    /// The pane on stage per project filter ("*" for all projects).
    pub(crate) stages: HashMap<String, String>,
    pub(crate) files: FilesDrawer,
    pub(crate) changes: ChangesPanel,
    pub(crate) board: BoardView,
    pub(crate) usage: crate::usage_watch::UsageWatch,
    pub(crate) extras: crate::glue_extras::Extras,
}

impl ShellApp {
    pub fn new() -> anyhow::Result<Self> {
        let (git_tx, git_rx) = channel();
        let (stuck_tx, stuck_rx) = channel();
        let (launch_tx, launch_rx) = channel();
        let mut sessions = SessionsPanel::new();
        let projects_path = store_path();
        let mut projects = projects_path.as_deref().map(Projects::load).unwrap_or_default();
        let prefs_path = prefs_path();
        let prefs = prefs_path.as_deref().map(Prefs::load).unwrap_or_default();
        if !prefs.reopen_projects {
            // Nothing open: the start screen offers last run's as recent.
            projects.close_all();
        }
        if let Some(f) = projects.filter.clone() {
            sessions.set_dir(&f);
        }
        let new_task = NewTaskSheet::new(sessions.dir.clone());
        let mut app = ShellApp {
            registry: Default::default(),
            panes: Vec::new(),
            next_id: 1,
            font: FontId::monospace(prefs.term_size),
            focused_once: false,
            sessions,
            captures: Vec::new(),
            last_poll: Instant::now(),
            side_tab: SideTab::from_env(),
            focus_pane: None,
            zoom: 1.0,
            stage: None,
            review: ReviewPanel::new(),
            history: HistoryPanel::new(),
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
            projects,
            projects_path,
            prefs,
            prefs_path,
            settings: SettingsSheet::default(),
            stages: HashMap::new(),
            files: FilesDrawer::default(),
            changes: ChangesPanel::default(),
            board: BoardView::default(),
            usage: Default::default(),
            extras: Default::default(),
        };
        // No shell of its own: the queue holds agents (Ctrl+K New Shell opens
        // one). Tests and screenshots get shells: BYTEHIVE_PANES=N boots with
        // N, BYTEHIVE_STAGE=1 puts the first on stage.
        let shells = if cfg!(test) { 1 } else { 0 };
        let extra: usize = std::env::var("BYTEHIVE_PANES").ok().and_then(|v| v.parse().ok()).unwrap_or(shells).min(6);
        for _ in 0..extra {
            app.run_command("shell");
        }
        // Last run's agents come back, each in its own conversation (not in
        // tests, nor when a debug variable sets the scene up).
        let staged = ["BYTEHIVE_PANES", "BYTEHIVE_TASK", "BYTEHIVE_SHEET", "BYTEHIVE_HOME", "BYTEHIVE_STAGE"].iter().any(|k| std::env::var_os(k).is_some());
        if !cfg!(test) && !staged && app.prefs.reopen_projects {
            app.reopen_agents();
        }
        // BYTEHIVE_HOME=1 opens on the start screen.
        app.extras.home = std::env::var("BYTEHIVE_HOME").is_ok();
        if std::env::var("BYTEHIVE_STAGE").is_ok() {
            if let Some(p) = app.panes.first() {
                app.stage = Some(p.id.clone());
            }
        }
        // BYTEHIVE_SHEET=new (or race) boots with the New task sheet open.
        if let Ok(key) = std::env::var("BYTEHIVE_SHEET") {
            app.run_command(&key);
        }
        // BYTEHIVE_TASK starts an agent on that job at boot (with the sheet
        // open it only fills it in), so a start can be tried end to end.
        if let Ok(task) = std::env::var("BYTEHIVE_TASK") {
            if app.new_task.open {
                app.new_task.form.prompt = task;
            } else {
                let dir = std::env::current_dir().map(|d| d.display().to_string()).unwrap_or_default();
                let mut t = crate::newtask::NewTask::new(dir);
                t.prompt = task;
                if let Err(e) = app.start_task(&t) {
                    app.notice = Some(e);
                }
            }
        }
        Ok(app)
    }


    /// Run a palette command by its key: `shell`, `spawn:<program>`, or
    /// `tab:<name>`. Unknown keys are ignored.
    pub(crate) fn run_command(&mut self, key: &str) {
        if key == "new" || key == "race" {
            let dir = self.projects.filter.clone().unwrap_or_else(|| self.sessions.dir.clone());
            self.new_task.open_with(&dir, (key == "race").then_some(3), &self.prefs);
        } else if key == "settings" {
            self.settings.toggle();
        } else if key == "files" {
            self.toggle_files();
        } else if key == "changes" {
            self.open_changes();
        } else if key == "board" {
            self.board.open = !self.board.open;
        } else if key == "plan" {
            self.board.open_plan();
        } else if key == "shell" {
            // In the project in view, not wherever ByteHive itself was started.
            let mut spec = launch_spec("powershell.exe", &[String::from("-NoLogo")]);
            spec.cwd = self.current_project().or_else(|| Some(self.sessions.dir.clone()).filter(|d| !d.is_empty()));
            let _ = self.spawn_pane("shell", spec, None, None);
        } else if let Some(program) = key.strip_prefix("spawn:") {
            if let Some(cli) = Cli::all().into_iter().find(|c| c.program() == program) {
                self.spawn_fresh(cli);
            }
        } else if let Some(tab) = key.strip_prefix("tab:") {
            self.side_tab = SideTab::from_key(tab);
        } else {
            self.run_more(key);
        }
    }
}

impl eframe::App for ShellApp {
    /// The background work, also run while the window is minimized (logic.rs).
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.tick(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // The ground under everything, margins included.
        ui.ctx().layer_painter(egui::LayerId::background()).rect_filled(ui.ctx().content_rect(), 0.0, crate::theme::BG);
        // One dark theme across panels and grid.
        let vis = ui.visuals_mut();
        vis.panel_fill = crate::theme::BG;
        vis.window_fill = crate::theme::BG;
        vis.widgets.noninteractive.bg_fill = crate::theme::SURFACE;
        vis.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, crate::theme::BORDER);
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
        self.font = FontId::monospace(self.prefs.term_size * self.zoom);

        // Ctrl/Cmd+K toggles the command palette. Consuming the key here keeps
        // it from reaching a focused terminal pane as a raw control byte.
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::K)) {
            self.palette.toggle();
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::N)) {
            self.run_command("new");
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Comma)) {
            self.run_command("settings");
        }

        let clashes = self.clash_map();
        let hidden = self.hidden_panes();
        let mut rows = queue_rows(&self.panes, now_ms(), &self.changed_counts(), &self.stuck_evidence, &clashes);
        rows.retain(|r| !hidden.contains(&r.pane_id));

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
        // The sentence counts agents, not plain shells.
        let agent = |id: &str| self.panes.iter().any(|p| p.id == id && p.cli.is_some());
        let count = |st: &[supervise::task_state::TaskState]| rows.iter().filter(|r| agent(&r.pane_id) && st.contains(&r.status.state)).count();
        use supervise::task_state::TaskState as T;
        let counts = (count(&[T::Needs, T::Blocked]), count(&[T::Review]), count(&[T::Working]));
        let order: Vec<String> = rows.iter().map(|r| r.pane_id.clone()).collect();
        egui::Panel::top("bar").exact_size(68.0).frame(egui::Frame::NONE).show_separator_line(false).show(ui, |ui| self.top_bar(ui, counts));
        self.dock(ui);
        if !self.projects.dirs.is_empty() || !self.panes.is_empty() {
            self.inbox_panel(ui, &rows, &hidden);
        }
        self.side_panels(ui);
        let grid_out = self.central(ui, &hidden, &order);
        let kill = grid_out.kill;
        let restart = grid_out.restart;

        if let Some(i) = restart {
            self.restart_pane(i);
        }
        for id in &kill {
            // Torn down outside the lock: a dashboard thread may want it.
            let gone = lock(&self.registry).take(id);
            drop(gone);
        }
        self.panes.retain(|p| !kill.contains(&p.id));
        for id in &grid_out.stop {
            drop(lock(&self.registry).take(id));
            if let Some(p) = self.panes.iter_mut().find(|p| &p.id == id) {
                p.exit_code.get_or_insert(0);
            }
        }

        // The palette floats over everything; a pick focuses a pane or runs a
        // command.
        if let Some(pick) = self.palette.show(ui, &palette_items) {
            match pick.kind {
                PaletteKind::Agent => self.focus_pane = Some(pick.key),
                PaletteKind::Command => self.run_command(&pick.key),
            }
        }
        self.sheets(ui.ctx());
        self.extra_sheets(ui.ctx());
        self.drawers(ui.ctx());
        crate::chrome::resize_edges(ui.ctx());
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
            pane.drain(&mut lock(registry));
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
        lock(&app.registry).write_input(&id, b"echo native-probe-7\r").unwrap();
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
        lock(&app.registry).write_input(&id0, b"echo marker-aaa\r").unwrap();
        lock(&app.registry).write_input(&id1, b"echo marker-bbb\r").unwrap();
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
        assert!(lock(&app.registry).has(&id));
        app.restart_pane(0);
        assert!(lock(&app.registry).has(&id));
        feed_until(&mut app, 0, ">", 20);
        lock(&app.registry).write_input(&id, b"echo back-again\r").unwrap();
        let grid = feed_until(&mut app, 0, "back-again", 15);
        assert!(grid.iter().any(|r| r.contains("back-again")), "respawned shell answers: {grid:?}");
    }
}
