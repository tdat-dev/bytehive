//! The Scheduled agents dialog drawn (#schedModal; .sched-* in modal.css,
//! the drop-downs as selectmenu.css draws them).

use std::f32::consts::PI;

use gpui_kit::component::Sizable as _;

use super::super::dialog::{self, MUTED_2, SURFACE_0, SURFACE_3};
use super::super::dropdown::track;
use super::*;

/// One line of an open drop-down.
struct Opt {
    label: String,
    on: bool,
    disabled: bool,
    pick: Option<Pick>,
}

/// What choosing a line sets.
#[derive(Clone)]
enum Pick {
    Preset(String),
    Repeat(Repeat),
}

/// .sched-en .pt-sw: the small switch at a row's start.
fn switch(on: bool) -> Div {
    div()
        .relative()
        .flex_none()
        .w(px(30.0))
        .h(px(17.0))
        .rounded(px(9.0))
        .bg(color(if on { INK } else { SURFACE_3 }))
        .child(div().absolute().top(px(2.0)).left(px(if on { 15.0 } else { 2.0 })).size(px(13.0)).rounded_full().bg(color(if on { GROUND } else { INK_3 })))
}

/// .sm-btn.input: a field-looking button with the chosen value and the arrow.
fn select(id: &'static str, label: String, open: bool, disabled: bool) -> Stateful<Div> {
    dialog::input_box()
        .id(id)
        .relative()
        .justify_between()
        .gap(px(8.0))
        .when(open, |b| b.bg(white(0.09)))
        .when(!disabled, |b| b.hover(|b| b.bg(white(0.09))))
        .when(disabled, |b| b.opacity(0.5))
        .child(div().min_w_0().truncate().child(label))
        .child(icon("chev-down", 10.0, color(INK)).opacity(0.7).when(open, |i| i.with_transformation(Transformation::rotate(radians(PI)))))
}

