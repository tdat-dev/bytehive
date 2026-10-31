//! The pane grid: tidy tiles, focus stage + avatar rail, per-pane headers
//! with state badge and Kill/Restart/Focus buttons. Pure geometry comes
//! from `layout`; this module only paints. Extracted from app.rs to keep
//! every file under the 500-line gate.

use eframe::egui::{self, FontId, Pos2, Rect, Vec2};

use crate::app::{now_ms, ShellApp, HEADER_H, RAIL_W};
use crate::color::{c32, BG_DEFAULT};
use crate::layout::{header_layout, tidy_tiles, Tile};
use crate::states::{pane_cwd, pane_status, state_badge};
use crate::theme::{BORDER, DIM, SURFACE, TEXT, UI_BASE};

#[derive(Default)]
pub(crate) struct GridOut {
    pub restart: Option<usize>,
    pub kill: Vec<String>,
}

/// `hidden`: panes the project filter keeps off the grid (they keep running).
pub(crate) fn show_grid(app: &mut ShellApp, ui: &mut egui::Ui, hidden: &std::collections::HashSet<String>) -> GridOut {
    let mut out = GridOut::default();
    let mut toggle_stage: Option<String> = None;
    let mut rail_pick: Option<String> = None;
    // A killed stage pane drops back to the grid on its own.
    if app.stage.as_ref().is_some_and(|id| hidden.contains(id) || !app.panes.iter().any(|p| &p.id == id)) {
        app.stage = None;
    }
    let staged = app.stage.clone();
    // The rail is a real side panel so the central area shrinks by itself;
    // the stage pane below simply fills what's left.
    if let Some(ref sid) = staged {
        if app.panes.len() - hidden.len() > 1 && app.panes.iter().any(|p| &p.id == sid) {
            egui::Panel::right("stage-rail").resizable(false).exact_size(RAIL_W).show(ui, |ui| {
                ui.vertical(|ui| {
                    for p in &app.panes {
                        if Some(p.id.as_str()) == staged.as_deref() || hidden.contains(&p.id) {
                            continue;
                        }
                        let initial: String = p.name.chars().take(1).collect();
                        if ui.small_button(initial).on_hover_text(&p.name).clicked() {
                            rail_pick = Some(p.id.clone());
                        }
                    }
                });
            });
        }
    }
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(c32(BG_DEFAULT))).show(ui, |ui| {
        // Cell metrics from one measured run of glyphs.
        let (cell_w, cell_h) = {
            let g = ui.painter().layout_no_wrap("MMMMMMMMMM".to_string(), app.font.clone(), TEXT);
            (g.rect.width() / 10.0, g.rect.height())
        };
        let avail = ui.available_rect_before_wrap();
        ui.allocate_rect(avail, egui::Sense::hover());

        let n = (app.panes.len() - hidden.len()).max(1);
        let tiles = tidy_tiles(n, avail.width(), avail.height());

        let font = app.font.clone();
        let want_focus = app.focus_pane.clone();
        let git_changed = app.git_changed.clone();
        let staged_id = staged.clone();
        let ShellApp { panes, registry, focused_once, focus_pane, .. } = app;
        let multi = n > 1;
        let mut i = 0;
        for (idx, pane) in panes.iter_mut().enumerate() {
            if hidden.contains(&pane.id) {
                continue;
            }
            let slot = i;
            i += 1;
            let staged = staged_id.as_deref() == Some(pane.id.as_str());
            if staged_id.is_some() && !staged {
                continue;
            }
            let cell_rect = if staged {
                Rect::from_min_max(avail.min, avail.max)
            } else {
                let t = tiles.get(slot).copied().unwrap_or(Tile { x: 0.0, y: 0.0, w: 0.0, h: 0.0 });
                Rect::from_min_size(
                    Pos2::new(avail.left() + t.x, avail.top() + t.y),
                    Vec2::new(t.w, t.h),
                )
            };
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
            // Right-side chrome, laid out from the edge inward so no
            // label ever wraps: Kill, then Restart, then Focus.
            let focus_text = if staged { "Unfocus" } else { "Focus" };
            let mut btn_texts: Vec<&str> = vec!["Kill"];
            if pane.exit_code.is_some() {
                btn_texts.push("Restart");
            }
            if multi {
                btn_texts.push(focus_text);
            }
            let btn_font = FontId::proportional(UI_BASE);
            let btn_ws: Vec<f32> = btn_texts
                .iter()
                .map(|t| ui.painter().layout_no_wrap((*t).to_string(), btn_font.clone(), TEXT).rect.width() + 18.0)
                .collect();
            let changed = git_changed.get(&pane_cwd(pane)).map(|f| f.len() as u32);
            let (dot, word) = state_badge(pane_status(pane, now_ms(), changed).state);
            let word_w = ui.painter().layout_no_wrap(word.to_string(), FontId::proportional(12.0), DIM).rect.width();
            let laid = header_layout(&label, head.left(), head.width(), word_w, &btn_ws);
            ui.painter().text(
                Pos2::new(head.left() + 8.0, head.top() + 4.0),
                egui::Align2::LEFT_TOP,
                laid.label,
                FontId::proportional(12.0),
                TEXT,
            );
            ui.painter().circle_filled(Pos2::new(laid.word_x - 8.0, head.top() + 12.0), 4.0, dot);
            ui.painter().text(
                Pos2::new(laid.word_x, head.top() + 4.0),
                egui::Align2::LEFT_TOP,
                word,
                FontId::proportional(12.0),
                DIM,
            );
            for (text, b) in btn_texts.iter().zip(laid.buttons.iter()) {
                let resp = ui.put(
                    Rect::from_min_size(Pos2::new(b.x, head.top() + 2.0), Vec2::new(b.w, 20.0)),
                    egui::Button::new(egui::RichText::new(*text).size(UI_BASE)),
                );
                if resp.clicked() {
                    if *text == "Kill" {
                        out.kill.push(pane.id.clone());
                    } else if *text == "Restart" {
                        out.restart = Some(idx);
                    } else {
                        toggle_stage = Some(pane.id.clone());
                    }
                }
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

    if let Some(id) = toggle_stage {
        if app.stage.as_deref() == Some(id.as_str()) {
            app.stage = None;
        } else {
            app.stage = Some(id);
        }
    }
    if let Some(id) = rail_pick {
        app.stage = Some(id);
    }
    if app.stage.as_ref().is_some_and(|id| !app.panes.iter().any(|p| &p.id == id)) {
        app.stage = None;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::ShellApp;
    use agent_core::launch::launch_spec;

    /// The whole stage path — rail panel, staged paint, header buttons —
    /// must run without panicking, even though clicks only happen live.
    #[test]
    fn stage_with_two_panes_paints_without_panic() {
        let mut app = ShellApp::new().unwrap();
        let spec = launch_spec("powershell.exe", &[String::from("-NoLogo")]);
        let id2 = app.spawn_pane("shell", spec, None, None).unwrap();
        app.stage = Some(id2);
        egui::__run_test_ui(|ui| {
            show_grid(&mut app, ui, &Default::default());
        });
        assert!(app.stage.is_some());
    }
}
