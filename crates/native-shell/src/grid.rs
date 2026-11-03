//! The Grid view: every agent's terminal at once, each on its own floating
//! panel like the stage (face, name, state, icon buttons), tiled with the
//! web app's gaps. Focus blows one tile up and lines the rest up as faces on
//! a rail. The Grid keeps its own focus, apart from the Focus view's stage.

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use supervise::task_state::TaskState;

use crate::app::{now_ms, ShellApp};
use crate::icons::{paint_in, Icon};
use crate::inbox::state_label;
use crate::layout::{tile_to_fit, Tile};
use crate::states::pane_status;
use crate::style::{w500, GUTTER, PANEL};
use crate::theme::{state_dot, ADD, AMBER, DEL, DIM, INK3, TEXT};

#[derive(Default)]
pub(crate) struct GridOut {
    pub restart: Option<usize>,
    pub kill: Vec<String>,
    /// Stopped but kept, with Resume, like the web app's stop.
    pub stop: Vec<String>,
}

/// Header height inside a tile, and the gap between tiles: tight, so the
/// terminals get the room; the ring and header keep tiles apart.
const HEAD: f32 = 34.0;
const GAP: f32 = 4.0;
/// The rail of faces beside a focused tile.
const RAIL: f32 = 64.0;

/// A tile's edge: green while working, amber when it needs you.
fn ring(state: TaskState, running: bool) -> Color32 {
    match state {
        TaskState::Working if running => ADD.gamma_multiply(0.35),
        TaskState::Needs | TaskState::Blocked if running => AMBER.gamma_multiply(0.45),
        _ => Color32::from_white_alpha(18),
    }
}

/// A round icon button at a fixed spot (.ibtn); `danger` hovers red.
fn icon_at(ui: &mut egui::Ui, rect: Rect, id: egui::Id, icon: Icon, danger: bool) -> egui::Response {
    let r = ui.interact(rect, id, Sense::click());
    let (fill, ink) = match (r.hovered(), danger) {
        (true, true) => (Color32::from_rgba_unmultiplied(242, 118, 107, 41), DEL),
        (true, false) => (Color32::from_white_alpha(15), TEXT),
        _ => (Color32::TRANSPARENT, INK3),
    };
    ui.painter().rect_filled(rect, rect.height() / 2.0, fill);
    paint_in(ui.painter(), rect, 14.0, icon, ink, 1.6);
    r
}

