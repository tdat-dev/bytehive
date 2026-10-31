//! The app: a registry of agents, a grid of panes, sessions to resume.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use agent_core::{launch::launch_spec, Registry};
use eframe::egui::{self, FontId};
use supervise::stuck::{StuckEvidence, StuckWatch};

use crate::flow_ui::FlowPanel;
use crate::history_ui::HistoryPanel;
use crate::layout::clamp_zoom;
use crate::queue_ui::{is_attention, queue_rows, QueueAction, QueuePanel};
use crate::review_ui::ReviewPanel;
use crate::polls::StuckRead;
use crate::sessions_ui::{Cli, ResumeRequest, SessionsPanel, WorktreeRequest};
use crate::spawn::PendingCapture;
use crate::term_pane::Pane;
use crate::theme::DIM;

pub(crate) const HEADER_H: f32 = 24.0;
/// Avatar rail width when a pane owns the stage.
pub(crate) const RAIL_W: f32 = 76.0;

pub(crate) fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}


#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SideTab {
    Queue,
    Review,
    History,
    Sessions,
    Flow,
}

impl SideTab {
    /// Starting tab override for screenshots and debugging: MAESTRO_TAB=review.
    fn from_env() -> SideTab {
        match std::env::var("MAESTRO_TAB").unwrap_or_default().to_lowercase().as_str() {
            "review" => SideTab::Review,
            "history" => SideTab::History,
            "sessions" => SideTab::Sessions,
            "flow" => SideTab::Flow,
            _ => SideTab::Queue,
        }
    }
}

/// The header label cut to fit what's left of the chrome on the right: the
/// badge word (~78px with its dot and gaps) plus every button's width.
pub(crate) fn fit_header(label: &str, head_width: f32, reserve_right: f32) -> String {
    let max_chars = ((head_width - reserve_right) / 7.0).floor().max(8.0) as usize;
    if label.chars().count() > max_chars {
        format!("{}…", label.chars().take(max_chars.saturating_sub(1)).collect::<String>())
    } else {
        label.to_string()
    }
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
    pub(crate) flow: FlowPanel,
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
}

impl ShellApp {
    pub fn new() -> anyhow::Result<Self> {
        let (git_tx, git_rx) = channel();
        let (stuck_tx, stuck_rx) = channel();
        let mut app = ShellApp {
            registry: Registry::new(),
            panes: Vec::new(),
            next_id: 1,
            font: FontId::monospace(crate::theme::TERM_SIZE),
            focused_once: false,
            sessions: SessionsPanel::new(),
            captures: Vec::new(),
            last_poll: Instant::now(),
            side_tab: SideTab::from_env(),
            focus_pane: None,
            zoom: 1.0,
            stage: None,
            queue: QueuePanel::default(),
            review: ReviewPanel::new(),
            history: HistoryPanel::new(),
            flow: FlowPanel::default(),
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
        if std::env::var("MAESTRO_STAGE").is_ok() {
            if let Some(p) = app.panes.first() {
                app.stage = Some(p.id.clone());
            }
        }
        Ok(app)
    }


    /// Spawn presets on the top bar: a shell plus fresh agent boots.
    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("native shell").strong());
            if ui.button("+ Shell").clicked() {
                let spec = launch_spec("powershell.exe", &[String::from("-NoLogo")]);
                let _ = self.spawn_pane("shell", spec, None, None);
            }
            for cli in Cli::all() {
                if ui.button(format!("+ {}", cli.name())).clicked() {
                    self.spawn_fresh(cli);
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
        let clashes = self.clash_map();
        let needs = queue_rows(&self.panes, now_ms(), &self.git_changed, &self.stuck_evidence, &clashes)
            .iter()
            .filter(|r| is_attention(r.status.state))
            .count();
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

    #[test]
    fn header_label_fits_beside_badge_and_buttons() {
        // 340px of right-side chrome on a 700px header leaves 51 chars.
        assert_eq!(fit_header("shell 1", 700.0, 340.0), "shell 1");
        let long = "agent-with-a-long-name · feat/branch · abcdef12 · exited (1)";
        let cut = fit_header(long, 700.0, 340.0);
        assert!(cut.ends_with('…') && cut.chars().count() == 51, "{cut}");
        // Narrow cells never squeeze below 8 chars, whatever the reserve.
        assert_eq!(fit_header(long, 200.0, 340.0).chars().count(), 8);
        assert_eq!(fit_header(long, 900.0, 0.0), long);
    }
}
