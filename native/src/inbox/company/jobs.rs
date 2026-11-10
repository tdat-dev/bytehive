//! Jobs (rail.ts jobHtml, confirmEnd) and the box that gives the Founder a job
//! (createJobBox): one line per job still going, Stop behind its menu and a
//! confirm, and a box whose Send is honey only when nothing else waits.

use crate::company::types::JobStatus;

use super::look::*;
use super::model::{OfficeJob, OfficeModel};
use super::words::*;
use super::*;

/// The dot before a job: paused and planning are quiet, waiting is yours.
fn job_dot(status: &JobStatus) -> u32 {
    match status {
        JobStatus::Running => WORK,
        JobStatus::AwaitingPlan => HONEY,
        _ => STOP,
    }
}

impl Inbox {
    pub(super) fn co_jobs(&self, m: &OfficeModel, cx: &mut Context<Self>) -> AnyElement {
        if m.jobs.is_empty() {
            return text(BODY, INK_2).py(px(S1)).child(NO_JOB).into_any_element();
        }
        let lines: Vec<AnyElement> = m.jobs.iter().map(|j| self.co_job(j, cx)).collect();
        div().id("co-jobs").flex().flex_col().flex_none().max_h(relative(0.4)).overflow_y_scroll().children(lines).into_any_element()
    }

    fn co_job(&self, j: &OfficeJob, cx: &mut Context<Self>) -> AnyElement {
        let paused = j.status == JobStatus::Paused;
        let done = done_percent(j.cards) as f32 / 100.0;
        let id = j.id.clone();
        let menu_open = self.company.menu.as_deref() == Some(j.id.as_str());
        let more = div()
            .id(SharedString::from(format!("co-menu-{id}")))
            .flex_none()
            .size(px(24.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(R_CONTROL))
            .cursor_pointer()
            .when(menu_open, |s| s.bg(c(OVER)))
            .hover(|s| s.bg(c(RAISED)))
            .tooltip(tip(format!("More for {}", j.title)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.company.menu = if this.company.menu.as_deref() == Some(id.as_str()) { None } else { Some(id.clone()) };
                cx.notify();
            }))
            .child(svg().path("icons/ellipsis.svg").size(px(16.0)).text_color(c(INK_2)));
        let line = div()
            .relative()
            .flex()
            .items_center()
            .gap(px(S2))
            .min_h(px(36.0))
            .border_b_1()
            .border_color(line())
            .child(div().flex_none().size(px(8.0)).rounded_full().bg(c(job_dot(&j.status))))
            .child(text(ROW, if paused { INK_2 } else { INK }).flex_1().min_w_0().truncate().child(j.title.clone()))
            .child(
                div()
                    .id(SharedString::from(format!("co-state-{}", j.id)))
                    .flex_none()
                    .tooltip(tip(job_tokens(j.tokens_used, j.token_cap)))
                    .child(text(META, INK_3).whitespace_nowrap().child(job_state(&j.status, j.label, j.tokens_used, j.token_cap))),
            )
            .child(more)
            // How far along it is, from its cards, under the line.
            .child(div().absolute().bottom_0().left_0().h(px(2.0)).w(relative(done)).bg(c(if paused { INK_3 } else { INK_2 })));
        let menu = menu_open.then(|| {
            let id = j.id.clone();
            div()
                .ml_auto()
                .mb(px(S2))
                .min_w(px(160.0))
                .p(px(S1))
                .flex()
                .flex_col()
                .rounded(px(R_CARD))
                .bg(c(OVER))
                .shadow(vec![ring(line())])
                .child(button(SharedString::from(format!("co-stop-{id}")), stop_word(&j.status), Btn::Danger, false, false).justify_start().w_full().on_click(cx.listener(move |this, _, _, cx| {
                    this.company.menu = None;
                    this.company.confirm = Some(id.clone());
                    cx.notify();
                })))
        });
        div().flex().flex_col().child(line).children(menu).into_any_element()
    }

    /// Ending a job is its own decision: it says what stops and what stays.
    pub(super) fn co_confirm(&self, m: &OfficeModel, cx: &mut Context<Self>) -> Option<AnyElement> {
        let id = self.company.confirm.clone()?;
        let j = m.jobs.iter().find(|j| j.id == id)?;
        let word = stop_word(&j.status);
        let keep = button("co-keep", KEEP_JOB, Btn::Secondary, false, false).on_click(cx.listener(|this, _, _, cx| {
            this.company.confirm = None;
            cx.notify();
        }));
        let stop = button("co-stop-yes", word, Btn::Danger, false, false).bg(c(DANGER)).text_color(c(GROUND)).font_weight(FontWeight::SEMIBOLD).on_click(cx.listener(move |this, _, _, cx| {
            this.company.confirm = None;
            this.co_stop(&id, cx);
        }));
        let dialog = div()
            .id("co-confirm")
            .w(px(420.0))
            .max_w_full()
            .flex()
            .flex_col()
            .gap(px(S3))
            .p(px(S6))
            .rounded(px(14.0))
            .bg(c(PANEL))
            .shadow(vec![ring(line()), lift()])
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(text(TITLE, INK).child(format!("{word}?")))
            .child(text(BODY, INK_2).child(format!("\u{201c}{}\u{201d} ends here, and its people stop working on it. Its cards and branches stay.", j.title)))
            .child(div().flex().justify_end().gap(px(S2)).child(keep).child(stop));
        Some(
            div()
                .id("co-scrim")
                .absolute()
                .inset_0()
                .bg(c(0x080a0e).opacity(0.6))
                .flex()
                .items_center()
                .justify_center()
                .px(px(S4))
                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                    this.company.confirm = None;
                    cx.notify();
                }))
                .child(dialog)
                .into_any_element(),
        )
    }

    /// The box that gives the Founder a job, with examples while it is empty.
    pub(super) fn co_job_box(&self, m: Option<&OfficeModel>, compact: bool, cx: &mut Context<Self>) -> Option<Div> {
        let ask = self.company.ask.as_ref()?;
        let typed = ask.read(cx).value().trim().to_string();
        let ready = m.is_some();
        // Written and ready is your move, unless something above it is waiting for you first.
        let honey = !typed.is_empty() && m.is_some_and(|m| m.needs.is_empty());
        let send = button("co-send", SEND, if honey { Btn::Primary } else { Btn::Secondary }, false, !ready)
            .on_click(cx.listener(|this, _, window, cx| this.co_give(window, cx)));
        let chips = (ask.read(cx).value().is_empty() && !compact).then(|| {
            div().flex().flex_wrap().gap(px(S2)).children(EXAMPLES.iter().map(|x| {
                button(SharedString::from(format!("co-chip-{x}")), *x, Btn::Secondary, true, false).text_color(c(INK_2)).on_click(cx.listener(move |this, _, window, cx| this.co_chip(x, window, cx)))
            }))
        });
        let foot = div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(S2))
            .when(compact, |f| f.justify_end())
            .when(!compact, |f| f.child(text(META, INK_3).child(SEND_HINT)))
            .child(send);
        let error = self.company.box_failure.clone().map(|e| div().px(px(S3)).py(px(S2)).rounded(px(R_CONTROL)).bg(danger_soft()).child(text(BODY, DANGER).child(e)));
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(S2))
                .child(text(if compact { TITLE } else { PAGE }, INK).child(GIVE_JOB))
                .child(field().child(Textarea::new(ask).appearance(false).with_size(px(LEAD.0 / 0.875))))
                .children(chips)
                .child(foot)
                .children(error),
        )
    }
}
