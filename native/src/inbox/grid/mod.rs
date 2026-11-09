//! The Grid (Split in src/inbox.ts): the agent you work with as a big real
//! terminal, every other one in it as a card beside it (gridfit.rs,
//! gridcards.rs). Alt+S switches Focus and Grid, Alt+P adds the current agent
//! or takes it out; the agents you pinned come first, topped up from the list
//! to Settings > Inbox > Agents in the Grid. view.rs draws the terminal,
//! cards.rs the cards.

mod cards;
mod view;

use crate::gridcards::{Glances, Source, claude_file};
use crate::gridfit::{Area, rail_plan};
use crate::gridpins::{fill_pins, grid_order, toggle_pin};

use super::*;

/// How often a card reads what its agent wrote.
const RAIL_READ_MS: u64 = 2000;
/// The list's column, less the gutter the stage gets back when it is gone.
const LIST_W: f32 = QUEUE_W - 16.0;

#[derive(Default)]
pub(super) struct Grid {
    pub on: bool,
    /// The agents pinned to it, by id, in the order they came.
    pins: Vec<String>,
    /// The agent in the terminal; the others are cards beside it.
    big: Option<String>,
    /// Agents closed out with ✕; they stay out until the Grid is turned off.
    closed: HashSet<String>,
    /// Cards answered from a button: which question, so its buttons go.
    answered: HashMap<String, String>,
    /// The list stepped aside so the terminal has room to read.
    pub wide: bool,
    /// That was said once while the app runs.
    wide_noted: bool,
    /// The stage's size as it was last drawn.
    area: Option<Area>,
    /// What shows now; None draws the Focus stage.
    shown: Option<Shown>,
    /// The terminal whose font the Grid grew, and to what.
    grown: Option<(String, f32)>,
    glances: Glances,
    reader: Option<Task<()>>,
}

#[derive(Clone)]
struct Shown {
    big: usize,
    cards: Vec<usize>,
    cols: u32,
}

/// What a question is, so an answered one is told from the next.
fn ask_key(a: &supervise::ask::Ask) -> String {
    let labels: Vec<&str> = a.options.iter().map(|o| o.label.as_str()).collect();
    format!("{}|{}|{}", a.prompt, a.detail.as_deref().unwrap_or(""), labels.join("/"))
}

impl Inbox {
    /// Agents in the order the list shows them: project by project.
    fn list_order(&self) -> Vec<usize> {
        let mut projects: Vec<String> = Vec::new();
        for seat in &self.fleet.seats {
            let name = seat.project();
            if !projects.contains(&name) {
                projects.push(name);
            }
        }
        projects.iter().flat_map(|p| (0..self.fleet.seats.len()).filter(move |i| &self.fleet.seats[*i].project() == p)).collect()
    }

    fn seat_at(&self, id: &str) -> Option<usize> {
        self.fleet.seats.iter().position(|s| s.spec.id == id)
    }

    /// A dialog is up (Settings, New agent, the palette, a confirm): Alt keys wait.
    pub(super) fn dialog_up(&self) -> bool {
        self.sheet.is_some() || self.palette.is_some() || self.settings.is_some() || self.removing.is_some() || self.memory.is_some() || self.dialog_open()
    }

    fn focused_id(&self) -> Option<String> {
        self.fleet.focused_seat().map(|s| s.spec.id.clone())
    }

    /// setSplit: Grid on or off.
    pub(super) fn set_grid(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.grid.on = on;
        if on {
            let order = self.list_order();
            let alive = order.iter().filter(|i| !ended(self.fleet.seats[**i].state(cx))).count();
            if alive < 2 {
                let parked = self.fleet.seats.iter().any(|s| s.state(cx) == State::Stopped);
                self.toast(if parked {
                    "The Grid shows a running agent with the others beside it: resume a stopped one or start another with New agent"
                } else {
                    "The Grid shows an agent with the others beside it: start another with New agent"
                }, cx);
            }
        } else {
            self.grid.closed.clear();
            self.grid.answered.clear();
            self.grid_off(cx);
            if let Some(index) = self.fleet.focused {
                self.fleet.focus(index, window, cx);
            }
        }
        cx.notify();
    }

    /// Back to one agent on the stage: its font, the list, the cards' reader.
    fn grid_off(&mut self, cx: &mut Context<Self>) {
        self.grid.shown = None;
        self.grid.wide = false;
        self.grid.reader = None;
        self.grid.glances.keep(&HashSet::new());
        if let Some((id, _)) = self.grid.grown.take() {
            if let Some(t) = self.seat_at(&id).and_then(|i| self.fleet.seats[i].terminal.clone()) {
                t.update(cx, |t, cx| t.set_font(None, cx));
            }
        }
    }

