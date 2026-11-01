//! Windows notifications: an agent that needs you, a run that finished
//! (done, ready to review, or crashed; bursts go out as one), an agent out of
//! usage or back at work. Sent while ByteHive is in the background; clicking
//! one brings ByteHive forward with that agent on stage. Settings > System
//! turns each kind off. Port of the web app's finishwatch.ts / setAttention
//! and src-tauri's toast.rs (WinRT straight, so a click is heard).

use std::collections::HashSet;
use std::sync::mpsc::{channel, Receiver, Sender};

use eframe::egui;
use supervise::finish::{finish_note, Burst, FinishSnap, FinishWatch};

use crate::app::{now_ms, ShellApp};
use crate::projects::{pane_home, project_name};
use crate::states::{pane_cwd, pane_status};

/// The Start-menu identity toasts are filed under: the installed app's, or
/// PowerShell's for a build run from target\ (it has no shortcut of its own).
fn app_id() -> String {
    let dev = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.display().to_string())).is_some_and(|d| d.ends_with("target\\debug") || d.ends_with("target\\release"));
    if dev { tauri_winrt_notification::Toast::POWERSHELL_APP_ID.to_string() } else { crate::APP_ID.to_string() }
}

pub struct Notifier {
    /// Agents whose notification was clicked.
    open_tx: Sender<String>,
    open_rx: Receiver<String>,
    finish: FinishWatch,
    burst: Burst,
    /// Agents at their usage limit at the last look.
    limited: HashSet<String>,
    /// Browser asks already told about.
    pub asks_told: HashSet<u64>,
}

impl Default for Notifier {
    fn default() -> Self {
        let (open_tx, open_rx) = channel();
        Notifier { open_tx, open_rx, finish: FinishWatch::default(), burst: Burst::default(), limited: HashSet::new(), asks_told: HashSet::new() }
    }
}

impl ShellApp {
    /// Show a notification; a click opens `target` (a pane id) when given.
    pub(crate) fn toast(&self, ctx: &egui::Context, title: String, body: String, target: Option<String>) {
        if cfg!(test) {
            return;
        }
        let (tx, hwnd, repaint) = (self.extras.notify.open_tx.clone(), self.extras.tray.hwnd(), ctx.clone());
        // WinRT calls block for a moment; keep them off the UI thread.
        std::thread::spawn(move || {
            let toast = tauri_winrt_notification::Toast::new(&app_id()).title(&title).text1(&body).sound(Some(tauri_winrt_notification::Sound::Default)).on_activated(move |_| {
                crate::tray::show_window(hwnd);
                if let Some(t) = target.clone() {
                    let _ = tx.send(t);
                }
                repaint.request_repaint();
                Ok(())
            });
            let _ = toast.show();
        });
    }

    /// ByteHive is in the background: minimized, hidden, or another app in front.
    pub(crate) fn away(ctx: &egui::Context) -> bool {
        ctx.input(|i| !i.viewport().focused.unwrap_or(true) || i.viewport().minimized.unwrap_or(false))
    }

    /// Finishes, limits, and clicked notifications (logic pass).
    pub(crate) fn poll_notify(&mut self, ctx: &egui::Context) {
        while let Ok(id) = self.extras.notify.open_rx.try_recv() {
            if self.panes.iter().any(|p| p.id == id) {
                self.stage = Some(id.clone());
                self.focus_pane = Some(id);
            }
        }
        let now = now_ms();
        let snaps: Vec<FinishSnap> = self
            .panes
            .iter()
            .filter(|p| p.cli.is_some())
            .map(|p| {
                let changed = self.git_changed.get(&pane_cwd(p)).map(|f| f.len() as u32);
                let state = pane_status(p, now, changed).state;
                FinishSnap { pane_id: p.id.clone(), name: p.name.clone(), project: project_name(&pane_home(p)), state, limited: p.limit.is_some(), files: changed, exit_code: p.exit_code, stop_asked: false }
            })
            .collect();
        let ended = self.extras.notify.finish.step(&snaps, now);
        let away = Self::away(ctx);
        if self.prefs.notify_done {
            let on = self.stage.clone();
            self.extras.notify.burst.add(ended.into_iter().filter(|f| away || on.as_deref() != Some(f.pane_id.as_str())).collect(), now);
            if let Some(list) = self.extras.notify.burst.take(now) {
                let (title, body, open) = finish_note(&list);
                if away {
                    self.toast(ctx, title, body, Some(open));
                }
            }
        }
        let limited: HashSet<String> = self.panes.iter().filter(|p| p.limit.is_some()).map(|p| p.id.clone()).collect();
        if self.prefs.limit_notify && self.prefs.handover_to.is_none() {
            for p in self.panes.iter().filter(|p| limited.contains(&p.id) && !self.extras.notify.limited.contains(&p.id)) {
                let back = p.limit.as_ref().and_then(|l| l.resets_at).map(|at| format!("Back at {}.", at.format("%H:%M"))).unwrap_or_default();
                let carry = if self.prefs.auto_resume { " ByteHive tells it to carry on then." } else { "" };
                self.toast(ctx, format!("{} is out of usage", p.name), format!("{back}{carry}").trim().to_string(), Some(p.id.clone()));
            }
        }
        if self.prefs.limit_notify && self.prefs.auto_resume {
            for p in self.panes.iter().filter(|p| self.extras.notify.limited.contains(&p.id) && !limited.contains(&p.id) && p.exit_code.is_none()) {
                self.toast(ctx, format!("{} is back at work", p.name), "Its usage limit reset, so ByteHive told it to carry on.".into(), Some(p.id.clone()));
            }
        }
        self.extras.notify.limited = limited;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_puts_its_agent_on_stage() {
        let mut app = ShellApp::new().unwrap();
        let id = app.panes[0].id.clone();
        app.extras.notify.open_tx.send(id.clone()).unwrap();
        app.extras.notify.open_tx.send("gone".into()).unwrap();
        let ctx = egui::Context::default();
        app.poll_notify(&ctx);
        assert_eq!(app.stage.as_deref(), Some(id.as_str()));
    }

    /// Shows a real notification: `cargo test real_toast -- --ignored`.
    #[test]
    #[ignore]
    fn real_toast() {
        let t = tauri_winrt_notification::Toast::new(&app_id()).title("Ana is ready to review").text1("3 files changed in bytehive, after 4 min.");
        assert!(t.show().is_ok());
    }
}
