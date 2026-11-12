//! Every Windows key goes through the real hook and comes out as the console
//! record a console window would get: all 254 virtual keys, bare and under
//! Ctrl, Alt and Shift. The app's own four (Ctrl+V, Ctrl+K, Ctrl+Shift+T,
//! Alt+F4) are the only ones kept back, and IME and UniKey keys come back as
//! text by their own road.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64};

use windows_sys::Win32::UI::Input::KeyboardAndMouse::{SetKeyboardState, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_MENU, VK_PACKET, VK_PROCESSKEY, VK_SHIFT};
use windows_sys::Win32::UI::WindowsAndMessaging::{MSG, WM_KEYDOWN, WM_KEYUP, WM_NULL, WM_SYSKEYDOWN};

use super::*;

/// Hold modifiers down for this thread, as GetKeyState will read them.
fn hold(ctrl: bool, alt: bool, shift: bool) {
    let mut state = [0u8; 256];
    for (on, keys) in [(ctrl, [VK_CONTROL, VK_LCONTROL]), (alt, [VK_MENU, VK_LMENU]), (shift, [VK_SHIFT, VK_LSHIFT])] {
        if on {
            for k in keys {
                state[k as usize] = 0x80;
            }
        }
    }
    unsafe { SetKeyboardState(state.as_ptr()) };
}

/// One key message through the hook, taken as GPUI's busy drain takes it.
fn through_hook(message: u32, vk: u16) -> (u32, Vec<Vec<u8>>, std::sync::mpsc::Receiver<Vec<u8>>) {
    let (tx, rx) = std::sync::mpsc::channel();
    set_sink(tx, Arc::new(AtomicU64::new(0)), Arc::new(AtomicBool::new(true)));
    let sc = unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::MapVirtualKeyW(vk as u32, 0) };
    let mut msg: MSG = unsafe { std::mem::zeroed() };
    msg.message = message;
    msg.wParam = vk as usize;
    msg.lParam = (1 | (sc << 16)) as isize;
    // What the hook does with a key off the queue while a terminal has focus; the
    // global focus flag is left out, as other tests open Home or Ctrl+K meanwhile.
    to_terminal(&mut msg);
    let out: Vec<Vec<u8>> = rx.try_iter().collect();
    (msg.message, out, rx)
}

#[test]
fn every_windows_key_reaches_the_program_as_a_console_record() {
    let _one = TEST_KEYS.lock().unwrap_or_else(|e| e.into_inner());
    set_active(true);
    let mut missing = Vec::new();
    for (ctrl, alt, shift) in [(false, false, false), (true, false, false), (false, true, false), (false, false, true), (true, false, true), (true, true, false)] {
        hold(ctrl, alt, shift);
        for vk in 1u16..=254 {
            if vk == VK_PACKET || vk == VK_PROCESSKEY || local_key(vk, ctrl, alt, shift) {
                continue;
            }
            for message in [if alt { WM_SYSKEYDOWN } else { WM_KEYDOWN }, WM_KEYUP] {
                let (left, out, _) = through_hook(message, vk);
                let record = out.first().map(|r| String::from_utf8_lossy(r).to_string()).unwrap_or_default();
                let prefix = format!("\x1b[{vk};");
                if left != WM_NULL || out.len() != 1 || !record.starts_with(&prefix) || !record.ends_with('_') {
                    missing.push(format!("vk {vk:#04x} ctrl={ctrl} alt={alt} shift={shift} msg={message:#x}: {record:?}"));
                }
            }
        }
    }
    hold(false, false, false);
    set_active(false);
    assert!(missing.is_empty(), "{} keys did not reach the program, first: {:?}", missing.len(), &missing[..missing.len().min(5)]);
}

#[test]
fn modifiers_ride_in_the_record() {
    let _one = TEST_KEYS.lock().unwrap_or_else(|e| e.into_inner());
    set_active(true);
    hold(true, false, true);
    // Ctrl+Shift+Left: Claude Code's word jumps, a shell's selection.
    let (_, out, _) = through_hook(WM_KEYDOWN, 0x25);
    hold(false, false, false);
    set_active(false);
    let record = String::from_utf8_lossy(&out[0]).to_string();
    let fields: Vec<&str> = record.trim_start_matches("\x1b[").trim_end_matches('_').split(';').collect();
    let state: u32 = fields[4].parse().unwrap();
    assert!(state & LEFT_CTRL != 0 && state & SHIFT != 0, "control state {state:#x} in {record:?}");
}

#[test]
fn the_app_keeps_only_its_four_keys() {
    let _one = TEST_KEYS.lock().unwrap_or_else(|e| e.into_inner());
    set_active(true);
    hold(true, false, false);
    let (left, out, _) = through_hook(WM_KEYDOWN, b'K' as u16);
    hold(false, false, false);
    set_active(false);
    assert!(left == WM_KEYDOWN && out.is_empty(), "Ctrl+K stays with the app");
}
