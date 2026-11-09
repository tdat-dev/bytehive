//! Scheduled agents (scheduler.ts, #schedModal in index.html): start a saved
//! preset (New agent → Save as preset) at a time of day, once or daily. The
//! engine is crate::schedule; this is the dialog, and the loop that checks
//! every 30 seconds while the app is open and starts what is due, quietly.

mod view;

use crate::schedule::{self, Repeat, Schedule};

use super::dropdown::Spot;
use super::*;

/// scheduler.ts: a 30s cadence is plenty for minute-resolution times.
const EVERY: Duration = Duration::from_secs(30);

/// Which of the dialog's drop-downs is open.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SchedPick {
    Preset,
    Repeat,
}

pub(super) struct SchedulesView {
    /// schedules.json as last read or written.
    list: Vec<Schedule>,
    /// The preset to add, by name; None picks the first.
    preset: Option<String>,
    time: Entity<InputState>,
    repeat: Repeat,
    /// The open drop-down and where its list stands.
    pick: Option<(SchedPick, Point<Pixels>)>,
    preset_spot: Spot,
    repeat_spot: Spot,
    /// Keys land here, never in a terminal behind.
    focus: FocusHandle,
}

impl Inbox {
    /// Start the loop once, from the first render (it needs the window to
    /// start agents). A schedule whose time passed while the app was closed
    /// fires on the first check after start, as long as it is still today.
    pub(super) fn start_scheduler(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.scheduler.is_some() {
            return;
        }
        self.scheduler = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(EVERY).await;
                if this.update_in(cx, |this, window, cx| this.fire_due(window, cx)).is_err() {
                    break;
                }
            }
        }));
    }

    /// Fire any due schedule, then stamp it (a once schedule turns itself off).
    fn fire_due(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = chrono::Local::now();
        let list = schedule::load();
        let due = schedule::due_schedules(&list, now);
        if due.is_empty() {
            return;
        }
        for s in &due {
            if let Some(t) = self.prefs.templates.iter().find(|t| t.name == s.template_id).cloned() {
                self.launch_template(&t, window, cx);
            }
        }
        let list: Vec<Schedule> = list.iter().map(|s| if due.iter().any(|d| d.id == s.id) { schedule::after_fire(s, now) } else { s.clone() }).collect();
        self.sched_keep(list, cx);
    }

    /// Start a saved preset now, quietly: the stage and the person's choices stay.
    fn launch_template(&mut self, t: &Template, window: &mut Window, cx: &mut Context<Self>) {
        if t.count == 0 {
            return;
        }
        let command = match t.cli.strip_prefix("custom:") {
            Some(typed) => Self::custom_command(typed),
            None => Self::preset_command(&t.cli, t.skip_perms),
        };
        let Some((program, args)) = command else { return };
        let launch = Launch { program, args, dir: t.dir.clone(), count: t.count, job: String::new(), remembered: t.cli.clone(), quiet: true };
        if let Err(e) = self.launch(launch, window, cx) {
            self.toast(e, cx);
        }
    }

    pub(super) fn open_schedules(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette = None;
        self.menu = None;
        self.popup = None;
        crate::keys::set_active(false);
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let time = cx.new(|cx| InputState::new(window, cx).default_value("09:00"));
        self.schedules = Some(SchedulesView { list: schedule::load(), preset: None, time, repeat: Repeat::Daily, pick: None, preset_spot: Default::default(), repeat_spot: Default::default(), focus });
        cx.notify();
    }

    pub(super) fn close_schedules(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.schedules = None;
        if !self.home {
            if let Some(index) = self.fleet.focused {
                self.fleet.focus(index, window, cx);
            }
        }
        cx.notify();
    }

    /// Esc closes an open drop-down first, then the dialog.
    pub(super) fn schedules_escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match &mut self.schedules {
            Some(s) if s.pick.is_some() => {
                s.pick = None;
                cx.notify();
            }
            _ => self.close_schedules(window, cx),
        }
    }

    /// The preset Add would schedule: the picked one while it still exists, else the first.
    fn sched_preset(&self) -> Option<String> {
        let s = self.schedules.as_ref()?;
        let names = || self.prefs.templates.iter().map(|t| &t.name);
        s.preset.clone().filter(|p| names().any(|n| n == p)).or_else(|| names().next().cloned())
    }

    fn sched_add(&mut self, cx: &mut Context<Self>) {
        let Some(template_id) = self.sched_preset() else { return }; // no presets saved
        let Some(s) = &self.schedules else { return };
        let time = s.time.read(cx).value().trim().to_string();
        if schedule::parse_time(&time).is_none() {
            return; // guard malformed time
        }
        let mut list = schedule::load();
        list.push(Schedule { id: schedule::new_id(), template_id, time, repeat: s.repeat, enabled: true, last_fired: None });
        self.sched_keep(list, cx);
    }

    /// Save the list and show it.
    fn sched_keep(&mut self, list: Vec<Schedule>, cx: &mut Context<Self>) {
        schedule::save(&list);
        if let Some(v) = &mut self.schedules {
            v.list = list;
        }
        cx.notify();
    }

    fn sched_toggle(&mut self, id: &str, cx: &mut Context<Self>) {
        let list: Vec<Schedule> = schedule::load().into_iter().map(|s| if s.id == id { Schedule { enabled: !s.enabled, ..s } } else { s }).collect();
        self.sched_keep(list, cx);
    }

    fn sched_delete(&mut self, id: &str, cx: &mut Context<Self>) {
        let list: Vec<Schedule> = schedule::load().into_iter().filter(|s| s.id != id).collect();
        self.sched_keep(list, cx);
    }

    /// selectmenu.ts: a list opens 6px under its button; the same button again closes it.
    fn sched_open_pick(&mut self, pick: SchedPick, cx: &mut Context<Self>) {
        let Some(s) = &mut self.schedules else { return };
        let under = if pick == SchedPick::Preset { s.preset_spot.get() } else { s.repeat_spot.get() };
        s.pick = if s.pick.is_some_and(|(p, _)| p == pick) { None } else { Some((pick, point(under.left(), under.bottom() + px(6.0)))) };
        cx.notify();
    }
}
