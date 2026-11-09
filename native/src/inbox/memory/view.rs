//! The memory panel drawn (.inbox-modal.mm in inbox.css): head, project
//! tabs, the add box, the facts, and the foot.

use chrono::{Local, TimeZone as _};
use gpui_kit::component::Sizable as _;

use super::*;

/// `.im-btn`: white for the main one, quiet without a background.
fn im_btn(id: impl Into<ElementId>, label: &'static str, primary: bool, quiet: bool) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .h(px(36.0))
        .px(px(16.0))
        .flex()
        .items_center()
        .rounded_full()
        .text_size(px(13.0))
        .font_weight(FontWeight::SEMIBOLD)
        .when(primary, |b| b.bg(color(0xffffff)).text_color(color(0x000000)).hover(|b| b.bg(color(0xe9e9ec))))
        .when(!primary && quiet, |b| b.text_color(color(INK_3)).hover(|b| b.bg(white(0.06)).text_color(color(INK))))
        .when(!primary && !quiet, |b| b.bg(white(0.06)).text_color(color(INK_2)).hover(|b| b.bg(white(0.12)).text_color(color(INK))))
        .child(label)
}

/// `.mm-b`: the small buttons on a fact.
fn mm_b(id: impl Into<ElementId>, label: &'static str, danger: bool) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(26.0))
        .px(px(9.0))
        .flex()
        .items_center()
        .rounded_full()
        .text_size(px(12.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color(INK_3))
        .when(danger, |b| b.hover(|b| b.text_color(color(DANGER)).bg(tint(DANGER, 0.12))))
        .when(!danger, |b| b.hover(|b| b.text_color(color(INK)).bg(white(0.07))))
        .child(label)
}

/// `.inbox-modal textarea`.
fn field(state: &Entity<TextareaState>) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .py(px(10.0))
        .px(px(14.0))
        .rounded(px(12.0))
        .bg(hsla(0.0, 0.0, 0.0, 0.35))
        .shadow(vec![ring(white(0.09))])
        .text_size(px(14.0))
        .line_height(relative(1.5))
        .child(Textarea::new(state).appearance(false).with_size(px(14.0 / 0.875)))
}

fn day(at: u64) -> String {
    Local.timestamp_millis_opt(at as i64).single().map(|d| d.format("%b %-d").to_string()).unwrap_or_default()
}

