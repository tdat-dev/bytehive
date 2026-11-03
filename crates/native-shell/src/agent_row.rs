//! A row of the left panel and a project's header over its rows. A row is
//! the agent's face with its state dot, what it works on (its name after,
//! small) and how long since it moved, then its state and what it is on,
//! with its cost in a column at the right. The ⋯ takes the time's place
//! while the pointer is on the row; a stopped agent keeps a Resume button
//! there instead. A right-click opens the same menu as the ⋯.

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use supervise::task_state::TaskState;

use crate::icons::{paint_in, Icon};
use crate::inbox::{headline, line_color, row_line, since};
use crate::queue_ui::{is_attention, QueueRow};
use crate::row_menu::{agent_menu, RowAct};
use crate::style::{medium, PANEL, RAISE};
use crate::term_pane::Pane;
use crate::theme::{ADD, AMBER, DIM, FOCUS, INK3, TEXT};

pub const ROW_H: f32 = 54.0;
/// The space from a row's edge to its face, and from its right edge to its
/// right column; the project header keeps the same edges.
const PAD: f32 = 10.0;
const BTN: f32 = 24.0;

/// A small round icon button at `at`, quiet until hovered.
fn mini_btn(ui: &mut egui::Ui, at: Rect, id: egui::Id, icon: Icon, tip: &str) -> egui::Response {
    let r = ui.interact(at, id, Sense::click()).on_hover_text(tip);
    let p = ui.painter();
    if r.hovered() {
        p.rect_filled(at, 7.0, Color32::from_white_alpha(20));
    }
    paint_in(p, at, 14.0, icon, if r.hovered() { TEXT } else { DIM }, 1.6);
    r
}

/// "1.2M tokens", "48k tokens".
pub fn tokens(n: u64) -> String {
    match n {
        0..1_000 => format!("{n} tokens"),
        1_000..1_000_000 => format!("{}k tokens", n / 1_000),
        _ => format!("{:.1}M tokens", n as f64 / 1e6),
    }
}

/// A row's usage column: "86% context" in amber once its context is nearly
/// full, then what it used. None when there is nothing to say.
fn usage_line(p: &Pane) -> Option<egui::text::LayoutJob> {
    let u = p.usage.as_ref()?;
    let used = u.usd.map(supervise::pricing::money).or_else(|| u.tokens.filter(|&t| t > 0).map(tokens));
    let full = u.context_pct.filter(|&c| c >= 80.0);
    if used.is_none() && full.is_none() {
        return None;
    }
    let mut job = egui::text::LayoutJob::default();
    if let Some(c) = full {
        job.append(&format!("{c:.0}% context"), 0.0, egui::TextFormat::simple(FontId::proportional(12.0), AMBER));
    }
    if let Some(s) = used {
        job.append(&s, if full.is_some() { 8.0 } else { 0.0 }, egui::TextFormat::simple(FontId::monospace(11.5), INK3));
    }
    Some(job)
}

