//! Keys as Windows gives them, straight to the console: there is no key table
//! here, on purpose. egui only offered abstract keys, so the old shell had to
//! declare each one by hand and anything undeclared never reached the CLI.
//!
//! GPUI eats WM_KEYDOWN in its message pump (translate_accelerator) before any
//! window procedure sees it, so a subclass would miss keys. A WH_GETMESSAGE
//! hook runs inside GetMessage, ahead of the pump: while a terminal has focus
//! it turns each key into a win32-input-mode record, `ESC [ Vk ; Sc ; Uc ; Kd ;
//! Cs ; Rc _`, and blanks the message. ConPTY then hands the program the same
//! key event a console window would get.

use std::sync::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
use std::sync::mpsc::Sender;

use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, GetKeyboardLayout, GetKeyboardState, ToUnicodeEx, VK_CAPITAL, VK_CONTROL, VK_F4, VK_LMENU, VK_LCONTROL, VK_MENU, VK_NUMLOCK,
    VK_PACKET, VK_PROCESSKEY, VK_RCONTROL, VK_RMENU, VK_SCROLL, VK_SHIFT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, MSG, PM_QS_POSTMESSAGE, PM_REMOVE, PeekMessageW, SetWindowsHookExW, TranslateMessage, WH_GETMESSAGE, WM_CHAR, WM_KEYDOWN, WM_KEYUP, WM_NULL, WM_SYSKEYDOWN,
    WM_SYSKEYUP,
};

const RIGHT_ALT: u32 = 0x01;
const LEFT_ALT: u32 = 0x02;
const RIGHT_CTRL: u32 = 0x04;
const LEFT_CTRL: u32 = 0x08;
const SHIFT: u32 = 0x10;
const NUMLOCK: u32 = 0x20;
const SCROLLLOCK: u32 = 0x40;
const CAPSLOCK: u32 = 0x80;
const ENHANCED: u32 = 0x100;

/// ToUnicodeEx leaves the keyboard state alone, so dead keys still compose.
const NO_STATE_CHANGE: u32 = 0x4;

/// A terminal has focus: keys belong to it, not to GPUI.
static ACTIVE: AtomicBool = AtomicBool::new(false);
/// ConPTY opened win32-input-mode (it does on every session).
static WIN32_MODE: AtomicBool = AtomicBool::new(false);
/// The focused terminal's writer, and where it notes when you last typed into it.
static SINK: Mutex<Option<(Sender<Vec<u8>>, Arc<AtomicU64>)>> = Mutex::new(None);
static HOOK: AtomicIsize = AtomicIsize::new(0);
/// A UniKey character is being pulled out of its WM_CHAR right now.
static IN_PACKET: AtomicBool = AtomicBool::new(false);

/// Hook this thread's message queue. Call once, on the thread running GPUI.
pub fn install() {
    if HOOK.load(Ordering::Relaxed) != 0 {
        return;
    }
    let hook = unsafe { SetWindowsHookExW(WH_GETMESSAGE, Some(on_message), std::ptr::null_mut(), GetCurrentThreadId()) };
    HOOK.store(hook as isize, Ordering::Relaxed);
}

/// Where keys go from now on: the focused terminal's writer, and its last-input time.
pub fn set_sink(sink: Sender<Vec<u8>>, last_input: Arc<AtomicU64>) {
    *SINK.lock().unwrap() = Some((sink, last_input));
}

pub fn set_active(active: bool) {
    ACTIVE.store(active, Ordering::Relaxed);
}

/// Follow ConPTY switching win32-input-mode in this output.
pub fn watch_mode(bytes: &[u8]) {
    let last = |needle: &[u8]| bytes.windows(needle.len()).rposition(|w| w == needle);
    match (last(b"\x1b[?9001h"), last(b"\x1b[?9001l")) {
        (Some(on), Some(off)) => WIN32_MODE.store(on > off, Ordering::Relaxed),
        (Some(_), None) => WIN32_MODE.store(true, Ordering::Relaxed),
        (None, Some(_)) => WIN32_MODE.store(false, Ordering::Relaxed),
        (None, None) => {}
    }
}

