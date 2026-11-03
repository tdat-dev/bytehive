//! Keys as Windows itself gives them, for a console that asked for them.
//! ConPTY opens every session with `ESC [?9001h` (win32-input-mode): from
//! then on each press and release travels whole, as Windows Terminal sends
//! it: `ESC [ Vk ; Sc ; Uc ; Kd ; Cs ; Rc _`, virtual key, scan code, the
//! character Windows makes of it, down or up, the Ctrl/Alt/Shift state, and
//! the repeat count. ConPTY hands the program the very key event a console
//! window would, so every combination any CLI binds reaches it, none picked
//! or dropped by ByteHive.

use eframe::egui::{Key, Modifiers};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, GetKeyboardLayout, MapVirtualKeyW, ToUnicodeEx, MAPVK_VK_TO_VSC};

/// Whether this output turned win32-input-mode on (Some(true)) or off.
pub fn mode_switch(bytes: &[u8]) -> Option<bool> {
    let on = find(bytes, b"\x1b[?9001h");
    let off = find(bytes, b"\x1b[?9001l");
    match (on, off) {
        (Some(a), Some(b)) => Some(a > b),
        (Some(_), None) => Some(true),
        (None, Some(_)) => Some(false),
        (None, None) => None,
    }
}

/// Where `needle` last starts in `hay`.
fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).rposition(|w| w == needle)
}

const RIGHT_ALT: u32 = 0x01;
const LEFT_ALT: u32 = 0x02;
const LEFT_CTRL: u32 = 0x08;
const SHIFT: u32 = 0x10;
const NUMLOCK: u32 = 0x20;
const SCROLLLOCK: u32 = 0x40;
const CAPSLOCK: u32 = 0x80;
const ENHANCED: u32 = 0x100;
const RIGHT_CTRL: u32 = 0x04;

/// The win32-input-mode bytes for one key going down or up, or None for a
/// key whose character the Text event types (plain letters, digits, space).
pub fn key_record(key: Key, physical: Option<Key>, mods: Modifiers, down: bool) -> Option<Vec<u8>> {
    let (vk, mut flags) = vk_of(physical.unwrap_or(key)).or_else(|| vk_of(key))?;
    let sc = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) };
    flags |= state(mods);
    let ch = to_unicode(vk, sc, mods);
    // A printable character with no Ctrl or Alt comes in as typed text
    // (which also carries what an IME or Unikey composes): not twice.
    let printable = ch.is_some_and(|c| c >= 0x20 && c != 0x7f);
    if printable && !mods.ctrl && !mods.alt {
        return None;
    }
    Some(record(vk, sc, ch.unwrap_or(0), down, flags))
}

/// One press and release of `vk` holding Ctrl (Ctrl+C, Ctrl+X: egui hands
/// those over as Copy and Cut, not keys).
pub fn ctrl_tap(vk: u16, ch: u16) -> Vec<u8> {
    let sc = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) };
    let flags = LEFT_CTRL | toggles();
    let mut out = record(vk, sc, ch, true, flags);
    out.extend(record(vk, sc, ch, false, flags));
    out
}

/// Shift+Delete, which egui hands over as Cut.
pub fn shift_delete() -> Vec<u8> {
    let sc = unsafe { MapVirtualKeyW(0x2E, MAPVK_VK_TO_VSC) };
    let flags = SHIFT | ENHANCED | toggles();
    let mut out = record(0x2E, sc, 0, true, flags);
    out.extend(record(0x2E, sc, 0, false, flags));
    out
}

fn record(vk: u16, sc: u32, ch: u16, down: bool, flags: u32) -> Vec<u8> {
    format!("\x1b[{vk};{sc};{ch};{};{flags};1_", u8::from(down)).into_bytes()
}

fn state(mods: Modifiers) -> u32 {
    let mut s = toggles();
    if mods.shift {
        s |= SHIFT;
    }
    if mods.ctrl {
        s |= LEFT_CTRL;
    }
    if mods.alt {
        // Ctrl+Alt is AltGr on the layouts that have one: Windows reports
        // it as right Alt with left Ctrl.
        s |= if mods.ctrl { RIGHT_ALT } else { LEFT_ALT };
    }
    s
}

/// Caps, Num and Scroll Lock as the keyboard has them now.
fn toggles() -> u32 {
    let on = |vk: i32| unsafe { GetKeyState(vk) } & 1 != 0;
    (if on(0x14) { CAPSLOCK } else { 0 }) | (if on(0x90) { NUMLOCK } else { 0 }) | (if on(0x91) { SCROLLLOCK } else { 0 })
}