/// One agent row; what it asks for.
pub(crate) fn row(ui: &mut egui::Ui, p: &Pane, r: &QueueRow, on: bool, now: u64, brief: Option<&crate::brief::Brief>) -> Option<RowAct> {
    let state = r.status.state;
    let stopped = p.exit_code.is_some();
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, ROW_H), Sense::click());
    let needs = is_attention(state) && !stopped;
    let painter = ui.painter().clone();
    let more_at = Rect::from_min_size(Pos2::new(rect.right() - PAD + 4.0 - BTN, rect.top() + 6.0), Vec2::splat(BTN));
    let more_id = ui.make_persistent_id(("row-more", &p.id));
    // As Popup::default_response_id names the ⋯'s menu.
    let menu_open = egui::Popup::is_id_open(ui.ctx(), more_id.with("popup"));
    let hot = resp.hovered() || resp.contains_pointer() || menu_open;
    // Picked: a raised ground. Needs you: an amber wash that stays.
    let fill = match (needs, on, hot) {
        (true, true, _) => Some(AMBER.gamma_multiply(0.20)),
        (true, _, true) => Some(AMBER.gamma_multiply(0.16)),
        (true, ..) => Some(AMBER.gamma_multiply(0.11)),
        (_, true, _) => Some(RAISE),
        (_, _, true) => Some(Color32::from_white_alpha(9)),
        _ => None,
    };
    if let Some(f) = fill {
        painter.rect_filled(rect, 10.0, f);
    }
    let ground = match fill {
        Some(_) if on => RAISE,
        _ => PANEL,
    };
    // The face, centred on the row, its state dot at the corner.
    let face = Rect::from_min_size(Pos2::new(rect.left() + PAD, rect.center().y - 14.0), Vec2::splat(28.0));
    crate::face::avatar(&painter, face, p.cli, p.custom.as_deref(), &p.name, 8.0, stopped);
    let dot = face.right_bottom() + Vec2::new(-1.0, -1.0);
    painter.circle_filled(dot, 6.0, ground);
    if stopped {
        painter.circle_stroke(dot, 3.5, Stroke::new(1.5, INK3));
    } else {
        let c = match state {
            TaskState::Needs | TaskState::Blocked => AMBER,
            TaskState::Review => FOCUS,
            TaskState::Working => ADD,
            _ => INK3,
        };
        painter.circle_filled(dot, if state == TaskState::Merged { 2.5 } else { 4.0 }, c);
    }
    let left = face.right() + 10.0;
    let right = rect.right() - PAD;
    let top = rect.top() + 9.0;
    let mut act = None;
    // The right column, top: Resume on a stopped agent, the time on a live
    // one; the ⋯ while the pointer is on the row.
    let mut used = 0.0;
    if stopped {
        let at = Rect::from_min_size(Pos2::new(right + 4.0 - BTN, rect.top() + 6.0), Vec2::splat(BTN));
        if mini_btn(ui, at, ui.make_persistent_id(("row-resume", &p.id)), Icon::Refresh, "Resume: carry its conversation on").clicked() {
            act = Some(RowAct::Restart(p.id.clone()));
        }
        used = BTN;
    }
    let more_at = more_at.translate(Vec2::new(-used, 0.0));
    let more = ui.interact(more_at, more_id, Sense::click());
    if hot || more.hovered() {
        if more.hovered() || menu_open {
            painter.rect_filled(more_at, 7.0, Color32::from_white_alpha(20));
        }
        paint_in(&painter, more_at, 14.0, Icon::Dots, if more.hovered() || menu_open { TEXT } else { DIM }, 1.6);
        used += BTN;
    } else if !stopped {
        let g = painter.layout_no_wrap(since(p.last_output_ms, now), FontId::monospace(11.5), INK3);
        used = g.size().x;
        painter.galley(Pos2::new(right - g.size().x, top + 1.0), g, INK3);
    }
    egui::Popup::menu(&more).show(|ui| {
        if let Some(a) = agent_menu(ui, p, on) {
            act = Some(a);
        }
    });
    // What it works on, its name after, small; dimmed while stopped.
    let (name_f, name_c) = if stopped { (FontId::proportional(13.5), DIM) } else { (crate::style::w500(13.5), TEXT) };
    let title = headline(p, brief);
    let mut head = egui::text::LayoutJob::default();
    head.append(title.as_deref().unwrap_or(&p.name), 0.0, egui::TextFormat::simple(name_f, name_c));
    if title.is_some() {
        head.append(&p.name, 7.0, egui::TextFormat::simple(FontId::proportional(12.0), INK3));
    }
    head.wrap = egui::text::TextWrapping::truncate_at_width((right - used - 8.0 - left).max(20.0));
    painter.galley(Pos2::new(left, top), painter.layout_job(head), name_c);
    // What it used, in its own column so the rows line up: money when its
    // CLI prices it, else tokens; ahead of it, its context once nearly full.
    let mut cost_w = 0.0;
    if let Some(g) = usage_line(p).map(|job| painter.layout_job(job)) {
        cost_w = g.size().x + 10.0;
        painter.galley(Pos2::new(right - g.size().x, top + 20.0), g, INK3);
    }
    // The state, then what it is on (or its branch).
    let (label, rest) = row_line(&p.name, state, &r.line);
    let rest = match brief.and_then(|b| b.doing.clone()) {
        Some(d) if !stopped && matches!(state, TaskState::Working | TaskState::Needs | TaskState::Blocked) => d,
        _ => rest,
    };
    let rest = match (&p.worktree, rest.is_empty()) {
        (Some(w), true) => w.branch.clone(),
        (Some(w), false) => format!("{rest} \u{00B7} {}", w.branch),
        (None, _) => rest,
    };
    let fmt = |c: Color32| egui::TextFormat::simple(FontId::proportional(12.5), c);
    let mut sub = egui::text::LayoutJob::default();
    sub.append(&label, 0.0, fmt(if stopped { INK3 } else { line_color(state) }));
    if !rest.is_empty() {
        sub.append(&format!("\u{00B7} {rest}"), 6.0, fmt(INK3));
    }
    sub.wrap = egui::text::TextWrapping::truncate_at_width((right - cost_w - left).max(20.0));
    painter.galley(Pos2::new(left, top + 19.0), painter.layout_job(sub), INK3);
    resp.context_menu(|ui| {
        if let Some(a) = agent_menu(ui, p, on) {
            act = Some(a);
        }
    });
    if act.is_none() {
        if stopped && resp.double_clicked() {
            act = Some(RowAct::Restart(p.id.clone()));
        } else if resp.clicked() {
            act = Some(RowAct::Pick(p.id.clone()));
        }
    }
    if let Some(t) = &p.task {
        resp.on_hover_text(t);
    }
    act
}

