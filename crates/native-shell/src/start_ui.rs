//! The start screen (Home), as the web app had it: one job, say what an
//! agent should do and start it. A greeting, one sentence about what waits
//! on you, the composer (the job, its folder, the CLI, how many), jobs to
//! try, the agents waiting on you, and the projects. First run swaps the
//! projects for three steps. The composer is the only loud thing here.

use eframe::egui::{self, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Vec2};

use crate::icons::{paint_in, Icon};
use crate::newtask::NewTask;
use crate::projects::project_name;
use crate::projects_ui::pick_folder;
use crate::sessions_ui::Cli;
use crate::style::{medium, PANEL, RAISE};
use crate::theme::{AMBER, DIM, INK3, TEXT};

/// Jobs to try: a short label and the job it fills in.
const STARTERS: [(&str, &str); 4] = [
    ("Explain this codebase", "Explain how this codebase is put together, and where to start reading"),
    ("Find and fix a bug", "Find a bug, fix it, and add a test that would have caught it"),
    ("Add missing tests", "Add tests for the code that has none"),
    ("Review the last commit", "Review the last commit and point out anything risky"),
];

pub enum StartAction {
    /// Open this folder as a project.
    Open(String),
    /// Start agents on a job.
    Start(NewTask),
    /// A plain shell, no agent.
    Shell,
    /// Put this agent on the stage.
    Show(String),
}

/// What the composer holds between frames.
pub struct Home {
    pub job: String,
    pub dir: Option<String>,
    pub cli: Cli,
    pub count: usize,
}

impl Default for Home {
    fn default() -> Self {
        Home { job: String::new(), dir: None, cli: Cli::Claude, count: 1 }
    }
}

/// A project as the start screen lists it.
pub struct ProjectCard {
    pub dir: String,
    pub agents: usize,
    pub needs: usize,
}

/// An agent waiting on you: (pane id, name, project, what it wants).
pub type Waiting = (String, String, String, String);

/// What the start screen shows.
pub struct HomeView<'a> {
    pub projects: &'a [ProjectCard],
    pub waiting: &'a [Waiting],
    pub working: usize,
    pub review: usize,
    pub installed: &'a [Cli],
    /// How much room the chosen CLI's plan has, when little.
    pub room: String,
    pub hour: u32,
}

/// "Good morning" and friends.
pub fn greeting(hour: u32) -> &'static str {
    match hour {
        0..5 => "Working late",
        5..12 => "Good morning",
        12..18 => "Good afternoon",
        _ => "Good evening",
    }
}

/// The one sentence under the greeting: what, if anything, waits on you.
pub fn status_sentence(v: &HomeView) -> String {
    match v.waiting {
        [one] => format!("{} is waiting on you.", one.1),
        [_, _, ..] => format!("{} agents are waiting on you.", v.waiting.len()),
        [] if v.review > 0 => {
            if v.review == 1 { "One agent has finished and is ready to review.".into() } else { format!("{} agents have finished and are ready to review.", v.review) }
        }
        [] if v.working > 0 => {
            if v.working == 1 { "One agent is working. Nothing needs you yet.".into() } else { format!("{} agents are working. Nothing needs you yet.", v.working) }
        }
        [] if v.projects.is_empty() => "Pick a folder, say what you want done, and an agent starts on it.".into(),
        [] => "Nothing is running. What should an agent do next?".into(),
    }
}

/// A path as people say it: the home folder reads as ~.
pub fn short_path(dir: &str) -> String {
    let home = std::env::var("USERPROFILE").unwrap_or_default();
    if !home.is_empty() && dir.to_lowercase().starts_with(&home.to_lowercase()) {
        return format!("~{}", &dir[home.len()..]);
    }
    dir.to_string()
}

fn logo(ui: &egui::Ui) -> egui::TextureId {
    let id = egui::Id::unique("bytehive-logo-128");
    if let Some(t) = ui.ctx().data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return t.id();
    }
    let img = image::load_from_memory(include_bytes!("../assets/icons/128x128.png")).map(|i| i.into_rgba8()).unwrap_or_default();
    let color = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
    let t = ui.ctx().load_texture("logo-128", color, egui::TextureOptions::LINEAR);
    let tid = t.id();
    ui.ctx().data_mut(|d| d.insert_temp(id, t));
    tid
}

