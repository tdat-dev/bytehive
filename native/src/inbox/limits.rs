//! The Limit Guard (limitguard.ts): no agent left sitting at its plan's
//! usage limit. When one runs out, another CLI whose plan has room takes its
//! task over at once: an agent already open in the same folder first, else a
//! new one briefed from its conversation (Settings: Hand over when an agent
//! runs out of usage). When the limit lifts, it is told to carry on (Settings:
//! Carry on when a usage limit resets). Windows notifications are not ported
//! yet: what happens shows as a toast.

use supervise::screens::LimitHit;

use crate::limits::{CARRY_ON, GRACE_MS, LIMIT_WHY, RESUME_GAP_MS, cli_label, handed_title, handoff_brief, plan_of, plan_reset_for, take_over_line};
use crate::transcript::Item;

use super::*;

/// One run-out: the notice it came with and what was done about it.
struct Episode {
    what: String,
    reset: Option<u64>,
    notified: bool,
    /// Its task went to another agent: no carry-on.
    handed: bool,
    /// A handover was tried: never twice in one episode.
    tried: bool,
}

#[derive(Default)]
pub(super) struct LimitGuard {
    episodes: HashMap<String, Episode>,
    resumed_at: HashMap<String, u64>,
    /// Who has an agent's task, for the run it was handed over in.
    handed_to: HashMap<String, (String, u64)>,
}

fn cli_of(seat: &Seat) -> String {
    let program = seat.spec.cli.rsplit(['\\', '/']).next().unwrap_or(&seat.spec.cli).to_ascii_lowercase();
    program.trim_end_matches(".exe").trim_end_matches(".cmd").to_string()
}

fn is_shell(cli: &str) -> bool {
    matches!(cli, "powershell" | "pwsh" | "cmd")
}

impl Inbox {
    /// When a CLI's plan is out until, by the plan meter.
    fn plan_out_until(&self, cli: &str, now: u64) -> Option<u64> {
        let usage = match plan_of(cli) {
            "Claude" => self.plans.0.usage.as_ref(),
            "Codex" => self.plans.1.usage.as_ref(),
            _ => None,
        };
        plan_reset_for(usage, now as i64).map(|t| t as u64)
    }

    /// Look at every agent that hit its limit (called from the tick).
    pub(super) fn check_limits(&mut self, hits: Vec<(usize, LimitHit)>, cx: &mut Context<Self>) {
        let now = now_ms();
        let ids: HashSet<String> = hits.iter().map(|(i, _)| self.fleet.seats[*i].spec.id.clone()).collect();
        self.limits.episodes.retain(|id, _| ids.contains(id));
        for (index, hit) in hits {
            let seat = &self.fleet.seats[index];
            let (id, name, cli, started) = (seat.spec.id.clone(), seat.spec.name.clone(), cli_of(seat), seat.spec.started_ms);
            let reset_of = |this: &Self| hit.resets_at.or_else(|| this.plan_out_until(&cli, now));
            let fresh = self.limits.episodes.get(&id).is_none_or(|e| e.what != hit.what);
            if fresh {
                let reset = reset_of(self);
                self.limits.episodes.insert(id.clone(), Episode { what: hit.what.clone(), reset, notified: false, handed: false, tried: false });
            } else if self.limits.episodes[&id].reset.is_none() {
                let reset = reset_of(self);
                self.limits.episodes.get_mut(&id).expect("episode").reset = reset;
            }
            let handed_now = self.limits.handed_to.get(&id).is_some_and(|(_, run)| *run == started);
            let e = self.limits.episodes.get_mut(&id).expect("episode");
            if !e.tried && !handed_now && self.prefs.auto_handover {
                e.tried = true;
                if let Some(label) = self.take_over(index, now, cx) {
                    let e = self.limits.episodes.get_mut(&id).expect("episode");
                    e.handed = true;
                    e.notified = true;
                    self.toast_for(format!("{name} ran out of usage. {label} takes its task over."), 8000, cx);
                }
            }
            let e = self.limits.episodes.get_mut(&id).expect("episode");
            if e.handed {
                continue;
            }
            let reset = e.reset;
            if !e.notified {
                e.notified = true;
                let words = match reset {
                    Some(at) => format!("{name} is out of usage. Its limit resets at {}.{}", supervise::task_state::clock_of(at, now), if self.prefs.auto_resume { " ByteHive will tell it to carry on then." } else { "" }),
                    None => format!("{name} is out of usage. Its plan's usage limit ran out."),
                };
                self.toast_for(words, 8000, cx);
            }
            if handed_now || !self.prefs.auto_resume {
                continue;
            }
            let Some(reset) = reset.filter(|r| now >= r + GRACE_MS) else { continue };
            let _ = reset;
            let seat = &self.fleet.seats[index];
            let running = seat.terminal.as_ref().is_some_and(|t| t.read(cx).exit_code().is_none());
            if !running || seat.state(cx) == State::Working || now - self.limits.resumed_at.get(&id).copied().unwrap_or(0) < RESUME_GAP_MS {
                continue;
            }
            self.limits.resumed_at.insert(id.clone(), now);
            self.queue_message(id, CARRY_ON.to_string(), cx);
            self.toast(format!("{name} is back at work: its usage limit reset, so ByteHive told it to carry on."), cx);
        }
    }

