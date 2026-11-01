//! The dock, floating along the bottom: the views (Focus, one agent on the
//! stage; Grid, every agent side by side), the search that opens Ctrl+K,
//! New agent, and round buttons for Sessions, help and Settings.

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};

use crate::app::ShellApp;
use crate::icons::{paint_in, Icon};
use crate::style::{medium, DOCK_H, RAISE};
use crate::theme::{DIM, INK3, TEXT};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    Queue,
    Split,
}

const HOVER: Color32 = Color32::from_rgb(0x1d, 0x1e, 0x23);

fn float_shadow() -> egui::Shadow {
    egui::Shadow { offset: [0, 18], blur: 40, spread: 0, color: Color32::from_black_alpha(140) }
}

/// Paint the dock's lift under `rect`: the shadow and a faint white ring.
fn lift(ui: &egui::Ui, rect: Rect, radius: f32, fill: Color32) {
    let p = ui.painter();
    p.add(float_shadow().as_shape(rect, radius));
    p.rect_filled(rect, radius, fill);
    p.rect_stroke(rect, radius, Stroke::new(1.0, Color32::from_white_alpha(20)), egui::StrokeKind::Inside);
}

/// Focus | Grid (.seg): which one was clicked.
fn views(ui: &mut egui::Ui, now: View, narrow: bool) -> Option<View> {
    let items = [(View::Queue, "Focus", Icon::Focus), (View::Split, "Grid", Icon::Grid)];
    let widths: Vec<f32> = items
        .iter()
        .map(|(_, l, _)| if narrow { 40.0 } else { 14.0 + 15.0 + 7.0 + ui.painter().layout_no_wrap(l.to_string(), crate::style::w500(13.0), DIM).size().x + 14.0 })
        .collect();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(widths.iter().sum::<f32>() + 8.0 + 2.0, 40.0), Sense::hover());
    lift(ui, rect, 20.0, RAISE);
    let mut x = rect.left() + 4.0;
    let mut hit = None;
    for ((view, label, icon), w) in items.into_iter().zip(widths) {
        let b = Rect::from_min_size(Pos2::new(x, rect.top() + 4.0), Vec2::new(w, 32.0));
        let r = ui.interact(b, ui.make_persistent_id(("view", label)), Sense::click());
        let on = view == now;
        if on || r.hovered() {
            ui.painter().rect_filled(b, 16.0, Color32::from_white_alpha(if on { 26 } else { 13 }));
        }
        let ink = if on || r.hovered() { TEXT } else { DIM };
        let ir = Rect::from_min_size(Pos2::new(if narrow { b.center().x - 7.5 } else { b.left() + 14.0 }, b.center().y - 7.5), Vec2::splat(15.0));
        paint_in(ui.painter(), ir, 15.0, icon, ink, 1.6);
        if !narrow {
            ui.painter().text(Pos2::new(ir.right() + 7.0, b.center().y), egui::Align2::LEFT_CENTER, label, crate::style::w500(13.0), ink);
        }
        if r.on_hover_text(if view == View::Queue { "One agent, large" } else { "Every agent side by side" }).clicked() {
            hit = Some(view);
        }
        x += w + 2.0;
    }
    hit
}

/// The search field (.kbtn): a magnifier, the hint, the key.
fn search(ui: &mut egui::Ui, w: f32, narrow: bool) -> bool {
    let (rect, r) = ui.allocate_exact_size(Vec2::new(w, 40.0), Sense::click());
    lift(ui, rect, 20.0, RAISE);
    let p = ui.painter();
    let ink = if r.hovered() { DIM } else { INK3 };
    let ir = Rect::from_min_size(Pos2::new(rect.left() + 14.0, rect.center().y - 7.5), Vec2::splat(15.0));
    paint_in(p, ir, 15.0, Icon::Search, ink, 2.0);
    let key_w = if narrow { 0.0 } else { 58.0 };
    let hint = p.layout(
        "Jump to an agent or run a command".into(),
        FontId::proportional(13.0),
        ink,
        f32::INFINITY,
    );
    let room = rect.right() - 8.0 - key_w - (ir.right() + 10.0);
    let mut job = egui::text::LayoutJob::simple_singleline(hint.text().to_string(), FontId::proportional(13.0), ink);
    job.wrap = egui::text::TextWrapping::truncate_at_width(room);
    p.galley(Pos2::new(ir.right() + 10.0, rect.center().y - hint.size().y / 2.0), p.layout_job(job), ink);
    if !narrow {
        let k = p.layout_no_wrap("Ctrl+K".into(), FontId::monospace(11.0), INK3);
        let kr = Rect::from_min_size(Pos2::new(rect.right() - 8.0 - k.size().x - 12.0, rect.center().y - 10.0), Vec2::new(k.size().x + 12.0, 20.0));
        p.rect_stroke(kr, 6.0, Stroke::new(1.0, Color32::from_white_alpha(31)), egui::StrokeKind::Inside);
        p.galley(kr.center() - k.size() / 2.0, k, INK3);
    }
    r.on_hover_text("Agents, commands and settings (Ctrl+K)").clicked()
}

