//! The right side of Sessions: one conversation read back, drawn as today's
//! History draws it (transcript.ts, chat.css): your messages in bubbles on
//! the right, the agent's words as markdown, what it did as quiet steps.

use gpui_kit::component::text::TextView;

use crate::transcript::Item;

use super::list::clock;
use super::*;

/// A conversation opens with this much of its end; "Read all of it" loads the rest.
const TAIL_BYTES: u64 = 4 * 1024 * 1024;
/// Messages and steps drawn at once; "Show earlier" widens it.
const WINDOW: usize = 200;
/// A run of steps longer than this folds all but its last three.
const LONG_RUN: usize = 5;

pub(in crate::inbox) struct Reading {
    pub s: SessionInfo,
    items: Vec<Item>,
    /// Only the end of it was read.
    partial: bool,
    loading: bool,
    failed: Option<String>,
    window: usize,
    /// Step runs unfolded, by their first step's id.
    expanded: HashSet<String>,
    scroll: ScrollHandle,
    copied: bool,
}

/// ago: "just now", "5m ago", "3h ago", else the date.
fn ago(at_ms: u64, now_ms: u64) -> String {
    let m = (now_ms.saturating_sub(at_ms) as f64 / 60_000.0).round() as u64;
    match m {
        0 => "just now".to_string(),
        1..60 => format!("{m}m ago"),
        _ if m < 24 * 60 => format!("{}h ago", (m as f64 / 60.0).round() as u64),
        _ => super::list::day_label(at_ms, u64::MAX).split_once(' ').map(|(_, d)| d.to_string()).unwrap_or_default(),
    }
}

