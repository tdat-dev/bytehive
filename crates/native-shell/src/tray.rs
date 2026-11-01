//! The system tray: with Settings > System "Hide to tray when minimized" on,
//! minimizing hides the window and a tray icon brings it back (click it, or
//! Show in its menu); Quit in the menu goes through the same quit guard as
//! the window's X. The tip says how many agents are running. Port of the web
//! app's hide-to-tray (titlebar.ts, quitlife.ts, src-tauri's build_tray).

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::app::ShellApp;

const ICON_32: &[u8] = include_bytes!("../assets/icons/32x32.png");
const ICON_128: &[u8] = include_bytes!("../assets/icons/128x128.png");

fn rgba(png: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    Some((img.into_raw(), w, h))
}

/// The window's own icon (title bar, taskbar, Alt+Tab).
pub fn window_icon() -> egui::IconData {
    rgba(ICON_128).map(|(rgba, width, height)| egui::IconData { rgba, width, height }).unwrap_or_default()
}

/// The tray tip: "Maestro" and, when any, how many agents run.
pub fn tip(running: usize) -> String {
    match running {
        0 => "Maestro".into(),
        1 => "Maestro \u{00B7} 1 agent running".into(),
        n => format!("Maestro \u{00B7} {n} agents running"),
    }
}

#[derive(Default)]
pub struct Tray {
    icon: Option<TrayIcon>,
    /// The main window, for showing it from the tray's own thread.
    hwnd: Arc<AtomicIsize>,
    /// Quit was picked in the tray menu.
    quit: Arc<AtomicBool>,
    hidden: bool,
    tip: String,
    tip_at: Option<Instant>,
    show_id: String,
    quit_id: String,
}

#[cfg(windows)]
pub(crate) fn show_window(hwnd: isize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW};
    if hwnd != 0 {
        unsafe {
            ShowWindow(hwnd as _, SW_SHOW);
            ShowWindow(hwnd as _, SW_RESTORE);
            SetForegroundWindow(hwnd as _);
        }
    }
}

#[cfg(windows)]
fn hide_window(hwnd: isize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
    if hwnd != 0 {
        unsafe {
            ShowWindow(hwnd as _, SW_HIDE);
        }
    }
}

#[cfg(not(windows))]
pub(crate) fn show_window(_: isize) {}
#[cfg(not(windows))]
fn hide_window(_: isize) {}

impl Tray {
    pub fn hwnd(&self) -> isize {
        self.hwnd.load(Ordering::Relaxed)
    }

    /// Learn the main window from eframe at start.
    pub fn set_window(&self, cc: &eframe::CreationContext<'_>) {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        if let Ok(h) = cc.window_handle() {
            if let RawWindowHandle::Win32(w) = h.as_raw() {
                self.hwnd.store(w.hwnd.get(), Ordering::Relaxed);
            }
        }
    }

    fn build(&mut self, ctx: &egui::Context) {
        let Some((rgba, w, h)) = rgba(ICON_32) else { return };
        let Ok(icon) = tray_icon::Icon::from_rgba(rgba, w, h) else { return };
        let show = MenuItem::new("Show Maestro", true, None);
        let quit = MenuItem::new("Quit", true, None);
        let menu = Menu::new();
        if menu.append(&show).is_err() || menu.append(&quit).is_err() {
            return;
        }
        self.show_id = show.id().0.clone();
        self.quit_id = quit.id().0.clone();
        let Ok(tray) = TrayIconBuilder::new().with_icon(icon).with_tooltip("Maestro").with_menu(Box::new(menu)).with_menu_on_left_click(false).build() else { return };
        self.icon = Some(tray);
        let (hwnd, repaint) = (Arc::clone(&self.hwnd), ctx.clone());
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                show_window(hwnd.load(Ordering::Relaxed));
                repaint.request_repaint();
            }
        }));
        let (hwnd, quit, repaint) = (Arc::clone(&self.hwnd), Arc::clone(&self.quit), ctx.clone());
        let (show_id, quit_id) = (self.show_id.clone(), self.quit_id.clone());
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            if e.id.0 == show_id || e.id.0 == quit_id {
                show_window(hwnd.load(Ordering::Relaxed));
            }
            if e.id.0 == quit_id {
                quit.store(true, Ordering::Relaxed);
            }
            repaint.request_repaint();
        }));
    }
}

impl ShellApp {
    pub(crate) fn poll_tray(&mut self, ctx: &egui::Context) {
        let on = self.prefs.hide_to_tray && !cfg!(test);
        let tray = &mut self.extras.tray;
        if on && tray.icon.is_none() {
            tray.build(ctx);
        } else if !on && tray.icon.is_some() {
            tray.icon = None;
        }
        if tray.quit.swap(false, Ordering::Relaxed) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        let minimized = ctx.input(|i| i.viewport().minimized).unwrap_or(false);
        if tray.icon.is_some() && minimized && !tray.hidden {
            tray.hidden = true;
            hide_window(tray.hwnd.load(Ordering::Relaxed));
        } else if !minimized {
            tray.hidden = false;
        }
        if tray.icon.is_some() && tray.tip_at.is_none_or(|t| t.elapsed() >= Duration::from_secs(3)) {
            tray.tip_at = Some(Instant::now());
            let running = self.panes.iter().filter(|p| p.cli.is_some() && p.exit_code.is_none()).count();
            let tip = tip(running);
            if tip != tray.tip {
                if let Some(i) = &tray.icon {
                    let _ = i.set_tooltip(Some(&tip));
                }
                tray.tip = tip;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_decode_and_tips_count() {
        let i = window_icon();
        assert_eq!((i.width, i.height), (128, 128));
        assert_eq!(i.rgba.len(), 128 * 128 * 4);
        assert!(rgba(ICON_32).is_some());
        assert_eq!(tip(0), "Maestro");
        assert_eq!(tip(3), "Maestro \u{00B7} 3 agents running");
    }
}