/// Text as console key events, one press and release per UTF-16 unit. What an
/// IME commits or UniKey types has no key behind it, so Vk and Sc stay 0.
pub fn text_records(text: &str) -> Vec<u8> {
    if !WIN32_MODE.load(Ordering::Relaxed) {
        return text.as_bytes().to_vec();
    }
    let mut out = Vec::new();
    for unit in text.encode_utf16() {
        out.extend(record(0, 0, unit, true, 0, 1));
        out.extend(record(0, 0, unit, false, 0, 1));
    }
    out
}

fn record(vk: u16, sc: u32, ch: u16, down: bool, flags: u32, repeat: u32) -> Vec<u8> {
    format!("\x1b[{vk};{sc};{ch};{};{flags};{repeat}_", u8::from(down)).into_bytes()
}

fn pressed(vk: u16) -> bool {
    (unsafe { GetKeyState(vk as i32) } as u16) & 0x8000 != 0
}

fn toggled(vk: u16) -> bool {
    (unsafe { GetKeyState(vk as i32) }) & 1 != 0
}

fn control_state(enhanced: bool) -> u32 {
    let mut s = 0;
    for (on, bit) in [
        (pressed(VK_RMENU), RIGHT_ALT),
        (pressed(VK_LMENU), LEFT_ALT),
        (pressed(VK_RCONTROL), RIGHT_CTRL),
        (pressed(VK_LCONTROL), LEFT_CTRL),
        (pressed(VK_SHIFT), SHIFT),
        (toggled(VK_NUMLOCK), NUMLOCK),
        (toggled(VK_SCROLL), SCROLLLOCK),
        (toggled(VK_CAPITAL), CAPSLOCK),
        (enhanced, ENHANCED),
    ] {
        if on {
            s |= bit;
        }
    }
    s
}

/// The character Windows makes of this key right now, 0 when none.
fn character(vk: u16, sc: u32) -> u16 {
    let mut state = [0u8; 256];
    let mut out = [0u16; 4];
    unsafe {
        if GetKeyboardState(state.as_mut_ptr()) == 0 {
            return 0;
        }
        let n = ToUnicodeEx(vk as u32, sc, state.as_ptr(), out.as_mut_ptr(), out.len() as i32, NO_STATE_CHANGE, GetKeyboardLayout(0));
        if n == 1 { out[0] } else { 0 }
    }
}

/// Keys that stay ours even over a terminal, the same few today's app takes
/// before xterm sees them: Ctrl+V pastes, Ctrl+K opens the palette,
/// Ctrl+Shift+T starts a new agent, Alt+F4 closes the window. Everything
/// else is the program's: Ctrl+Shift+O reaches the CLI too, and works only
/// when no terminal has the keyboard.
fn stays_local(vk: u16) -> bool {
    local_key(vk, pressed(VK_CONTROL), pressed(VK_MENU), pressed(VK_SHIFT))
}

fn local_key(vk: u16, ctrl: bool, alt: bool, shift: bool) -> bool {
    (ctrl && !alt && !shift && (vk == b'V' as u16 || vk == b'K' as u16)) || (ctrl && shift && !alt && vk == b'T' as u16) || (alt && vk == VK_F4)
}

#[cfg(test)]
mod tests {
    #[test]
    fn notes_when_you_last_typed_into_the_terminal() {
        let (tx, rx) = std::sync::mpsc::channel();
        let last = std::sync::Arc::new(super::AtomicU64::new(0));
        super::set_sink(tx, last.clone());
        let before = crate::fleet::now_ms();
        super::send(b"a".to_vec());
        assert_eq!(rx.recv().unwrap(), b"a");
        assert!(last.load(super::Ordering::Relaxed) >= before);
    }

