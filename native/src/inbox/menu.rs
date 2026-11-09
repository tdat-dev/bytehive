//! An agent's right-click menu and the Remove confirmation.

use super::*;

impl Inbox {
    /// selectmenu.css: the right-click menu of an agent.
    pub(super) fn menu_view(&self, menu: &Menu, view: &View, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let index = menu.index;
        let running = view.statuses.get(index).is_some_and(|s| !ended(s.state));
        let pinned = self.in_grid(index);
        let here = self.fleet.focused == Some(index);
        let item = |id: &'static str, label: &'static str, danger: bool, disabled: bool| {
            div()
                .id(id)
                .min_h(px(32.0))
                .px(px(12.0))
                .py(px(6.0))
                .flex()
                .items_center()
                .rounded(px(9.0))
                .text_color(color(if disabled { INK_3 } else if danger { DANGER } else { INK }))
                .when(!disabled && !danger, |i| i.hover(|i| i.bg(white(0.08))))
                .when(!disabled && danger, |i| i.hover(|i| i.bg(tint(DANGER, 0.12))))
                .child(label)
        };
        let close = cx.listener(|this, _: &MouseDownEvent, _, cx| {
            this.menu = None;
            cx.notify();
        });
        div().absolute().inset_0().on_mouse_down(MouseButton::Left, close).child(
            deferred(
                anchored().position(menu.at).snap_to_window().child(
                    div()
                        .id("menu")
                        .min_w(px(210.0))
                        .p(px(5.0))
                        .rounded(px(14.0))
                        .bg(color(0x16171b))
                        .shadow(vec![
                            BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.6), offset: point(px(0.0), px(22.0)), blur_radius: px(50.0), spread_radius: px(0.0), inset: false },
                            BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.4), offset: point(px(0.0), px(2.0)), blur_radius: px(6.0), spread_radius: px(0.0), inset: false },
                            ring(white(0.1)),
                        ])
                        .text_size(px(13.5))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(item("menu-open", "Open", false, false).on_click(cx.listener(move |this, _, window, cx| {
                            this.menu = None;
                            this.open_agent(index, window, cx);
                        })))
                        // Alt+P acts on the agent on the stage, so only its menu says so.
                        .child(
                            item("menu-grid", if pinned { "Take out of Grid" } else { "Add to Grid" }, false, false)
                                .justify_between()
                                .when(here, |i| i.child(div().flex_none().ml(px(12.0)).text_size(px(11.5)).text_color(color(INK_3)).child("Alt+P")))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.menu = None;
                                    this.toggle_in_grid(index, cx);
                                })),
                        )
                        .when(!running, |m| {
                            m.child(item("menu-resume", "Resume", false, false).on_click(cx.listener(move |this, _, window, cx| {
                                this.menu = None;
                                this.resume(index, window, cx);
                            })))
                        })
                        .child(item("menu-stop", "Stop", false, !running).when(running, |i| {
                            i.on_click(cx.listener(move |this, _, _, cx| {
                                this.menu = None;
                                this.fleet.stop(index);
                                cx.notify();
                            }))
                        }))
                        .child(div().h(px(1.0)).mx(px(8.0)).my(px(5.0)).bg(white(0.08)))
                        .child(item("menu-remove", "Remove agent…", true, false).on_click(cx.listener(move |this, _, _, cx| {
                            this.menu = None;
                            this.removing = Some(index);
                            cx.notify();
                        }))),
                ),
            )
            .with_priority(1),
        )
    }

    /// confirmModal: "Remove Bob?" before an agent goes.
    pub(super) fn confirm_view(&self, index: usize, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let seat = &self.fleet.seats[index];
        let running = seat.terminal.as_ref().is_some_and(|t| t.read(cx).exit_code().is_none());
        let message = if running {
            "It stops and its terminal closes. Its branch and the changes on it stay in git."
        } else {
            "Its terminal closes. Its branch and the changes on it stay in git."
        };
        let button = |id: &'static str, label: &'static str, danger: bool| {
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
        div()
            .id("confirm-scrim")
            .absolute()
            .inset_0()
            .bg(hsla(0.0, 0.0, 0.0, 0.55))
            .flex()
            .justify_center()
            .items_start()
            .pt(relative(0.14))
            .child(
                div()
                    .id("confirm")
                    .w(px(440.0))
                    .h_auto()
                    .flex()
                    .flex_col()
                    .gap(px(14.0))
                    .p(px(22.0))
                    .rounded(px(18.0))
                    .bg(color(PANEL))
                    .shadow(vec![BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.75), offset: point(px(0.0), px(40.0)), blur_radius: px(100.0), spread_radius: px(0.0), inset: false }, ring(white(0.09))])
                    .child(div().text_size(px(20.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child(format!("Remove {}?", seat.spec.name)))
                    .child(div().text_size(px(14.0)).line_height(relative(1.5)).text_color(color(INK_2)).child(message))
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(px(8.0))
                            .child(button("confirm-cancel", "Cancel", false).on_click(cx.listener(|this, _, _, cx| {
                                this.removing = None;
                                cx.notify();
                            })))
                            .child(button("confirm-ok", "Remove", true).on_click(cx.listener(move |this, _, window, cx| {
                                this.removing = None;
                                this.fleet.remove(index);
                                if let Some(f) = this.fleet.focused {
                                    this.fleet.focus(f, window, cx);
                                }
                                cx.notify();
                            }))),
                    ),
            )
    }
}
