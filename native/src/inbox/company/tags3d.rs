//! The words over the 3D office (shell.ts wantedTags and place): at the
//! Building a sign hung off each floor and the top, and the name of whoever
//! the pointer is over; at a Floor a tag over each head and the board's
//! counts. Tags step apart so none covers another or a mark over someone.

use crate::office3d::layout::Level;
use crate::office3d::stage::{Anchor, AnchorKind};

use super::look::*;
use super::model::{OfficeFloor, OfficeModel, OfficePerson, OfficeState};
use super::rows::Target;
use super::*;

/// A floor's sign hangs this far off the building, on a line.
const LEADER: f32 = 28.0;
const HEAD_GAP: f32 = 3.0;
const EDGE: f32 = 6.0;
const TOUCH: f32 = 2.0;
/// A mark is a little wider than it is tall.
const BUBBLE_WIDE: f32 = 1.2;
/// Past this many people a floor's tags hold a name and no more.
const FULL_TAGS: usize = 4;
const TAG_H: f32 = 28.0;
const CHEVRON: f32 = 16.0;

#[derive(Clone, Copy, PartialEq)]
enum Tone {
    Name,
    Plain,
    Soft,
    Honey,
}

/// What a tag says, in runs, and what pressing it does.
struct Tag {
    key: (AnchorKind, String),
    runs: Vec<(String, Tone)>,
    needs: bool,
    chevron: bool,
    to: Option<Go>,
}

#[derive(Clone)]
enum Go {
    Floor(String),
    Desk(String),
}

fn need_you(n: usize) -> Option<(String, Tone)> {
    match n {
        0 => None,
        1 => Some((" 1 needs you".into(), Tone::Honey)),
        n => Some((format!(" {n} need you"), Tone::Honey)),
    }
}

fn floor_tag(f: &OfficeFloor) -> Tag {
    let people = if f.people.len() == 1 { " 1 person".to_string() } else { format!(" {} people", f.people.len()) };
    let mut runs = vec![(f.name.clone(), Tone::Name), (people, Tone::Soft)];
    runs.extend(need_you(f.needs));
    if !f.open {
        runs.push((" closed".into(), Tone::Soft));
    }
    Tag { key: (AnchorKind::Floor, f.id.clone()), runs, needs: f.needs > 0, chevron: true, to: Some(Go::Floor(f.id.clone())) }
}

fn person_tag(p: &OfficePerson, wanted: bool, short: bool) -> Tag {
    let who = if p.rank == crate::company::types::Rank::Lead { "Lead".to_string() } else { p.title.clone() };
    let mood = if wanted { Some("needs you") } else { match p.state {
        OfficeState::Review => Some("to review"),
        OfficeState::Idle => Some("idle"),
        OfficeState::Stopped => Some("stopped"),
        _ => None,
    } };
    let detail = mood.map_or(who.clone(), |m| format!("{who}, {m}"));
    let tone = if wanted { Tone::Honey } else { Tone::Plain };
    let mut runs = vec![(p.name.clone(), tone)];
    if !short {
        runs.push((format!(" {detail}"), if wanted { Tone::Honey } else { Tone::Soft }));
    }
    Tag { key: (AnchorKind::Person, p.id.clone()), runs, needs: wanted, chevron: false, to: Some(Go::Desk(p.id.clone())) }
}

