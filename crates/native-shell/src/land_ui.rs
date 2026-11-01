//! The branch actions under Review for an agent in its own worktree, as the
//! approved design has them: Merge into the project's branch (commits what is
//! left, refuses when the project folder has edits the merge would touch,
//! removes the worktree and branch after), Send back with a note, and
//! Discard branch (asks first). Picking the agent checks in memory whether
//! the merge would go cleanly (`agent_core::land`, a dry run).

use std::sync::mpsc::{channel, Receiver, Sender};

use agent_core::land::{discard, land, LandResult};
use eframe::egui::{self, RichText};

use crate::term_pane::{Pane, WorktreeInfo};
use crate::theme::{ADD, AMBER, DEL, INK3, TEXT};

/// What a branch action asks the app to do.
#[derive(Debug, PartialEq)]
pub enum LandOut {
    /// It merged: mark the pane merged and read its diff again.
    Merged(String),
    /// Type this into the agent.
    Tell(String, String),
    /// Its branch is gone: stop the agent.
    Discarded(String),
}

enum Msg {
    Check(String, LandResult),
    Land(String, LandResult),
    Discard(String, Result<(), String>),
}

pub struct LandBox {
    pane: Option<String>,
    check: Option<LandResult>,
    busy: bool,
    note: Option<(String, bool)>,
    asking_discard: bool,
    send_back: Option<String>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

impl Default for LandBox {
    fn default() -> Self {
        let (tx, rx) = channel();
        LandBox { pane: None, check: None, busy: false, note: None, asking_discard: false, send_back: None, tx, rx }
    }
}

/// A land result in words, and whether it is good news.
pub fn land_words(r: &LandResult, dry: bool) -> (String, bool) {
    match r {
        LandResult::Ok { base, branch, note, .. } if dry => (format!("{branch} merges cleanly into {base}."), note.is_none()),
        LandResult::Ok { base, branch, removed, note, .. } => {
            let tail = if *removed { " Its worktree and branch are gone.".to_string() } else { note.as_ref().map(|n| format!(" {n}")).unwrap_or_default() };
            (format!("Merged {branch} into {base}.{tail}"), true)
        }
        LandResult::Conflict { base, files, .. } => (format!("Would conflict with {base} in {}. Send it back to resolve.", files.join(", ")), false),
        LandResult::Error { message } => (message.clone(), false),
    }
}

/// What Send back types when the merge conflicts.
pub fn conflict_note(r: &LandResult) -> Option<String> {
    match r {
        LandResult::Conflict { base, files, .. } => Some(format!("Your branch conflicts with {base} in {}. Merge {base} into your branch, resolve the conflicts, run the tests, and commit.", files.join(", "))),
        _ => None,
    }
}

impl LandBox {
    fn start_check(&mut self, id: &str, w: &WorktreeInfo) {
        self.pane = Some(id.to_string());
        self.check = None;
        self.note = None;
        self.asking_discard = false;
        self.send_back = None;
        let (tx, id, path) = (self.tx.clone(), id.to_string(), w.path.clone());
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Check(id, land(&path, None, None, true, None)));
        });
    }

    pub fn show(&mut self, ui: &mut egui::Ui, p: &Pane) -> Option<LandOut> {
        let w = p.worktree.clone()?;
        if p.merged {
            ui.label(RichText::new(format!("Merged {} into {}.", w.branch, w.base)).size(12.0).color(INK3));
            return None;
        }
        if self.pane.as_deref() != Some(p.id.as_str()) {
            self.start_check(&p.id, &w);
        }
        let mut out = None;
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Check(id, r) if Some(&id) == self.pane.as_ref() => self.check = Some(r),
                Msg::Land(id, r) => {
                    self.busy = false;
                    self.note = Some(land_words(&r, false));
                    if matches!(r, LandResult::Ok { .. }) {
                        out = Some(LandOut::Merged(id));
                    } else {
                        self.check = Some(r);
                    }
                }
                Msg::Discard(id, r) => {
                    self.busy = false;
                    match r {
                        Ok(()) => out = Some(LandOut::Discarded(id)),
                        Err(e) => self.note = Some((e, false)),
                    }
                }
                _ => {}
            }
        }
        match &self.check {
            None => {
                ui.label(RichText::new(format!("Checking whether {} merges cleanly\u{2026}", w.branch)).size(12.0).color(INK3));
            }
            Some(r) => {
                let (t, good) = land_words(r, true);
                ui.label(RichText::new(t).size(12.0).color(if good { ADD } else { AMBER }));
            }
        }
        if let Some((t, good)) = &self.note {
            ui.label(RichText::new(t).size(12.0).color(if *good { ADD } else { DEL }));
        }
        ui.horizontal_wrapped(|ui| {
            ui.add_enabled_ui(!self.busy, |ui| {
                let merge = egui::Button::new(RichText::new(format!("Merge into {}", w.base)).strong().color(TEXT)).fill(ADD.gamma_multiply(0.35));
                if ui.add(merge).on_hover_text("Commits what is left, merges, then removes the worktree and branch").clicked() {
                    self.busy = true;
                    self.note = None;
                    let (tx, id, path, title) = (self.tx.clone(), p.id.clone(), w.path.clone(), p.task.clone());
                    std::thread::spawn(move || {
                        let _ = tx.send(Msg::Land(id, land(&path, None, title.as_deref(), false, None)));
                    });
                }
                if ui.button("Send back").clicked() {
                    self.send_back = Some(self.check.as_ref().and_then(conflict_note).unwrap_or_default());
                }
                if ui.add(egui::Button::new(RichText::new("Discard branch").color(DEL))).clicked() {
                    self.asking_discard = true;
                }
            });
        });
        let (mut send, mut cancel) = (false, false);
        if let Some(text) = self.send_back.as_mut() {
            ui.add(egui::TextEdit::multiline(text).hint_text("What should change?").desired_rows(2).desired_width(f32::INFINITY));
            let ready = !text.trim().is_empty();
            ui.horizontal(|ui| {
                send = ui.add_enabled(ready, egui::Button::new("Send")).clicked();
                cancel = ui.add(egui::Button::new(RichText::new("Cancel").color(INK3)).frame(false)).clicked();
            });
        }
        if send {
            out = self.send_back.take().map(|t| LandOut::Tell(p.id.clone(), t.trim().to_string()));
        } else if cancel {
            self.send_back = None;
        }
        if self.asking_discard {
            ui.label(RichText::new(format!("Throw {} away, uncommitted work and all? This can't be undone.", w.branch)).size(12.0).color(DEL));
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(RichText::new("Discard it").color(DEL))).clicked() {
                    self.asking_discard = false;
                    self.busy = true;
                    let (tx, id, path) = (self.tx.clone(), p.id.clone(), w.path.clone());
                    std::thread::spawn(move || {
                        let _ = tx.send(Msg::Discard(id, discard(&path)));
                    });
                } else if ui.add(egui::Button::new(RichText::new("Keep it").color(INK3)).frame(false)).clicked() {
                    self.asking_discard = false;
                }
            });
        }
        if self.busy || self.check.is_none() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn says_what_a_land_did() {
        let ok = LandResult::Ok { base: "main".into(), branch: "ana/fix".into(), sha: "abc".into(), removed: true, note: None };
        assert_eq!(land_words(&ok, true), ("ana/fix merges cleanly into main.".to_string(), true));
        assert_eq!(land_words(&ok, false).0, "Merged ana/fix into main. Its worktree and branch are gone.");
        let c = LandResult::Conflict { base: "main".into(), branch: "b".into(), files: vec!["a.rs".into(), "b.rs".into()] };
        assert!(!land_words(&c, true).1);
        assert!(conflict_note(&c).unwrap().contains("in a.rs, b.rs"));
        assert_eq!(conflict_note(&ok), None);
    }
}
