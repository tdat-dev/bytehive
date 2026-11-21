//! Pages Chrome keeps from every extension (chrome://, the Web Store, other
//! extensions' pages, Chrome's own dialogs): the extension brings the tab to the
//! front and answers `{ desktop: {...} }` instead of failing, and the step is
//! done from Windows by the app's hook (UI Automation and real input), which
//! Chrome treats like a person. The hook runs off the hub's lock; its answer
//! comes back through `on_message` as if the browser had sent it, so holding a
//! risky step for the user's OK works the same way.

use std::sync::{Arc, Weak};

use serde_json::{json, Value};

use super::{Hub, Inner, Pending};

/// The app's hook: (agent, tool, args, what the extension said about the tab)
/// to a tool result, or the reason it could not be done.
pub type DesktopFn = Box<dyn Fn(&str, &str, &Value, &Value) -> Result<Value, String> + Send + Sync>;

pub(super) const NO_DESKTOP: &str = "Chrome keeps this page from extensions, and this ByteHive can't drive it from Windows. Ask the user to do this step.";

impl Hub {
    /// Let the app do steps on pages Chrome keeps from extensions.
    pub fn set_desktop(&self, f: DesktopFn) {
        let _ = self.desktop.set(f);
    }

    pub(super) fn set_me(self: &Arc<Self>) {
        let _ = self.me.set(Arc::downgrade(self));
    }

    fn me(&self) -> Option<Arc<Hub>> {
        self.desktop.get()?;
        self.me.get().and_then(Weak::upgrade)
    }

    /// Hand one step to the desktop hook. False when there is none: the caller
    /// then answers with NO_DESKTOP.
    pub(super) fn to_desktop(&self, inner: &mut Inner, browser: u64, hid: u64, p: Pending, desk: Value) -> Result<(), Pending> {
        let Some(me) = self.me() else { return Err(p) };
        let agent = p.agent.clone();
        let tool = p.call["tool"].as_str().unwrap_or("").to_string();
        let args = p.call["args"].clone();
        inner.pending.insert(hid, p);
        std::thread::spawn(move || {
            let Some(run) = me.desktop.get() else { return };
            let msg = match run(&agent, &tool, &args, &desk) {
                Ok(result) => json!({ "type": "result", "id": hid, "result": result }),
                Err(e) => json!({ "type": "error", "id": hid, "error": e }),
            };
            me.on_message(browser, &msg.to_string());
        });
        Ok(())
    }
}