/// A pill chip of the composer bar (.st-chip) holding a menu.
fn chip_menu(ui: &mut egui::Ui, id: &str, icon: Option<Icon>, text: &str, add: impl FnOnce(&mut egui::Ui)) {
    let g = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(13.0), DIM);
    let lead = if icon.is_some() { 21.0 } else { 0.0 };
    let (rect, r) = ui.allocate_exact_size(Vec2::new(10.0 + lead + g.size().x + 10.0, 32.0), Sense::click());
    let hot = r.hovered();
    ui.painter().rect_filled(rect, 16.0, Color32::from_white_alpha(if hot { 26 } else { 15 }));
    let ink = if hot { TEXT } else { DIM };
    if let Some(i) = icon {
        paint_in(ui.painter(), Rect::from_min_size(Pos2::new(rect.left() + 10.0, rect.center().y - 7.5), Vec2::splat(15.0)), 15.0, i, ink, 1.4);
    }
    ui.painter().galley(Pos2::new(rect.left() + 10.0 + lead, rect.center().y - g.size().y / 2.0), g, ink);
    egui::Popup::menu(&r).id(egui::Id::unique(id)).show(add);
}

/// An outlined pill (.st-starter).
fn starter(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let g = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(13.0), DIM);
    let (rect, r) = ui.allocate_exact_size(Vec2::new(g.size().x + 26.0, 31.0), Sense::click());
    let hot = r.hovered();
    if hot {
        ui.painter().rect_filled(rect, 16.0, Color32::from_white_alpha(10));
    }
    ui.painter().rect_stroke(rect, 16.0, Stroke::new(1.0, Color32::from_white_alpha(18)), egui::StrokeKind::Inside);
    ui.painter().galley(rect.center() - g.size() / 2.0, g, if hot { TEXT } else { DIM });
    r
}

/// A project card (.st-proj); `quiet` is the outlined kind for actions.
fn card(ui: &mut egui::Ui, w: f32, glyph: Result<Icon, &str>, name: &str, sub: &str, note: Option<(String, bool)>, quiet: bool) -> egui::Response {
    // One height for every card, so a row lines up.
    let (rect, r) = ui.allocate_exact_size(Vec2::new(w, 118.0), Sense::click());
    let p = ui.painter();
    let fill = if r.hovered() { RAISE } else if quiet { Color32::TRANSPARENT } else { PANEL };
    p.rect_filled(rect, 14.0, fill);
    p.rect_stroke(rect, 14.0, Stroke::new(1.0, Color32::from_white_alpha(if quiet { 26 } else { 18 })), egui::StrokeKind::Inside);
    let tile = Rect::from_min_size(rect.min + Vec2::splat(14.0), Vec2::splat(28.0));
    p.rect_filled(tile, 8.0, Color32::from_white_alpha(15));
    match glyph {
        Ok(i) => paint_in(p, tile, 15.0, i, DIM, 1.4),
        Err(t) => {
            p.text(tile.center(), egui::Align2::CENTER_CENTER, t, FontId::new(13.0, egui::FontFamily::Name("monomedium".into())), DIM);
        }
    }
    let x = rect.left() + 14.0;
    let width = rect.width() - 28.0;
    let fit = |text: &str, font: FontId, c: Color32| {
        let mut job = egui::text::LayoutJob::simple_singleline(text.to_string(), font, c);
        job.wrap = egui::text::TextWrapping::truncate_at_width(width);
        p.layout_job(job)
    };
    p.galley(Pos2::new(x, tile.bottom() + 10.0), fit(name, medium(14.0), TEXT), TEXT);
    let sub_font = if quiet { FontId::proportional(12.0) } else { FontId::monospace(12.0) };
    p.galley(Pos2::new(x, tile.bottom() + 32.0), fit(sub, sub_font, INK3), INK3);
    if let Some((n, needs)) = note {
        let c = if needs { AMBER } else { INK3 };
        p.galley(Pos2::new(x, tile.bottom() + 56.0), fit(&n, FontId::proportional(12.5), c), c);
    }
    r
}

fn h2(ui: &mut egui::Ui, text: &str) {
    ui.add_space(40.0);
    ui.label(RichText::new(text).size(13.0).color(INK3));
    ui.add_space(8.0);
}

pub fn show_start(ui: &mut egui::Ui, home: &mut Home, v: &HomeView) -> Option<StartAction> {
    let mut out = None;
    let first_run = v.projects.is_empty();
    if home.dir.is_none() {
        home.dir = v.projects.first().map(|p| p.dir.clone());
    }
    if !v.installed.is_empty() && !v.installed.contains(&home.cli) {
        home.cli = v.installed[0];
    }
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let width = 760.0_f32.min(ui.available_width() - 48.0);
        // Centered with space: a Margin is an i8 and tops out at 127px.
        let pad = ((ui.available_width() - width) / 2.0).max(24.0);
        ui.add_space(48.0);
        ui.horizontal_top(|ui| {
            ui.add_space(pad);
            ui.vertical(|ui| {
                ui.set_width(width);
                body(ui, home, v, first_run, width, &mut out);
                ui.add_space(56.0);
            });
        });
    });
    out
}

