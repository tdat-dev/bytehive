//! History (inboxhistory.ts, describe and log in tasks.ts): what one agent
//! did, oldest at the top, grouped by day, so "what happened while I was
//! away?" has an answer that is not scrolling back through a terminal. Kept
//! while the app is open.

use supervise::ask::AskKind;

use super::sessions::list::{clock, day_label};
use super::*;

/// Enough to scroll back through a long session without growing forever.
const HISTORY_MAX: usize = 200;

#[derive(Clone)]
pub(super) struct Event {
    pub at: u64,
    pub state: State,
    pub text: String,
}

/// What a state change is checked by: the state, what it asks, a limit.
type Seen = (State, Option<String>, Option<String>, bool);

#[derive(Default)]
pub(super) struct Histories {
    events: HashMap<String, Vec<Event>>,
    seen: HashMap<String, Seen>,
    /// The agent whose History is open, by id.
    pub open: Option<String>,
    scroll: ScrollHandle,
    /// Events drawn last, to follow new ones to the bottom.
    drawn: usize,
}

/// The History line for a task entering its state, or none for states not
/// worth a line of their own.
pub(super) fn describe(s: &TaskStatus) -> Option<String> {
    match s.state {
        State::Needs => Some(match &s.ask {
            Some(a) if a.kind == AskKind::Run => a.detail.as_ref().map_or("Asked to run a command".to_string(), |d| format!("Asked to run {d}")),
            Some(a) if a.kind == AskKind::Edit => a.detail.as_ref().map_or("Asked to edit a file".to_string(), |d| format!("Asked to edit {d}")),
            Some(a) => format!("Asked: {}", a.prompt),
            None => "Waited for you".to_string(),
        }),
        State::Idle => s.limit.as_ref().map(|_| "Hit its usage limit".to_string()),
        State::Working => Some("Started working".to_string()),
        State::Stopped => Some("Stopped".to_string()),
        _ => None,
    }
}

impl Histories {
    /// Log a line, unless it repeats the last: a burst of output flips
    /// working on and off, one "Started working" per stretch is enough.
    pub fn log(&mut self, id: &str, ev: Event) {
        let list = self.events.entry(id.to_string()).or_default();
        if let Some(last) = list.last() {
            if last.text == ev.text || (ev.state == State::Working && last.state == State::Working) {
                return;
            }
        }
        list.push(ev);
        if list.len() > HISTORY_MAX {
            list.drain(..list.len() - HISTORY_MAX);
        }
    }

    /// Note an agent's status on a tick; a line when it moved.
    pub fn observe(&mut self, id: &str, s: &TaskStatus, now: u64) {
        let seen: Seen = (s.state, s.ask.as_ref().map(|a| a.prompt.clone()), s.ask.as_ref().and_then(|a| a.detail.clone()), s.limit.is_some());
        if self.seen.get(id) == Some(&seen) {
            return;
        }
        self.seen.insert(id.to_string(), seen);
        if let Some(text) = describe(s) {
            self.log(id, Event { at: now, state: s.state, text });
        }
    }

    pub fn of(&self, id: &str) -> &[Event] {
        self.events.get(id).map_or(&[], Vec::as_slice)
    }
}

impl Inbox {
    pub(super) fn open_history(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(seat) = self.fleet.seats.get(index) else { return };
        self.changes = None;
        self.palette = None;
        self.menu = None;
        self.history.open = Some(seat.spec.id.clone());
        self.history.drawn = usize::MAX;
        crate::keys::set_active(false);
        window.blur(cx);
        cx.notify();
    }

    pub(super) fn close_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.history.open.take();
        if let Some(index) = id.and_then(|id| self.fleet.seats.iter().position(|s| s.spec.id == id)) {
            self.fleet.focus(index, window, cx);
        }
        cx.notify();
    }

    pub(super) fn history_view(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let id = self.history.open.clone()?;
        let Some(seat) = self.fleet.seats.iter().find(|s| s.spec.id == id) else {
            self.history.open = None;
            return None;
        };
        let name = seat.spec.name.clone();
        let events = self.history.of(&id).to_vec();
        // New lines arrive at the bottom: follow them there.
        if events.len() != self.history.drawn {
            self.history.drawn = events.len();
            self.history.scroll.scroll_to_bottom();
        }
        let now = now_ms();
        let mut body: Vec<AnyElement> = Vec::new();
        if events.is_empty() {
            body.push(div().my(px(18.0)).text_size(px(13.0)).text_color(color(INK_3)).child(format!("Nothing yet. Questions, answers and finished work show up here as {name} goes.")).into_any_element());
        }
        let mut day = String::new();
        for e in &events {
            let d = day_label(e.at, now);
            if d != day {
                body.push(div().mt(px(14.0)).mb(px(6.0)).text_size(px(12.0)).font_weight(FontWeight::MEDIUM).text_color(color(INK_3)).child(d.clone()).into_any_element());
                day = d;
            }
            let dot = match e.state {
                State::Needs => div().size(px(7.0)).rounded(px(2.0)).bg(color(NEEDS)),
                State::Review => div().size(px(8.0)).rounded_full().bg(color(REVIEW)),
                State::Working => div().size(px(8.0)).rounded_full().bg(color(WORK)),
                _ => div().size(px(8.0)).rounded_full().bg(color(INK_3)),
            };
            body.push(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .py(px(6.0))
                    .text_size(px(13.5))
                    .child(mono().w(px(44.0)).flex_none().text_size(px(12.0)).text_color(color(INK_3)).child(clock(e.at)))
                    .child(div().w(px(14.0)).flex_none().flex().justify_center().child(dot))
                    .child(div().flex_1().min_w_0().child(e.text.clone()))
                    .into_any_element(),
            );
        }
        Some(
            div()
                .id("history")
                .absolute()
                .top(px(60.0))
                .right(px(16.0))
                .bottom(px(DOCK_H))
                .w(px(620.0))
                .max_w(relative(0.48))
                .flex()
                .flex_col()
                .overflow_hidden()
                .rounded(px(18.0))
                .bg(color(PANEL))
                .text_color(color(INK))
                .shadow(vec![BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.7), offset: point(px(0.0), px(30.0)), blur_radius: px(80.0), spread_radius: px(0.0), inset: false }, ring(white(0.09))])
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .gap(px(10.0))
                        .pt(px(14.0))
                        .pr(px(14.0))
                        .pb(px(10.0))
                        .pl(px(18.0))
                        .border_b_1()
                        .border_color(hairline())
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w_0()
                                .gap(px(2.0))
                                .child(div().text_size(px(15.0)).font_weight(FontWeight::SEMIBOLD).child(format!("{name}'s history")))
                                .child(mono().truncate().text_size(px(12.0)).text_color(color(INK_3)).child("Kept while ByteHive is open")),
                        )
                        .child(super::changes::close_button("ih-close").on_click(cx.listener(|this, _, window, cx| this.close_history(window, cx)))),
                )
                .child(div().id("ih-body").flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.history.scroll).pt(px(8.0)).px(px(18.0)).pb(px(18.0)).children(body))
                .into_any_element(),
        )
    }
}