/// The white New agent pill.
fn new_agent(ui: &mut egui::Ui, narrow: bool) -> egui::Response {
    let g = ui.painter().layout_no_wrap("New agent".into(), medium(13.0), Color32::BLACK);
    let pad = if narrow { 12.0 } else { 18.0 };
    let (rect, r) = ui.allocate_exact_size(Vec2::new(g.size().x + pad * 2.0, 40.0), Sense::click());
    let fill = if r.hovered() { Color32::from_rgb(0xe9, 0xe9, 0xec) } else { Color32::WHITE };
    ui.painter().add(float_shadow().as_shape(rect, 20.0));
    ui.painter().rect_filled(rect, 20.0, fill);
    ui.painter().galley(rect.center() - g.size() / 2.0, g, Color32::BLACK);
    r.on_hover_text("Start an agent on a task (Ctrl+N)")
}

/// A round dock button (.rbtn) with an icon, or "?" for help.
fn round(ui: &mut egui::Ui, icon: Option<Icon>, on: bool) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::click());
    let hot = r.hovered() || on;
    lift(ui, rect, 20.0, if hot { HOVER } else { RAISE });
    let ink = if hot { TEXT } else { DIM };
    match icon {
        Some(i) => paint_in(ui.painter(), rect, 17.0, i, ink, 1.8),
        None => {
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "?", medium(15.0), ink);
        }
    }
    r
}

const KEYS: [(&str, &str); 7] = [
    ("Ctrl+K", "Jump to an agent or run a command"),
    ("Ctrl+N", "New agent"),
    ("Ctrl+1..9", "Put that agent on the stage"),
    ("Ctrl+Shift+F", "Find in the terminal"),
    ("Ctrl+= / Ctrl+-", "Bigger or smaller terminal text"),
    ("Ctrl+,", "Settings"),
    ("Right-click", "Copy, paste, select all in a terminal"),
];

impl ShellApp {
    pub(crate) fn dock(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("dock").exact_size(DOCK_H).frame(egui::Frame::NONE).show_separator_line(false).show(ui, |ui| {
            let narrow = ui.ctx().content_rect().width() <= 1400.0;
            let w = self.extras.dock_w;
            ui.add_space(DOCK_H - 18.0 - 40.0 - 8.0);
            ui.horizontal(|ui| {
                ui.add_space(((ui.available_width() - w) / 2.0).max(16.0));
                let start = ui.cursor().left();
                ui.spacing_mut().item_spacing.x = 10.0;
                let now = if self.board.open { None } else { Some(self.extras.view) };
                if let Some(v) = views(ui, now.unwrap_or(View::Queue), narrow) {
                    self.extras.view = v;
                    self.board.open = false;
                    self.extras.home = false;
                }
                let sw = if narrow { 220.0 } else { 320.0_f32.min(ui.ctx().content_rect().width() * 0.24) };
                if search(ui, sw, narrow) {
                    self.palette.toggle();
                }
                if new_agent(ui, narrow).clicked() {
                    self.run_command("new");
                }
                if round(ui, Some(Icon::History), self.side_tab == crate::commands::SideTab::Sessions).on_hover_text("Sessions: pick up an earlier conversation").clicked() {
                    self.run_command("tab:sessions");
                }
                let help = round(ui, None, false).on_hover_text("Keys");
                egui::Popup::menu(&help).show(|ui| {
                    ui.set_min_width(300.0);
                    egui::Grid::new("keys").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                        for (k, what) in KEYS {
                            ui.label(egui::RichText::new(k).font(FontId::monospace(12.0)).color(TEXT));
                            ui.label(egui::RichText::new(what).size(12.5).color(DIM));
                            ui.end_row();
                        }
                    });
                });
                if round(ui, Some(Icon::Gear), self.settings.open).on_hover_text("Settings (Ctrl+,)").clicked() {
                    self.run_command("settings");
                }
                self.extras.dock_w = ui.cursor().left() - start - 10.0;
            });
        });
    }
}
