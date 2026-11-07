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

/// Two ids, so a new key can be taken while the old one is still held.
const IDS: [i32; 2] = [1, 2];
/// ERROR_HOTKEY_ALREADY_REGISTERED.
const TAKEN: u32 = 1409;

#[derive(Debug, PartialEq, Clone, Copy)]
struct Key {
    mods: u32,
    vk: u32,
}

/// "Win+Alt+N" as Windows wants it. At least one modifier: a bare key would be
/// taken from every app on the machine.
fn parse(combo: &str) -> Result<Key, String> {
    if combo.trim().is_empty() {
        return Err("Write a key like Win+Alt+N, or turn it off with the switch.".into());
    }
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

type Ask = (Option<Key>, Sender<windows::core::Result<()>>);

/// The thread that owns the key, how to ask it for a change, and the key it holds.
struct Owner {
    thread: u32,
    asks: Sender<Ask>,
    held: Option<String>,
}

/// Hold `want` instead of what `held` has. The new key is taken first, so a key
/// Windows refuses leaves the old one working.
fn change(held: &mut Option<(i32, Key)>, want: Option<Key>) -> windows::core::Result<()> {
    if held.map(|h| h.1) == want {
        return Ok(());
    }
    let Some(key) = want else {
        if let Some((id, _)) = held.take() {
            unsafe { UnregisterHotKey(None, id) }?;
        }
        return Ok(());
    };
    let id = if held.is_some_and(|h| h.0 == IDS[0]) { IDS[1] } else { IDS[0] };
    unsafe { RegisterHotKey(None, id, HOT_KEY_MODIFIERS(key.mods) | MOD_NOREPEAT, key.vk) }?;
    if let Some((old, _)) = held.replace((id, key)) {
        let _ = unsafe { UnregisterHotKey(None, old) };
    }
    Ok(())
}

/// Why Windows said no, and what still works.
fn refusal(combo: &str, e: &windows::core::Error, held: Option<&str>) -> String {
    let why = if e.code().0 as u32 & 0xFFFF == TAKEN {
        format!("{combo} is already taken, by another app or by Windows itself.")
    } else {
        format!("Windows would not register {combo}: {}", e.message())
    };
    match held {
        Some(old) => format!("{why} {old} still works."),
        None => why,
    }
}

static OWNER: OnceLock<Mutex<Owner>> = OnceLock::new();

fn start(app: AppHandle, show: fn(&AppHandle)) -> Mutex<Owner> {
    let (asks, inbox) = channel::<Ask>();
    let (told, thread) = channel();
    std::thread::spawn(move || unsafe {
        let mut msg = MSG::default();
        let mut held = None;
        // A thread has no queue until it asks for one; a change posted before that is lost.
        let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
        let _ = told.send(GetCurrentThreadId());
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if msg.message == WM_HOTKEY {
                show(&app);
                let _ = app.emit("hotkey-pressed", ());
            } else if msg.message == WM_APP {
                while let Ok((key, reply)) = inbox.try_recv() {
                    let _ = reply.send(change(&mut held, key));
                }
            }
        }
    });
    Mutex::new(Owner { thread: thread.recv().unwrap_or(0), asks, held: None })
}

/// Use this key from now on, or none when `combo` is None. An error says why in
/// words fit to show, and leaves the key that was held working.
#[tauri::command]
pub async fn hotkey_set(app: AppHandle, combo: Option<String>) -> Result<(), CommandError> {
    run_blocking(move || {
        let key = combo.as_deref().map(parse).transpose().map_err(CommandError::Failed)?;
        let (reply, answer) = channel();
        let mut owner = OWNER.get_or_init(|| start(app.clone(), crate::show_main_window)).lock().unwrap();
        let gone = |e: String| CommandError::Failed(format!("The key could not be changed: {e}"));
        owner.asks.send((key, reply)).map_err(|e| gone(e.to_string()))?;
        unsafe { PostThreadMessageW(owner.thread, WM_APP, WPARAM(0), LPARAM(0)) }.map_err(|e| gone(e.to_string()))?;
        match answer.recv().map_err(|e| gone(e.to_string()))? {
            Ok(()) => {
                owner.held = combo;
                Ok(())
            }
            Err(e) => Err(CommandError::Failed(refusal(combo.as_deref().unwrap_or(""), &e, owner.held.as_deref()))),
        }
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
    fn an_empty_field_is_an_error_not_a_quiet_off() {
        assert!(parse("").unwrap_err().contains("turn it off with the switch"));
        assert!(parse("   ").unwrap_err().contains("turn it off with the switch"));
    }

    #[test]
    fn a_refused_key_leaves_the_old_one_working() {
        let old = parse("Ctrl+Alt+Shift+F24").unwrap();
        let new = parse("Ctrl+Alt+Shift+F23").unwrap();
        let free = parse("Ctrl+Alt+Shift+F22").unwrap();
        let hold = |id, k: Key| unsafe { RegisterHotKey(None, id, HOT_KEY_MODIFIERS(k.mods) | MOD_NOREPEAT, k.vk) };
        let mut held = None;
        change(&mut held, Some(old)).unwrap();
        hold(9, new).expect("someone else has it");

        let e = change(&mut held, Some(new)).unwrap_err();
        assert_eq!(held.map(|h| h.1), Some(old));
        assert!(hold(8, old).is_err(), "the old key is still held");
        let words = refusal("Win+D", &e, Some("Win+Alt+N"));
        assert_eq!(words, "Win+D is already taken, by another app or by Windows itself. Win+Alt+N still works.");

        change(&mut held, Some(free)).unwrap();
        hold(8, old).expect("moved on: the old key is free");
        change(&mut held, Some(free)).unwrap();
        change(&mut held, None).unwrap();
        hold(7, free).expect("turned off: free again");
        unsafe {
            for id in [7, 8, 9] {
                UnregisterHotKey(None, id).unwrap();
            }
        }
    }
}
