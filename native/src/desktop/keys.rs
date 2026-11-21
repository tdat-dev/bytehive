//! Real input for the desktop path: the mouse and keyboard as Windows sees
//! them, so Chrome takes them like a person's. Only sent while the agent's
//! Chrome window (or a dialog it opened) is in front.

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindow, SetCursorPos, SetForegroundWindow, GW_OWNER};

/// The agent's window or one of its dialogs is in front, or the reason not.
pub fn ensure_front(hwnd: isize) -> Result<(), String> {
    let front = || unsafe { GetForegroundWindow() } as isize;
    let ours = |f: isize| f == hwnd || unsafe { GetWindow(f as HWND, GW_OWNER) } as isize == hwnd;
    if ours(front()) {
        return Ok(());
    }
    unsafe { SetForegroundWindow(hwnd as HWND) };
    std::thread::sleep(std::time::Duration::from_millis(150));
    if ours(front()) {
        return Ok(());
    }
    Err("Chrome's window is not in front, and real clicks and keys go to the window in front. Ask the user to bring Chrome forward, or use a ref: buttons and fields with a ref work without it.".into())
}

fn send(inputs: &[INPUT]) {
    unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
}

fn mouse(flags: MOUSE_EVENT_FLAGS, data: i32) -> INPUT {
    INPUT { r#type: INPUT_MOUSE, Anonymous: INPUT_0 { mi: MOUSEINPUT { dx: 0, dy: 0, mouseData: data as u32, dwFlags: flags, time: 0, dwExtraInfo: 0 } } }
}

fn key(vk: u16, up: bool) -> INPUT {
    let mut flags = if up { KEYEVENTF_KEYUP } else { 0 };
    if EXTENDED.contains(&vk) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } } }
}

fn unit(c: u16, up: bool) -> INPUT {
    let flags = KEYEVENTF_UNICODE | if up { KEYEVENTF_KEYUP } else { 0 };
    INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: 0, wScan: c, dwFlags: flags, time: 0, dwExtraInfo: 0 } } }
}

/// Keys that need the extended flag, or Windows takes them from the number pad.
const EXTENDED: [u16; 10] = [0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x2D, 0x2E];

pub fn move_to(x: i32, y: i32) {
    unsafe { SetCursorPos(x, y) };
}

/// Click at a screen point: `button` left or right, `clicks` 1 to 3.
pub fn click(x: i32, y: i32, right: bool, clicks: u32) {
    move_to(x, y);
    std::thread::sleep(std::time::Duration::from_millis(30));
    let (down, up) = if right { (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP) } else { (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP) };
    for _ in 0..clicks.clamp(1, 3) {
        send(&[mouse(down, 0), mouse(up, 0)]);
        std::thread::sleep(std::time::Duration::from_millis(40));
    }
}

/// Turn the wheel at a screen point: `notches` down (positive) or up, or sideways.
pub fn wheel(x: i32, y: i32, notches: i32, sideways: bool) {
    move_to(x, y);
    let flag = if sideways { MOUSEEVENTF_HWHEEL } else { MOUSEEVENTF_WHEEL };
    // The wheel turns up for a positive delta: down means negative.
    let delta = if sideways { notches * 120 } else { -notches * 120 };
    send(&[mouse(flag, delta)]);
}

/// Type text as it is, any language: each UTF-16 unit as a Unicode key.
pub fn type_text(text: &str) {
    let units: Vec<u16> = text.replace("\r\n", "\n").replace('\n', "\r").encode_utf16().collect();
    for chunk in units.chunks(64) {
        let inputs: Vec<INPUT> = chunk.iter().flat_map(|c| [unit(*c, false), unit(*c, true)]).collect();
        send(&inputs);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// The virtual key of one key name: "enter", "a", "f5", "ctrl".
pub fn vk_of(name: &str) -> Option<u16> {
    let n = name.trim().to_ascii_lowercase();
    let k = match n.as_str() {
        "enter" | "return" => 0x0D,
        "tab" => 0x09,
        "escape" | "esc" => 0x1B,
        "backspace" => 0x08,
        "delete" | "del" => 0x2E,
        "insert" => 0x2D,
        "space" | " " => 0x20,
        "left" | "arrowleft" => 0x25,
        "up" | "arrowup" => 0x26,
        "right" | "arrowright" => 0x27,
        "down" | "arrowdown" => 0x28,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        "ctrl" | "control" => 0x11,
        "shift" => 0x10,
        "alt" => 0x12,
        "meta" | "cmd" | "win" | "super" => 0x5B,
        _ => {
            if let Some(f) = n.strip_prefix('f').and_then(|d| d.parse::<u16>().ok()).filter(|d| (1..=12).contains(d)) {
                0x6F + f
            } else if n.len() == 1 && n.as_bytes()[0].is_ascii_alphanumeric() {
                n.to_ascii_uppercase().as_bytes()[0] as u16
            } else {
                return None;
            }
        }
    };
    Some(k)
}

/// Parse "ctrl+a", "Enter" or "Tab Tab Enter" into combos of virtual keys.
pub fn parse(spec: &str) -> Result<Vec<Vec<u16>>, String> {
    spec.split_whitespace()
        .map(|combo| combo.split('+').map(|k| vk_of(k).ok_or_else(|| format!("Unknown key \"{k}\" in \"{spec}\"."))).collect())
        .collect()
}

/// Press each combo: modifiers down in order, then up in reverse.
pub fn press(combos: &[Vec<u16>]) {
    for combo in combos {
        let mut inputs: Vec<INPUT> = combo.iter().map(|k| key(*k, false)).collect();
        inputs.extend(combo.iter().rev().map(|k| key(*k, true)));
        send(&inputs);
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_combos_and_sequences() {
        assert_eq!(parse("ctrl+a"), Ok(vec![vec![0x11, 0x41]]));
        assert_eq!(parse("Tab Tab Enter"), Ok(vec![vec![0x09], vec![0x09], vec![0x0D]]));
        assert_eq!(parse("F5"), Ok(vec![vec![0x74]]));
        assert!(parse("ctrl+nope").is_err());
    }
}
