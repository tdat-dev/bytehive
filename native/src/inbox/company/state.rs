//! What the Company page holds between frames (rail.ts's sets and the job box),
//! opening and closing it, and what the owner's clicks do.

use std::collections::{HashMap, HashSet};

use super::rows::Target;
use super::words::*;
use super::*;

#[derive(Default)]
pub(in crate::inbox) struct CompanyUi {
    /// The page is up, in place of the agents.
    pub open: bool,
    /// The person whose desk is on the stage, for the crumb back.
    pub desk: Option<String>,
    pub engine: Option<crate::company::engine::Engine>,
    /// The engine's view as of its change count: a frame reads this, not a fresh clone.
    pub view: std::cell::RefCell<Option<(u64, crate::company::engine::CompanyView)>>,
    /// The fixture company a debug build shows with BYTEHIVE_COMPANY_DEMO=1, in place of the engine.
    pub demo: Option<super::demo::Demo>,
    /// When the engine last ticked.
    pub ticked: u64,
    /// Why the last action failed, in the owner's words.
    pub failure: Option<String>,
    pub box_failure: Option<String>,
    /// When a failure was said: it clears on its own a few seconds on.
    pub failed_at: u64,
    /// Notes being typed, by job.
    pub notes: HashMap<String, Entity<TextareaState>>,
    /// Jobs whose note box is open, the ones told they need a note, and plans unfolded.
    pub asking: HashSet<String>,
    pub short: HashSet<String>,
    pub unfolded: HashSet<String>,
    /// The job whose menu is open, and the job waiting on its Stop to be confirmed.
    pub menu: Option<String>,
    /// The Needs you list, to tell which cards are cut at its foot.
    pub needs_scroll: ScrollHandle,
    /// The window size and card count the cards were last laid out at.
    pub laid_out: (Size<Pixels>, usize),
    /// The narrow layout's rail, which scrolls as one, and where its cards and its Jobs
    /// heading were painted last (window coordinates), to say what waits below.
    pub rail_scroll: ScrollHandle,
    pub cards_at: std::rc::Rc<std::cell::RefCell<Vec<Bounds<Pixels>>>>,
    pub jobs_at: std::rc::Rc<std::cell::RefCell<Option<Bounds<Pixels>>>>,
    pub confirm: Option<String>,
    /// The job box, made the first time the page shows.
    pub ask: Option<Entity<TextareaState>>,
    pub hint: String,
    /// The engine found a Job on disk at start: open on the Company once.
    pub land: bool,
    /// The boot has not decided yet whether to land on the Company.
    pub boot: bool,
    /// The owner pressed a key or clicked since start: the app stays where they are.
    pub touched: bool,
    pub subs: Vec<Subscription>,
    /// What each person has used in fresh tokens, by person id, and when it was last read.
    pub fresh: HashMap<String, Option<u64>>,
    pub fresh_at: u64,
    pub fresh_busy: bool,
    /// Where the 3D office's camera is: the Building or one floor.
    pub level3d: crate::office3d::layout::Level,
}

impl CompanyUi {
    pub(super) fn note_typed(&self, job: &str, cx: &App) -> bool {
        self.notes.get(job).is_some_and(|n| !n.read(cx).value().is_empty())
    }
}

impl Inbox {
    pub(in crate::inbox) fn open_company(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette = None;
        self.menu = None;
        self.popup = None;
        self.board = None;
        crate::keys::set_active(false);
        self.company.open = true;
        self.company.desk = None;
        if self.company.ask.is_none() {
            let ask = cx.new(|cx| TextareaState::new(window, cx).auto_grow(2, 8).submit_on_enter(true));
            let sub = cx.subscribe_in(&ask, window, |this, _, event: &InputEvent, window, cx| match event {
                // Enter sends, Shift+Enter starts a new line; the box keeps Enter while an IME composes.
                InputEvent::PressEnter { secondary: false, shift: false } => this.co_give(window, cx),
                InputEvent::Change => cx.notify(),
                _ => {}
            });
            self.company.subs.push(sub);
            self.company.ask = Some(ask);
        }
        cx.notify();
    }

    pub(in crate::inbox) fn close_company(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.company.open = false;
        self.company.menu = None;
        self.company.confirm = None;
        self.home = self.fleet.seats.is_empty();
        if !self.home
            && let Some(index) = self.fleet.focused
        {
            self.fleet.focus(index, window, cx);
        }
        cx.notify();
    }

