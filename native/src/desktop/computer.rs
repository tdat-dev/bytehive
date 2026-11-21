//! The `computer` tool on a page Chrome keeps from extensions. Every step tries
//! UI Automation first, which needs neither the mouse nor the window in front:
//! a point is pressed through the element under it, text goes into the field
//! the agent picked, scrolling goes to the box that scrolls, Escape presses a
//! dialog's Cancel. Only what none of that covers borrows real input
//! (keys::borrow: once the person is idle, and their window and pointer are
//! given back).

use std::collections::HashMap;
use std::time::Duration;

use serde_json::Value;
use windows::Win32::Foundation::{HWND as WinHwnd, POINT};
use windows::Win32::UI::Accessibility::*;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

use super::worker::{Job, Page, guard, index};
use super::{act, keys, shot, tree, words};

fn pid_of(hwnd: isize) -> u32 {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd as HWND, &mut pid) };
    pid
}

/// The element at a screen point, when it is this Chrome's: a window covering
/// Chrome there would give another program's element.
fn element_at(uia: &IUIAutomation, x: i32, y: i32, pid: u32) -> Option<IUIAutomationElement> {
    unsafe {
        let e = uia.ElementFromPoint(POINT { x, y }).ok()?;
        (e.CurrentProcessId().ok()? as u32 == pid).then_some(e)
    }
}

fn name_of(e: &IUIAutomationElement) -> String {
    unsafe { e.CurrentName().map(|n| n.to_string()).unwrap_or_default() }
}

/// Close a dialog Chrome opened with its Cancel (or Close, No) button.
fn dismiss(uia: &IUIAutomation, hwnd: isize) -> Option<String> {
    unsafe {
        let cond = uia.CreateTrueCondition().ok()?;
        for d in tree::dialogs(hwnd) {
            let root = uia.ElementFromHandle(WinHwnd(d as *mut _)).ok()?;
            let all = root.FindAll(TreeScope_Descendants, &cond).ok()?;
            for i in 0..all.Length().unwrap_or(0) {
                let Ok(e) = all.GetElement(i) else { continue };
                let name = name_of(&e);
                if matches!(name.as_str(), "Cancel" | "Close" | "No") {
                    if let Ok(p) = e.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId) {
                        if p.Invoke().is_ok() {
                            return Some(format!("Closed the dialog with {name}."));
                        }
                    }
                }
            }
        }
        None
    }
}