fn body(ui: &mut egui::Ui, home: &mut Home, v: &HomeView, first_run: bool, width: f32, out: &mut Option<StartAction>) {
    ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
    ui.horizontal(|ui| {
        ui.add(egui::Image::new((logo(ui), Vec2::splat(44.0))));
        ui.add_space(4.0);
        ui.label(RichText::new("ByteHive").font(medium(22.0)).extra_letter_spacing(-0.44).color(TEXT));
    });
    ui.add_space(28.0);
    ui.label(RichText::new(format!("{}.", greeting(v.hour))).font(medium(40.0)).extra_letter_spacing(-1.2).color(TEXT));
    ui.add_space(10.0);
    let needs = !v.waiting.is_empty();
    ui.label(RichText::new(status_sentence(v)).size(17.0).color(if needs { AMBER } else { INK3 }));
    ui.add_space(28.0);
    composer(ui, home, v, out);
    if !v.room.is_empty() {
        ui.add_space(8.0);
        ui.label(RichText::new(&v.room).size(12.5).color(AMBER));
    }
    ui.add_space(14.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
        for (label, job) in STARTERS {
            if starter(ui, label).on_hover_text(job).clicked() {
                home.job = job.to_string();
            }
        }
    });
    if !v.waiting.is_empty() {
        h2(ui, "Waiting on you");
        for (id, name, project, what) in v.waiting {
            if waiting_row(ui, name, project, what).clicked() {
                *out = Some(StartAction::Show(id.clone()));
            }
            ui.add_space(6.0);
        }
    }
    if first_run {
        h2(ui, "Getting started");
        steps(ui, width, out, home);
        return;
    }
    h2(ui, "Projects");
    let cols = ((width + 10.0) / 230.0).floor().max(1.0);
    let w = (width - 10.0 * (cols - 1.0)) / cols;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(10.0, 10.0);
        for p in v.projects {
            let note = if p.needs > 0 {
                (format!("{} waiting on you", p.needs), true)
            } else if p.agents > 0 {
                (format!("{} agent{}", p.agents, if p.agents == 1 { "" } else { "s" }), false)
            } else {
                ("Not open".into(), false)
            };
            if card(ui, w, Ok(Icon::Folder), &project_name(&p.dir), &short_path(&p.dir), Some(note), false).on_hover_text(&p.dir).clicked() {
                *out = Some(StartAction::Open(p.dir.clone()));
            }
        }
        if card(ui, w, Err("+"), "Open a folder", "Start a new project", None, true).clicked() {
            if let Some(dir) = pick_folder(home.dir.as_deref().unwrap_or("")) {
                *out = Some(StartAction::Open(dir));
            }
        }
        if card(ui, w, Err("\u{203A}_"), "Plain terminal", "A shell, no agent", None, true).clicked() {
            *out = Some(StartAction::Shell);
        }
    });
}