impl Inbox {
    pub(in crate::inbox) fn memory_view(&self, m: &MemoryView, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let agents = self.memory_agents(&m.project, cx);
        // Every project with agents, to switch between their memories.
        let mut projects: Vec<String> = Vec::new();
        for s in &self.fleet.seats {
            if !s.spec.cwd.is_empty() && !projects.iter().any(|p| presets::same_folder(p, &s.spec.cwd)) {
                projects.push(s.spec.cwd.clone());
            }
        }
        let mut list: Vec<AnyElement> = Vec::new();
        if !m.error.is_empty() {
            list.push(div().mb(px(8.0)).text_size(px(13.0)).text_color(color(DANGER)).child(m.error.clone()).into_any_element());
        }
        if m.mem.entries.is_empty() {
            list.push(div().py(px(18.0)).text_size(px(13.0)).text_color(color(INK_3)).child(format!("Nothing yet. Agents save what they learn about {} here; add your own above.", m.name)).into_any_element());
        }
        for (i, e) in m.mem.entries.iter().enumerate() {
            list.push(self.memory_entry(m, i, e, &agents, cx));
        }
        let ask_all = (m.asking.as_deref() == Some("*")).then(|| div().py(px(12.0)).px(px(14.0)).rounded(px(14.0)).bg(white(0.03)).shadow(vec![ring(hairline())]).child(self.ask_form(m, "*", &agents, cx)));
        div().id("mm-back").absolute().inset_0().bg(hsla(0.0, 0.0, 0.0, 0.55)).flex().justify_center().pt(relative(0.14)).px(px(16.0)).pb(px(16.0)).on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| this.close_memory(window, cx))).child(
            div()
                .id("mm")
                .w(px(720.0))
                .max_w_full()
                .max_h(relative(0.8))
                .flex()
                .flex_col()
                .gap(px(14.0))
                .pt(px(22.0))
                .px(px(22.0))
                .pb(px(18.0))
                .rounded(px(18.0))
                .bg(color(PANEL))
                .text_color(color(INK))
                .shadow(vec![BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.75), offset: point(px(0.0), px(40.0)), blur_radius: px(100.0), spread_radius: px(0.0), inset: false }, ring(white(0.09))])
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .flex()
                        .items_start()
                        .gap(px(12.0))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(div().text_size(px(20.0)).font_weight(FontWeight::SEMIBOLD).child("Project memory"))
                                .child(div().mt(px(4.0)).flex().gap(px(4.0)).text_size(px(12.5)).text_color(color(INK_3)).child("What every agent in").child(div().font_weight(FontWeight::SEMIBOLD).text_color(color(INK_2)).child(m.name.clone())).child("knows when it starts. Agents add to it as they learn.")),
                        )
                        .child(div().id("mm-x").size(px(30.0)).flex().items_center().justify_center().rounded_full().bg(white(0.05)).hover(|b| b.bg(white(0.1))).on_click(cx.listener(|this, _, window, cx| this.close_memory(window, cx))).child(icon("x", 14.0, color(INK_3)))),
                )
                .when(projects.len() > 1, |p| {
                    p.child(div().flex().gap(px(4.0)).mt(px(-4.0)).children(projects.iter().enumerate().map(|(i, dir)| {
                        let on = presets::same_folder(dir, &m.project);
                        let to = dir.clone();
                        div()
                            .id(("mm-tab", i))
                            .flex_none()
                            .h(px(30.0))
                            .px(px(12.0))
                            .flex()
                            .items_center()
                            .rounded_full()
                            .text_size(px(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .when(on, |t| t.bg(color(0xffffff)).text_color(color(0x000000)))
                            .when(!on, |t| t.bg(white(0.04)).text_color(color(INK_3)).hover(|t| t.bg(white(0.08)).text_color(color(INK))))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if this.memory.as_ref().is_some_and(|m| !presets::same_folder(&m.project, &to)) {
                                    this.open_memory(to.clone(), window, cx);
                                }
                            }))
                            .child(crate::fleet::project_of(dir))
                    })))
                })
                .child(div().flex().items_end().gap(px(8.0)).child(field(&m.add)).child(im_btn("mm-add", "Add", true, false).on_click(cx.listener(|this, _, window, cx| this.memory_add(window, cx)))))
                .child(div().id("mm-body").flex_1().min_h_0().overflow_y_scroll().children(list))
                .children(ask_all)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .min_h(px(36.0))
                        .when(!m.status.is_empty(), |f| {
                            f.child(div().flex().items_center().gap(px(4.0)).text_size(px(12.5)).text_color(color(INK_2)).child(m.status.clone()).when(m.undo.is_some(), |s| s.child(mm_b("mm-undo", "Undo", false).on_click(cx.listener(|this, _, _, cx| this.memory_undo(cx))))))
                        })
                        .child(div().flex_1())
                        .when(!m.mem.entries.is_empty(), |f| {
                            f.child(im_btn("mm-ask-all", "Ask an agent to check all of it", false, false).tooltip(tip("An agent checks every fact against the code, fixes, merges and fills gaps")).on_click(cx.listener(|this, _, window, cx| this.memory_ask("*".to_string(), window, cx))))
                        }),
                ),
        )
    }

    fn memory_entry(&self, m: &MemoryView, i: usize, e: &Entry, agents: &[(String, String)], cx: &mut Context<Self>) -> AnyElement {
        let row = div().py(px(12.0)).when(i > 0, |r| r.border_t_1().border_color(hairline()));
        if m.editing.as_deref() == Some(e.id.as_str()) {
            return row
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .child(field(&m.edit))
                        .child(div().flex().items_center().gap(px(8.0)).child(div().flex_1()).child(im_btn("mm-cancel", "Cancel", false, true).on_click(cx.listener(|this, _, window, cx| this.memory_escape(window, cx)))).child(im_btn("mm-save", "Save", true, false).on_click(cx.listener(|this, _, _, cx| this.memory_save_edit(cx))))),
                )
                .into_any_element();
        }
        let meta = [if e.by == "you" { "You".to_string() } else { e.by.clone() }, if e.at > 0 { day(e.at) } else { String::new() }].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
        let (ask, edit, del) = (e.id.clone(), e.id.clone(), e.id.clone());
        let text = e.text.clone();
        row.group("mm-e")
            .child(div().text_size(px(14.0)).line_height(relative(1.55)).child(e.text.clone()))
            .child(
                div()
                    .mt(px(6.0))
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .text_size(px(12.0))
                    .text_color(color(INK_3))
                    .child(mono().text_size(px(12.0)).child(e.id.clone()))
                    .when(!meta.is_empty(), |r| r.child(meta))
                    .child(div().flex_1())
                    .child(
                        div()
                            .flex()
                            .gap(px(2.0))
                            .invisible()
                            .group_hover("mm-e", |b| b.visible())
                            .child(mm_b(SharedString::from(format!("mm-ask-{}", e.id)), "Ask an agent", false).tooltip(tip("Send this fact to an agent to check against the code and fix")).on_click(cx.listener(move |this, _, window, cx| this.memory_ask(ask.clone(), window, cx))))
                            .child(mm_b(SharedString::from(format!("mm-edit-{}", e.id)), "Edit", false).on_click(cx.listener(move |this, _, window, cx| {
                                if let Some(m) = &mut this.memory {
                                    m.editing = Some(edit.clone());
                                    m.asking = None;
                                    let (box_, text) = (m.edit.clone(), text.clone());
                                    box_.update(cx, |b, cx| {
                                        b.set_value(text, window, cx);
                                        b.focus(window, cx);
                                    });
                                }
                                cx.notify();
                            })))
                            .child(mm_b(SharedString::from(format!("mm-del-{}", e.id)), "Delete", true).on_click(cx.listener(move |this, _, _, cx| this.memory_delete(del.clone(), cx)))),
                    ),
            )
            .when(m.asking.as_deref() == Some(e.id.as_str()), |r| r.child(div().mt(px(10.0)).child(self.ask_form(m, &e.id, agents, cx))))
            .into_any_element()
    }

    fn memory_ask(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(m) = &mut self.memory {
            m.asking = if m.asking.as_deref() == Some(id.as_str()) { None } else { Some(id) };
            m.editing = None;
            let hint = if m.asking.as_deref() == Some("*") { "Anything to look at? (optional)" } else { "What is wrong with it? (optional)" };
            m.note.update(cx, |n, cx| {
                n.set_placeholder(hint, window, cx);
                n.focus(window, cx);
            });
        }
        cx.notify();
    }

    /// Send a fact (or all of them) to one of the project's running agents.
    fn ask_form(&self, m: &MemoryView, id: &str, agents: &[(String, String)], cx: &mut Context<Self>) -> AnyElement {
        let cancel = im_btn("mm-ask-cancel", if agents.is_empty() { "Close" } else { "Cancel" }, false, true).on_click(cx.listener(|this, _, window, cx| this.memory_escape(window, cx)));
        if agents.is_empty() {
            let what = if id == "*" { "the memory" } else { "this fact" };
            return div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(div().text_size(px(13.0)).text_color(color(INK_3)).child(format!("Start an agent in {} first: it checks the code and fixes {what} for you.", m.name)))
                .child(div().flex().child(cancel))
                .into_any_element();
        }
        let picked = m.to.clone().filter(|t| agents.iter().any(|a| &a.0 == t)).unwrap_or_else(|| agents[0].0.clone());
        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(field(&m.note))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(div().text_size(px(12.5)).text_color(color(INK_2)).child("Send to"))
                    .children(agents.iter().enumerate().map(|(i, (aid, name))| {
                        let on = *aid == picked;
                        let pick = aid.clone();
                        div()
                            .id(("mm-to", i))
                            .h(px(32.0))
                            .px(px(10.0))
                            .flex()
                            .items_center()
                            .rounded_full()
                            .text_size(px(13.0))
                            .font_weight(FontWeight::MEDIUM)
                            .when(on, |b| b.bg(white(0.12)).text_color(color(INK)))
                            .when(!on, |b| b.bg(white(0.04)).text_color(color(INK_3)).hover(|b| b.text_color(color(INK))))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(m) = &mut this.memory {
                                    m.to = Some(pick.clone());
                                }
                                cx.notify();
                            }))
                            .child(name.clone())
                    }))
                    .child(div().flex_1())
                    .child(cancel)
                    .child(im_btn("mm-send", "Send", true, false).on_click(cx.listener(|this, _, window, cx| this.memory_send(window, cx)))),
            )
            .into_any_element()
    }
}