    /// Who takes the task: an open agent in its folder whose plan has room,
    /// else a new agent of an installed CLI whose plan has room. Its words.
    fn take_over(&mut self, index: usize, now: u64, cx: &mut Context<Self>) -> Option<String> {
        let seat = &self.fleet.seats[index];
        let (from_id, from_name, from_cli) = (seat.spec.id.clone(), seat.spec.name.clone(), cli_of(seat));
        let (dir, project, title, started) = (seat.spec.run_dir().to_string(), seat.spec.cwd.clone(), seat.spec.title.clone(), seat.spec.started_ms);
        let full = |this: &Self, cli: &str| this.plan_out_until(cli, now).is_some();
        // Open agents first, the ones whose plan has room.
        let open = self.fleet.seats.iter().find(|s| {
            let cli = cli_of(s);
            s.spec.id != from_id
                && cli != from_cli
                && !is_shell(&cli)
                && presets::same_folder(s.spec.run_dir(), &dir)
                && s.terminal.as_ref().is_some_and(|t| t.read(cx).exit_code().is_none())
                && matches!(s.state(cx), State::Idle | State::Review)
                && !full(self, &cli)
        });
        let (to_name, label) = if let Some(to) = open {
            let (to_id, to_name) = (to.spec.id.clone(), to.spec.name.clone());
            let label = format!("{to_name} ({})", cli_label(&cli_of(to)));
            self.queue_message(to_id, take_over_line(&from_name, &cli_label(&from_cli), LIMIT_WHY), cx);
            (to_name, label)
        } else {
            let preset = PRESETS.iter().find(|p| p.id != from_cli && presets::installed(p.program) && !full(self, p.id))?;
            let (asks, last) = conversation_of(&self.fleet.seats[index]);
            let brief = handoff_brief(&from_name, &cli_label(&from_cli), LIMIT_WHY, title.as_deref(), &asks, &last);
            let args = if self.prefs.skip_perms { preset.skip_args.iter().map(|a| a.to_string()).collect() } else { Vec::new() };
            let carries = format!("Carries on {from_name}'s task{}", title.as_deref().map(|t| format!(": {t}")).unwrap_or_default());
            let at = self.fleet.add(preset.program, args, &project, Some(carries), None, cx).ok()?;
            let new = &self.fleet.seats[at];
            let (new_id, new_name) = (new.spec.id.clone(), new.spec.name.clone());
            self.queue_job(new_id, brief, cx);
            (new_name, format!("A new {} agent", preset.label))
        };
        let seat = &mut self.fleet.seats[index];
        seat.spec.title = Some(handed_title(&to_name, seat.spec.title.as_deref()));
        self.limits.handed_to.insert(from_id, (to_name, started));
        self.fleet.save();
        Some(label)
    }
}

/// Its first asks and last words, from a Claude agent's conversation.
fn conversation_of(seat: &Seat) -> (Vec<String>, String) {
    let Some(id) = seat.spec.session_id.as_deref().filter(|_| seat.spec.cli.eq_ignore_ascii_case("claude")) else { return (Vec::new(), String::new()) };
    let Ok(read) = crate::transcript::read_session(seat.spec.run_dir(), id, Some(4 * 1024 * 1024)) else { return (Vec::new(), String::new()) };
    let asks = read.chat.items.iter().filter_map(|i| if let Item::User { text, .. } = i { Some(text.clone()) } else { None }).collect();
    let last = read.chat.items.iter().rev().find_map(|i| if let Item::Text { text, .. } = i { Some(text.clone()) } else { None }).unwrap_or_default();
    (asks, last)
}
