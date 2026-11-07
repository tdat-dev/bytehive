//! The one global key: registered with Windows, so it works from a focused
//! terminal and from any other app, and takes nothing from a CLI. A press brings
//! the window forward and tells the page, which decides where the keyboard goes
//! (hotkey.ts).
//!
//! RegisterHotKey, not the Tauri plugin: the plugin moved the whole Tauri stack in
//! Cargo.lock. WM_HOTKEY goes to the thread that registered the key, so one thread
//! owns it and waits on its message queue.

use crate::error::{run_blocking, CommandError};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN,
};
use windows::Win32::UI::WindowsAndMessaging::{GetMessageW, PeekMessageW, PostThreadMessageW, MSG, PM_NOREMOVE, WM_APP, WM_HOTKEY};

const ID: i32 = 1;

#[derive(Debug, PartialEq, Clone, Copy)]
struct Key {
    mods: u32,
    vk: u32,
}

/// "Win+Alt+N" as Windows wants it. At least one modifier: a bare key would be
/// taken from every app on the machine.
fn parse(combo: &str) -> Result<Key, String> {
    let bad = || format!("{combo} is not a key ByteHive can use. Write it like Win+Alt+N.");
    let mut mods = 0;
    let mut vk = None;
    for part in combo.split('+').map(str::trim) {
        match part.to_ascii_lowercase().as_str() {
            "win" => mods |= MOD_WIN.0,
            "alt" => mods |= MOD_ALT.0,
            "ctrl" | "control" => mods |= MOD_CONTROL.0,
            "shift" => mods |= MOD_SHIFT.0,
            name if vk.is_some() => return Err(format!("{combo} has two keys in it: {name} is one too many.")),
            "space" => vk = Some(0x20),
            name if name.len() == 1 && name.as_bytes()[0].is_ascii_alphanumeric() => vk = Some(name.as_bytes()[0].to_ascii_uppercase() as u32),
            name => {
                let n = name.strip_prefix('f').and_then(|n| n.parse::<u32>().ok()).filter(|n| (1..=24).contains(n)).ok_or_else(bad)?;
                vk = Some(0x70 + n - 1);
            }
        }
    }
    match vk {
        Some(vk) if mods != 0 => Ok(Key { mods, vk }),
        _ => Err(bad()),
    }
}

type Ask = (Option<Key>, Sender<bool>);

/// The thread that owns the key, and how to ask it for a change.
struct Owner {
    thread: u32,
    asks: Sender<Ask>,
}

static OWNER: OnceLock<Mutex<Owner>> = OnceLock::new();

fn start(app: AppHandle, show: fn(&AppHandle)) -> Mutex<Owner> {
    let (asks, inbox) = channel::<Ask>();
    let (told, thread) = channel();
    std::thread::spawn(move || unsafe {
        let mut msg = MSG::default();
        // A thread has no queue until it asks for one; a change posted before that is lost.
        let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
        let _ = told.send(GetCurrentThreadId());
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if msg.message == WM_HOTKEY {
                show(&app);
                let _ = app.emit("hotkey-pressed", ());
            } else if msg.message == WM_APP {
                while let Ok((key, reply)) = inbox.try_recv() {
                    let _ = UnregisterHotKey(None, ID);
                    let ok = key.is_none_or(|k| RegisterHotKey(None, ID, HOT_KEY_MODIFIERS(k.mods) | MOD_NOREPEAT, k.vk).is_ok());
                    let _ = reply.send(ok);
                }
            }
        }
    });
    Mutex::new(Owner { thread: thread.recv().unwrap_or(0), asks })
}

/// Use this key from now on, or none. An error says why in words fit to show.
#[tauri::command]
pub async fn hotkey_set(app: AppHandle, combo: Option<String>) -> Result<(), CommandError> {
    run_blocking(move || {
        let combo = combo.filter(|c| !c.trim().is_empty());
        let key = combo.as_deref().map(parse).transpose().map_err(CommandError::Failed)?;
        let (reply, answer) = channel();
        {
            let owner = OWNER.get_or_init(|| start(app.clone(), crate::show_main_window)).lock().unwrap();
            owner.asks.send((key, reply)).map_err(|e| CommandError::Failed(e.to_string()))?;
            unsafe { PostThreadMessageW(owner.thread, WM_APP, WPARAM(0), LPARAM(0)) }.map_err(|e| CommandError::Failed(e.to_string()))?;
        }
        if answer.recv().unwrap_or(false) {
            return Ok(());
        }
        Err(CommandError::Failed(format!(
            "Windows would not give ByteHive {}: another app or another ByteHive window has it.",
            combo.unwrap_or_default()
        )))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_key_as_people_write_it() {
        let wanted = Key { mods: MOD_WIN.0 | MOD_ALT.0, vk: 'N' as u32 };
        assert_eq!(parse("Win+Alt+N"), Ok(wanted));
        assert_eq!(parse(" win + alt + n "), Ok(wanted));
        assert_eq!(parse("Ctrl+Shift+F5"), Ok(Key { mods: MOD_CONTROL.0 | MOD_SHIFT.0, vk: 0x74 }));
        assert_eq!(parse("Win+Space").map(|k| k.vk), Ok(0x20));
    }

    #[test]
    fn says_so_when_it_cannot_read_it() {
        assert!(parse("Win+Alt+Nope").unwrap_err().contains("Win+Alt+Nope"));
        assert!(parse("Win+Alt").unwrap_err().contains("not a key"));
        assert!(parse("N").unwrap_err().contains("not a key"), "a bare key would leave every app without it");
        assert!(parse("Win+A+B").unwrap_err().contains("two keys"));
    }

    #[test]
    fn windows_gives_the_key_once() {
        // Registered from one thread, a second owner is refused: what the Settings row reports.
        let k = parse("Ctrl+Alt+Shift+F24").unwrap();
        let (a, b) = (5, 6);
        unsafe {
            RegisterHotKey(None, a, HOT_KEY_MODIFIERS(k.mods) | MOD_NOREPEAT, k.vk).expect("first owner");
            assert!(RegisterHotKey(None, b, HOT_KEY_MODIFIERS(k.mods) | MOD_NOREPEAT, k.vk).is_err());
            UnregisterHotKey(None, a).unwrap();
        }
    }
}
