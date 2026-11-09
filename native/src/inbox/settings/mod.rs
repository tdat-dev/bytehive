//! Settings (settingsmodal.ts, prefsview.ts; .set-shell in modal.css and
//! inbox.css): sections down the left, one scrolling column of rows on the
//! right. Every change is saved the moment it is made.

mod browser;
mod remote;
mod rows;
mod sections;

use super::dropdown::{Pick, Spot, track};
use super::*;

/// The sections, as the left column lists them.
const SECTIONS: [&str; 7] = ["Agents", "Inbox", "Terminal", "Browser", "Remote", "Sessions", "System"];

pub(super) struct Settings {
    scroll: ScrollHandle,
    /// The section lit in the left column.
    on: usize,
    /// "Reset settings?" waiting for an answer.
    resetting: bool,
    cli_spot: Spot,
    /// Keys land here while Settings is open, never in a terminal behind it.
    focus: FocusHandle,
}

impl Inbox {
    pub(super) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette = None;
        self.menu = None;
        self.popup = None;
        crate::keys::set_active(false);
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        self.settings = Some(Settings { scroll: ScrollHandle::new(), on: 0, resetting: false, cli_spot: Default::default(), focus });
        self.remote_open_settings(window, cx);
        cx.notify();
    }

    pub(super) fn close_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings = None;
        self.popup = None;
        if self.home {
            if let Some(c) = &self.compose {
                c.job.update(cx, |input, cx| input.focus(window, cx));
            }
        } else if let Some(index) = self.fleet.focused {
            self.fleet.focus(index, window, cx);
        }
        cx.notify();
    }

    fn reset_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.prefs.reset_settings();
        self.show_tray();
        crate::terminal::set_font_size(self.prefs.font_size);
        window.refresh();
        if let Some(s) = &mut self.settings {
            s.resetting = false;
        }
        cx.notify();
    }

    pub(super) fn settings_view(&self, s: &Settings, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let nav = SECTIONS.iter().enumerate().map(|(i, name)| {
            let on = s.on == i;
            div()
                .id(("set-nav", i))
                .py(px(9.0))
                .px(px(12.0))
                .rounded(px(10.0))
                .text_size(px(13.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(color(if on { INK } else { INK_3 }))
                .when(on, |n| n.bg(white(0.08)))
                .when(!on, |n| n.hover(|n| n.bg(white(0.04)).text_color(color(INK))))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(s) = &mut this.settings {
                        s.on = i;
                        s.scroll.scroll_to_item(i);
                    }
                    cx.notify();
                }))
                .child(*name)
        });
        let nav: Vec<_> = nav.collect();
        let confirm = s.resetting.then(|| self.reset_confirm(cx));
        let remote_confirm = self.remote_confirm(cx);
        div()
            .id("set-scrim")
            .absolute()
            .inset_0()
            .bg(hsla(0.0, 0.0, 0.0, 0.6))
            .flex()
            .items_center()
            .justify_center()
            .px(px(16.0))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| this.close_settings(window, cx)))
            .child(
                div()
                    .id("set-shell")
                    .track_focus(&s.focus)
                    .relative()
                    .w(px(1000.0))
                    .max_w_full()
                    .h(relative(0.82))
                    .max_h(px(740.0))
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .rounded(px(18.0))
                    .bg(color(PANEL))
                    .shadow(vec![BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.75), offset: point(px(0.0), px(40.0)), blur_radius: px(100.0), spread_radius: px(0.0), inset: false }, ring(white(0.09))])
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .justify_between()
                            .py(px(16.0))
                            .px(px(22.0))
                            .border_b_1()
                            .border_color(hairline())
                            .child(div().text_size(px(20.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child("Settings"))
                            .child(
                                div()
                                    .id("set-x")
                                    .tooltip(tip("Close · Esc"))
                                    .size(px(34.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(9.0))
                                    .hover(|x| x.bg(white(0.06)))
                                    .on_click(cx.listener(|this, _, window, cx| this.close_settings(window, cx)))
                                    .child(icon("x", 18.0, color(INK_3))),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_h_0()
                            .child(div().w(px(196.0)).flex_none().flex().flex_col().gap(px(2.0)).p(px(14.0)).border_r_1().border_color(hairline()).children(nav))
                            .child(div().id("set-content").flex_1().min_w_0().overflow_y_scroll().track_scroll(&s.scroll).py(px(20.0)).px(px(26.0)).children(self.sections(s, cx))),
                    )
                    .children(confirm)
                    .children(remote_confirm),
            )
    }

    /// confirmModal: "Reset settings?" before anything goes back.
    fn reset_confirm(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let pill = |id: &'static str, label: &'static str, danger: bool| {
            div()
                .id(id)
                .h(px(36.0))
                .px(px(16.0))
                .flex()
                .items_center()
                .rounded_full()
                .text_size(px(13.0))
                .font_weight(FontWeight::SEMIBOLD)
                .when(danger, |b| b.bg(color(DANGER)).text_color(color(GROUND)).hover(|b| b.opacity(0.9)))
                .when(!danger, |b| b.text_color(color(INK_2)).hover(|b| b.bg(white(0.06)).text_color(color(INK))))
                .child(label)
        };
        div().absolute().inset_0().bg(hsla(0.0, 0.0, 0.0, 0.55)).flex().items_center().justify_center().child(
            div()
                .w(px(440.0))
                .flex()
                .flex_col()
                .gap(px(14.0))
                .p(px(22.0))
                .rounded(px(18.0))
                .bg(color(PANEL))
                .shadow(vec![BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.75), offset: point(px(0.0), px(40.0)), blur_radius: px(100.0), spread_radius: px(0.0), inset: false }, ring(white(0.09))])
                .child(div().text_size(px(20.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child("Reset settings?"))
                .child(div().text_size(px(14.0)).line_height(relative(1.5)).text_color(color(INK_2)).child("Agents, text size, startup and the tray go back to how ByteHive ships. Projects and agents are not touched."))
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(px(8.0))
                        .child(pill("reset-cancel", "Cancel", false).on_click(cx.listener(|this, _, _, cx| {
                            if let Some(s) = &mut this.settings {
                                s.resetting = false;
                            }
                            cx.notify();
                        })))
                        .child(pill("reset-ok", "Reset", true).on_click(cx.listener(|this, _, window, cx| this.reset_settings(window, cx)))),
                ),
        )
    }
}
