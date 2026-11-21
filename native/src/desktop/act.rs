//! Acting on one element the way assistive tools do: press it, tick it, pick
//! it, open it, or set its text, without moving the mouse or needing the
//! window in front. What has none of those is clicked for real (keys.rs).

use serde_json::Value;
use windows::Win32::UI::Accessibility::*;
use windows::core::BSTR;

use super::tree::Node;

fn gone(e: windows::core::Error) -> String {
    format!("That element did not answer ({e}); the page may have changed: read_page again.")
}

/// Press, toggle, select or open the element. Err(None) when it offers none of
/// those, so the caller clicks it for real.
pub fn press(node: &Node) -> Result<&'static str, Option<String>> {
    unsafe {
        if let Ok(p) = node.el.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId) {
            let _ = node.el.SetFocus();
            return p.Invoke().map(|_| "Pressed").map_err(|e| Some(gone(e)));
        }
        if let Ok(p) = node.el.GetCurrentPatternAs::<IUIAutomationTogglePattern>(UIA_TogglePatternId) {
            return p.Toggle().map(|_| "Switched").map_err(|e| Some(gone(e)));
        }
        if let Ok(p) = node.el.GetCurrentPatternAs::<IUIAutomationSelectionItemPattern>(UIA_SelectionItemPatternId) {
            return p.Select().map(|_| "Selected").map_err(|e| Some(gone(e)));
        }
        if let Ok(p) = node.el.GetCurrentPatternAs::<IUIAutomationExpandCollapsePattern>(UIA_ExpandCollapsePatternId) {
            let open = p.CurrentExpandCollapseState().map(|s| s != ExpandCollapseState_Collapsed).unwrap_or(false);
            let done = if open { p.Collapse() } else { p.Expand() };
            return done.map(|_| if open { "Closed" } else { "Opened" }).map_err(|e| Some(gone(e)));
        }
    }
    Err(None)
}

/// Fill a field: text into a text box or combo box, true or false into a checkbox
/// or switch, an option's name into a list that has no text.
pub fn set_value(node: &Node, value: &Value) -> Result<String, String> {
    unsafe {
        if let Some(want) = value.as_bool() {
            // No focus first: moving focus onto Chrome's switch flipped it back
            // (measured on chrome://extensions' Developer mode).
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
        let _ = node.el.SetFocus();
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