/// The character Windows makes of `vk` with these modifiers, on the
/// keyboard layout in use, without touching dead-key state.
fn to_unicode(vk: u16, sc: u32, mods: Modifiers) -> Option<u16> {
    let mut keys = [0u8; 256];
    let held = |keys: &mut [u8; 256], vks: &[usize]| vks.iter().for_each(|&v| keys[v] = 0x80);
    if mods.shift {
        held(&mut keys, &[0x10, 0xA0]);
    }
    if mods.ctrl {
        held(&mut keys, &[0x11, 0xA2]);
    }
    if mods.alt {
        held(&mut keys, &[0x12, if mods.ctrl { 0xA5 } else { 0xA4 }]);
    }
    for v in [0x14usize, 0x90, 0x91] {
        keys[v] |= (unsafe { GetKeyState(v as i32) } & 1) as u8;
    }
    let mut buf = [0u16; 8];
    // Flag 4: leave the keyboard's dead-key state as it was.
    let n = unsafe { ToUnicodeEx(vk as u32, sc, keys.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, 4, GetKeyboardLayout(0)) };
    (n == 1).then_some(buf[0])
}

/// The Windows virtual key for an egui key, and ENHANCED for the keys of
/// the navigation block (as opposed to their numpad twins).
fn vk_of(key: Key) -> Option<(u16, u32)> {
    use Key::*;
    let plain = |vk: u16| Some((vk, 0));
    let ext = |vk: u16| Some((vk, ENHANCED));
    match key {
        Backspace => plain(0x08),
        Tab => plain(0x09),
        Enter => plain(0x0D),
        Escape => plain(0x1B),
        Space => plain(0x20),
        PageUp => ext(0x21),
        PageDown => ext(0x22),
        End => ext(0x23),
        Home => ext(0x24),
        ArrowLeft => ext(0x25),
        ArrowUp => ext(0x26),
        ArrowRight => ext(0x27),
        ArrowDown => ext(0x28),
        Insert => ext(0x2D),
        Delete => ext(0x2E),
        Num0 => plain(0x30),
        Num1 | Exclamationmark => plain(0x31),
        Num2 => plain(0x32),
        Num3 => plain(0x33),
        Num4 => plain(0x34),
        Num5 => plain(0x35),
        Num6 => plain(0x36),
        Num7 => plain(0x37),
        Num8 => plain(0x38),
        Num9 => plain(0x39),
        Semicolon | Colon => plain(0xBA),
        Equals | Plus => plain(0xBB),
        Comma => plain(0xBC),
        Minus => plain(0xBD),
        Period => plain(0xBE),
        Slash | Questionmark => plain(0xBF),
        Backtick => plain(0xC0),
        OpenBracket | OpenCurlyBracket => plain(0xDB),
        Backslash | Pipe => plain(0xDC),
        CloseBracket | CloseCurlyBracket => plain(0xDD),
        Quote => plain(0xDE),
        IntlBackslash => plain(0xE2),
        ShiftLeft => plain(0x10),
        ShiftRight => plain(0x10),
        ControlLeft => plain(0x11),
        ControlRight => Some((0x11, ENHANCED | RIGHT_CTRL)),
        AltLeft => plain(0x12),
        AltRight => Some((0x12, ENHANCED | RIGHT_ALT)),
        SuperLeft => ext(0x5B),
        SuperRight => ext(0x5C),
        Menu => ext(0x5D),
        CapsLock => plain(0x14),
        NumLock => ext(0x90),
        ScrollLock => plain(0x91),
        Pause => plain(0x13),
        PrintScreen => ext(0x2C),
        Help => plain(0x2F),
        Clear => plain(0x0C),
        Sleep => plain(0x5F),
        BrowserBack => ext(0xA6),
        BrowserForward => ext(0xA7),
        BrowserRefresh => ext(0xA8),
        BrowserStop => ext(0xA9),
        BrowserSearch => ext(0xAA),
        BrowserFavorites => ext(0xAB),
        BrowserHome => ext(0xAC),
        AudioVolumeMute => ext(0xAD),
        AudioVolumeDown => ext(0xAE),
        AudioVolumeUp => ext(0xAF),
        MediaTrackNext => ext(0xB0),
        MediaTrackPrevious => ext(0xB1),
        MediaStop => ext(0xB2),
        MediaPlayPause => ext(0xB3),
        LaunchMail => ext(0xB4),
        LaunchApp1 => ext(0xB6),
        LaunchApp2 => ext(0xB7),
        _ => {
            let name = key.name();
            let mut c = name.chars();
            match (c.next(), c.next()) {
                // A..Z share their virtual keys with ASCII.
                (Some(l), None) if l.is_ascii_uppercase() => plain(l as u16),
                // F1..F24 are 0x70.. in order.
                _ => name.strip_prefix('F').and_then(|n| n.parse::<u16>().ok()).filter(|n| (1..=24).contains(n)).map(|n| (0x6F + n, 0)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(b: &[u8]) -> Vec<u32> {
        let s = std::str::from_utf8(b).unwrap();
        s.trim_start_matches("\x1b[").split('_').next().unwrap().split(';').map(|f| f.parse().unwrap()).collect()
    }

    #[test]
    fn conpty_asking_turns_the_mode_on_and_off() {
        assert_eq!(mode_switch(b"\x1b[?9001h\x1b[?1004h"), Some(true));
        assert_eq!(mode_switch(b"\x1b[?9001h..\x1b[?9001l"), Some(false));
        assert_eq!(mode_switch(b"plain output"), None);
    }

    #[test]
    fn every_key_goes_whole_with_what_windows_makes_of_it() {
        let none = Modifiers::NONE;
        let ctrl = Modifiers { ctrl: true, ..Default::default() };
        let shift = Modifiers { shift: true, ..Default::default() };
        let enter = fields(&key_record(Key::Enter, None, none, true).unwrap());
        assert_eq!((enter[0], enter[2], enter[3], enter[4] & 0x1f), (0x0D, 13, 1, 0));
        assert!(enter[1] > 0, "a scan code");
        let up = fields(&key_record(Key::Enter, None, shift, false).unwrap());
        assert_eq!((up[3], up[4] & SHIFT), (0, SHIFT), "Shift+Enter released, Shift seen");
        let c = fields(&key_record(Key::C, Some(Key::C), ctrl, true).unwrap());
        assert_eq!((c[0], c[2], c[4] & LEFT_CTRL), (0x43, 3, LEFT_CTRL), "Ctrl+C is ETX");
        let f13 = fields(&key_record(Key::F13, None, none, true).unwrap());
        assert_eq!(f13[0], 0x7C);
        let left = fields(&key_record(Key::ArrowLeft, None, ctrl, true).unwrap());
        assert_eq!(left[4] & ENHANCED, ENHANCED);
        // Plain letters come in as typed text, not twice.
        assert_eq!(key_record(Key::A, Some(Key::A), none, true), None);
        assert_eq!(key_record(Key::A, Some(Key::A), shift, true), None);
    }

    /// Starts a real shell: run by hand with --ignored.
    #[test]
    #[ignore]
    fn a_real_shell_takes_whole_key_events() {
        use crate::app::ShellApp;
        use std::time::{Duration, Instant};
        let mut app = ShellApp::new().unwrap();
        let id = app.panes[0].id.clone();
        let pump = |app: &mut ShellApp, secs: u64, until: &dyn Fn(&ShellApp) -> bool| {
            let end = Instant::now() + Duration::from_secs(secs);
            while Instant::now() < end && !until(app) {
                let ShellApp { panes, registry, .. } = &mut *app;
                panes[0].drain(&mut agent_core::lock(registry));
                std::thread::sleep(Duration::from_millis(100));
            }
        };
        pump(&mut app, 20, &|a| a.panes[0].screen_text().contains("PS "));
        let w = |app: &ShellApp, b: &[u8]| {
            let _ = agent_core::lock(&app.registry).write_input(&id, b);
            std::thread::sleep(Duration::from_millis(300));
        };
        let ctrl = Modifiers { ctrl: true, ..Default::default() };
        w(&app, b"echo abc def");
        w(&app, &key_record(Key::ArrowLeft, None, ctrl, true).unwrap());
        w(&app, &key_record(Key::ArrowLeft, None, ctrl, false).unwrap());
        w(&app, b"X");
        w(&app, &key_record(Key::Enter, None, Modifiers::NONE, true).unwrap());
        w(&app, &key_record(Key::Enter, None, Modifiers::NONE, false).unwrap());
        pump(&mut app, 5, &|a| a.panes[0].screen_text().lines().any(|l| l.trim() == "Xdef"));
        let screen = app.panes[0].screen_text();
        assert!(screen.contains("echo abc Xdef") && screen.lines().any(|l| l.trim() == "Xdef"), "keys did not arrive whole:\n{screen}");
    }

    #[test]
    fn copy_and_cut_go_as_the_keys_they_were() {
        let f = fields(&ctrl_tap(0x43, 3)[..]);
        assert_eq!((f[0], f[2], f[3]), (0x43, 3, 1));
        assert!(shift_delete().starts_with(b"\x1b[46;"));
    }
}
