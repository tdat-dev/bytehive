//! ByteHive Browser in the native app (browser/mod.rs, browserview.ts and
//! browserprefs.ts of the app before). It hosts the browser hub: agents call
//! browser tools through it, the "ByteHive for Chrome" extension answers.
//! Around it: a live view of the tab the agent on stage works in, with Stop
//! (card.rs); clicks held for your OK (asks.rs); a note when a page wants a
//! person; and Settings > Browser (settings/browser.rs).

mod asks;
mod card;
mod frame;
mod live;
mod prefs;

use std::cell::Cell;
use std::sync::Arc;

use browser_hub::profiles::ProfileRow;
use browser_hub::{Activity, Ask, BrowserInfo, Hub, HubEvent};

use super::*;

/// How often the live view and the asks look again (browserview.ts's interval).
const POLL: Duration = Duration::from_millis(1000);
/// One note per page that wants a person, per two minutes.
const BLOCKER_QUIET_MS: u64 = 120_000;

/// The last frame of an agent's tab.
pub(super) struct Shot {
    agent: String,
    image: Arc<RenderImage>,
    size: (u32, u32),
}

#[derive(Default)]
pub(super) struct BrowserWatch {
    pub(super) hub: Option<Arc<Hub>>,
    pub(super) browsers: Vec<BrowserInfo>,
    pub(super) activity: HashMap<String, Activity>,
    pub(super) asks: Vec<Ask>,
    /// Hold sending, posting, paying and deleting clicks for your OK.
    pub(super) ask_risky: bool,
    blockers: HashMap<String, u64>,
    /// The agent the live view shows, and its frame.
    shown_for: String,
    shot: Option<Shot>,
    /// Folded down to its head.
    small: bool,
    big: bool,
    busy: bool,
    /// A frame asked for while one was on its way: fetch it right after.
    again: bool,
    last_frame: u64,
    behind: bool,
    note: &'static str,
    /// The frame's width on screen, in device pixels, as last drawn.
    shown_w: Cell<f32>,
    /// Settings > Browser: every profile on this computer, read when it opens.
    pub(super) profiles: Vec<ProfileRow>,
    pub(super) profiles_asked: Cell<bool>,
    pub(super) ext_dir: String,
    /// The profile the setup steps are open for.
    pub(super) setting_up: Option<ProfileRow>,
    /// The Copy button that just copied, and when.
    pub(super) copied: Option<(&'static str, u64)>,
    _events: Option<Task<()>>,
    _poll: Option<Task<()>>,
}

impl Inbox {
    /// Start the hub once, at launch, before any agent: they find it through
    /// BYTEHIVE_HUB_FILE, which it sets for this process.
    pub(super) fn start_browser(&mut self, cx: &mut Context<Self>) {
        // Tests never publish a hub file or register a host.
        if cfg!(test) {
            return;
        }
        // A test run keeps its hub beside its data, off the owner's file; so
        // does a run beside the Tauri ByteHive, which keeps the browser until
        // the native app replaces it.
        if let Some(dir) = std::env::var_os("BYTEHIVE_NATIVE_DIR") {
            browser_hub::hub::use_own_file(std::path::PathBuf::from(dir).join("browser-hub.json"));
        } else if crate::single::other_app_running() {
            if let Some(dir) = crate::fleet::data_dir() {
                browser_hub::hub::use_own_file(dir.join("browser-hub.json"));
            }
        }
        let w = &mut self.browser;
        w.ask_risky = prefs::load_ask();
        w.ext_dir = browser_hub::extension_dir();
        let (tx, rx) = async_channel::unbounded::<HubEvent>();
        match browser_hub::start(Box::new(move |ev| {
            let _ = tx.try_send(ev);
        })) {
            Ok(hub) => {
                // The hub starts with asking on; match what you chose last time.
                hub.set_ask_risky(w.ask_risky);
                // Pages Chrome keeps from the extension are driven from Windows.
                hub.set_desktop(Box::new(crate::desktop::run));
                w.hub = Some(hub);
            }
            Err(e) => eprintln!("browser hub: {e}"),
        }
        w._events = Some(cx.spawn(async move |this, cx| {
            while let Ok(ev) = rx.recv().await {
                if this.update(cx, |this, cx| this.on_hub_event(ev, cx)).is_err() {
                    break;
                }
            }
        }));
        w._poll = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                if this.update(cx, |this, cx| this.browser_tick(cx)).is_err() {
                    break;
                }
            }
        }));
    }

    fn on_hub_event(&mut self, ev: HubEvent, cx: &mut Context<Self>) {
        match ev {
            HubEvent::Browsers(list) => {
                self.browser.browsers = list;
                self.check_setup(cx);
            }
            HubEvent::Activity(list) => {
                self.browser.activity = list.into_iter().map(|a| (a.agent.clone(), a)).collect();
                self.browser_follow(cx);
                self.browser_frame(false, cx);
            }
            HubEvent::Asks(list) => self.browser.asks = list,
            HubEvent::Blocker(b) => {
                let key = format!("{}|{}|{}", b.agent, b.kind, b.url);
                let now = now_ms();
                if now.saturating_sub(self.browser.blockers.get(&key).copied().unwrap_or(0)) >= BLOCKER_QUIET_MS {
                    self.browser.blockers.insert(key, now);
                    self.toast_for(live::blocker_note(&b.agent, &b.kind, &b.url), 6000, cx);
                }
            }
        }
        cx.notify();
    }

    /// Once a second: follow the stage, keep "3s ago" true, a frame about
    /// once a second, and let an ask the agent stopped waiting for go.
    fn browser_tick(&mut self, cx: &mut Context<Self>) {
        if self.settings.is_none() {
            self.browser.profiles_asked.set(false);
        }
        let shown = !self.browser.shown_for.is_empty();
        self.browser_follow(cx);
        self.browser_frame(false, cx);
        let now = now_ms();
        if shown || !self.browser.shown_for.is_empty() || self.browser.asks.iter().any(|a| !live::live(a, now)) {
            self.browser.asks.retain(|a| live::live(a, now));
            cx.notify();
        }
    }

    /// The hub, or the words the app before said when it was not running.
    fn hub_or_say(&mut self, cx: &mut Context<Self>) -> Option<Arc<Hub>> {
        let hub = self.browser.hub.clone();
        if hub.is_none() {
            self.toast("The browser hub isn't running.", cx);
        }
        hub
    }

    pub(super) fn set_browser_ask(&mut self, on: bool, cx: &mut Context<Self>) {
        self.browser.ask_risky = on;
        prefs::save_ask(on);
        if let Some(hub) = &self.browser.hub {
            hub.set_ask_risky(on);
        }
        cx.notify();
    }

    /// A profile being set up that just connected: say so and fold the steps.
    fn check_setup(&mut self, cx: &mut Context<Self>) {
        let Some(s) = &self.browser.setting_up else { return };
        if self.browser.browsers.iter().any(|c| same(s, c)) {
            let name = s.name.clone();
            self.browser.setting_up = None;
            self.toast(format!("{name} is connected. Your agents can use it now."), cx);
        }
    }
}

/// A profile and a connected browser are the same when the browser and the
/// signed-in email match.
pub(super) fn same(p: &ProfileRow, c: &BrowserInfo) -> bool {
    !p.email.is_empty() && p.email.eq_ignore_ascii_case(&c.email) && p.browser == c.browser
}

/// "Chrome" from "Google Chrome", "Edge" from "Microsoft Edge".
pub(super) fn short(browser: &str) -> &str {
    browser.strip_prefix("Google ").or_else(|| browser.strip_prefix("Microsoft ")).unwrap_or(browser)
}
