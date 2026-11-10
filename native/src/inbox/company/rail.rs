//! Needs you (rail.ts createRail): what the owner has to act on, as cards, each
//! with its one clear action, and the rail's one honey button on the first.

use crate::company::needs::{NeedKind, NeedRow};

use super::look::*;
use super::model::OfficeModel;
use super::rows::{Act, Button, Kind, RowLook, Target, lead_row, row_buttons, row_words, target_of, who};
use super::words::*;
use super::*;

impl Inbox {
    /// The rail: Needs you, then Jobs under it, always in sight.
    pub(super) fn co_rail(&self, snap: &Snapshot, cx: &mut Context<Self>) -> Div {
        let co = &self.company;
        let error = co.failure.clone().or_else(|| snap.error.clone());
        let mut rail = div().flex().flex_col().gap(px(S3)).min_h_0().text_color(c(INK));
        if let Some(e) = error {
            rail = rail.child(div().px(px(S3)).py(px(S2)).rounded(px(R_CONTROL)).bg(danger_soft()).child(text(BODY, DANGER).child(e)));
        }
        let Some(m) = &snap.model else {
            let note = snap.note.clone().unwrap_or_else(|| STARTING.into());
            return rail.child(heading(NEEDS_YOU, None, None)).child(text(BODY, INK_2).child(note));
        };
        let count = (!m.needs.is_empty()).then(|| m.needs.len().to_string());
        let lead = lead_row(&m.needs);
        let cards: Vec<AnyElement> = m.needs.iter().enumerate().map(|(i, r)| self.co_card(r, m, Some(i) == lead, cx)).collect();
        let needs = if cards.is_empty() {
            div()
                .flex()
                .flex_col()
                .gap(px(S1))
                .py(px(S1))
                .child(text(LEAD, INK).child(NOTHING_NEEDS_YOU))
                .child(text(BODY, INK_3).child(calm_line(m.counts.people, m.counts.working)))
                .into_any_element()
        } else {
            div()
                .id("co-needs")
                .flex()
                .flex_col()
                .gap(px(S2))
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(&co.needs_scroll)
                // The count of cards below follows a scroll.
                .on_scroll(cx.listener(|_, _, _, cx| cx.notify()))
                .children(cards)
                .into_any_element()
        };
        let below = self.co_below(m.needs.len(), cx);
        let meta = (!m.jobs.is_empty()).then_some(JOBS_META);
        rail.child(heading(NEEDS_YOU, count, None)).child(needs).children(below).child(heading(JOBS, None, meta)).child(self.co_jobs(m, cx))
    }

    /// Cards past the bottom of the list, at least partly: the scrollbar hides until
    /// hovered, so a cut list would read as the whole of it. A card counts once its
    /// buttons are cut. Measured on the last frame, where the cut was.
    fn co_below(&self, n: usize, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let scroll = &self.company.needs_scroll;
        // Child bounds are laid out unscrolled: the fold moves down as the list scrolls (offset is negative).
        let fold = scroll.bounds().bottom() - scroll.offset().y + px(CUT);
        let cut: Vec<usize> = (0..n).filter(|&i| scroll.bounds_for_item(i).is_some_and(|b| b.bottom() > fold)).collect();
        let first = *cut.first()?;
        let handle = scroll.clone();
        Some(
            div()
                .id("co-below")
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .gap(px(S1))
                .h(px(24.0))
                .mt(px(-S2))
                .rounded(px(R_CONTROL))
                .cursor_pointer()
                .hover(|s| s.bg(c(RAISED)))
                .on_click(cx.listener(move |_, _, _, cx| {
                    handle.scroll_to_top_of_item(first);
                    // The scroll lands on the next frame: count again once it has.
                    cx.spawn(async move |this, cx| {
                        cx.background_executor().timer(std::time::Duration::from_millis(50)).await;
                        let _ = this.update(cx, |_, cx| cx.notify());
                    })
                    .detach();
                }))
                .child(svg().path("icons/chevron-down.svg").size(px(16.0)).text_color(c(INK_2)))
                .child(text(ROW, INK_2).child(format!("{} below", cut.len()))),
        )
    }

