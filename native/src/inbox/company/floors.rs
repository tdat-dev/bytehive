//! The floors in 2D: one block per project, its people with their state, and
//! what each is on. A person opens their desk, their seat on the stage. The
//! Founder sits on top, as in the office. Words follow the office's tags.

use super::look::*;
use super::model::{OfficeFloor, OfficeModel, OfficePerson, OfficeState};
use super::rows::Target;
use super::words::{NOBODY_HERE, doing_line, floor_meta, open_desk};
use super::*;

/// The state's word, as the Inbox says it, and its colour: amber for who waits
/// on you, as the Inbox's rows.
fn state_look(s: OfficeState) -> (&'static str, u32) {
    match s {
        OfficeState::Needs => ("Needs you", HONEY),
        OfficeState::Working => ("Working", WORK),
        OfficeState::Review => ("Ready to review", REVIEW),
        OfficeState::Idle => ("Idle", STOP),
        OfficeState::Stopped => ("Stopped", STOP),
    }
}

impl Inbox {
    pub(super) fn co_floors(&self, m: &OfficeModel, cx: &mut Context<Self>) -> AnyElement {
        let mut blocks: Vec<AnyElement> = Vec::new();
        if let Some(f) = &m.founder {
            // The Founder needs no heading: their name says it.
            blocks.push(block(None, None, true, vec![self.co_person(f, m, cx)]).into_any_element());
        }
        for f in &m.floors {
            blocks.push(self.co_floor(f, m, cx).into_any_element());
        }
        div().id("co-floors").size_full().overflow_y_scroll().p(px(10.0)).flex().flex_col().gap(px(10.0)).children(blocks).into_any_element()
    }

    fn co_floor(&self, f: &OfficeFloor, m: &OfficeModel, cx: &mut Context<Self>) -> Div {
        let people: Vec<AnyElement> = f.people.iter().map(|p| self.co_person(p, m, cx)).collect();
        let meta = floor_meta(f.people.len() as u64, f.needs);
        let body = if people.is_empty() { vec![text(META, INK_3).px(px(10.0)).py(px(S1)).child(NOBODY_HERE).into_any_element()] } else { people };
        block(Some(f.name.clone().into()), Some(meta), f.lit || f.open, body)
    }

    fn co_person(&self, p: &OfficePerson, m: &OfficeModel, cx: &mut Context<Self>) -> AnyElement {
        let (word, tone) = state_look(p.state);
        let to = Target::Desk(p.id.clone());
        // A role that only repeats the name says nothing.
        let role = Some(m.role_of(p)).filter(|r| *r != p.name);
        let line = doing_line(word, p.doing.as_deref().filter(|_| p.state != OfficeState::Stopped));
        let waits = p.state == OfficeState::Needs;
        div()
            .id(SharedString::from(format!("co-person-{}", p.id)))
            .flex()
            .items_center()
            .gap(px(S3))
            .px(px(10.0))
            .py(px(S2))
            .rounded(px(12.0))
            .cursor_pointer()
            // The Inbox's rows: amber for who waits on you, a faint lift for the rest.
            .when(waits, |s| s.bg(c(HONEY).opacity(0.12)).shadow(vec![ring(c(HONEY).opacity(0.3))]).hover(|s| s.bg(c(HONEY).opacity(0.17))))
            .when(!waits, |s| s.hover(|s| s.bg(white_at(0.03))))
            .tooltip(tip(open_desk(&p.name)))
            .on_click(cx.listener(move |this, _, window, cx| this.co_go(to.clone(), window, cx)))
            .child(div().flex_none().size(px(32.0)).rounded_full().bg(c(OVER)).flex().items_center().justify_center().child(text(ROW, INK).child(p.name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default())))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(div().flex().items_baseline().gap(px(S2)).min_w_0().child(text(ROW, INK).flex_none().child(p.name.clone())).children(role.map(|r| text(META, INK_3).min_w_0().truncate().child(r))))
                    .child(div().flex().items_center().gap(px(S2)).min_w_0().child(div().flex_none().size(px(8.0)).rounded_full().bg(c(tone))).child(text(META, INK_2).min_w_0().truncate().child(line))),
            )
            .child(svg().path("icons/chevron-right.svg").size(px(16.0)).text_color(c(INK_3)))
            .into_any_element()
    }
}

/// A floor: its name and a quiet count over its people, as the Inbox heads a
/// project in its queue (no box: the panel is the box). A closed project dims.
fn block(name: Option<SharedString>, meta: Option<String>, lit: bool, people: Vec<AnyElement>) -> Div {
    let head = name.map(|name| {
        div()
            .flex()
            .items_center()
            .gap(px(7.0))
            .min_h(px(32.0))
            .px(px(10.0))
            .text_size(px(12.5))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(c(if lit { INK } else { INK_2 }))
            .child(div().min_w_0().truncate().child(name))
            .children(meta.map(|m| text(META, INK_3).font_weight(FontWeight::NORMAL).child(m)))
    });
    div().flex().flex_col().gap(px(2.0)).children(head).children(people)
}