/// The composer (.st-compose): the job, then its folder, CLI, how many, Start.
fn composer(ui: &mut egui::Ui, home: &mut Home, v: &HomeView, out: &mut Option<StartAction>) {
    let shadow = egui::Shadow { offset: [0, 30], blur: 80, spread: 0, color: Color32::from_black_alpha(140) };
    let job_id = egui::Id::unique("st-job");
    let focused = ui.memory(|m| m.focused() == Some(job_id));
    egui::Frame::new()
        .fill(PANEL)
        .corner_radius(20.0)
        .shadow(shadow)
        .stroke(Stroke::new(1.0, Color32::from_white_alpha(if focused { 71 } else { 26 })))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::Frame::NONE.inner_margin(egui::Margin { left: 22, right: 22, top: 20, bottom: 8 }).show(ui, |ui| {
                let edit = egui::TextEdit::multiline(&mut home.job)
                    .id(job_id)
                    .frame(egui::Frame::NONE)
                    .font(FontId::proportional(17.0))
                    .hint_text(RichText::new("What should an agent do?").size(17.0).color(INK3))
                    .desired_rows(3)
                    .desired_width(f32::INFINITY);
                let r = ui.add(edit);
                if r.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift) {
                    home.job = home.job.trim_end_matches('\n').to_string();
                    *out = start(home);
                }
            });
            egui::Frame::NONE.inner_margin(egui::Margin { left: 14, right: 12, top: 10, bottom: 12 }).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let dir_name = home.dir.as_deref().map(project_name).unwrap_or_else(|| "Choose a folder\u{2026}".into());
                    chip_menu(ui, "st-dir", Some(Icon::Folder), &dir_name, |ui| {
                        for p in v.projects {
                            if ui.button(format!("{}    {}", project_name(&p.dir), short_path(&p.dir))).clicked() {
                                home.dir = Some(p.dir.clone());
                            }
                        }
                        if ui.button("Choose a folder\u{2026}").clicked() {
                            if let Some(d) = pick_folder(home.dir.as_deref().unwrap_or("")) {
                                home.dir = Some(d);
                            }
                        }
                    });
                    chip_menu(ui, "st-cli", None, home.cli.name(), |ui| {
                        for c in Cli::all() {
                            let ok = v.installed.is_empty() || v.installed.contains(&c);
                            let label = if ok { c.name().to_string() } else { format!("{}  (not installed)", c.name()) };
                            if ui.add_enabled(ok, egui::Button::new(label)).clicked() {
                                home.cli = c;
                            }
                        }
                    });
                    let count = if home.count == 1 { "1 agent".to_string() } else { format!("{} agents, same job", home.count) };
                    chip_menu(ui, "st-count", None, &count, |ui| {
                        for n in 1..=3 {
                            if ui.button(if n == 1 { "1 agent".to_string() } else { format!("{n} agents, same job") }).clicked() {
                                home.count = n;
                            }
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if go(ui).clicked() {
                            *out = start(home);
                        }
                    });
                });
            });
        });
}

/// Start (.st-go): white, with the Enter key.
fn go(ui: &mut egui::Ui) -> egui::Response {
    let g = ui.painter().layout_no_wrap("Start".into(), crate::style::on_white(13.5), Color32::BLACK);
    let k = ui.painter().layout_no_wrap("\u{21B5}".into(), FontId::monospace(11.0), Color32::from_gray(85));
    let (rect, r) = ui.allocate_exact_size(Vec2::new(16.0 + g.size().x + 8.0 + k.size().x + 10.0 + 12.0, 36.0), Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, 18.0, if r.hovered() { Color32::from_rgb(0xe9, 0xe9, 0xec) } else { Color32::WHITE });
    p.galley(Pos2::new(rect.left() + 16.0, rect.center().y - g.size().y / 2.0), g.clone(), Color32::BLACK);
    let kr = Rect::from_min_size(Pos2::new(rect.left() + 16.0 + g.size().x + 8.0, rect.center().y - 9.0), Vec2::new(k.size().x + 10.0, 18.0));
    p.rect_stroke(kr, 5.0, Stroke::new(1.0, Color32::from_black_alpha(51)), egui::StrokeKind::Inside);
    p.galley(kr.center() - k.size() / 2.0, k, Color32::from_gray(85));
    r.on_hover_text("Start (Enter)")
}

/// The job as a task to start, when there is one and a folder for it.
fn start(home: &mut Home) -> Option<StartAction> {
    if home.job.trim().is_empty() {
        return None;
    }
    let dir = match home.dir.clone() {
        Some(d) => d,
        None => {
            let d = pick_folder("")?;
            home.dir = Some(d.clone());
            d
        }
    };
    let mut t = NewTask::new(dir.clone());
    t.prompt = home.job.trim().to_string();
    t.cli = home.cli;
    t.set_count(home.count);
    // Each agent on its own branch where there is git to branch from.
    t.worktree = agent_core::git::repo_root(std::path::Path::new(&dir)).is_some();
    home.job.clear();
    Some(StartAction::Start(t))
}

