//! What each Needs you card holds (rail.ts rowHtml, targetOf, nav.ts rowTarget):
//! its head, its words and its buttons, with one honey button on the whole rail.

use crate::company::needs::{NeedKind, NeedRow};

use super::model::{OfficeModel, OfficePerson};
use super::words::plan_sentence;

/// Where a card takes you. The office has no floors to fly to here: a person's
/// desk is their seat on the stage, and a merge opens their Changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Target {
    Company,
    Desk(String),
    Changes(String),
}

/// A plan or a paused job is the Founder's to bring you, so it goes to the
/// Founder's desk, as its face says.
pub(super) fn target_of(r: &NeedRow, m: &OfficeModel) -> Target {
    if matches!(r.kind, NeedKind::Plan | NeedKind::Paused)
        && let Some(f) = &m.founder
    {
        return Target::Desk(f.id.clone());
    }
    match r.person_id.as_deref().and_then(|id| m.person(id)) {
        Some(p) if r.kind == NeedKind::Merge => Target::Changes(p.id.clone()),
        Some(p) => Target::Desk(p.id.clone()),
        None => Target::Company,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Act {
    Approve,
    SendBack,
    Raise,
    Finish,
    Dismiss,
    /// Go where the card points (Review, Go to, Answer at, Open).
    Open,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Primary,
    Secondary,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Button {
    pub act: Act,
    pub label: String,
    pub kind: Kind,
    /// Held while the note box says a note is missing.
    pub held: bool,
}

/// How a card is drawn: whether it takes you anywhere from here, whether it holds
/// the rail's one primary, and the state of its note box.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct RowLook {
    pub goes: bool,
    pub primary: bool,
    pub asking: bool,
    pub short: bool,
}

/// The one honey button: the first thing that asks for something, and nothing below it.
pub(super) fn lead_row(rows: &[NeedRow]) -> Option<usize> {
    rows.iter().position(|r| r.kind != NeedKind::Alert || !r.standing)
}

fn btn(act: Act, label: impl Into<String>, kind: Kind) -> Button {
    Button { act, label: label.into(), kind, held: false }
}

pub(super) fn row_buttons(r: &NeedRow, o: RowLook, m: &OfficeModel) -> Vec<Button> {
    // Asking for a note, the one thing to press is Send back, and Approve steps down.
    let main = if o.primary && !o.asking { Kind::Primary } else { Kind::Secondary };
    let person = r.person_id.as_deref().and_then(|id| m.person(id));
    let open = |label: String| o.goes.then(|| btn(Act::Open, label, main));
    match r.kind {
        NeedKind::Plan => vec![
            Button { held: o.short, ..btn(Act::Approve, "Approve", main) },
            btn(Act::SendBack, "Send back", if o.primary && o.asking { Kind::Primary } else { Kind::Secondary }),
        ],
        NeedKind::Paused => vec![btn(Act::Raise, "Add 1M tokens", main)],
        NeedKind::Merge => {
            let review = r.person_id.as_ref().and_then(|_| open("Review".into()));
            let finish = if o.goes && r.person_id.is_some() { Kind::Secondary } else { main };
            review.into_iter().chain([btn(Act::Finish, "Mark done", finish)]).collect()
        }
        NeedKind::Alert if r.standing => vec![btn(Act::Dismiss, "Dismiss", Kind::Secondary)],
        NeedKind::Alert => {
            let label = match person {
                Some(p) if r.key.starts_with("ask-") => format!("Answer at {}'s desk", p.name),
                Some(p) => format!("Go to {}", p.name),
                None => "Open".into(),
            };
            open(label).into_iter().collect()
        }
    }
}

/// Who the card is about: a plan and a paused job are the Founder's to bring you.
pub(super) fn who<'a>(r: &NeedRow, m: &'a OfficeModel) -> Option<&'a OfficePerson> {
    if matches!(r.kind, NeedKind::Plan | NeedKind::Paused) {
        return m.founder.as_ref();
    }
    r.person_id.as_deref().and_then(|id| m.person(id))
}

/// The card's head (name, the line under it) and its words (what, a quieter line).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RowWords {
    pub name: String,
    pub sub: Option<String>,
    pub what: String,
    pub more: String,
}

pub(super) fn row_words(r: &NeedRow, m: &OfficeModel) -> RowWords {
    let founders = matches!(r.kind, NeedKind::Plan | NeedKind::Paused);
    let person = who(r, m);
    let (name, sub) = match person {
        Some(p) => (p.name.clone(), Some(if founders { r.title.clone() } else { m.role_of(p) })),
        None => (r.title.clone(), None),
    };
    let named = person.is_some();
    // With a name on top, the title is what they need. Without one, the title is already up there.
    let what = match (&r.plan, r.kind) {
        (Some(p), NeedKind::Plan) => plan_sentence(p),
        _ if named && r.kind != NeedKind::Paused => r.title.clone(),
        _ => r.detail.clone(),
    };
    let more = if named && !founders { r.detail.clone() } else { String::new() };
    RowWords { name, sub, what, more }
}