impl Inbox {
    /// Read a session: its end first. `whole` reads it from the start.
    pub(super) fn read_session(&mut self, info: SessionInfo, whole: bool, cx: &mut Context<Self>) {
        let Some(s) = &mut self.sessions else { return };
        s.read_no += 1;
        let no = s.read_no;
        s.reading = Some(Reading { s: info.clone(), items: Vec::new(), partial: false, loading: true, failed: None, window: WINDOW, expanded: HashSet::new(), scroll: ScrollHandle::new(), copied: false });
        let read = cx.background_spawn(async move { crate::transcript::read_session(&info.cwd, &info.id, (!whole).then_some(TAIL_BYTES)) });
        cx.spawn(async move |this, cx| {
            let got = read.await;
            let _ = this.update(cx, |this, cx| {
                let Some(r) = this.sessions.as_mut().filter(|s| s.read_no == no).and_then(|s| s.reading.as_mut()) else { return };
                match got {
                    Ok(read) => {
                        r.items = read.chat.items;
                        r.partial = read.partial;
                    }
                    Err(e) => r.failed = Some(e),
                }
                r.loading = false;
                if !whole {
                    r.scroll.scroll_to_bottom();
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn copy_session_id(&mut self, cx: &mut Context<Self>) {
        let Some(r) = self.sessions.as_mut().and_then(|s| s.reading.as_mut()) else { return };
        cx.write_to_clipboard(ClipboardItem::new_string(r.s.id.clone()));
        r.copied = true;
        let id = r.s.id.clone();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(1400)).await;
            let _ = this.update(cx, |this, cx| {
                if let Some(r) = this.sessions.as_mut().and_then(|s| s.reading.as_mut()).filter(|r| r.s.id == id) {
                    r.copied = false;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn sessions_reader(&self, s: &Sessions, _window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let base = div().flex().flex_col().flex_1().min_w_0().min_h_0().text_color(color(INK));
        let Some(r) = &s.reading else {
            return base.items_center().justify_center().child(div().text_size(px(13.5)).text_color(color(INK_3)).child("Pick a session to read it back.")).into_any_element();
        };
        let now = now_ms();
        let meta = [
            r.s.cwd.clone(),
            format!("{} {} ({})", super::list::day_label(r.s.modified_ms, now), clock(r.s.modified_ms), ago(r.s.modified_ms, now)),
            format!("{} message{}", r.s.messages, if r.s.messages == 1 { "" } else { "s" }),
        ]
        .into_iter()
        .filter(|x| !x.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");

        let start = r.items.len().saturating_sub(r.window);
        let mut thread: Vec<AnyElement> = Vec::new();
        let note = |text: String, err: bool| div().self_center().my(px(24.0)).text_size(px(13.5)).text_color(color(if err { DANGER } else { INK_3 })).child(text).into_any_element();
        if let Some(e) = &r.failed {
            thread.push(note(e.clone(), true));
        }
        if r.loading && r.items.is_empty() {
            thread.push(note("Reading the conversation…".to_string(), false));
        }
        if !r.loading && r.failed.is_none() && r.items.is_empty() {
            thread.push(note("Nothing to show in this session.".to_string(), false));
        }
        let earlier = |id: &'static str, label: String| {
            div()
                .id(id)
                .self_center()
                .min_h(px(26.0))
                .mt(px(2.0))
                .mb(px(8.0))
                .py(px(4.0))
                .px(px(11.0))
                .rounded_full()
                .bg(white(0.04))
                .text_size(px(12.5))
                .text_color(color(INK_3))
                .hover(|b| b.bg(white(0.08)).text_color(color(INK)))
                .child(label)
        };
        if r.partial && start == 0 && !r.loading {
            let info = r.s.clone();
            thread.push(earlier("sv-whole", "This is the end of a long session. Read all of it".to_string()).on_click(cx.listener(move |this, _, _, cx| this.read_session(info.clone(), true, cx))).into_any_element());
        } else if start > 0 {
            thread.push(
                earlier("sv-earlier", format!("Show {} earlier messages", start.min(WINDOW)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(r) = this.sessions.as_mut().and_then(|s| s.reading.as_mut()) {
                            r.window += WINDOW;
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        self.thread(&r.items[start..], &r.expanded, &mut thread, cx);

        base.child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(12.0))
                .pt(px(14.0))
                .pr(px(16.0))
                .pb(px(12.0))
                .pl(px(22.0))
                .border_b_1()
                .border_color(hairline())
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .gap(px(2.0))
                        .child(div().truncate().text_size(px(15.0)).font_weight(FontWeight::SEMIBOLD).child(if r.s.title.is_empty() { "Untitled".to_string() } else { r.s.title.clone() }))
                        .child(mono().truncate().text_size(px(12.0)).text_color(color(INK_3)).child(meta)),
                )
                .child(
                    div()
                        .id("sv-copyid")
                        .tooltip(tip("Copy this session's id"))
                        .flex_none()
                        .h(px(28.0))
                        .px(px(12.0))
                        .flex()
                        .items_center()
                        .rounded_full()
                        .bg(white(0.06))
                        .text_size(px(12.5))
                        .text_color(color(INK_2))
                        .hover(|b| b.bg(white(0.12)).text_color(color(INK)))
                        .on_click(cx.listener(|this, _, _, cx| this.copy_session_id(cx)))
                        .child(if r.copied { "Copied" } else { "Copy ID" }),
                ),
        )
        .child(
            div().id("sv-scroll").flex_1().min_h_0().overflow_y_scroll().track_scroll(&r.scroll).child(
                div().flex().justify_center().child(div().w(px(780.0)).max_w_full().flex().flex_col().gap(px(14.0)).pt(px(28.0)).px(px(28.0)).pb(px(24.0)).children(thread)),
            ),
        )
        .into_any_element()
    }

    /// runsBlocks: messages as they are; steps between two messages sit
    /// together, a long run folding its start.
    fn thread(&self, items: &[Item], expanded: &HashSet<String>, out: &mut Vec<AnyElement>, cx: &mut Context<Self>) {
        let mut run: Vec<&Item> = Vec::new();
        for item in items {
            if matches!(item, Item::Step { .. }) {
                run.push(item);
                continue;
            }
            self.flush_steps(&mut run, expanded, out, cx);
            out.push(match item {
                Item::User { text, .. } => div()
                    .flex()
                    .flex_col()
                    .items_end()
                    .mt(px(10.0))
                    .child(
                        div()
                            .max_w(px(620.0))
                            .py(px(11.0))
                            .px(px(15.0))
                            .rounded_tl(px(18.0))
                            .rounded_tr(px(18.0))
                            .rounded_br(px(6.0))
                            .rounded_bl(px(18.0))
                            .bg(color(0x1d1e23))
                            .shadow(vec![ring(white(0.07))])
                            .text_size(px(14.5))
                            .line_height(relative(1.55))
                            .child(text.clone()),
                    )
                    .into_any_element(),
                Item::Text { id, text } => TextView::markdown(SharedString::from(format!("sv-md-{id}")), text.clone())
                    .selectable(true)
                    .text_size(px(14.5))
                    .line_height(relative(1.65))
                    .text_color(color(0xe7e7ea))
                    .into_any_element(),
                Item::Step { .. } => unreachable!(),
            });
        }
        self.flush_steps(&mut run, expanded, out, cx);
    }

    fn flush_steps(&self, run: &mut Vec<&Item>, expanded: &HashSet<String>, out: &mut Vec<AnyElement>, cx: &mut Context<Self>) {
        let Some(Item::Step { id: key, .. }) = run.first() else { return };
        let key = key.clone();
        let long = run.len() > LONG_RUN;
        let fold = long && !expanded.contains(&key);
        let shown: &[&Item] = if fold { &run[run.len() - 3..] } else { &run[..] };
        // Only read when the run is long; a short one has nothing hidden.
        let hidden = run.len().saturating_sub(3);
        let mut col = div().flex().flex_col().gap(px(1.0)).my(px(-4.0));
        if long {
            let label = if fold { format!("{hidden} earlier steps") } else { format!("Fold {hidden} earlier steps") };
            let k = key.clone();
            col = col.child(
                div()
                    .id(SharedString::from(format!("sv-more-{key}")))
                    .self_start()
                    .min_h(px(26.0))
                    .mt(px(2.0))
                    .mb(px(4.0))
                    .py(px(4.0))
                    .px(px(11.0))
                    .rounded_full()
                    .bg(white(0.04))
                    .text_size(px(12.5))
                    .text_color(color(INK_3))
                    .hover(|b| b.bg(white(0.08)).text_color(color(INK)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(r) = this.sessions.as_mut().and_then(|s| s.reading.as_mut()) {
                            if !r.expanded.remove(&k) {
                                r.expanded.insert(k.clone());
                            }
                        }
                        cx.notify();
                    }))
                    .child(label),
            );
        }
        for step in shown {
            let Item::Step { verb, target, error, .. } = step else { continue };
            col = col.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .min_h(px(30.0))
                    .py(px(4.0))
                    .text_size(px(13.0))
                    .text_color(color(INK_3))
                    .child(div().flex_none().size(px(15.0)).flex().items_center().justify_center().child(div().size(px(5.0)).rounded_full().bg(color(if *error { DANGER } else { INK_3 }))))
                    .child(div().flex_none().child(verb.clone()))
                    .child(div().min_w_0().truncate().text_color(color(INK_2)).child(target.clone()))
                    .when(*error, |row| row.child(div().flex_none().text_size(px(12.0)).text_color(color(DANGER)).child("Failed"))),
            );
        }
        out.push(col.into_any_element());
        run.clear();
    }
}
