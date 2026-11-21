//! Finding the agent's Chrome window and reading it the way a screen reader
//! does (UI Automation): every button, field and link of the page, of Chrome's
//! own toolbar, and of any dialog Chrome opened (the folder chooser of "Load
//! unpacked", a "Remove extension?" box), each with where it is on screen.

use serde_json::Value;
use windows::Win32::Foundation::HWND as WinHwnd;
use windows::Win32::UI::Accessibility::*;
use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT};
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::{EnumWindows, GetClassNameW, GetForegroundWindow, GetWindow, GetWindowRect, GetWindowTextW, IsWindowVisible, GW_OWNER};

/// One element of the window, with its place on screen (left, top, right, bottom).
pub struct Node {
    pub el: IUIAutomationElement,
    pub kind: &'static str,
    pub name: String,
    pub rect: (i32, i32, i32, i32),
    pub extra: String,
}

impl Node {
    pub fn center(&self) -> (i32, i32) {
        ((self.rect.0 + self.rect.2) / 2, (self.rect.1 + self.rect.3) / 2)
    }
}

fn wide(buf: &[u16], n: i32) -> String {
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

/// Every visible top-level window: (hwnd, class, title, owner).
fn windows() -> Vec<(isize, String, String, isize)> {
    unsafe extern "system" fn each(h: HWND, out: LPARAM) -> windows_sys::core::BOOL {
        let list = unsafe { &mut *(out as *mut Vec<(isize, String, String, isize)>) };
        if unsafe { IsWindowVisible(h) } != 0 {
            let mut c = [0u16; 128];
            let mut t = [0u16; 512];
            let cn = unsafe { GetClassNameW(h, c.as_mut_ptr(), c.len() as i32) };
            let tn = unsafe { GetWindowTextW(h, t.as_mut_ptr(), t.len() as i32) };
            list.push((h as isize, wide(&c, cn), wide(&t, tn), unsafe { GetWindow(h, GW_OWNER) } as isize));
        }
        1
    }
    let mut list: Vec<(isize, String, String, isize)> = Vec::new();
    unsafe { EnumWindows(Some(each), &mut list as *mut _ as LPARAM) };
    list
}

/// The Chrome window showing the agent's tab: its title starts with the tab's
/// title (the extension brought the tab forward); among several, the one in
/// front, else the one whose place matches what the extension said.
pub fn find_window(desk: &Value) -> Result<isize, String> {
    let title = desk["title"].as_str().unwrap_or("").trim().to_string();
    let all = windows();
    let chrome: Vec<_> = all.iter().filter(|w| w.1 == "Chrome_WidgetWin_1" && !w.2.is_empty() && w.3 == 0).collect();
    let mut hits: Vec<isize> = chrome.iter().filter(|w| !title.is_empty() && w.2.starts_with(&title)).map(|w| w.0).collect();
    if hits.is_empty() {
        return Err(format!("ByteHive could not find the Chrome window showing \"{title}\". Is the tab still open?"));
    }
    let front = unsafe { GetForegroundWindow() } as isize;
    if hits.len() > 1 {
        if hits.contains(&front) {
            return Ok(front);
        }
        let b = &desk["bounds"];
        let want = (b["left"].as_f64().unwrap_or(0.0), b["top"].as_f64().unwrap_or(0.0));
        hits.sort_by_key(|h| {
            let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            let scale = unsafe { GetDpiForWindow(*h as HWND) }.max(96) as f64 / 96.0;
            unsafe { GetWindowRect(*h as HWND, &mut r) };
            ((r.left as f64 - want.0 * scale).abs() + (r.top as f64 - want.1 * scale).abs()) as i64
        });
    }
    Ok(hits[0])
}

/// Dialogs the window opened (file and folder choosers, message boxes).
pub fn dialogs(hwnd: isize) -> Vec<isize> {
    windows().into_iter().filter(|w| w.3 == hwnd).map(|w| w.0).collect()
}

fn kind_of(id: i32, all: bool) -> Option<&'static str> {
    Some(match id {
        50000 => "button",
        50002 => "checkbox",
        50003 => "combobox",
        50004 => "textbox",
        50005 => "link",
        50007 => "listitem",
        50011 => "menuitem",
        50013 => "radio",
        50015 => "slider",
        50016 => "spinner",
        50019 => "tab",
        50024 => "treeitem",
        50029 => "dataitem",
        50031 => "splitbutton",
        50020 if all => "text",
        50006 if all => "image",
        _ => return None,
    })
}

/// Chrome's own minimise, maximise and close: never an agent's to press.
const CAPTION: [&str; 4] = ["Minimize", "Maximize", "Restore", "Close"];

/// Every element of the windows, dialogs first (a dialog blocks the page).
pub fn read(uia: &IUIAutomation, roots: &[isize], all: bool) -> Result<Vec<Node>, String> {
    let fail = |e: windows::core::Error| format!("UI Automation: {e}");
    let mut out = Vec::new();
    unsafe {
        let cache = uia.CreateCacheRequest().map_err(fail)?;
        for p in [UIA_NamePropertyId, UIA_ControlTypePropertyId, UIA_BoundingRectanglePropertyId, UIA_IsOffscreenPropertyId, UIA_IsEnabledPropertyId] {
            cache.AddProperty(p).map_err(fail)?;
        }
        let cond = uia.ControlViewCondition().map_err(fail)?;
        for root in roots {
            let el = uia.ElementFromHandle(WinHwnd(*root as *mut _)).map_err(fail)?;
            let arr = el.FindAllBuildCache(TreeScope_Descendants, &cond, &cache).map_err(fail)?;
            for i in 0..arr.Length().map_err(fail)? {
                let Ok(e) = arr.GetElement(i) else { continue };
                let Some(kind) = e.CachedControlType().ok().and_then(|t| kind_of(t.0, all)) else { continue };
                let name = e.CachedName().map(|b| b.to_string()).unwrap_or_default().trim().to_string();
                let r = e.CachedBoundingRectangle().unwrap_or_default();
                let mut extra = String::new();
                if matches!(kind, "textbox" | "combobox") {
                    if let Ok(v) = e.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId).and_then(|p| p.CurrentValue()) {
                        let v = v.to_string();
                        if !v.is_empty() {
                            extra.push_str(&format!(" value=\"{}\"", v.chars().take(200).collect::<String>()));
                        }
                    }
                }
                if kind == "checkbox" {
                    if let Ok(s) = e.GetCurrentPatternAs::<IUIAutomationTogglePattern>(UIA_TogglePatternId).and_then(|p| p.CurrentToggleState()) {
                        extra.push_str(if s == ToggleState_On { " (checked)" } else { " (not checked)" });
                    }
                }
                if name.is_empty() && extra.is_empty() && !matches!(kind, "textbox" | "combobox") {
                    continue;
                }
                if kind == "button" && CAPTION.contains(&name.as_str()) {
                    continue;
                }
                if !e.CachedIsEnabled().map(|b| b.as_bool()).unwrap_or(true) {
                    extra.push_str(" (disabled)");
                }
                if e.CachedIsOffscreen().map(|b| b.as_bool()).unwrap_or(false) {
                    extra.push_str(" (off screen: scroll, or use the ref)");
                }
                out.push(Node { el: e, kind, name, rect: (r.left, r.top, r.right, r.bottom), extra });
            }
        }
    }
    Ok(out)
}