    /// setCurrent: the agent the keyboard talks to; in the Grid, the one in
    /// the terminal.
    pub(super) fn set_current(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.grid.on {
            self.grid.big = self.fleet.seats.get(index).map(|s| s.spec.id.clone());
        }
        self.fleet.focus(index, window, cx);
        cx.notify();
    }

    /// openTask: show an agent. In the Grid it joins the others instead of
    /// replacing the whole view.
    pub(super) fn open_agent(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.fleet.seats.get(index).map(|s| s.spec.id.clone()) else { return };
        if self.grid.on && self.fleet.focused.is_some() {
            self.grid.closed.remove(&id);
            if !self.grid.pins.contains(&id) {
                let keep = self.focused_id();
                self.grid.pins = toggle_pin(&self.grid.pins, &id, keep.as_deref(), self.prefs.split_max);
            }
            self.set_current(index, window, cx);
            return;
        }
        self.fleet.focus(index, window, cx);
        cx.notify();
    }

    /// Alt+P: the current agent into the Grid, or out of it.
    pub(super) fn pin_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(cur) = self.focused_id() else { return };
        self.grid.pins = toggle_pin(&self.grid.pins, &cur, Some(&cur), self.prefs.split_max);
        let pinned = self.grid.pins.contains(&cur);
        if !self.grid.on {
            let name = self.fleet.focused_seat().map(|s| s.spec.name.clone()).unwrap_or_default();
            self.toast(if pinned { format!("{name} is in the Grid \u{00b7} Alt+S shows it") } else { format!("{name} is out of the Grid") }, cx);
        }
        if !pinned && self.grid.on {
            self.grid.closed.insert(cur);
            // Taking the agent in the terminal out hands it to another card.
            if let Some(next) = self.grid.pins.iter().find_map(|id| self.seat_at(id)) {
                self.set_current(next, window, cx);
            }
        }
        cx.notify();
    }

    /// The menu's Add to Grid / Take out of Grid.
    pub(super) fn toggle_in_grid(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(seat) = self.fleet.seats.get(index) else { return };
        let (id, name) = (seat.spec.id.clone(), seat.spec.name.clone());
        if self.grid.pins.contains(&id) {
            self.grid.pins.retain(|p| p != &id);
            if self.grid.on {
                self.grid.closed.insert(id);
            }
        } else {
            self.grid.closed.remove(&id);
            let keep = self.focused_id();
            self.grid.pins = toggle_pin(&self.grid.pins, &id, keep.as_deref(), self.prefs.split_max);
            if !self.grid.on {
                self.toast(format!("{name} is in the Grid \u{00b7} Alt+S shows it"), cx);
            }
        }
        cx.notify();
    }

    pub(super) fn in_grid(&self, index: usize) -> bool {
        self.fleet.seats.get(index).is_some_and(|s| self.grid.pins.contains(&s.spec.id))
    }

    /// ✕ on the Grid's terminal: take that agent out, hand the keyboard to another.
    fn close_from_grid(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.fleet.seats.get(index).map(|s| s.spec.id.clone()) else { return };
        self.grid.closed.insert(id.clone());
        self.grid.pins.retain(|p| p != &id);
        if self.fleet.focused == Some(index) {
            if let Some(next) = self.grid.pins.iter().find_map(|p| self.seat_at(p)) {
                self.set_current(next, window, cx);
            }
        }
        cx.notify();
    }

    /// ⤢: back to Focus with this agent full size.
    fn open_full(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.set_grid(false, window, cx);
        self.open_agent(index, window, cx);
    }

    /// layoutSplit, once per render: who is in it, who is in the terminal, how
    /// much room it has, and whether the list steps aside for it.
    pub(super) fn grid_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.grid.on || self.home {
            if self.grid.shown.is_some() || self.grid.grown.is_some() || self.grid.wide {
                self.grid_off(cx);
            }
            return;
        }
        let order = self.list_order();
        let alive: Vec<(&str, bool)> = order.iter().map(|i| (self.fleet.seats[*i].spec.id.as_str(), !ended(self.fleet.seats[*i].state(cx)))).collect();
        self.grid.pins = fill_pins(&self.grid.pins, &grid_order(alive), &self.grid.closed, self.prefs.split_max);
        let in_grid: Vec<usize> = self.grid.pins.iter().filter_map(|id| self.seat_at(id)).collect();
        if in_grid.len() < 2 {
            if self.grid.shown.is_some() || self.grid.grown.is_some() || self.grid.wide {
                self.grid_off(cx);
            }
            return;
        }
        let big = self.grid.big.as_deref().and_then(|id| self.seat_at(id)).filter(|i| in_grid.contains(i));
        let big = big.or(self.fleet.focused.filter(|i| in_grid.contains(i))).unwrap_or(in_grid[0]);
        self.grid.big = Some(self.fleet.seats[big].spec.id.clone());
        let cards: Vec<usize> = in_grid.iter().copied().filter(|i| *i != big).collect();
        let base = crate::terminal::font_size();
        let area = self.grid.area.unwrap_or_else(|| {
            let size = window.viewport_size();
            let list = if self.grid.wide { 16.0 } else { QUEUE_W };
            Area { width: f32::from(size.width) - list - 16.0, height: f32::from(size.height) - 68.0 - DOCK_H }
        });
        // Judged as if the list were there, so it can't flip back and forth once it has gone.
        let judged = if self.grid.wide { Area { width: area.width - LIST_W, ..area } } else { area };
        let wide = !rail_plan(cards.len(), judged, base).readable;
        if wide != self.grid.wide {
            self.grid.wide = wide;
            if wide && !self.grid.wide_noted {
                self.grid.wide_noted = true;
                self.toast("The list steps aside so the terminal has room to read \u{00b7} Alt+S brings it back", cx);
            }
        }
        let plan = rail_plan(cards.len(), area, base);
        // The keyboard follows the terminal, unless you are typing elsewhere.
        if self.fleet.focused != Some(big) {
            if self.busy_elsewhere(window, cx) {
                self.fleet.focused = Some(big);
            } else {
                self.fleet.focus(big, window, cx);
            }
        }
        // Everyone in the Grid keeps full history, as the one on the stage
        // does; focusing one trims the others', so this runs every time.
        for i in &in_grid {
            if let Some(t) = &self.fleet.seats[*i].terminal {
                t.read(cx).keep_lines(crate::terminal::SCROLLBACK);
            }
        }
        self.grow_font(big, if plan.font > base { Some(plan.font) } else { None }, cx);
        let ids: HashSet<String> = cards.iter().map(|i| self.fleet.seats[*i].spec.id.clone()).collect();
        self.grid.glances.keep(&ids);
        self.grid.shown = Some(Shown { big, cards, cols: plan.cols });
        self.start_reader(cx);
    }

    /// The terminal reads at its grown font; the one before goes back to the setting's.
    fn grow_font(&mut self, big: usize, font: Option<f32>, cx: &mut Context<Self>) {
        let id = self.fleet.seats[big].spec.id.clone();
        if let Some((was, _)) = self.grid.grown.as_ref().filter(|(was, _)| *was != id) {
            if let Some(t) = self.seat_at(was).and_then(|i| self.fleet.seats[i].terminal.clone()) {
                t.update(cx, |t, cx| t.set_font(None, cx));
            }
        }
        if let Some(t) = self.fleet.seats[big].terminal.clone() {
            t.update(cx, |t, cx| t.set_font(font, cx));
        }
        self.grid.grown = font.map(|f| (id, f));
    }

    /// What the cards read, and from where: Claude agents that run.
    fn card_sources(&self, cx: &App) -> Vec<Source> {
        let Some(shown) = &self.grid.shown else { return Vec::new() };
        shown
            .cards
            .iter()
            .filter_map(|i| self.fleet.seats.get(*i))
            .map(|s| {
                let running = s.terminal.as_ref().is_some_and(|t| t.read(cx).exit_code().is_none());
                let path = (running && s.spec.cli.eq_ignore_ascii_case("claude")).then(|| claude_file(s.spec.run_dir(), s.spec.session_id.as_deref())).flatten();
                Source { id: s.spec.id.clone(), run: s.spec.started_ms, path }
            })
            .collect()
    }

    /// Every couple of seconds while there are cards: what their agents wrote.
    fn start_reader(&mut self, cx: &mut Context<Self>) {
        if self.grid.reader.is_some() {
            return;
        }
        let glances = self.grid.glances.clone();
        self.grid.reader = Some(cx.spawn(async move |this, cx| {
            loop {
                let Ok(sources) = this.update(cx, |this, cx| this.card_sources(cx)) else { break };
                let g = glances.clone();
                let changed = cx.background_executor().spawn(async move { g.refresh(&sources) }).await;
                if changed && this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
                cx.background_executor().timer(Duration::from_millis(RAIL_READ_MS)).await;
            }
        }));
    }
}