/// `hidden`: panes the project filter keeps off the grid (they keep running).
pub(crate) fn show_grid(app: &mut ShellApp, ui: &mut egui::Ui, hidden: &std::collections::HashSet<String>) -> GridOut {
    let mut out = GridOut::default();
    let mut toggle: Option<String> = None;
    // A killed or hidden focused pane drops back to the tiles on its own.
    if app.extras.grid_focus.as_ref().is_some_and(|id| hidden.contains(id) || !app.panes.iter().any(|p| &p.id == id)) {
        app.extras.grid_focus = None;
    }
    let shown: Vec<usize> = (0..app.panes.len()).filter(|&i| !hidden.contains(&app.panes[i].id)).collect();
    let focused = app.extras.grid_focus.clone().filter(|_| shown.len() > 1);
    let now = now_ms();
    let changed_by = app.changed_counts();
    let states: Vec<TaskState> = app.panes.iter().map(|p| pane_status(p, now, changed_by.get(&p.id).copied()).state).collect();
    // Under All every tile says its project; a project's tab needs no reminder.
    let projects: Vec<Option<String>> = app
        .panes
        .iter()
        .map(|p| {
            let home = crate::projects::pane_home(p);
            app.projects.filter.is_none().then(|| crate::projects::project_name(app.projects.owner(&home).unwrap_or(&home)))
        })
        .collect();

    let frame = egui::Frame::NONE.inner_margin(egui::Margin { left: 6, right: (GUTTER as i8).min(6), top: 0, bottom: 0 });
    egui::CentralPanel::default().frame(frame).show(ui, |ui| {
        let (cell_w, cell_h) = crate::term_pane::cell_metrics(ui.painter(), &app.font);
        let mut area = ui.available_rect_before_wrap();
        ui.allocate_rect(area, Sense::hover());

        // Focused: the rest as faces on a rail at the right.
        if let Some(fid) = &focused {
            let rail = Rect::from_min_max(Pos2::new(area.right() - RAIL, area.top()), area.max);
            area.max.x = rail.left() - GAP;
            let mut y = rail.top() + 4.0;
            for &i in &shown {
                let p = &app.panes[i];
                if &p.id == fid {
                    continue;
                }
                let at = Rect::from_center_size(Pos2::new(rail.center().x, y + 22.0), Vec2::splat(44.0));
                let r = ui.interact(at, ui.make_persistent_id(("rail", &p.id)), Sense::click());
                if r.hovered() {
                    ui.painter().rect_filled(at.expand(4.0), 14.0, Color32::from_white_alpha(15));
                }
                crate::face::avatar(ui.painter(), at, p.cli, p.custom.as_deref(), &p.name, 12.0, p.exit_code.is_some());
                ui.painter().circle_filled(at.right_bottom() - Vec2::splat(4.0), 5.0, state_dot(states[i]));
                if r.on_hover_text(format!("{}: {}", p.name, state_label(states[i]))).clicked() {
                    toggle = Some(p.id.clone());
                }
                y += 56.0;
            }
        }

        let order: Vec<usize> = match &focused {
            Some(fid) => shown.iter().copied().filter(|&i| &app.panes[i].id == fid).collect(),
            None => shown.clone(),
        };
        let tiles = tile_to_fit(order.len(), area.width(), area.height(), GAP, 0.0, 0.0, 0.0);
        let font = app.font.clone();
        let want_focus = app.focus_pane.clone();
        let multi = shown.len() > 1;
        let ShellApp { panes, registry, focused_once, focus_pane, .. } = app;
        for (slot, &idx) in order.iter().enumerate() {
            let t = tiles.get(slot).copied().unwrap_or(Tile { x: 0.0, y: 0.0, w: 0.0, h: 0.0 });
            let tile = Rect::from_min_size(Pos2::new(area.left() + t.x, area.top() + t.y), Vec2::new(t.w, t.h));
            let pane = &mut panes[idx];
            let state = states[idx];
            let running = pane.exit_code.is_none();
            ui.painter().rect(tile, 10.0, PANEL, Stroke::new(1.0, ring(state, running)), egui::StrokeKind::Inside);

            // Head: face, name, state; the buttons from the right edge in.
            let head = Rect::from_min_size(tile.min, Vec2::new(tile.width(), HEAD));
            let face = Rect::from_min_size(Pos2::new(head.left() + 8.0, head.center().y - 10.0), Vec2::splat(20.0));
            crate::face::avatar(ui.painter(), face, pane.cli, pane.custom.as_deref(), &pane.name, 6.0, !running);
            let mut bx = head.right() - 6.0;
            let mut button = |ui: &mut egui::Ui, icon: Icon, danger: bool, key: &str, tip: &str| {
                bx -= 24.0;
                let at = Rect::from_min_size(Pos2::new(bx, head.center().y - 12.0), Vec2::splat(24.0));
                bx -= 2.0;
                icon_at(ui, at, ui.make_persistent_id((key, &pane.id)), icon, danger).on_hover_text(tip).clicked()
            };
            if button(ui, Icon::Close, true, "grid-kill", "Stop and remove") {
                out.kill.push(pane.id.clone());
            }
            if !running && button(ui, Icon::Refresh, false, "grid-restart", "Start it again") {
                out.restart = Some(idx);
            }
            let staged = focused.as_deref() == Some(pane.id.as_str());
            if multi && button(ui, if staged { Icon::Grid } else { Icon::Focus }, false, "grid-focus", if staged { "Back to all" } else { "Make it big" }) {
                toggle = Some(pane.id.clone());
            }
            let text_left = face.right() + 8.0;
            let room = (bx - 8.0 - text_left).max(0.0);
            let mut job = egui::text::LayoutJob::default();
            job.append(&pane.name, 0.0, egui::TextFormat::simple(w500(13.5), if running { TEXT } else { DIM }));
            if let Some(proj) = &projects[idx] {
                job.append(&format!("  {proj}"), 0.0, egui::TextFormat::simple(w500(12.5), DIM));
            }
            let word = if running { state_label(state) } else { "Stopped" };
            job.append(&format!("  {word}"), 0.0, egui::TextFormat::simple(FontId::proportional(12.0), crate::inbox::line_color(state)));
            let mut extra = Vec::new();
            if pane.recording.is_some() {
                extra.push("recording".to_string());
            }
            if let Some(w) = pane.worktree.as_ref() {
                extra.push(if pane.merged { format!("{} (merged)", w.branch) } else { w.branch.clone() });
            }
            if let Some(usd) = pane.usage.as_ref().and_then(|u| u.usd) {
                extra.push(supervise::pricing::money(usd));
            }
            if let Some(ctx) = pane.usage.as_ref().and_then(|u| u.context_pct) {
                extra.push(format!("{ctx:.0}% context"));
            }
            if !extra.is_empty() {
                job.append(&format!("  \u{00B7}  {}", extra.join("  \u{00B7}  ")), 0.0, egui::TextFormat::simple(FontId::proportional(12.0), INK3));
            }
            job.wrap = egui::text::TextWrapping::truncate_at_width(room);
            let g = ui.painter().layout_job(job);
            ui.painter().galley(Pos2::new(text_left, head.center().y - g.size().y / 2.0), g, TEXT);

            let body = Rect::from_min_max(Pos2::new(tile.left() + 8.0, head.bottom()), Pos2::new(tile.right() - 6.0, tile.bottom() - 6.0));
            pane.paint(ui, body, &mut agent_core::lock(registry), &font, (cell_w, cell_h));
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

    if let Some(id) = toggle {
        app.extras.grid_focus = if app.extras.grid_focus.as_deref() == Some(id.as_str()) { None } else { Some(id) };
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::ShellApp;
    use agent_core::launch::launch_spec;

    /// The whole focus path (rail of faces, the big tile, header buttons)
    /// must run without panicking, even though clicks only happen live.
    #[test]
    fn focus_with_two_panes_paints_without_panic() {
        let mut app = ShellApp::new().unwrap();
        let spec = launch_spec("powershell.exe", &[String::from("-NoLogo")]);
        let id2 = app.spawn_pane("shell", spec, None, None).unwrap();
        app.extras.grid_focus = Some(id2);
        egui::__run_test_ui(|ui| {
            show_grid(&mut app, ui, &Default::default());
        });
        assert!(app.extras.grid_focus.is_some());
    }

    #[test]
    fn tiles_paint_without_focus() {
        let mut app = ShellApp::new().unwrap();
        let spec = launch_spec("powershell.exe", &[String::from("-NoLogo")]);
        app.spawn_pane("shell", spec, None, None).unwrap();
        egui::__run_test_ui(|ui| {
            show_grid(&mut app, ui, &Default::default());
        });
        assert!(app.extras.grid_focus.is_none());
    }
}
