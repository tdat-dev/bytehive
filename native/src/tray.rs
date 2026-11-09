//! The tray icon (build_tray in today's lib.rs). It shows only while
//! Settings → Hide to tray is on: minimising then tucks the window away and
//! the agents keep running. A click on it, or Show, brings the window back;
//! Quit closes the app.

use std::sync::atomic::{AtomicBool, Ordering};

use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use windows_sys::Win32::Foundation::{HWND, LPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, IsIconic, IsWindowVisible, SW_HIDE, ShowWindow};

const SHOW: &str = "show";
const QUIT: &str = "quit";

/// Quit was picked in the tray's menu; the app's tick acts on it.
static QUIT_ASKED: AtomicBool = AtomicBool::new(false);

pub struct Tray {
    icon: TrayIcon,
    tip: String,
}

impl Tray {
    /// Built hidden, like today's: the setting turns it on.
    pub fn new() -> Option<Self> {
        let rgba = include_bytes!("../assets/tray32.rgba").to_vec();
        let menu = Menu::new();
        menu.append(&MenuItem::with_id(SHOW, "Show ByteHive", true, None)).ok()?;
        menu.append(&MenuItem::with_id(QUIT, "Quit", true, None)).ok()?;
        let icon = TrayIconBuilder::new()
            .with_tooltip("ByteHive")
            .with_icon(Icon::from_rgba(rgba, 32, 32).ok()?)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build()
            .ok()?;
        let _ = icon.set_visible(false);
        // Both run inside the window's message loop: no app state is touched,
        // only Windows' own window calls and a flag.
        TrayIconEvent::set_event_handler(Some(|event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                crate::single::raise(std::process::id());
            }
        }));
        MenuEvent::set_event_handler(Some(|event: MenuEvent| match event.id().as_ref() {
            SHOW => crate::single::raise(std::process::id()),
            QUIT => QUIT_ASKED.store(true, Ordering::Relaxed),
            _ => {}
        }));
        Some(Self { icon, tip: "ByteHive".to_string() })
    }

    pub fn set_visible(&self, on: bool) {
        let _ = self.icon.set_visible(on);
    }

    /// "ByteHive · 2 running", so a hidden window still shows it is alive.
    pub fn set_running(&mut self, running: usize) {
        let tip = if running > 0 { format!("ByteHive · {running} running") } else { "ByteHive".to_string() };
        if tip != self.tip {
            let _ = self.icon.set_tooltip(Some(&tip));
            self.tip = tip;
        }
    }
}

pub fn quit_asked() -> bool {
    QUIT_ASKED.swap(false, Ordering::Relaxed)
}

/// Hide this app's window once it is minimised, so only the tray holds it.
pub fn tuck_away_if_minimised() {
    unsafe extern "system" fn each(hwnd: HWND, lparam: LPARAM) -> i32 {
        let mut owner = 0u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut owner);
            if owner == lparam as u32 && IsWindowVisible(hwnd) != 0 && IsIconic(hwnd) != 0 {
                ShowWindow(hwnd, SW_HIDE);
            }
        }
        1
    }
    unsafe {
        EnumWindows(Some(each), std::process::id() as LPARAM);
    }
}