/// One agent waiting on you (.st-w).
fn waiting_row(ui: &mut egui::Ui, name: &str, project: &str, what: &str) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 44.0), Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, 14.0, AMBER.gamma_multiply(0.12));
    p.rect_stroke(rect, 14.0, Stroke::new(1.0, if r.hovered() { AMBER } else { AMBER.gamma_multiply(0.3) }), egui::StrokeKind::Inside);
    let c = Pos2::new(rect.left() + 18.0, rect.center().y);
    p.add(egui::Shape::convex_polygon(vec![c + Vec2::new(0.0, -4.5), c + Vec2::new(4.5, 0.0), c + Vec2::new(0.0, 4.5), c + Vec2::new(-4.5, 0.0)], AMBER, Stroke::NONE));
    let n = p.layout_no_wrap(name.to_string(), medium(13.5), TEXT);
    let mut x = rect.left() + 32.0;
    p.galley(Pos2::new(x, rect.center().y - n.size().y / 2.0), n.clone(), TEXT);
    x += n.size().x + 10.0;
    let pr = p.layout_no_wrap(project.to_string(), FontId::proportional(12.5), INK3);
    p.galley(Pos2::new(x, rect.center().y - pr.size().y / 2.0), pr.clone(), INK3);
    x += pr.size().x + 10.0;
    let open = p.layout_no_wrap("Answer".into(), medium(12.5), AMBER);
    let right = rect.right() - 14.0 - open.size().x;
    p.galley(Pos2::new(right, rect.center().y - open.size().y / 2.0), open, AMBER);
    let mut job = egui::text::LayoutJob::simple_singleline(what.to_string(), FontId::proportional(13.0), DIM);
    job.wrap = egui::text::TextWrapping::truncate_at_width((right - 10.0 - x).max(10.0));
    let g = p.layout_job(job);
    p.galley(Pos2::new(x, rect.center().y - g.size().y / 2.0), g, DIM);
    r
}

/// First run: the three steps (.st-steps).
fn steps(ui: &mut egui::Ui, width: f32, out: &mut Option<StartAction>, home: &mut Home) {
    let items = [
        ("Choose a folder", "Any project on your disk. Each agent gets its own git branch there, so your checkout stays clean."),
        ("Say what you want done", "In plain words, in the box above. Pick Claude Code, Codex or another CLI you have installed."),
        ("Answer when it asks", "When an agent wants to run a command or has a question, it moves to the top of the list. Answer it in its terminal."),
    ];
    let cols = if width > 660.0 { 3.0 } else { 1.0 };
    let w = (width - 10.0 * (cols - 1.0)) / cols;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(10.0, 10.0);
        for (i, (title, text)) in items.iter().enumerate() {
            egui::Frame::new().fill(PANEL).corner_radius(14.0).stroke(Stroke::new(1.0, Color32::from_white_alpha(18))).inner_margin(egui::Margin::same(16)).show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.set_width(w - 32.0);
                    ui.set_min_height(118.0);
                    ui.spacing_mut().item_spacing.y = 6.0;
                    let (b, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
                    ui.painter().circle_filled(b.center(), 11.0, Color32::from_white_alpha(20));
                    ui.painter().text(b.center(), egui::Align2::CENTER_CENTER, (i + 1).to_string(), FontId::new(12.0, egui::FontFamily::Name("monomedium".into())), DIM);
                    if i == 0 {
                        let r = ui.add(egui::Label::new(RichText::new(*title).font(medium(14.0)).color(crate::theme::ADD)).sense(Sense::click()));
                        if r.clicked() {
                            if let Some(d) = pick_folder("") {
                                home.dir = Some(d.clone());
                                *out = Some(StartAction::Open(d));
                            }
                        }
                    } else {
                        ui.label(RichText::new(*title).font(medium(14.0)).color(TEXT));
                    }
                    ui.label(RichText::new(*text).size(13.0).color(INK3));
                });
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view<'a>(projects: &'a [ProjectCard], waiting: &'a [Waiting]) -> HomeView<'a> {
        HomeView { projects, waiting, working: 0, review: 0, installed: &[], room: String::new(), hour: 9 }
    }

    #[test]
    fn the_sentence_says_what_waits_on_you() {
        assert_eq!(greeting(9), "Good morning");
        assert_eq!(greeting(23), "Good evening");
        assert!(status_sentence(&view(&[], &[])).starts_with("Pick a folder"));
        let p = [ProjectCard { dir: r"D:\maestro".into(), agents: 0, needs: 0 }];
        assert!(status_sentence(&view(&p, &[])).starts_with("Nothing is running"));
        let w = [("1".into(), "Ana".into(), "maestro".into(), "Wants to run npm".into())];
        assert_eq!(status_sentence(&view(&p, &w)), "Ana is waiting on you.");
    }

    #[test]
    fn start_screen_paints_with_and_without_projects() {
        let ctx = egui::Context::default();
        let p = [ProjectCard { dir: r"D:\maestro".into(), agents: 2, needs: 1 }];
        for projects in [&[][..], &p[..]] {
            let mut home = Home::default();
            let mut got = None;
            ctx.run_ui(egui::RawInput::default(), |ui| {
                got = show_start(ui, &mut home, &view(projects, &[]));
            })
            .drop_without_applying_deltas();
            assert!(got.is_none(), "nothing happens without a click");
        }
    }
}
