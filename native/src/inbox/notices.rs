//! Windows notifications from the Inbox (pane.ts setAttention, finishwatch.ts):
//! "Ana wants to run npm test" when an agent starts waiting on you while
//! ByteHive is in the background, and "Ana is done" when a run ends, unless
//! you are looking right at that agent. A click brings ByteHive forward on
//! that agent. Settings > Inbox turns each one off.

use crate::finishnote::{Burst, FinishSnap, FinishWatch, finish_note};
use crate::notify;

use super::*;

#[derive(Default)]
pub(super) struct Notices {
    finishes: FinishWatch,
    burst: Burst,
    /// Agents waiting on you now: told (or not) once, when they started.
    waiting: HashSet<String>,
    /// A clicked notification's agent, shown on the next render.
    open: Option<String>,
}

/// What a waiting agent wants, in a notification's title (taskLine in pane.ts).
fn wants(name: &str, status: &TaskStatus) -> String {
    match &status.ask {
        Some(a) => match (a.kind, &a.detail) {
            (AskKind::Run, Some(d)) => format!("{name} wants to run {d}"),
            (AskKind::Run, None) => format!("{name} wants to run a command"),
            (AskKind::Edit, Some(d)) => format!("{name} wants to edit {d}"),
            (AskKind::Edit, None) => format!("{name} wants to edit a file"),
            _ => format!("{name} asks: {}", a.prompt),
        },
        None => match &status.blocked {
            Some(b) => format!("{name}: {}", b.what),
            None => format!("{name} needs you"),
        },
    }
}

impl Inbox {
    /// Each tick, with every agent's status in seat order.
    pub(super) fn notices_tick(&mut self, statuses: &[TaskStatus], now: u64, cx: &mut Context<Self>) {
        if let Some(id) = notify::take_clicked() {
            self.notices.open = Some(id);
            crate::single::raise(std::process::id());
            cx.notify();
        }
        let front = notify::in_front();
        let mut snaps = Vec::with_capacity(statuses.len());
        let mut waiting = HashSet::new();
        for (seat, status) in self.fleet.seats.iter().zip(statuses) {
            let id = &seat.spec.id;
            if waits(status.state) {
                waiting.insert(id.clone());
                if !self.notices.waiting.contains(id) && self.prefs.notify_needs && !front {
                    notify::show(wants(&seat.spec.name, status), seat.project(), id.clone());
                }
            }
            let exit_code = seat.terminal.as_ref().and_then(|t| t.read(cx).exit_code());
            snaps.push(FinishSnap { id: id.clone(), name: seat.spec.name.clone(), project: seat.project(), state: status.state, limited: status.limit.is_some(), files: seat.changes.count, exit_code });
        }
        self.notices.waiting = waiting;

        let ended = self.notices.finishes.step(&snaps, now);
        if !self.prefs.notify_done {
            self.notices.burst.clear();
            return;
        }
        let on_screen = if self.home { None } else { self.fleet.focused_seat().map(|s| s.spec.id.clone()) };
        let worth: Vec<_> = ended.into_iter().filter(|f| !(front && on_screen.as_ref() == Some(&f.id))).collect();
        self.notices.burst.add(worth, now);
        if let Some(n) = self.notices.burst.take(now).and_then(|due| finish_note(&due)) {
            notify::show(n.title, n.body, n.open);
        }
    }

    /// From render (it has the window): show the agent a notification named.
    pub(super) fn open_noticed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.notices.open.take() else { return };
        let Some(index) = self.fleet.seats.iter().position(|s| s.spec.id == id) else { return };
        self.home = false;
        self.fleet.focus(index, window, cx);
    }
}
