//! Acting on elements the way assistive tools do: press, tick, pick, open,
//! set or add text, scroll. None of it moves the mouse or needs the window in
//! front (not even focus: UI Automation's SetFocus brings the whole window
//! forward, measured), so the person keeps working while an agent does. What offers
//! none of those falls back to real input (keys.rs).

use serde_json::Value;
use windows::Win32::UI::Accessibility::*;
use windows::core::BSTR;

use super::tree::Node;

fn gone(e: windows::core::Error) -> String {
    format!("That element did not answer ({e}); the page may have changed: read_page again.")
}

/// Press, toggle, select or open an element. Err(None) when it offers none of
/// those, so the caller clicks it for real.
pub fn press(el: &IUIAutomationElement) -> Result<&'static str, Option<String>> {
    unsafe {
        if let Ok(p) = el.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId) {
            return p.Invoke().map(|_| "Pressed").map_err(|e| Some(gone(e)));
        }
        // No focus first: moving focus onto Chrome's switch flipped it back
        // (measured on chrome://extensions' Developer mode).
        if let Ok(p) = el.GetCurrentPatternAs::<IUIAutomationTogglePattern>(UIA_TogglePatternId) {
            return p.Toggle().map(|_| "Switched").map_err(|e| Some(gone(e)));
        }
        if let Ok(p) = el.GetCurrentPatternAs::<IUIAutomationSelectionItemPattern>(UIA_SelectionItemPatternId) {
            return p.Select().map(|_| "Selected").map_err(|e| Some(gone(e)));
        }
        if let Ok(p) = el.GetCurrentPatternAs::<IUIAutomationExpandCollapsePattern>(UIA_ExpandCollapsePatternId) {
            let open = p.CurrentExpandCollapseState().map(|s| s != ExpandCollapseState_Collapsed).unwrap_or(false);
            let done = if open { p.Collapse() } else { p.Expand() };
            return done.map(|_| if open { "Closed" } else { "Opened" }).map_err(|e| Some(gone(e)));
        }
        // A text box: clicking it means "type here next".
        if writable(el) {
            return Ok("Selected the field");
        }
    }
    Err(None)
}

/// Whether the element takes text through its Value pattern.
pub fn writable(el: &IUIAutomationElement) -> bool {
    unsafe { el.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId).is_ok_and(|p| !p.CurrentIsReadOnly().map(|b| b.as_bool()).unwrap_or(true)) }
}

/// Type into a field without keys: its text plus `text`. False when it takes none.
pub fn append(el: &IUIAutomationElement, text: &str) -> Result<bool, String> {
    unsafe {
        let Ok(p) = el.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) else { return Ok(false) };
        if p.CurrentIsReadOnly().map(|b| b.as_bool()).unwrap_or(true) {
            return Ok(false);
        }
        let now = p.CurrentValue().map(|v| v.to_string()).unwrap_or_default();
        p.SetValue(&BSTR::from(format!("{now}{text}").as_str())).map_err(gone)?;
        Ok(true)
    }
}

/// Scroll the nearest scrollable box around `el` by `notches` (down or right
/// when positive). False when nothing around it scrolls.
pub fn scroll(uia: &IUIAutomation, el: &IUIAutomationElement, notches: i32, sideways: bool) -> Result<bool, String> {
    unsafe {
        let walker = uia.ControlViewWalker().map_err(gone)?;
        let mut at = Some(el.clone());
        while let Some(e) = at {
            if let Ok(p) = e.GetCurrentPatternAs::<IUIAutomationScrollPattern>(UIA_ScrollPatternId) {
                let can = if sideways { p.CurrentHorizontallyScrollable() } else { p.CurrentVerticallyScrollable() };
                if can.map(|b| b.as_bool()).unwrap_or(false) {
                    let step = if notches >= 0 { ScrollAmount_SmallIncrement } else { ScrollAmount_SmallDecrement };
                    for _ in 0..notches.unsigned_abs().max(1) {
                        let r = if sideways { p.Scroll(step, ScrollAmount_NoAmount) } else { p.Scroll(ScrollAmount_NoAmount, step) };
                        if r.is_err() {
                            break;
                        }
                    }
                    return Ok(true);
                }
            }
            at = walker.GetParentElement(&e).ok();
        }
        Ok(false)
    }
}

/// Fill a field: text into a text box or combo box, true or false into a checkbox
/// or switch, an option's name into a list that has no text.
pub fn set_value(node: &Node, value: &Value) -> Result<String, String> {
    unsafe {
        if let Some(want) = value.as_bool() {
            // No focus here either (see press).
            let p = node.el.GetCurrentPatternAs::<IUIAutomationTogglePattern>(UIA_TogglePatternId).map_err(|_| format!("{} \"{}\" is not something to switch on or off.", node.kind, node.name))?;
            for _ in 0..3 {
                let on = p.CurrentToggleState().map_err(gone)? == ToggleState_On;
                if on == want {
                    return Ok(format!("{} \"{}\" is {}.", node.kind, node.name, if want { "on" } else { "off" }));
                }
                p.Toggle().map_err(gone)?;
                std::thread::sleep(std::time::Duration::from_millis(400));
            }
            return Err(format!("{} \"{}\" would not switch.", node.kind, node.name));
        }
        let text = value.as_str().map(str::to_string).unwrap_or_else(|| value.to_string());
        if let Ok(p) = node.el.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) {
            if !p.CurrentIsReadOnly().map(|b| b.as_bool()).unwrap_or(false) {
                p.SetValue(&BSTR::from(text.as_str())).map_err(gone)?;
                return Ok(format!("Set {} \"{}\".", node.kind, node.name));
            }
        }
        // A list without text: open it and pick the option by its name.
        if let Ok(p) = node.el.GetCurrentPatternAs::<IUIAutomationExpandCollapsePattern>(UIA_ExpandCollapsePatternId) {
            let _ = p.Expand();
            std::thread::sleep(std::time::Duration::from_millis(250));
            let uia: IUIAutomation = windows::Win32::System::Com::CoCreateInstance(&CUIAutomation, None, windows::Win32::System::Com::CLSCTX_INPROC_SERVER).map_err(gone)?;
            let cond = uia.CreateTrueCondition().map_err(gone)?;
            let items = node.el.FindAll(TreeScope_Descendants, &cond).map_err(gone)?;
            for i in 0..items.Length().map_err(gone)? {
                let Ok(item) = items.GetElement(i) else { continue };
                if item.CurrentName().map(|n| n.to_string().trim().eq_ignore_ascii_case(text.trim())).unwrap_or(false) {
                    if let Ok(s) = item.GetCurrentPatternAs::<IUIAutomationSelectionItemPattern>(UIA_SelectionItemPatternId) {
                        s.Select().map_err(gone)?;
                        let _ = p.Collapse();
                        return Ok(format!("Picked \"{text}\" in {} \"{}\".", node.kind, node.name));
                    }
                }
            }
            let _ = p.Collapse();
            return Err(format!("{} \"{}\" has no option \"{text}\".", node.kind, node.name));
        }
    }
    Err(format!("{} \"{}\" takes no text here: click it, then type.", node.kind, node.name))
}
