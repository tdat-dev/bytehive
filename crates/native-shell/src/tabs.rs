//! Projects as tabs, like a browser's: one tab per open project on the top
//! bar, All first for every agent at once, + to open another folder, x (or a
//! middle click) to close one. A tab shows how many agents work there and a
//! dot when one of them needs you. New agent starts agents in the project
//! on show.

use eframe::egui::{self, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use supervise::task_state::TaskState;

use crate::app::ShellApp;
use crate::icons::{paint_in, Icon};
use crate::projects::{pane_home, project_name};
use crate::projects_ui::ProjectAction;
use crate::style::{medium, PANEL};
use crate::theme::{AMBER, DIM, INK3, TEXT};

/// Height of a tab.
pub const TAB_H: f32 = 34.0;

/// One tab as drawn: its project (None for All), title, agents, needs you.
pub struct Tab {
    pub dir: Option<String>,
    pub title: String,
    pub agents: usize,
    pub needs: bool,
}

/// What a tab click asks for.
enum TabAct {
    Show(Option<String>),
    Close(String),
}

/// A tab's width for its words, kept between `min` and 220.
fn tab_width(ui: &egui::Ui, t: &Tab, closable: bool, min: f32) -> f32 {
    let g = ui.painter().layout_no_wrap(t.title.clone(), medium(13.0), TEXT);
    let count = if t.agents > 0 { 26.0 } else { 0.0 };
    (14.0 + if t.needs { 14.0 } else { 0.0 } + g.size().x + count + if closable { 34.0 } else { 14.0 }).clamp(min, 220.0)
}

/// Paint one tab at `rect`; the act it was asked for, if any.
fn tab(ui: &mut egui::Ui, rect: Rect, t: &Tab, on: bool) -> Option<TabAct> {
    let id = ui.make_persistent_id(("tab", &t.dir));
    let r = ui.interact(rect, id, Sense::click());
    let hot = r.hovered();
    let p = ui.painter();
    let fill = if on { PANEL } else if hot { Color32::from_white_alpha(10) } else { Color32::TRANSPARENT };
    p.rect_filled(rect, CornerRadius { nw: 10, ne: 10, sw: 0, se: 0 }, fill);
    if on {
        p.rect_stroke(rect, CornerRadius { nw: 10, ne: 10, sw: 0, se: 0 }, Stroke::new(1.0, Color32::from_white_alpha(18)), egui::StrokeKind::Inside);
    }
    let ink = if on { TEXT } else if hot { DIM } else { INK3 };
    let mut x = rect.left() + 14.0;
    if t.needs {
        p.circle_filled(Pos2::new(x + 3.0, rect.center().y), 3.5, AMBER);
        x += 14.0;
    }
    let closable = t.dir.is_some() && (on || hot);
    let right = rect.right() - if closable { 28.0 } else { 10.0 };
    let count_w = if t.agents > 0 { 26.0 } else { 0.0 };
    let mut job = egui::text::LayoutJob::simple_singleline(t.title.clone(), medium(13.0), ink);
    job.wrap = egui::text::TextWrapping::truncate_at_width((right - count_w - x).max(10.0));
    let g = p.layout_job(job);
    let gw = g.size().x;
    p.galley(Pos2::new(x, rect.center().y - g.size().y / 2.0), g, ink);
    if t.agents > 0 {
        let n = p.layout_no_wrap(t.agents.to_string(), FontId::new(11.0, egui::FontFamily::Name("monomedium".into())), INK3);
        let chip = Rect::from_min_size(Pos2::new(x + gw + 6.0, rect.center().y - 8.0), Vec2::new(n.size().x + 10.0, 16.0));
        p.rect_filled(chip, 8.0, Color32::from_white_alpha(14));
        p.galley(chip.center() - n.size() / 2.0, n, INK3);
    }
    if closable {
        let at = Rect::from_center_size(Pos2::new(rect.right() - 16.0, rect.center().y), Vec2::splat(20.0));
        let x_r = ui.interact(at, ui.make_persistent_id(("tab-x", &t.dir)), Sense::click());
        if x_r.hovered() {
            ui.painter().rect_filled(at, 6.0, Color32::from_white_alpha(20));
        }
        paint_in(ui.painter(), at.shrink(4.0), 12.0, Icon::Close, if x_r.hovered() { TEXT } else { INK3 }, 1.6);
        if x_r.on_hover_text("Close project (its agents keep running)").clicked() {
            return t.dir.clone().map(TabAct::Close);
        }
    }
    if r.middle_clicked() {
        return t.dir.clone().map(TabAct::Close);
    }
    let r = r.on_hover_text(t.dir.as_deref().unwrap_or("Every project"));
    r.clicked().then(|| TabAct::Show(t.dir.clone()))
}

impl ShellApp {
    /// The tabs as they stand: All, then every open project.
    pub(crate) fn project_tabs(&self) -> Vec<Tab> {
        let now = crate::app::now_ms();
        let agents: Vec<(String, TaskState)> = self.panes.iter().filter(|p| p.cli.is_some()).map(|p| (pane_home(p), crate::states::pane_status(p, now, None).state)).collect();
        let needy = |s: &TaskState| matches!(s, TaskState::Needs | TaskState::Blocked);
        let mut tabs = vec![Tab { dir: None, title: "All".into(), agents: agents.len(), needs: agents.iter().any(|(_, s)| needy(s)) }];
        for d in &self.projects.dirs {
            let mine: Vec<&TaskState> = agents.iter().filter(|(h, _)| self.projects.owner(h).is_some_and(|o| o.eq_ignore_ascii_case(d))).map(|(_, s)| s).collect();
            tabs.push(Tab { dir: Some(d.clone()), title: project_name(d), agents: mine.len(), needs: mine.iter().any(|s| needy(s)) });
        }
        tabs
    }

    /// The tab strip in `strip` (the top bar's free room).
    pub(crate) fn tab_strip(&mut self, ui: &mut egui::Ui, strip: Rect) {
        let tabs = self.project_tabs();
        let plus_w = 34.0;
        // Room shared out like a browser's: tabs shrink before they hide.
        let room = (strip.width() - plus_w - 4.0).max(0.0);
        let want: Vec<f32> = tabs.iter().map(|t| tab_width(ui, t, t.dir.is_some(), 64.0)).collect();
        let total: f32 = want.iter().sum::<f32>() + 2.0 * tabs.len() as f32;
        let scale = if total > room { room / total } else { 1.0 };
        let mut x = strip.left();
        let mut act = None;
        for (t, w) in tabs.iter().zip(&want) {
            let w = (w * scale).max(40.0);
            let rect = Rect::from_min_size(Pos2::new(x, strip.bottom() - TAB_H), Vec2::new(w, TAB_H));
            // The start screen is a page of its own: no tab is on show there.
            let on = !self.extras.home
                && match (&t.dir, &self.projects.filter) {
                    (None, None) => true,
                    (Some(d), Some(f)) => d.eq_ignore_ascii_case(f),
                    _ => false,
                };
            if let Some(a) = tab(ui, rect, t, on) {
                act = Some(a);
            }
            x += w + 2.0;
        }
        let plus = Rect::from_min_size(Pos2::new(x + 2.0, strip.bottom() - TAB_H + 3.0), Vec2::splat(TAB_H - 6.0));
        let pr = ui.interact(plus, ui.make_persistent_id("tab-plus"), Sense::click());
        if pr.hovered() {
            ui.painter().rect_filled(plus, 8.0, Color32::from_white_alpha(18));
        }
        let c = plus.center();
        let ink = if pr.hovered() { TEXT } else { INK3 };
        ui.painter().line_segment([c - Vec2::new(6.0, 0.0), c + Vec2::new(6.0, 0.0)], Stroke::new(1.6, ink));
        ui.painter().line_segment([c - Vec2::new(0.0, 6.0), c + Vec2::new(0.0, 6.0)], Stroke::new(1.6, ink));
        if pr.on_hover_text("Open a project folder").clicked() {
            if let Some(dir) = crate::projects_ui::pick_folder(self.current_project().as_deref().unwrap_or("")) {
                // Opening shows it: its tab becomes the one on show.
                self.extras.home = false;
                self.apply_project(ProjectAction::Open(dir));
            }
        }
        match act {
            // Any tab leaves the start screen, the one it came from too.
            Some(TabAct::Show(dir)) => {
                self.extras.home = false;
                if dir != self.projects.filter {
                    self.apply_project(ProjectAction::Filter(dir));
                }
            }
            Some(TabAct::Close(dir)) => self.apply_project(ProjectAction::Close(dir)),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_comes_first_then_each_open_project() {
        let mut app = ShellApp::new().unwrap();
        app.projects.open("D:/one");
        app.projects.open("D:/two");
        let tabs = app.project_tabs();
        // The test app may hold projects already: All first, the new ones last.
        let titles: Vec<&str> = tabs.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles[0], "All");
        assert_eq!(&titles[titles.len() - 2..], ["one", "two"]);
        assert!(tabs[0].dir.is_none());
        assert_eq!(tabs[0].agents, 0, "the test app's shell is no agent");
    }

    #[test]
    fn the_strip_and_row_paint() {
        let mut app = ShellApp::new().unwrap();
        app.projects.open("D:/one");
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            app.tab_strip(ui, Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 60.0)));
        })
        .drop_without_applying_deltas();
        assert!(app.panes.len() == 1, "painting starts nothing");
    }
}
