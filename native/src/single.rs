//! One ByteHive per data folder. A second launch brings the first window
//! forward and quits, so two copies never drive the same agents. The test
//! folder (BYTEHIVE_NATIVE_DIR) counts as its own, so a test run can sit
//! beside the owner's app.

use std::path::Path;

use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible, SW_RESTORE, SW_SHOW, SetForegroundWindow, ShowWindow,
};

const PID_FILE: &str = "instance.pid";

/// The mutex name for a data folder: one per folder, case and slashes aside.
pub fn lock_name(dir: &Path) -> String {
    let path = dir.to_string_lossy().to_lowercase().replace(['\\', '/', ':'], "_");
    format!("Local\\bytehive-native-{}", path.trim_end_matches('_'))
}

/// True when this is the only copy for its data folder. Otherwise the first
/// copy's window is brought forward and the caller should quit.
pub fn claim() -> bool {
    let Some(dir) = crate::fleet::data_dir() else { return true };
    let name: Vec<u16> = lock_name(&dir).encode_utf16().chain(Some(0)).collect();
    // The handle is kept for the life of the process: Windows drops the
    // mutex when it ends, crash or not.
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    let taken = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    if handle.is_null() || !taken {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join(PID_FILE), std::process::id().to_string());
        return true;
    }
    if let Some(pid) = std::fs::read_to_string(dir.join(PID_FILE)).ok().and_then(|s| s.trim().parse::<u32>().ok()) {
        raise(pid);
    }
    false
}

/// Show and raise a process's ByteHive window, hidden in the tray or not.
/// A process the person just launched may hand the foreground on, so this
/// works from a second launch; the tray's own click works too.
pub fn raise(pid: u32) {
    unsafe extern "system" fn each(hwnd: HWND, lparam: LPARAM) -> i32 {
        let mut owner = 0u32;
        let mut title = [0u16; 16];
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut owner);
            if owner != lparam as u32 {
                return 1;
            }
            let n = GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32);
            if String::from_utf16_lossy(&title[..n.max(0) as usize]) != "ByteHive" {
                return 1;
            }
            if IsWindowVisible(hwnd) == 0 {
                ShowWindow(hwnd, SW_SHOW);
            }
            if IsIconic(hwnd) != 0 {
                ShowWindow(hwnd, SW_RESTORE);
            }
            SetForegroundWindow(hwnd);
        }
        1
    }
    unsafe {
        EnumWindows(Some(each), pid as LPARAM);
    }
}

#[cfg(test)]
mod tests {
    use super::lock_name;

    #[test]
    fn one_lock_per_folder() {
        let a = lock_name(std::path::Path::new(r"C:\Users\Me\.bytehive\native\"));
        assert_eq!(a, lock_name(std::path::Path::new("c:/users/me/.bytehive/native")));
        assert_ne!(a, lock_name(std::path::Path::new(r"D:\Temp\native-data")));
        assert!(a.starts_with(r"Local\bytehive-native-") && !a[6..].contains('\\'));
    }
}