fn wanted_tags(l: &Level, m: &OfficeModel, hovered: Option<&str>) -> Vec<Tag> {
    let wanted = |id: &str| m.needs.iter().any(|r| r.person_id.as_deref() == Some(id));
    match l {
        Level::Building => {
            let here: Vec<&str> = m.floors.iter().flat_map(|f| f.people.iter().map(|p| p.id.as_str())).collect();
            let loose = m.needs.iter().filter(|r| r.person_id.as_deref().is_none_or(|id| !here.contains(&id))).count();
            let mut out = Vec::new();
            if let Some(f) = &m.founder {
                let mut runs = vec![("Founder".to_string(), Tone::Plain)];
                runs.extend(need_you(loose));
                out.push(Tag { key: (AnchorKind::Top, "top".into()), runs, needs: loose > 0, chevron: true, to: Some(Go::Desk(f.id.clone())) });
            }
            out.extend(m.floors.iter().map(floor_tag));
            if let Some(p) = hovered.and_then(|id| m.person(id)) {
                out.push(person_tag(p, wanted(&p.id) || p.state == OfficeState::Needs, false));
            }
            out
        }
        Level::Floor(id) => {
            let Some(f) = m.floors.iter().find(|f| &f.id == id) else { return vec![] };
            let mut out: Vec<Tag> = f.people.iter().map(|p| person_tag(p, p.state == OfficeState::Needs || wanted(&p.id), f.people.len() > FULL_TAGS)).collect();
            if let Some((todo, doing, done)) = f.board {
                out.push(Tag { key: (AnchorKind::Board, f.id.clone()), runs: vec![(format!("To do {todo}, Doing {doing}, Done {done}"), Tone::Plain)], needs: false, chevron: false, to: None });
            }
            out
        }
        Level::Desk { .. } => vec![],
    }
}

#[derive(Clone, Copy, Debug)]
struct Box2 {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

fn hits(a: &Box2, b: &Box2) -> bool {
    a.x < b.x + b.w + TOUCH && b.x < a.x + a.w + TOUCH && a.y < b.y + b.h + TOUCH && b.y < a.y + a.h + TOUCH
}

/// Tags apart, in place: off a mark sideways, then nearest the viewer first,
/// each keeps its place and the one behind steps up past it (settleTags).
fn settle(tags: &mut [Box2], marks: &[Box2]) {
    for r in tags.iter_mut() {
        for o in marks {
            if hits(r, o) {
                r.x = EDGE.max(if r.x + r.w / 2.0 < o.x + o.w / 2.0 { o.x - r.w - TOUCH } else { o.x + o.w + TOUCH });
            }
        }
    }
    let mut order: Vec<usize> = (0..tags.len()).collect();
    order.sort_by(|a, b| tags[*b].y.total_cmp(&tags[*a].y));
    let mut done: Vec<usize> = Vec::new();
    for i in order {
        let mut k = 0;
        while k < done.len() {
            if hits(&tags[i], &tags[done[k]]) {
                tags[i].y = tags[done[k]].y - tags[i].h - TOUCH;
                k = 0;
                continue;
            }
            k += 1;
        }
        done.push(i);
    }
}

fn tone(t: Tone) -> (u32, FontWeight) {
    match t {
        Tone::Name => (INK, FontWeight::SEMIBOLD),
        Tone::Plain => (INK, FontWeight::NORMAL),
        Tone::Soft => (INK_2, FontWeight::NORMAL),
        Tone::Honey => (HONEY, FontWeight::SEMIBOLD),
    }
}

/// How wide a tag's words are, shaped as they will be drawn.
fn words_width(runs: &[(String, Tone)], window: &Window) -> f32 {
    let text: String = runs.iter().map(|(s, _)| s.as_str()).collect();
    let styled: Vec<TextRun> = runs
        .iter()
        .map(|(s, t)| {
            let (ink, weight) = tone(*t);
            TextRun { len: s.len(), font: Font { weight, ..font("Geist") }, color: c(ink), background_color: None, underline: None, strikethrough: None }
        })
        .collect();
    f32::from(window.text_system().shape_line(text.into(), px(META.0), &styled, None).width)
}

impl Inbox {
    pub(super) fn co_tags3d(&mut self, m: &OfficeModel, level: &Level, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        if crate::office3d::view::moving() {
            return div().into_any_element();
        }
        let anchors = crate::office3d::view::anchors();
        let hovered = crate::office3d::view::hovered();
        let (w, h) = crate::office3d::view::size();
        let find = |k: &(AnchorKind, String)| anchors.iter().find(|a| a.kind == k.0 && a.id == k.1);
        let mut live: Vec<(Tag, &Anchor)> = wanted_tags(level, m, hovered.as_deref()).into_iter().filter_map(|t| find(&t.key).map(|a| (t, a))).filter(|(_, a)| a.visible).collect();
        live.sort_by(|a, b| a.1.y.total_cmp(&b.1.y));
        let mut boxes: Vec<Box2> = live
            .iter()
            .map(|(t, a)| {
                let bw = words_width(&t.runs, window) + S2 * 2.0 + if t.chevron { CHEVRON + S1 } else { 0.0 };
                let above = matches!(t.key.0, AnchorKind::Person | AnchorKind::Board);
                let (x, y) = if above { (a.x as f32 - bw / 2.0, a.y as f32 - TAG_H - HEAD_GAP) } else { (a.x as f32 + LEADER, a.y as f32 - TAG_H / 2.0) };
                Box2 { x: x.min(w - bw - EDGE).max(EDGE), y: y.min(h - TAG_H - EDGE).max(EDGE), w: bw, h: TAG_H }
            })
            .collect();
        let marks: Vec<Box2> = live.iter().filter_map(|(_, a)| a.clear.map(|cl| Box2 { x: a.x as f32 - cl as f32 * BUBBLE_WIDE / 2.0, y: a.y as f32, w: cl as f32 * BUBBLE_WIDE, h: cl as f32 })).collect();
        settle(&mut boxes, &marks);
        let tags: Vec<AnyElement> = live.into_iter().zip(boxes).map(|((t, a), b)| self.co_tag3d(t, a, b, cx)).collect();
        div().absolute().inset_0().children(tags).into_any_element()
    }

