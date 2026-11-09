//! A card on its list (.kb-card, cardFace in kanban.ts): labels, title, who
//! works it, due, checklist progress, a note mark, and once done what changed.

use super::look::{CardDrag, Ghost, MUTED_2, SOON, SURFACE_3, accent_dim, line, line_strong};
use super::*;

/// The base name of a path (fileBase).
fn file_base(p: &str) -> &str {
    p.rsplit(['\\', '/']).next().unwrap_or(p)
}

/// .kb-badge: an 11px icon and mono text.
fn badge(icon_path: &'static str, tone: u32) -> Div {
    mono().flex().items_center().gap(px(4.0)).text_size(px(10.0)).text_color(color(tone)).child(svg().path(icon_path).size(px(11.0)).flex_none().opacity(0.85).text_color(color(tone)))
}

impl Inbox {
    pub(super) fn board_card(&self, list: &str, card: &model::Card, cx: &mut Context<Self>) -> AnyElement {
        let id = card.id.clone();
        let (open_id, drop_list, drop_before) = (id.clone(), list.to_string(), id.clone());
        let labels = (!card.labels.is_empty()).then(|| div().flex().flex_wrap().gap(px(4.0)).children(card.labels.iter().map(|k| div().w(px(34.0)).h(px(7.0)).rounded(px(4.0)).bg(color(model::label_hex(k))))));
        let mut badges: Vec<AnyElement> = Vec::new();
        if let Some(due) = &card.due {
            let (label, over, soon) = model::fmt_due(due, chrono::Local::now().date_naive());
            let tone = if over { DANGER } else if soon { SOON } else { MUTED_2 };
            badges.push(badge("icons/kb-due.svg", tone).py(px(1.0)).px(px(6.0)).rounded(px(5.0)).bg(if over { tint(DANGER, 0.14) } else { color(SURFACE_3) }).child(label).into_any_element());
        }
        if !card.checklist.is_empty() {
            let done = card.checklist.iter().filter(|i| i.done).count();
            let all = done == card.checklist.len();
            badges.push(badge("icons/kb-checklist.svg", if all { WORK } else { MUTED_2 }).child(format!("{done}/{}", card.checklist.len())).into_any_element());
        }
        if !card.desc.trim().is_empty() {
            badges.push(badge("icons/kb-desc.svg", MUTED_2).into_any_element());
        }
        let assignee = card.assignee.clone().map(|name| {
            let who = name.clone();
            div().flex().child(
                div()
                    .id(SharedString::from(format!("kb-assignee-{id}")))
                    .mt(px(6.0))
                    .max_w_full()
                    .py(px(2.0))
                    .px(px(8.0))
                    .rounded_full()
                    .border_1()
                    .border_color(white(0.08))
                    .bg(white(0.08))
                    .text_size(px(11.0))
                    .text_color(color(INK))
                    .truncate()
                    .hover(|s| s.bg(white(0.2)))
                    .tooltip(tip("Focus this agent's pane"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.board_focus_agent(&who, window, cx);
                    }))
                    .child(format!("⚡ {name}")),
            )
        });
        let done = card.done.as_ref().map(|d| {
            let chip = |text: String| mono().max_w(px(120.0)).truncate().py(px(1.0)).px(px(6.0)).rounded(px(4.0)).bg(color(SURFACE_3)).text_size(px(10.0)).text_color(color(INK_2)).child(text);
            div()
                .mt(px(7.0))
                .pt(px(7.0))
                .border_t_1()
                .border_color(line())
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(4.0))
                .child(div().text_size(px(11.5)).font_weight(FontWeight::MEDIUM).text_color(color(WORK)).child("✓ done"))
                .children(d.files.iter().take(5).enumerate().map(|(n, f)| div().id(SharedString::from(format!("kb-file-{id}-{n}"))).tooltip(tip(f.clone())).child(chip(file_base(f).to_string()))))
                .when(d.files.len() > 5, |r| r.child(chip(format!("+{}", d.files.len() - 5)).text_color(color(REVIEW)).bg(tint(0x5ec2f0, 0.12))))
        });
        let drag = CardDrag { id: id.clone(), title: card.title.trim().to_string().into() };
        div()
            .id(SharedString::from(format!("kb-card-{id}")))
            .relative()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .py(px(8.0))
            .px(px(10.0))
            .rounded(px(10.0))
            .bg(color(RAISE))
            .border_1()
            .border_color(line())
            .shadow(vec![BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.3), offset: point(px(0.0), px(1.0)), blur_radius: px(0.0), spread_radius: px(0.0), inset: false }])
            .cursor_pointer()
            .hover(|s| s.border_color(line_strong()).shadow(vec![BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.6), offset: point(px(0.0), px(3.0)), blur_radius: px(12.0), spread_radius: px(-5.0), inset: false }]))
            .drag_over::<CardDrag>(|s, _, _, _| s.border_color(accent_dim()))
            .on_drag(drag, |d, _, _, cx| cx.new(|_| Ghost { title: d.title.clone(), list: false }))
            .on_drop(cx.listener(move |this, d: &CardDrag, window, cx| this.board_drop_card(d.id.clone(), drop_list.clone(), Some(drop_before.clone()), window, cx)))
            .on_click(cx.listener(move |this, _, window, cx| this.board_open_card(open_id.clone(), window, cx)))
            .children(labels)
            .child(div().text_size(px(12.5)).line_height(relative(1.4)).text_color(color(INK)).child(card.title.clone()))
            .children(assignee)
            .when(!badges.is_empty(), |c| c.child(div().mt(px(1.0)).flex().flex_wrap().items_center().gap(px(8.0)).children(badges)))
            .children(done)
            .into_any_element()
    }
}