pub(super) fn run(uia: &IUIAutomation, pages: &mut HashMap<String, Page>, job: &Job, hwnd: isize, origin: (i32, i32), url: &str) -> Result<Value, String> {
    let a = &job.args;
    let act = a["action"].as_str().unwrap_or("");
    let pid = pid_of(hwnd);
    let field = pages.get(&job.agent).filter(|p| p.hwnd == hwnd).and_then(|p| p.field.map(|i| (p.nodes[i].el.clone(), p.nodes[i].kind, p.nodes[i].name.clone())));
    match act {
        "screenshot" => return shot::screenshot(hwnd),
        "wait" => {
            std::thread::sleep(Duration::from_secs_f64(a["duration"].as_f64().unwrap_or(1.0).clamp(0.0, 30.0)));
            return Ok(words::text("Waited."));
        }
        "type" => {
            let text = a["text"].as_str().unwrap_or("");
            if let Some((el, kind, name)) = &field {
                if keys::settle(hwnd, || act::append(el, text))? {
                    return Ok(words::text(format!("Typed into {kind} \"{name}\".")));
                }
            }
            keys::borrow(hwnd, false, || keys::type_text(text))?;
            return Ok(words::text("Typed."));
        }
        "key" => {
            let spec = a["text"].as_str().unwrap_or("");
            let combos = keys::parse(spec)?;
            if matches!(spec.trim().to_ascii_lowercase().as_str(), "escape" | "esc") {
                if let Some(done) = dismiss(uia, hwnd) {
                    return Ok(words::text(done));
                }
            }
            if combos.iter().any(|c| c.contains(&0x0D) || c.contains(&0x20)) {
                let focused = unsafe { uia.GetFocusedElement().map(|e| name_of(&e)).unwrap_or_default() };
                if let Some(h) = guard(a, &format!("Press {spec} on \"{focused}\""), &focused, url) {
                    return Ok(h);
                }
            }
            keys::borrow(hwnd, false, || keys::press(&combos))?;
            return Ok(words::text(format!("Pressed {spec}.")));
        }
        _ => {}
    }
    // Everything else has a place: a ref, a point of the screenshot, or (to scroll) the middle.
    let target = match a["ref"].as_str() {
        Some(r) => Some(index(pages, &job.agent, hwnd, r)?),
        None => None,
    };
    let node = target.and_then(|i| pages.get(&job.agent).map(|p| (p.nodes[i].el.clone(), p.nodes[i].center(), p.nodes[i].name.clone(), p.nodes[i].kind)));
    let coord = a["coordinate"].as_array().filter(|c| c.len() == 2).map(|c| (origin.0 + c[0].as_f64().unwrap_or(0.0) as i32, origin.1 + c[1].as_f64().unwrap_or(0.0) as i32));
    let r = shot::rect(hwnd);
    let middle = ((r.left + r.right) / 2, (r.top + r.bottom) / 2);
    let (x, y) = match (coord, &node) {
        (Some(p), _) => p,
        (None, Some(n)) => n.1,
        (None, None) if act == "scroll" => middle,
        _ => return Err("Give a ref (from read_page) or a coordinate [x, y] (from a screenshot).".into()),
    };
    let el = match (&node, coord) {
        (Some(n), None) => Some(n.0.clone()),
        _ => element_at(uia, x, y, pid),
    };
    match act {
        "hover" => {
            keys::borrow(hwnd, true, || keys::move_to(x, y))?;
            Ok(words::text("Hovering."))
        }
        "scroll" => {
            let n = a["scroll_amount"].as_i64().unwrap_or(3) as i32;
            let dir = a["scroll_direction"].as_str().unwrap_or("down");
            let notches = if matches!(dir, "up" | "left") { -n } else { n };
            let sideways = matches!(dir, "left" | "right");
            if let Some(e) = &el {
                if act::scroll(uia, e, notches, sideways)? {
                    return Ok(words::text(format!("Scrolled {dir}.")));
                }
            }
            keys::borrow(hwnd, false, || keys::wheel(x, y, notches, sideways))?;
            Ok(words::text(format!("Scrolled {dir}.")))
        }
        "left_click" | "right_click" | "double_click" | "triple_click" => {
            let name = node.as_ref().map(|n| n.2.clone()).or_else(|| el.as_ref().map(name_of)).unwrap_or_default();
            if act == "left_click" {
                if let Some(h) = guard(a, &format!("Click \"{name}\""), &name, url) {
                    return Ok(h);
                }
                if let Some(e) = &el {
                    match act::press(e) {
                        Ok(how) => {
                            if act::writable(e) {
                                if let (Some(i), Some(p)) = (target, pages.get_mut(&job.agent)) {
                                    p.field = Some(i);
                                }
                            }
                            std::thread::sleep(Duration::from_millis(400));
                            let kind = node.as_ref().map(|n| n.3).unwrap_or("element");
                            return Ok(words::text(format!("{how} {kind} \"{name}\".")));
                        }
                        Err(Some(e)) => return Err(e),
                        Err(None) => {}
                    }
                }
            }
            let clicks = match act {
                "double_click" => 2,
                "triple_click" => 3,
                _ => 1,
            };
            keys::borrow(hwnd, false, || keys::click(x, y, act == "right_click", clicks))?;
            std::thread::sleep(Duration::from_millis(400));
            Ok(words::text(format!("{act} on \"{name}\" at {},{}.", x - origin.0, y - origin.1)))
        }
        other => Err(format!("Unknown action \"{other}\".")),
    }
}