/// A project's header: its name and agent count, how many need you, and
/// its cost at the right, where the memory book shows while hovered. (The
/// header, memory clicked.)
pub(crate) fn project_head(ui: &mut egui::Ui, name: &str, open: bool, count: usize, needs: usize, cost: Option<f64>) -> (egui::Response, bool) {
    ui.add_space(6.0);
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 30.0), Sense::click());
    let book_at = Rect::from_min_size(Pos2::new(rect.right() - PAD + 4.0 - BTN, rect.center().y - BTN / 2.0), Vec2::splat(BTN));
    let book = ui.interact(book_at, ui.make_persistent_id(("proj-mem", name)), Sense::click()).on_hover_text("Project memory: what every agent here knows");
    let hot = resp.hovered() || book.hovered();
    let p = ui.painter().clone();
    if hot {
        p.rect_filled(rect, 8.0, Color32::from_white_alpha(8));
    }
    let ink = if hot { TEXT } else { DIM };
    let chev = Rect::from_center_size(Pos2::new(rect.left() + PAD + 6.0, rect.center().y), Vec2::splat(10.0));
    crate::icons::paint(&p, chev, Icon::Chevron { open }, INK3, 2.2);
    let mut x = rect.right() - PAD;
    if hot {
        if book.hovered() {
            p.rect_filled(book_at, 7.0, Color32::from_white_alpha(20));
        }
        paint_in(&p, book_at, 14.0, Icon::Book, if book.hovered() { TEXT } else { DIM }, 1.5);
        x = book_at.left() - 4.0;
    } else if let Some(usd) = cost {
        let g = p.layout_no_wrap(supervise::pricing::money(usd), FontId::monospace(11.5), INK3);
        x -= g.size().x;
        p.galley(Pos2::new(x, rect.center().y - g.size().y / 2.0), g, INK3);
        x -= 8.0;
    }
    if needs > 0 {
        let g = p.layout_no_wrap(needs.to_string(), FontId::new(11.0, egui::FontFamily::Name("monomedium".into())), Color32::BLACK);
        let bw = (g.size().x + 10.0).max(18.0);
        let badge = Rect::from_min_size(Pos2::new(x - bw, rect.center().y - 9.0), Vec2::new(bw, 18.0));
        p.rect_filled(badge, 9.0, AMBER);
        p.galley(badge.center() - g.size() / 2.0, g, Color32::BLACK);
        x = badge.left() - 8.0;
    }
    // The name, then how many agents, quiet beside it.
    let name_x = chev.right() + 9.0;
    let n = p.layout_no_wrap(count.to_string(), FontId::monospace(11.5), INK3);
    let room = (x - name_x - n.size().x - 7.0).max(20.0);
    let mut job = egui::text::LayoutJob::simple_singleline(name.to_string(), medium(12.5), ink);
    job.wrap = egui::text::TextWrapping::truncate_at_width(room);
    let title = p.layout_job(job);
    let y = rect.center().y - title.size().y / 2.0;
    let tw = title.size().x;
    p.galley(Pos2::new(name_x, y), title, ink);
    p.galley(Pos2::new(name_x + tw + 7.0, rect.center().y - n.size().y / 2.0), n, INK3);
    (resp, book.clicked())
}

#[cfg(test)]
mod tests {
    use super::tokens;

    #[test]
    fn tokens_read_short() {
        assert_eq!(tokens(950), "950 tokens");
        assert_eq!(tokens(48_200), "48k tokens");
        assert_eq!(tokens(1_250_000), "1.2M tokens");
    }
}