    fn co_tag3d(&mut self, t: Tag, a: &Anchor, b: Box2, cx: &mut Context<Self>) -> AnyElement {
        let floorish = matches!(t.key.0, AnchorKind::Floor | AnchorKind::Top);
        let id = SharedString::from(format!("tag3d-{:?}-{}", t.key.0, t.key.1));
        let to = t.to.clone();
        let words = t.runs.into_iter().map(|(s, tn)| {
            let (ink, weight) = tone(tn);
            div().flex_none().text_color(c(ink)).font_weight(weight).child(s)
        });
        let ring_tone = if t.needs { c(HONEY).opacity(0.45) } else { line() };
        let mut tag = div()
            .id(id)
            .absolute()
            .left(px(b.x))
            .top(px(b.y))
            .h(px(TAG_H))
            .px(px(S2))
            .flex()
            .items_center()
            .gap(px(0.0))
            .rounded(px(R_CONTROL))
            .bg(c(PANEL))
            .shadow(vec![ring(ring_tone)])
            .text_size(px(META.0))
            .whitespace_nowrap()
            .children(words)
            .when(t.chevron, |d| d.child(div().ml(px(S1)).child(svg().path("icons/chevron-right.svg").size(px(CHEVRON)).text_color(c(INK_3)))));
        if let Some(go) = to {
            tag = tag.cursor_pointer().hover(|s| s.bg(c(RAISED))).occlude().on_click(cx.listener(move |this, _, window, cx| match &go {
                Go::Floor(f) => this.co_level(Level::Floor(f.clone()), cx),
                Go::Desk(p) => this.co_go(Target::Desk(p.clone()), window, cx),
            }));
        }
        // A floor's sign hangs off the building on a line, a dot at its end.
        let leader = floorish.then(|| {
            let y = b.y + TAG_H / 2.0;
            let x0 = (a.x as f32).max(b.x - LEADER);
            div()
                .child(div().absolute().left(px(x0)).top(px(y)).w(px((b.x - x0).max(0.0))).h(px(1.0)).bg(line_control()))
                .child(div().absolute().left(px(x0 - 2.5)).top(px(y - 2.0)).size(px(5.0)).rounded_full().bg(c(INK_2)))
        });
        div().children(leader).child(tag).into_any_element()
    }
}
