//! The floors in 2D: one block per project, its people with their state, and
//! what each is on. A person opens their desk, their seat on the stage. The
//! Founder sits on top, as in the office. Words follow the office's tags.

use super::look::*;
use super::model::{OfficeFloor, OfficeModel, OfficePerson, OfficeState};
use super::rows::Target;
use super::words::plural;
use super::*;

/// The state's word, as the Inbox says it, and its colour. Honey marks who waits on you.
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
            blocks.push(block("co-top", "Founder", None, true, vec![self.co_person(f, m, cx)]).into_any_element());
        }
        for f in &m.floors {
            blocks.push(self.co_floor(f, m, cx).into_any_element());
        }
        div().id("co-floors").size_full().overflow_y_scroll().p(px(S4)).pb(px(240.0)).flex().flex_col().gap(px(S3)).children(blocks).into_any_element()
    }

    fn co_floor(&self, f: &OfficeFloor, m: &OfficeModel, cx: &mut Context<Self>) -> Div {
        let count = plural(f.people.len() as u64, "person").replace("persons", "people").replace("0 people", "0 people");
        let people: Vec<AnyElement> = f.people.iter().map(|p| self.co_person(p, m, cx)).collect();
        let meta = if f.needs > 0 { format!("{count}, {} needs you", f.needs) } else { count };
        let body = if people.is_empty() { vec![text(BODY, INK_3).px(px(S2)).child("Nobody works here yet.").into_any_element()] } else { people };
        block(SharedString::from(format!("co-floor-{}", f.id)), f.name.clone(), Some(meta), f.lit || f.open, body)
    }

    fn co_person(&self, p: &OfficePerson, m: &OfficeModel, cx: &mut Context<Self>) -> AnyElement {
        let (word, tone) = state_look(p.state);
        let to = Target::Desk(p.id.clone());
        let role = m.role_of(p);
        let line = match &p.doing {
            Some(d) if p.state != OfficeState::Stopped => format!("{word}: {d}"),
            _ => word.to_string(),
        };
        div()
            .id(SharedString::from(format!("co-person-{}", p.id)))
            .flex()
            .items_center()
            .gap(px(S3))
            .px(px(S2))
            .py(px(S2))
            .rounded(px(R_CONTROL))
            .cursor_pointer()
            .hover(|s| s.bg(c(OVER)))
            .tooltip(tip(format!("Open {}'s desk", p.name)))
            .on_click(cx.listener(move |this, _, window, cx| this.co_go(to.clone(), window, cx)))
            .child(div().flex_none().size(px(32.0)).rounded_full().bg(c(OVER)).flex().items_center().justify_center().child(text(ROW, INK).child(p.name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default())))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(div().flex().items_baseline().gap(px(S2)).min_w_0().child(text(ROW, INK).flex_none().child(p.name.clone())).child(text(META, INK_3).min_w_0().truncate().child(role)))
                    .child(div().flex().items_center().gap(px(S2)).min_w_0().child(div().flex_none().size(px(8.0)).rounded_full().bg(c(tone))).child(text(META, INK_2).min_w_0().truncate().child(line))),
            )
            .child(svg().path("icons/chevron-right.svg").size(px(16.0)).text_color(c(INK_3)))
            .into_any_element()
    }
}

/// A floor's block: its name, a quiet count, and its people. A closed project dims.
fn block(id: impl Into<ElementId>, name: impl Into<SharedString>, meta: Option<String>, lit: bool, people: Vec<AnyElement>) -> Div {
    let head = div().flex().items_baseline().gap(px(S2)).px(px(S2)).child(text(TITLE, if lit { INK } else { INK_2 }).child(name.into())).children(meta.map(|m| text(META, INK_3).child(m)));
    let _ = id.into();
    div()
        .flex()
        .flex_col()
        .gap(px(S2))
        .p(px(S3))
        .rounded(px(R_CARD))
        .bg(c(PANEL))
        .shadow(vec![ring(line())])
        .child(head)
        .child(div().flex().flex_col().gap(px(2.0)).children(people))
}