    use super::*;

    #[test]
    fn only_the_app_s_few_keys_stay_local_over_a_terminal() {
        let o = b'O' as u16;
        assert!(!local_key(o, true, false, true), "Ctrl+Shift+O is the program's: a focused terminal keeps every key");
        assert!(!local_key(o, true, false, false), "Ctrl+O is the program's");
        assert!(!local_key(o, false, false, true), "Shift+O is the program's");
        assert!(!local_key(o, true, true, true), "Ctrl+Alt+Shift+O is the program's");
        assert!(local_key(b'T' as u16, true, false, true) && local_key(b'K' as u16, true, false, false));
        assert!(!local_key(b'C' as u16, true, false, false), "Ctrl+C interrupts the program");
    }
}

/// Text typed in by SendInput (UniKey, EVKey) comes as VK_PACKET, and only
/// TranslateMessage knows its character: it posts a WM_CHAR. Left to GPUI,
/// that WM_CHAR waits in the posted queue while GPUI, busy with a terminal's
/// output, drains only input messages, so UniKey's backspaces overtook the
/// letters they made room for ("k lamf" came out "k àm"). Translate it here
/// and take the WM_CHAR at once, so the letter goes out in its place (as
/// Firefox does). Only posted messages are peeked: nothing sent to a window
/// runs from inside this hook. Should no WM_CHAR come, the key is left to GPUI.
fn take_packet(msg: &MSG) -> bool {
    let mut units = Vec::new();
    IN_PACKET.store(true, Ordering::Relaxed);
    unsafe {
        TranslateMessage(msg);
        let mut ch: MSG = std::mem::zeroed();
        while PeekMessageW(&mut ch, msg.hwnd, WM_CHAR, WM_CHAR, PM_REMOVE | PM_QS_POSTMESSAGE) != 0 {
            units.push(ch.wParam as u16);
        }
    }
    IN_PACKET.store(false, Ordering::Relaxed);
    if units.is_empty() {
        return false;
    }
    send(text_records(&String::from_utf16_lossy(&units)));
    true
}

fn send(bytes: Vec<u8>) {
    if let Some((sink, last_input)) = SINK.lock().unwrap().as_ref() {
        last_input.store(crate::fleet::now_ms(), Ordering::Relaxed);
        let _ = sink.send(bytes);
    }
}

unsafe extern "system" fn on_message(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && wparam == PM_REMOVE as usize && ACTIVE.load(Ordering::Relaxed) {
        let msg = unsafe { &mut *(lparam as *mut MSG) };
        match msg.message {
            WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP => {
                let vk = msg.wParam as u16;
                if vk == VK_PACKET {
                    let up = matches!(msg.message, WM_KEYUP | WM_SYSKEYUP);
                    if up || take_packet(msg) {
                        msg.message = WM_NULL;
                    }
                // An IME's key comes back as text, through GPUI's input handler.
                } else if vk != VK_PROCESSKEY && !stays_local(vk) {
                    let down = matches!(msg.message, WM_KEYDOWN | WM_SYSKEYDOWN);
                    let sc = ((msg.lParam >> 16) & 0xff) as u32;
                    let enhanced = msg.lParam & (1 << 24) != 0;
                    let repeat = (msg.lParam & 0xffff) as u32;
                    let ch = character(vk, sc);
                    if WIN32_MODE.load(Ordering::Relaxed) {
                        send(record(vk, sc, ch, down, control_state(enhanced), repeat.max(1)));
                    } else if down && ch != 0 {
                        send(String::from_utf16_lossy(&[ch]).into_bytes());
                    }
                    msg.message = WM_NULL;
                }
            }
            WM_CHAR if !IN_PACKET.load(Ordering::Relaxed) => {
                send(text_records(&String::from_utf16_lossy(&[msg.wParam as u16])));
                msg.message = WM_NULL;
            }
            _ => {}
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}
