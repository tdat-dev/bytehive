//! One ByteHive at a time. A second start (an installer that opened it as
//! well, a double click on the shortcut) brings the running one to the
//! front and quits. Builds from different places (the installed app, one
//! from target\) each count as their own, so a developer's never blocks the
//! real one.

/// True when this exe already runs: it was shown, quit this one.
#[cfg(windows)]
pub fn another_is_running() -> bool {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    let exe = std::env::current_exe().map(|p| p.to_string_lossy().to_lowercase()).unwrap_or_default();
    let name: Vec<u16> = format!("Local\\ByteHive-{:x}", fnv(&exe)).encode_utf16().chain([0]).collect();
    // The handle stays open for the life of the process: that is the lock.
    let held = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    if held.is_null() || unsafe { GetLastError() } != ERROR_ALREADY_EXISTS {
        return false;
    }
    if let Some(hwnd) = window_of_twin(&exe) {
        crate::tray::show_window(hwnd);
    }
    true
}

#[cfg(not(windows))]
pub fn another_is_running() -> bool {
    false
}

/// A stable hash of the exe path (FNV-1a), for the lock's name.
fn fnv(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3))
}

/// The main window of the other process running this same exe.
#[cfg(windows)]
fn window_of_twin(exe: &str) -> Option<isize> {
    use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM};
    use windows_sys::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows_sys::Win32::UI::WindowsAndMessaging::{EnumWindows, GetClassNameW, GetWindowThreadProcessId};

    struct Search {
        exe: String,
        me: u32,
        found: isize,
    }
    unsafe extern "system" fn each(hwnd: HWND, lp: LPARAM) -> i32 {
        let s = unsafe { &mut *(lp as *mut Search) };
        let mut class = [0u16; 64];
        let n = unsafe { GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32) };
        // winit's windows; the tray's hidden helper windows are skipped.
        if String::from_utf16_lossy(&class[..n.max(0) as usize]) != "Window Class" {
            return 1;
        }
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        if pid == 0 || pid == s.me {
            return 1;
        }
        let proc = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if proc.is_null() {
            return 1;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = unsafe { QueryFullProcessImageNameW(proc, 0, buf.as_mut_ptr(), &mut len) };
        unsafe { CloseHandle(proc) };
        if ok != 0 && String::from_utf16_lossy(&buf[..len as usize]).to_lowercase() == s.exe {
            s.found = hwnd as isize;
            return 0;
        }
        1
    }
    let mut s = Search { exe: exe.to_string(), me: std::process::id(), found: 0 };
    unsafe { EnumWindows(Some(each), &mut s as *mut Search as LPARAM) };
    (s.found != 0).then_some(s.found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lock_name_follows_the_exe_path() {
        assert_eq!(fnv("c:\\a\\bytehive.exe"), fnv("c:\\a\\bytehive.exe"));
        assert_ne!(fnv("c:\\a\\bytehive.exe"), fnv("d:\\maestro\\target\\release\\native-shell.exe"));
    }
}
