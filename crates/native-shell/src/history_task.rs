//! One task's History, as the approved design has it: what it cost, how long
//! it ran and how many commands, its recordings to replay, running the task
//! again on a schedule, and a timeline of what happened (you said, it ran,
//! it said, it stopped). The pure part builds the timeline; `cards` draws
//! the three cards and says what was picked.

use eframe::egui::{self, Color32, RichText};
use supervise::types::ChatItem;

use crate::theme::{ADD, DEL, INK3, TEXT};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tone {
    You,
    Say,
    Code,
    Err,
    Dim,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ev {
    pub at: u64,
    pub who: String,
    pub text: String,
    pub tone: Tone,
}

fn short(s: &str, n: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= n { s.to_string() } else { format!("{}\u{2026}", s.chars().take(n).collect::<String>()) }
}

/// What happened, oldest first.
pub fn timeline(items: &[ChatItem], name: &str) -> Vec<Ev> {
    items
        .iter()
        .filter_map(|it| {
            let (who, text, tone) = match it {
                ChatItem::User(u) => ("You".to_string(), short(&u.text, 400), Tone::You),
                ChatItem::Text(t) => (name.to_string(), short(&t.text, 400), Tone::Say),
                ChatItem::Step(s) if s.error => ("Stopped".to_string(), format!("{} {}", s.verb, s.target), Tone::Err),
                ChatItem::Step(s) => (s.verb.clone(), s.target.clone(), Tone::Code),
                ChatItem::Note(n) => (String::new(), short(&n.text, 200), Tone::Dim),
            };
            (!text.trim().is_empty()).then(|| Ev { at: it.at(), who, text, tone })
        })
        .collect()
}

/// Commands it ran.
pub fn commands(items: &[ChatItem]) -> usize {
    items.iter().filter(|i| matches!(i, ChatItem::Step(s) if s.verb == "Ran")).count()
}

/// The task as you first gave it: the first thing you said.
pub fn first_prompt(items: &[ChatItem]) -> Option<String> {
    items.iter().find_map(|i| match i {
        ChatItem::User(u) if !u.text.trim().is_empty() => Some(u.text.trim().to_string()),
        _ => None,
    })
}

/// "now", "12m ago", or the time of day.
pub fn when(at: u64, now: u64) -> String {
    if at == 0 {
        return String::new();
    }
    let mins = now.saturating_sub(at) / 60_000;
    if mins < 1 {
        "now".into()
    } else if mins < 60 {
        format!("{mins}m ago")
    } else {
        chrono::DateTime::from_timestamp_millis(at as i64).map(|t| t.with_timezone(&chrono::Local).format("%H:%M").to_string()).unwrap_or_default()
    }
}

/// What a History card asked for.
#[derive(Clone, Debug, PartialEq)]
pub enum HistAction {
    Replay(String),
    Record(String),
    /// Run this prompt again every day at this time.
    Repeat { pane_id: String, prompt: String, time: String },
}

pub struct CardFacts<'a> {
    pub pane_id: &'a str,
    pub usd: Option<f64>,
    pub tokens: Option<u64>,
    pub minutes: u64,
    pub commands: usize,
    pub recordings: usize,
    pub recording: bool,
    pub prompt: Option<String>,
    /// The time it already repeats at, when it does.
    pub repeats: Option<String>,
}

fn card(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new().fill(Color32::from_white_alpha(8)).corner_radius(12.0).inner_margin(egui::Margin::symmetric(12, 10)).show(ui, |ui| {
        ui.set_width(ui.available_width().min(200.0));
        ui.vertical(add);
    });
}

/// The three cards: cost, session, run again.
pub fn cards(ui: &mut egui::Ui, f: &CardFacts) -> Option<HistAction> {
    let mut out = None;
    ui.horizontal_wrapped(|ui| {
        card(ui, |ui| {
            ui.label(RichText::new("Cost so far").size(12.0).color(INK3));
            let v = f.usd.map(supervise::pricing::money).unwrap_or_else(|| "Not known yet".into());
            ui.label(RichText::new(v).size(18.0).color(TEXT));
            if let Some(t) = f.tokens {
                ui.label(RichText::new(format!("{}k tokens", t / 1000)).size(12.0).color(INK3));
            }
        });
        card(ui, |ui| {
            ui.label(RichText::new("Session").size(12.0).color(INK3));
            ui.label(RichText::new(format!("{} min \u{00B7} {} commands", f.minutes.max(1), f.commands)).size(15.0).color(TEXT));
            if f.recordings > 0 && ui.button(RichText::new("Replay session").size(12.5)).clicked() {
                out = Some(HistAction::Replay(f.pane_id.to_string()));
            }
            let rec = if f.recording { "Stop recording" } else if f.recordings > 0 { "Record again" } else { "Record it" };
            if ui.add(egui::Button::new(RichText::new(rec).size(12.0).color(if f.recording { DEL } else { INK3 })).frame(false)).clicked() {
                out = Some(HistAction::Record(f.pane_id.to_string()));
            }
        });
        card(ui, |ui| {
            ui.label(RichText::new("Run this task again").size(12.0).color(INK3));
            let Some(prompt) = f.prompt.clone() else {
                ui.label(RichText::new("Once it has a task.").size(12.5).color(INK3));
                return;
            };
            let shown = f.repeats.as_ref().map_or("Don't repeat".to_string(), |t| format!("Every day at {t}"));
            egui::ComboBox::from_id_salt(("hist-repeat", f.pane_id)).width(170.0).selected_text(RichText::new(shown).size(12.5)).show_ui(ui, |ui| {
                for t in ["09:00", "13:00", "18:00"] {
                    let label = if t == "09:00" { "Every morning at 09:00".to_string() } else { format!("Every day at {t}") };
                    if ui.selectable_label(f.repeats.as_deref() == Some(t), label).clicked() {
                        out = Some(HistAction::Repeat { pane_id: f.pane_id.to_string(), prompt: prompt.clone(), time: t.into() });
                    }
                }
            });
            if let Some(t) = &f.repeats {
                ui.label(RichText::new(format!("In Settings > Agents from {t}")).size(11.5).color(ADD));
            }
        });
    });
    out
}

