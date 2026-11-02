//! Projects as tabs, drawn like Chrome's: one tab per open project on the
//! top bar, All first for every agent at once, + to open another folder, x
//! (or a middle click) to close one. The tab on show wears the page's colour
//! and flares into it; the others sit on the strip with thin dividers. A tab shows how many agents work there and a
//! dot when one of them needs you. New agent starts agents in the project
//! on show.

use eframe::egui::{self, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use supervise::task_state::TaskState;

use crate::app::ShellApp;
use crate::icons::{paint_in, Icon};
use crate::projects::{pane_home, project_name};
use crate::projects_ui::ProjectAction;
use crate::style::medium;
use crate::theme::{AMBER, DIM, INK3, TEXT};

/// Height of a tab, and the strip's own colour: a shade lighter than the page,
/// so the tab on show reads as part of the page below it, as in Chrome.
pub const TAB_H: f32 = 34.0;
pub const FRAME: Color32 = Color32::from_rgb(22, 23, 27);
/// The page under the tabs (the app's ground): the tab on show wears it.
const PAGE: Color32 = crate::theme::BG;
/// How far a tab on show flares out at its foot, and its top corners.
const FLARE: f32 = 8.0;

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

/// The tab on show: rounded at the top, flaring into the page at its foot.
fn paint_active(p: &egui::Painter, rect: Rect) {
    p.rect_filled(rect, CornerRadius { nw: FLARE as u8, ne: FLARE as u8, sw: 0, se: 0 }, PAGE);
    for (x, side) in [(rect.left(), -1.0), (rect.right(), 1.0)] {
        // A square of page colour beside the foot, with a quarter circle of
        // strip colour cut out of it: the concave flare.
        let sq = Rect::from_min_max(Pos2::new(x.min(x + side * FLARE), rect.bottom() - FLARE), Pos2::new(x.max(x + side * FLARE), rect.bottom()));
        p.rect_filled(sq, 0.0, PAGE);
        p.circle_filled(Pos2::new(x + side * FLARE, rect.bottom() - FLARE), FLARE, FRAME);
    }
}

/// Paint one tab at `rect`; the act it was asked for, if any. `sep`: draw the
/// thin divider at its left edge.
fn tab(ui: &mut egui::Ui, rect: Rect, t: &Tab, on: bool, sep: bool) -> Option<TabAct> {
    let id = ui.make_persistent_id(("tab", &t.dir));
    let r = ui.interact(rect, id, Sense::click());
    let hot = r.hovered();
    let p = ui.painter().clone();
    if on {
        paint_active(&p, rect);
    } else if hot {
        p.rect_filled(rect.shrink2(Vec2::new(3.0, 4.0)).translate(Vec2::new(0.0, -1.0)), 8.0, Color32::from_white_alpha(12));
    } else if sep {
        p.vline(rect.left(), (rect.top() + 9.0)..=(rect.bottom() - 9.0), Stroke::new(1.0, Color32::from_white_alpha(28)));
    }
    let ink = if on { TEXT } else if hot { DIM } else { INK3 };
    // The favicon: a folder, or the grid for All; the needs-you dot rides on it.
    let icon = Rect::from_min_size(Pos2::new(rect.left() + 12.0, rect.center().y - 8.0), Vec2::splat(16.0));
    paint_in(&p, icon, 16.0, if t.dir.is_some() { Icon::Folder } else { Icon::Grid }, ink, 1.5);
    if t.needs {
        p.circle_filled(icon.right_top() + Vec2::new(-1.0, 2.0), 3.5, AMBER);
    }
    let closable = t.dir.is_some() && (on || rect.width() >= 96.0);
    let right = rect.right() - if closable { 30.0 } else { 10.0 };
    let count = (t.agents > 0).then(|| p.layout_no_wrap(t.agents.to_string(), FontId::new(11.0, egui::FontFamily::Name("monomedium".into())), INK3));
    let count_w = count.as_ref().map_or(0.0, |g| g.size().x + 8.0);
    let x = icon.right() + 8.0;
    let mut job = egui::text::LayoutJob::simple_singleline(t.title.clone(), medium(12.5), ink);
    job.wrap = egui::text::TextWrapping::truncate_at_width((right - count_w - x).max(8.0));
    let g = p.layout_job(job);
    p.galley(Pos2::new(x, rect.center().y - g.size().y / 2.0), g, ink);
    if let Some(n) = count {
        p.galley(Pos2::new(right - n.size().x, rect.center().y - n.size().y / 2.0), n, INK3);
    }
    if closable {
        let at = Rect::from_center_size(Pos2::new(rect.right() - 17.0, rect.center().y), Vec2::splat(18.0));
        let x_r = ui.interact(at, ui.make_persistent_id(("tab-x", &t.dir)), Sense::click());
        if x_r.hovered() {
            p.circle_filled(at.center(), 9.0, Color32::from_white_alpha(26));
        }
        paint_in(&p, at.shrink(4.0), 10.0, Icon::Close, if x_r.hovered() { TEXT } else { ink }, 1.6);
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
        let plus_w = 36.0;
        // Every tab one width, as in Chrome: 220 at most, narrower to fit.
        let room = (strip.width() - plus_w - FLARE * 2.0).max(0.0);
        let w = (room / tabs.len().max(1) as f32).clamp(56.0, 220.0);
        let mut x = strip.left() + FLARE;
        let mut act = None;
        let ons: Vec<bool> = tabs
            .iter()
            .map(|t| {
                // The start screen is a page of its own: no tab is on show there.
                !self.extras.home
                    && match (&t.dir, &self.projects.filter) {
                        (None, None) => true,
                        (Some(d), Some(f)) => d.eq_ignore_ascii_case(f),
                        _ => false,
                    }
            })
            .collect();
        for (k, t) in tabs.iter().enumerate() {
            let rect = Rect::from_min_size(Pos2::new(x, strip.bottom() - TAB_H), Vec2::new(w, TAB_H));
            // A divider between two tabs, unless one of them is on show.
            let sep = k > 0 && !ons[k] && !ons[k - 1];
            if let Some(a) = tab(ui, rect, t, ons[k], sep) {
                act = Some(a);
            }
            x += w;
        }
        let plus = Rect::from_center_size(Pos2::new(x + FLARE + 14.0, strip.bottom() - TAB_H / 2.0 - 1.0), Vec2::splat(28.0));
        let pr = ui.interact(plus, ui.make_persistent_id("tab-plus"), Sense::click());
        if pr.hovered() {
            ui.painter().circle_filled(plus.center(), 14.0, Color32::from_white_alpha(18));
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
