//! Whether the person is at the keyboard or mouse. Windows' own last-input
//! time counts every program's synthetic input too, and on this machine
//! something sends some every half second (measured: idle never passed 700
//! ms), so a borrow waited forever. Low-level hooks see each key and mouse
//! event with a flag for injected ones: only the person's own count here.
//! The hooks run on a thread of their own that does nothing else, so they add
//! no delay to anyone's typing.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::SystemInformation::GetTickCount;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

/// When the person last touched a real key or the real mouse (GetTickCount).
static REAL: AtomicU32 = AtomicU32::new(0);
/// When the hooks started: nothing is known about the time before.
static SINCE: OnceLock<u32> = OnceLock::new();

fn now() -> u32 {
    unsafe { GetTickCount() }
}

unsafe extern "system" fn keyboard(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    if code >= 0 {
        let k = unsafe { &*(l as *const KBDLLHOOKSTRUCT) };
        if k.flags & LLKHF_INJECTED == 0 {
            REAL.store(now(), Ordering::Relaxed);
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, w, l) }
}

unsafe extern "system" fn mouse(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    if code >= 0 {
        let m = unsafe { &*(l as *const MSLLHOOKSTRUCT) };
        if m.flags & LLMHF_INJECTED == 0 {
            REAL.store(now(), Ordering::Relaxed);
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, w, l) }
}

/// Start watching, once.
pub fn watch() {
    SINCE.get_or_init(|| {
        std::thread::Builder::new()
            .name("bytehive-idle".into())
            .spawn(|| unsafe {
                let module = GetModuleHandleW(std::ptr::null());
                SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard), module, 0);
                SetWindowsHookExW(WH_MOUSE_LL, Some(mouse), module, 0);
                let mut msg: MSG = std::mem::zeroed();
                while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {}
            })
            .expect("idle thread");
        now()
    });
}

/// Milliseconds since the person's own last key or mouse move; counted from
/// when watching started if they have done nothing since.
pub fn idle_ms() -> u32 {
    watch();
    let since = *SINCE.get().expect("watching");
    let last = REAL.load(Ordering::Relaxed);
    let from = if last == 0 || since.wrapping_sub(last) as i32 > 0 { since } else { last };
    now().wrapping_sub(from)
}