    fn co_card(&self, r: &NeedRow, m: &OfficeModel, primary: bool, cx: &mut Context<Self>) -> AnyElement {
        let co = &self.company;
        let job = r.job_id.clone().unwrap_or_default();
        let asking = r.job_id.as_ref().is_some_and(|j| co.asking.contains(j) || co.note_typed(j, cx));
        let short = r.job_id.as_ref().is_some_and(|j| co.short.contains(j));
        let target = target_of(r, m);
        let look = RowLook { goes: target != Target::Company, primary, asking, short };
        let w = row_words(r, m);
        let face = who(r, m).map(|p| p.name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".into()));
        let mut head = div().flex().items_center().gap(px(S3)).w_full().min_w_0();
        if let Some(initial) = face {
            head = head.child(div().flex_none().size(px(40.0)).rounded_full().bg(c(OVER)).flex().items_center().justify_center().child(text(TITLE, INK).child(initial)));
        }
        let mut who_col = div().flex().flex_col().min_w_0().child(text(ROW, INK).child(w.name.clone()));
        if let Some(sub) = w.sub.clone() {
            who_col = who_col.child(text(META, INK_3).truncate().child(sub));
        }
        head = head.child(who_col);
        let key = r.key.clone();
        let top = if look.goes {
            let to = target.clone();
            div()
                .id(SharedString::from(format!("co-go-{key}")))
                .group("co-go")
                .w_full()
                .flex()
                .items_center()
                .cursor_pointer()
                .rounded(px(R_CONTROL))
                .on_click(cx.listener(move |this, _, window, cx| this.co_go(to.clone(), window, cx)))
                .child(head)
                .child(div().ml_auto().flex_none().child(svg().path("icons/chevron-right.svg").size(px(16.0)).text_color(c(INK_3)).group_hover("co-go", |s| s.text_color(c(INK)))))
                .into_any_element()
        } else {
            head.into_any_element()
        };
        let buttons: Vec<AnyElement> = row_buttons(r, look, m).into_iter().map(|b| self.co_button(r, &target, b, cx)).collect();
        let acts = div().flex().flex_wrap().justify_end().gap(px(S2)).children(buttons);
        let what = (!w.what.is_empty()).then(|| text(BODY, INK).child(w.what.clone()));
        let more = (!w.more.is_empty()).then(|| text(META, INK_2).child(w.more.clone()));
        let mut card = card().id(SharedString::from(format!("co-card-{key}"))).child(top);
        if r.kind == NeedKind::Plan {
            card = card.children(what).children(self.co_details(r, cx)).children(self.co_note(&job, asking, short, cx)).child(acts);
        } else {
            let words = div().flex().flex_col().gap(px(2.0)).flex_1().min_w(px(150.0)).children(what).children(more);
            card = card.child(div().flex().flex_wrap().items_center().justify_end().gap(px(S2)).child(words).child(acts));
        }
        card.into_any_element()
    }

    fn co_button(&self, r: &NeedRow, target: &Target, b: Button, cx: &mut Context<Self>) -> AnyElement {
        let kind = if b.kind == Kind::Primary { Btn::Primary } else { Btn::Secondary };
        let job = r.job_id.clone().unwrap_or_default();
        let key = r.key.clone();
        let to = target.clone();
        let id = SharedString::from(format!("co-act-{:?}-{key}", b.act));
        button(id, b.label.clone(), kind, false, b.held)
            .tooltip(tip(format!("{}: {}", b.label, r.title)))
            .on_click(cx.listener(move |this, _, window, cx| match b.act {
                Act::Approve if !b.held => this.co_approve(&job, window, cx),
                Act::Approve => {}
                Act::SendBack => this.co_send_back(&job, window, cx),
                Act::Raise => this.co_raise(&job, cx),
                Act::Finish => this.co_finish(&job, cx),
                Act::Dismiss => this.co_dismiss(&key, cx),
                Act::Open => this.co_go(to.clone(), window, cx),
            }))
            .into_any_element()
    }

    /// The plan in full, folded under its sentence; open when it drafts a role.
    fn co_details(&self, r: &NeedRow, cx: &mut Context<Self>) -> Option<AnyElement> {
        let plan = r.plan.as_ref()?;
        let key = r.key.clone();
        let open = self.company.unfolded.contains(&r.key) || !plan.new_roles.is_empty();
        let summary = div()
            .id(SharedString::from(format!("co-more-{key}")))
            .flex()
            .items_center()
            .gap(px(S1))
            .cursor_pointer()
            .text_color(c(INK_2))
            .hover(|s| s.text_color(c(INK)))
            .on_click(cx.listener(move |this, _, _, cx| {
                if !this.company.unfolded.remove(&key) {
                    this.company.unfolded.insert(key.clone());
                }
                cx.notify();
            }))
            .child(svg().path(if open { "icons/chevron-down.svg" } else { "icons/chevron-right.svg" }).size(px(12.0)).text_color(c(INK_2)))
            .child(text(META, INK_2).child(DETAILS));
        let mut fold = div().flex().flex_col().child(summary);
        if open {
            let items = plan_items(plan, &r.detail).into_iter().map(|(term, words, prompt)| {
                let dd = text(META, INK).child(words);
                let dd = if prompt { dd.p(px(S2)).rounded(px(R_CONTROL)).bg(c(GROUND)) } else { dd };
                div().flex().flex_col().mt(px(S2)).child(text(META, INK_3).child(term)).child(dd)
            });
            fold = fold.child(div().mt(px(S1)).flex().flex_col().children(items));
        }
        Some(fold.into_any_element())
    }

    /// Why send it back: the note box under a plan, open once Send back is pressed.
    fn co_note(&self, job: &str, asking: bool, short: bool, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !asking {
            return None;
        }
        let note = self.company.notes.get(job)?;
        let j = job.to_string();
        let close = div()
            .id(SharedString::from(format!("co-unask-{job}")))
            .size(px(24.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(R_CONTROL))
            .cursor_pointer()
            .hover(|s| s.bg(c(RAISED)))
            .tooltip(tip(CLOSE_NOTE))
            .on_click(cx.listener(move |this, _, window, cx| this.co_unask(&j, window, cx)))
            .child(svg().path("icons/x.svg").size(px(16.0)).text_color(c(INK_2)));
        let head = div().flex().items_center().justify_between().gap(px(S2)).child(text(ROW, INK).child(WHY_LABEL)).child(close);
        let wrong = short.then(|| text(META, DANGER).mt(px(S1)).child(NOTE_NEEDED));
        Some(div().flex().flex_col().gap(px(S1)).child(head).child(field().child(Textarea::new(note).appearance(false).with_size(px(BODY.0 / 0.875)))).children(wrong).into_any_element())
    }
}

/// A rail heading: its words, the count of what needs you, and a quiet note at its end.
/// The count is ink: honey goes on the one button that waits for the owner.
fn heading(words: &'static str, count: Option<String>, meta: Option<&'static str>) -> Div {
    div()
        .flex()
        .items_baseline()
        .gap(px(S2))
        .child(text(TITLE, INK).child(words))
        .children(count.map(|n| text(TITLE, INK).child(n)))
        .children(meta.map(|m| text(META, INK_3).ml_auto().child(m)))
}
