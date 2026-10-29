//! The app: a registry of agents, a grid of panes, sessions to resume.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use agent_core::{launch::launch_spec, Registry};
use eframe::egui::{self, FontId, Pos2, Rect, Vec2};
use supervise::stuck::{StuckEvidence, StuckWatch};

use crate::color::{c32, BG_DEFAULT};
use crate::history_ui::HistoryPanel;
use crate::queue_ui::{QueueAction, QueuePanel};
use crate::review_ui::ReviewPanel;
use crate::polls::StuckRead;
use crate::sessions_ui::{Cli, ResumeRequest, SessionsPanel, WorktreeRequest};
use crate::spawn::PendingCapture;
use crate::states::{pane_cwd, pane_status, state_badge};
use crate::term_pane::Pane;
use crate::theme::{BORDER, DIM, SURFACE, TEXT};

const HEADER_H: f32 = 24.0;

pub(crate) fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}


#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SideTab {
    Queue,
    Review,
    History,
    Sessions,
}

pub struct ShellApp {
    pub(crate) registry: Registry,
    pub(crate) panes: Vec<Pane>,
    pub(crate) next_id: usize,
    pub(crate) font: FontId,
    pub(crate) focused_once: bool,
    pub(crate) sessions: SessionsPanel,
    pub(crate) captures: Vec<PendingCapture>,
    pub(crate) last_poll: Instant,
    pub(crate) side_tab: SideTab,
    pub(crate) focus_pane: Option<String>,
    pub(crate) queue: QueuePanel,
    pub(crate) review: ReviewPanel,
    pub(crate) history: HistoryPanel,
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
            font: FontId::monospace(13.0),
            focused_once: false,
            sessions: SessionsPanel::new(),
            captures: Vec::new(),
            last_poll: Instant::now(),
            side_tab: SideTab::Queue,
            focus_pane: None,
            queue: QueuePanel::default(),
            review: ReviewPanel::new(),
            history: HistoryPanel::new(),
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
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Title(format!("{} - Maestro Native", t)));
            }
        }
        self.poll_captures();
        self.poll_git();
        self.poll_stuck();
        if self.panes.iter().any(|p| p.exit_code.is_none()) {
            // Poll the PTYs at ~30fps; try_recv is cheap when idle.
            ui.ctx().request_repaint_after(Duration::from_millis(33));
        }

        egui::Panel::top("bar").show(ui, |ui| self.top_bar(ui));

        let mut resume: Option<ResumeRequest> = None;
        let mut worktree: Option<WorktreeRequest> = None;
        let mut focus: Option<String> = None;
        let clashes = self.clash_map();
        egui::Panel::left("side").default_size(280.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.selectable_label(self.side_tab == SideTab::Queue, "Queue").clicked() {
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

        let mut kill: Vec<String> = Vec::new();
        let mut restart: Option<usize> = None;
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(c32(BG_DEFAULT))).show(ui, |ui| {
            // Cell metrics from one measured run of glyphs.
            let (cell_w, cell_h) = {
                let g = ui.painter().layout_no_wrap("MMMMMMMMMM".to_string(), self.font.clone(), TEXT);
                (g.rect.width() / 10.0, g.rect.height())
            };
            let avail = ui.available_rect_before_wrap();
            ui.allocate_rect(avail, egui::Sense::hover());

            let n = self.panes.len().max(1);
            let cols = (n as f32).sqrt().ceil() as usize;
            let rows = n.div_ceil(cols);
            let cw = avail.width() / cols as f32;
            let ch = avail.height() / rows as f32;

            let font = self.font.clone();
            let want_focus = self.focus_pane.clone();
            let git_changed = self.git_changed.clone();
            let ShellApp { panes, registry, focused_once, focus_pane, .. } = self;
            for (i, pane) in panes.iter_mut().enumerate() {
                let r = i / cols;
                let c = i % cols;
                let cell_rect = Rect::from_min_size(
                    Pos2::new(avail.left() + c as f32 * cw, avail.top() + r as f32 * ch),
                    Vec2::new(cw, ch),
                );
                let head = Rect::from_min_size(cell_rect.min, Vec2::new(cell_rect.width(), HEADER_H));
                let body = Rect::from_min_max(Pos2::new(cell_rect.left(), cell_rect.top() + HEADER_H), cell_rect.max);
                ui.painter().rect_filled(head, 0.0, SURFACE);
                ui.painter().line_segment([head.left_bottom(), head.right_bottom()], egui::Stroke::new(1.0, BORDER));
                let mut label = pane.name.clone();
                if let Some(w) = pane.worktree.as_ref() {
                    label.push_str(&format!(" · {}", w.branch));
                    if pane.merged {
                        label.push_str(" · merged");
                    }
                }
                if let Some(sid) = pane.session_id.as_deref() {
                    label.push_str(&format!(" · {}", sid.chars().take(8).collect::<String>()));
                }
                if let Some(code) = pane.exit_code {
                    label.push_str(&format!(" · exited ({code})"));
                }
                // The badge and buttons own the right side: cut the label to
                // fit what's left, never under them.
                let max_chars = ((head.width() - 340.0) / 7.0).floor().max(8.0) as usize;
                let label: String = if label.chars().count() > max_chars {
                    format!("{}…", label.chars().take(max_chars.saturating_sub(1)).collect::<String>())
                } else {
                    label
                };
                ui.painter().text(
                    Pos2::new(head.left() + 8.0, head.top() + 4.0),
                    egui::Align2::LEFT_TOP,
                    label,
                    FontId::proportional(12.0),
                    TEXT,
                );
                let changed = git_changed.get(&pane_cwd(pane)).map(|f| f.len() as u32);
                let (dot, word) = state_badge(pane_status(pane, now_ms(), changed).state);
                let word_w = ui.painter().layout_no_wrap(word.to_string(), FontId::proportional(12.0), DIM).rect.width();
                let word_x = (head.right() - 124.0 - word_w).max(head.left() + 8.0);
                ui.painter().circle_filled(Pos2::new(word_x - 8.0, head.top() + 12.0), 4.0, dot);
                ui.painter().text(
                    Pos2::new(word_x, head.top() + 4.0),
                    egui::Align2::LEFT_TOP,
                    word,
                    FontId::proportional(12.0),
                    DIM,
                );
                let btn = |ui: &mut egui::Ui, x: f32, text: &str| ui.put(Rect::from_min_size(Pos2::new(x, head.top() + 2.0), Vec2::new(52.0, 20.0)), egui::Button::new(text));
                if pane.exit_code.is_some() && btn(ui, head.right() - 116.0, "Restart").clicked() {
                    restart = Some(i);
                }
                if btn(ui, head.right() - 58.0, "Kill").clicked() {
                    kill.push(pane.id.clone());
                }
                pane.paint(ui, body, registry, &font, (cell_w, cell_h));
                if !*focused_once {
                    *focused_once = true;
                    ui.memory_mut(|m| m.request_focus(ui.make_persistent_id(("term", &pane.id))));
                }
                if want_focus.as_deref() == Some(pane.id.as_str()) {
                    ui.memory_mut(|m| m.request_focus(ui.make_persistent_id(("term", &pane.id))));
                    *focus_pane = None;
                }
            }
        });

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
}