    /// Ctrl+Shift+O and the palette: Company and the Inbox, back and forth.
    pub(in crate::inbox) fn toggle_company(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // A dialog, a sheet, Settings or Sessions stays in front: no page swaps under it.
        if self.dialog_up() || self.sessions.is_some() {
            return;
        }
        if self.company.open { self.close_company(window, cx) } else { self.open_company(window, cx) }
    }

    /// Esc on the page puts away its confirm, then its menu, then takes the 3D
    /// office one level up. The page itself stays.
    pub(in crate::inbox) fn company_escape(&mut self, cx: &mut Context<Self>) -> bool {
        let co = &mut self.company;
        let done = co.confirm.take().is_some() || co.menu.take().is_some();
        if done {
            cx.notify();
        }
        done || self.co_level_up(cx)
    }

    /// The box's example names the company's first project.
    pub(super) fn co_hint(&mut self, first_floor: Option<&str>, window: &mut Window, cx: &mut Context<Self>) {
        let hint = ask_hint(first_floor);
        if self.company.hint == hint {
            return;
        }
        if let Some(ask) = &self.company.ask {
            ask.update(cx, |a, cx| a.set_placeholder(hint.clone(), window, cx));
        }
        self.company.hint = hint;
    }

    /// An example goes in the box; the sending stays the owner's.
    pub(super) fn co_chip(&mut self, words: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ask) = &self.company.ask {
            ask.update(cx, |a, cx| {
                a.set_value(words.to_string(), window, cx);
                a.focus(window, cx);
            });
        }
        cx.notify();
    }

    pub(super) fn co_give(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Behind the confirm's scrim the box takes no Enter.
        if self.company.confirm.is_some() {
            return;
        }
        let Some(ask) = self.company.ask.clone() else { return };
        let text = ask.read(cx).value().to_string();
        if text.trim().is_empty() || (self.company.engine.is_none() && self.company.demo.is_none()) {
            return;
        }
        self.company.box_failure = None;
        let r = match &mut self.company.demo {
            Some(d) => Some(Ok(d.give_job(&text))),
            None => self.co_engine(cx, |e, host| e.give_job(host, &text)),
        };
        match r {
            Some(Ok(())) => {
                if ask.read(cx).value() == text {
                    ask.update(cx, |a, cx| a.set_value("", window, cx));
                }
            }
            Some(Err(why)) => {
                self.company.box_failure = Some(why);
                self.company.failed_at = now_ms();
            }
            None => {}
        }
        cx.notify();
    }

    /// An owner action on the demo when it is up, else on the engine.
    fn co_act(
        &mut self,
        cx: &mut Context<Self>,
        demo: impl FnOnce(&mut super::demo::Demo),
        real: impl FnOnce(&mut crate::company::engine::Engine, &mut dyn crate::company::host::Host) -> Result<(), String>,
    ) -> Option<Result<(), String>> {
        match &mut self.company.demo {
            Some(d) => {
                demo(d);
                Some(Ok(()))
            }
            None => self.co_engine(cx, real),
        }
    }

    fn co_result(&mut self, r: Option<Result<(), String>>, cx: &mut Context<Self>) -> bool {
        let ok = matches!(r, Some(Ok(())));
        self.company.failed_at = now_ms();
        self.company.failure = match r {
            Some(Err(why)) => Some(if why.is_empty() { SOMETHING_WRONG.into() } else { why }),
            _ => None,
        };
        cx.notify();
        ok
    }

    pub(super) fn co_approve(&mut self, job: &str, cx: &mut Context<Self>) {
        // Told the note is missing, the owner answers that first: write it, or close it.
        if self.company.short.contains(job) {
            return;
        }
        let r = self.co_act(cx, |d| d.approve(job), |e, host| e.approve(host, job));
        self.co_result(r, cx);
    }

    /// The first press opens the note box. An empty note never reaches the job.
    pub(super) fn co_send_back(&mut self, job: &str, window: &mut Window, cx: &mut Context<Self>) {
        let note = self.co_note_box(job, window, cx);
        let words = note.read(cx).value().to_string();
        if words.trim().is_empty() {
            if self.company.asking.contains(job) {
                self.company.short.insert(job.to_string());
            }
            self.company.asking.insert(job.to_string());
            note.update(cx, |n, cx| n.focus(window, cx));
            cx.notify();
            return;
        }
        self.company.short.remove(job);
        let r = self.co_act(cx, |d| d.send_back(job, &words), |e, host| e.send_back(host, job, &words));
        if self.co_result(r, cx) && note.read(cx).value() == words {
            self.company.notes.remove(job);
            self.company.asking.remove(job);
        }
    }

    fn co_note_box(&mut self, job: &str, window: &mut Window, cx: &mut Context<Self>) -> Entity<TextareaState> {
        if let Some(n) = self.company.notes.get(job) {
            return n.clone();
        }
        let note = cx.new(|cx| TextareaState::new(window, cx).placeholder(WHY_HINT).auto_grow(2, 6));
        let j = job.to_string();
        let sub = cx.subscribe_in(&note, window, move |this, _, event: &InputEvent, _, cx| {
            if let InputEvent::Change = event {
                this.company.short.remove(&j);
                cx.notify();
            }
        });
        self.company.subs.push(sub);
        self.company.notes.insert(job.to_string(), note.clone());
        note
    }

    pub(super) fn co_unask(&mut self, job: &str, cx: &mut Context<Self>) {
        self.company.asking.remove(job);
        self.company.short.remove(job);
        self.company.notes.remove(job);
        cx.notify();
    }

    pub(super) fn co_raise(&mut self, job: &str, cx: &mut Context<Self>) {
        let r = self.co_act(cx, |d| d.raise_cap(job, RAISE_BY), |e, host| e.raise_cap(host, job, RAISE_BY));
        self.co_result(r, cx);
    }

    /// Start the stopped people of a Job again, the way the engine wakes anyone: on
    /// their own worktree, a fresh one if theirs is gone. Their folders are made
    /// ready off the UI thread and they start one a tick, so no frame freezes.
    pub(super) fn co_resume(&mut self, job: &str, cx: &mut Context<Self>) {
        let Some(engine) = &self.company.engine else { return };
        let people = engine.view().jobs.into_iter().find(|j| j.id == job).map(|j| j.people).unwrap_or_default();
        for p in people {
            let r = self.co_engine(cx, |e, host| e.wake_person(host, &p));
            if let Some(Err(why)) = r {
                self.co_result(Some(Err(why)), cx);
                return;
            }
        }
        cx.notify();
    }

    pub(super) fn co_finish(&mut self, job: &str, cx: &mut Context<Self>) {
        let r = self.co_act(cx, |d| d.finish(job), |e, host| e.finish_job(host, job));
        if self.co_result(r, cx)
            && let Some(e) = &self.company.engine
        {
            let jobs = e.view().jobs;
            // Someone still on another Job keeps their folder and their seat.
            let busy: Vec<&String> = jobs.iter().filter(|j| j.id != job && going(&j.status).is_some()).flat_map(|j| &j.people).collect();
            let done: Vec<String> = jobs.iter().filter(|j| j.id == job).flat_map(|j| j.people.clone()).filter(|p| !busy.contains(&p)).collect();
            self.co_tidy(&done, cx);
        }
    }

    pub(super) fn co_stop(&mut self, job: &str, cx: &mut Context<Self>) {
        let r = self.co_act(cx, |d| d.stop(job), |e, host| e.stop_job(host, job));
        self.co_result(r, cx);
    }

    pub(super) fn co_dismiss(&mut self, key: &str, cx: &mut Context<Self>) {
        if let Some(d) = &mut self.company.demo {
            d.dismiss(key);
        } else if let Some(e) = &mut self.company.engine {
            e.dismiss(key);
        }
        cx.notify();
    }

    /// The seat a person sits at, by the person its spec names.
    pub(super) fn co_seat(&self, person: &str) -> Option<usize> {
        self.fleet.seats.iter().position(|s| s.spec.person.as_deref() == Some(person))
    }

    /// Where a card or a person takes you: their desk is their seat on the
    /// stage, a merge opens their Changes. Someone with no seat is woken first.
    pub(super) fn co_go(&mut self, to: Target, window: &mut Window, cx: &mut Context<Self>) {
        let (id, changes) = match to {
            Target::Company => return,
            Target::Desk(id) => (id, false),
            Target::Changes(id) => (id, true),
        };
        if self.co_seat(&id).is_none() {
            let r = self.co_engine(cx, |e, host| e.wake_person(host, &id));
            if !self.co_result(r, cx) {
                return;
            }
        }
        let Some(index) = self.co_seat(&id) else { return };
        // A seat just started is still getting its folder: no desk to show yet.
        if self.fleet.seats[index].preparing {
            let name = self.fleet.seats[index].spec.name.clone();
            self.company.failure = Some(desk_not_ready(&name));
            self.company.failed_at = now_ms();
            cx.notify();
            return;
        }
        self.company.open = false;
        self.company.desk = Some(id);
        self.home = false;
        self.fleet.focus(index, window, cx);
        if changes {
            self.open_changes(index, window, cx);
        }
        cx.notify();
    }
}