/// The timeline rows, newest last.
pub fn timeline_ui(ui: &mut egui::Ui, evs: &[Ev], now: u64) {
    for e in evs.iter().rev().take(300).collect::<Vec<_>>().into_iter().rev() {
        ui.separator();
        ui.horizontal_top(|ui| {
            ui.add_sized([52.0, 16.0], egui::Label::new(RichText::new(when(e.at, now)).monospace().size(11.0).color(INK3)));
            ui.vertical(|ui| {
                let (color, mono) = match e.tone {
                    Tone::You | Tone::Say => (TEXT, false),
                    Tone::Code => (INK3, true),
                    Tone::Err => (DEL, true),
                    Tone::Dim => (INK3, false),
                };
                if !e.who.is_empty() {
                    ui.label(RichText::new(&e.who).size(11.5).color(INK3));
                }
                let t = RichText::new(&e.text).size(12.5).color(color);
                ui.label(if mono { t.monospace() } else { t });
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use supervise::types::{NoteItem, StepItem, TextItem, UserItem};

    #[test]
    fn builds_the_timeline_and_counts() {
        let step = |verb: &str, target: &str, error: bool| ChatItem::Step(StepItem { id: "s".into(), tool: "Bash".into(), verb: verb.into(), target: target.into(), full: None, code: true, output: None, error, done: true, diff: None, added: None, removed: None, todos: None, images: None, questions: None, at: 3 });
        let items = vec![
            ChatItem::User(UserItem { id: "u".into(), text: "Fix the login form".into(), images: 0, pics: None, at: 1 }),
            step("Ran", "npm test", false),
            step("Edited", "login.ts", false),
            step("Ran", "npm test", true),
            ChatItem::Text(TextItem { id: "t".into(), text: "Done.".into(), at: 5 }),
            ChatItem::Note(NoteItem { id: "n".into(), text: " ".into(), at: 6 }),
        ];
        let t = timeline(&items, "Ana");
        assert_eq!(t.iter().map(|e| e.who.as_str()).collect::<Vec<_>>(), ["You", "Ran", "Edited", "Stopped", "Ana"]);
        assert_eq!(t[3].tone, Tone::Err);
        assert_eq!(commands(&items), 2);
        assert_eq!(first_prompt(&items).as_deref(), Some("Fix the login form"));
        assert_eq!(when(0, 5), "");
        assert_eq!(when(1_000, 31_000), "now");
        assert_eq!(when(1_000, 1_000 + 12 * 60_000), "12m ago");
    }

    #[test]
    fn run_again_schedules_the_task_once() {
        let mut app = crate::app::ShellApp::new().unwrap();
        app.prefs_path = None;
        app.prefs.schedules.clear();
        app.panes[0].cli = Some(crate::sessions_ui::Cli::Codex);
        let id = app.panes[0].id.clone();
        let repeat = |t: &str| HistAction::Repeat { pane_id: id.clone(), prompt: "Run the tests".into(), time: t.into() };
        app.history_action(repeat("09:00"));
        app.history_action(repeat("18:00"));
        assert_eq!(app.prefs.schedules.len(), 1, "the same task repeats once");
        let s = &app.prefs.schedules[0];
        assert_eq!((s.time.as_str(), s.daily, s.task.cli, s.task.prompt.as_str()), ("18:00", true, crate::sessions_ui::Cli::Codex, "Run the tests"));
    }
}

impl crate::app::ShellApp {
    /// Do what a History card asked for.
    pub(crate) fn history_action(&mut self, a: HistAction) {
        let home = |app: &Self, id: &str| {
            let p = app.panes.iter().find(|p| p.id == id)?;
            let h = crate::projects::pane_home(p);
            Some((app.projects.owner(&h).map(str::to_string).unwrap_or(h), p.name.clone(), p.cli))
        };
        match a {
            HistAction::Replay(id) => {
                let Some((project, name, _)) = home(self, &id) else { return };
                let safe = crate::replay::safe_name(&name);
                let newest = crate::replay::list(&project).into_iter().find(|r| r.agent == safe).map(|r| r.path);
                self.extras.replay.open_on(&project, newest);
            }
            HistAction::Record(id) => {
                self.stage = Some(id);
                self.toggle_record();
            }
            HistAction::Repeat { pane_id, prompt, time } => {
                let Some((project, name, Some(cli))) = home(self, &pane_id) else { return };
                let mut task = crate::newtask::NewTask::new(project);
                task.prompt = prompt;
                task.cli = cli;
                task.worktree = self.panes.iter().any(|p| p.id == pane_id && p.worktree.is_some());
                task.perm = self.prefs.perm;
                let s = &mut self.prefs.schedules;
                match s.iter_mut().find(|s| s.daily && s.task.prompt == task.prompt) {
                    Some(old) => old.time = time.clone(),
                    None => s.push(crate::schedule::Schedule { id: format!("h{}", crate::app::now_ms()), task, time: time.clone(), daily: true, enabled: true, last_fired: 0 }),
                }
                self.save_prefs();
                self.notice = Some(format!("{name}'s task runs again every day at {time}."));
            }
        }
    }
}