impl Inbox {
    pub(in crate::inbox) fn schedules_view(&self, s: &SchedulesView, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let preset = self.sched_preset();
        let no_presets = self.prefs.templates.is_empty();
        let open = |p: SchedPick| s.pick.is_some_and(|(x, _)| x == p);
        let preset_label = preset.clone().unwrap_or_else(|| "No saved presets".to_string());
        let repeat_label = if s.repeat == Repeat::Once { "Once" } else { "Daily" };
        let add = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.0))
            .mb(px(12.0))
            .child(
                select("sched-tpl", preset_label, open(SchedPick::Preset), false)
                    .flex_1()
                    .min_w(px(90.0))
                    .child(track(&s.preset_spot))
                    .on_click(cx.listener(|this, _, _, cx| this.sched_open_pick(SchedPick::Preset, cx))),
            )
            .child(dialog::input_box().flex_none().w(px(96.0)).child(Input::new(&s.time).appearance(false).with_size(px(13.0 / 0.875))))
            .child(select("sched-repeat", repeat_label.to_string(), open(SchedPick::Repeat), false).flex_none().child(track(&s.repeat_spot)).on_click(cx.listener(|this, _, _, cx| this.sched_open_pick(SchedPick::Repeat, cx))))
            .child(dialog::btn("sched-add", "Add").on_click(cx.listener(|this, _, _, cx| this.sched_add(cx))));
        let now = chrono::Local::now();
        let mut rows: Vec<AnyElement> = Vec::new();
        for (i, sc) in s.list.iter().enumerate() {
            let name = self.prefs.templates.iter().find(|t| t.name == sc.template_id).map(|t| t.name.clone());
            let (flip, del) = (sc.id.clone(), sc.id.clone());
            let row = div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .py(px(8.0))
                .px(px(10.0))
                .border_1()
                .border_color(hairline())
                .rounded(px(8.0))
                .bg(color(SURFACE_0))
                .when(!sc.enabled, |r| r.opacity(0.55))
                .child(div().id(("sched-en", i)).flex_none().cursor_pointer().child(switch(sc.enabled)).on_click(cx.listener(move |this, _, _, cx| this.sched_toggle(&flip, cx))))
                .child(match name {
                    Some(n) => div().flex_1().min_w_0().truncate().text_size(px(12.5)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child(n),
                    None => div().flex_1().min_w_0().truncate().text_size(px(12.5)).italic().text_color(color(MUTED_2)).child("preset deleted"),
                })
                .child(mono().flex_none().text_size(px(11.5)).text_color(color(INK_3)).child(format!("{} · {}", sc.time, if sc.repeat == Repeat::Daily { "daily" } else { "once" })))
                .child(div().flex_none().text_size(px(11.5)).text_color(color(WORK)).child(schedule::next_label(sc, now)))
                .child(
                    div()
                        .id(("sched-del", i))
                        .flex_none()
                        .size(px(22.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(6.0))
                        .group("sched-del")
                        .hover(|b| b.bg(tint(0xf87171, 0.12)))
                        .tooltip(tip("Delete schedule"))
                        .on_click(cx.listener(move |this, _, _, cx| this.sched_delete(&del, cx)))
                        .child(icon("x", 11.0, color(MUTED_2)).group_hover("sched-del", |i| i.text_color(color(DANGER)))),
                );
            rows.push(row.into_any_element());
        }
        let empty = s.list.is_empty().then(|| {
            div().py(px(14.0)).px(px(2.0)).text_center().text_size(px(12.5)).text_color(color(MUTED_2)).child("No presets yet. In New agent, set up the agent and choose Save as preset, then schedule it here.")
        });
        let list = div().flex().flex_col().gap(px(6.0)).children(rows);
        let popup = s.pick.map(|(pick, at)| self.sched_popup(pick, at, s, no_presets, cx));
        dialog::backdrop("sched-back", cx.listener(|this, _, window, cx| this.close_schedules(window, cx)))
            .child(
                dialog::modal("sched", 460.0, &s.focus)
                    .child(dialog::head("Scheduled agents").child(dialog::x("sched-x").on_click(cx.listener(|this, _, window, cx| this.close_schedules(window, cx)))))
                    .child(dialog::body("sched-body").child(add).children(empty).child(list))
                    .child(dialog::foot().child(dialog::ghost("sched-close", "Close").on_click(cx.listener(|this, _, window, cx| this.close_schedules(window, cx))))),
            )
            .children(popup)
    }

    /// The open drop-down's list (selectmenu.css .sm-pop, .sm-opt), and a
    /// layer under it that closes it on a press anywhere else.
    fn sched_popup(&self, pick: SchedPick, at: Point<Pixels>, s: &SchedulesView, no_presets: bool, cx: &mut Context<Self>) -> AnyElement {
        let opts: Vec<Opt> = match pick {
            SchedPick::Preset if no_presets => vec![Opt { label: "No saved presets".to_string(), on: true, disabled: false, pick: None }],
            SchedPick::Preset => {
                let now = self.sched_preset();
                self.prefs.templates.iter().map(|t| Opt { label: t.name.clone(), on: now.as_deref() == Some(t.name.as_str()), disabled: false, pick: Some(Pick::Preset(t.name.clone())) }).collect()
            }
            SchedPick::Repeat => [(Repeat::Daily, "Daily"), (Repeat::Once, "Once")].into_iter().map(|(r, l)| Opt { label: l.to_string(), on: s.repeat == r, disabled: false, pick: Some(Pick::Repeat(r)) }).collect(),
        };
        let mut lines: Vec<AnyElement> = Vec::new();
        for (i, o) in opts.into_iter().enumerate() {
            let Opt { label, on, disabled, pick } = o;
            let line = div()
                .id(("sched-opt", i))
                .flex()
                .items_center()
                .gap(px(8.0))
                .min_h(px(34.0))
                .pl(px(8.0))
                .pr(px(12.0))
                .py(px(6.0))
                .rounded(px(9.0))
                .text_color(color(if disabled { INK_3 } else { INK }))
                .child(div().flex_none().size(px(16.0)).flex().items_center().justify_center().when(on, |t| t.child(icon("check", 14.0, color(INK)))))
                .child(div().flex_1().min_w_0().truncate().when(on, |l| l.font_weight(FontWeight::SEMIBOLD)).child(label))
                .when(!disabled, |line| {
                    line.hover(|l| l.bg(white(0.08))).on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(s) = &mut this.schedules {
                            match &pick {
                                Some(Pick::Preset(name)) => s.preset = Some(name.clone()),
                                Some(Pick::Repeat(r)) => s.repeat = *r,
                                None => {}
                            }
                            s.pick = None;
                        }
                        cx.notify();
                    }))
                });
            lines.push(line.into_any_element());
        }
        let close = cx.listener(|this, _: &MouseDownEvent, _, cx| {
            if let Some(s) = &mut this.schedules {
                s.pick = None;
            }
            cx.stop_propagation();
            cx.notify();
        });
        div()
            .absolute()
            .inset_0()
            .on_mouse_down(MouseButton::Left, close)
            .child(
                deferred(
                    anchored().position(at).snap_to_window().child(
                        div()
                            .id("sched-pop")
                            .min_w(px(220.0))
                            .max_w(px(360.0))
                            .p(px(5.0))
                            .rounded(px(14.0))
                            .bg(color(0x16171b))
                            .shadow(vec![
                                BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.6), offset: point(px(0.0), px(22.0)), blur_radius: px(50.0), spread_radius: px(0.0), inset: false },
                                BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.4), offset: point(px(0.0), px(2.0)), blur_radius: px(6.0), spread_radius: px(0.0), inset: false },
                                ring(white(0.1)),
                            ])
                            .text_size(px(13.5))
                            .line_height(relative(1.3))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .children(lines),
                    ),
                )
                .with_priority(1),
            )
            .into_any_element()
    }
}
