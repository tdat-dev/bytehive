//! Maestro Browser in the native shell: hosts the browser hub (agents call
//! browser tools through it; the "Maestro for Chrome" extension answers),
//! shows a live view of the tab the agent on stage works in with Stop, asks
//! you before a held click (sending, posting, paying, deleting), and says
//! when a page wants a person (a login or a CAPTCHA).

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine as _;
use browser_hub::{Activity, Ask, BrowserInfo, Hub, HubEvent};
use eframe::egui::{self, Color32, RichText, Stroke};

use crate::app::{now_ms, ShellApp};
use crate::browser_live::{ask_line, frame_of, host, shows, what_line, BACK_FRAME_MS, FRAME_MS, HOLD_MS};
use crate::theme::{AMBER, DEL, INK3, PRIMARY, PRIMARY_INK, SHEET, TEXT};

/// A decoded frame of an agent's tab, or why there is none.
type Frame = (String, Result<(egui::ColorImage, bool), String>);

pub struct BrowserWatch {
    pub hub: Option<Arc<Hub>>,
    pub error: Option<String>,
    events: Option<Receiver<HubEvent>>,
    pub browsers: Vec<BrowserInfo>,
    pub activity: HashMap<String, Activity>,
    pub asks: Vec<Ask>,
    frame_tx: Sender<Frame>,
    frame_rx: Receiver<Frame>,
    texture: Option<(String, egui::TextureHandle)>,
    busy: bool,
    last_frame: Option<Instant>,
    behind: bool,
    pub note: String,
    pub small: bool,
    pub big: bool,
    started: bool,
    /// Settings > Browser.
    pub form: crate::browser_prefs::BrowserForm,
    profiles_tx: Sender<Vec<browser_hub::profiles::ProfileRow>>,
    profiles_rx: Receiver<Vec<browser_hub::profiles::ProfileRow>>,
    profiles_asked: bool,
    ask_risky: Option<bool>,
}

impl Default for BrowserWatch {
    fn default() -> Self {
        let (frame_tx, frame_rx) = channel();
        let (profiles_tx, profiles_rx) = channel();
        BrowserWatch {
            hub: None,
            error: None,
            events: None,
            browsers: Vec::new(),
            activity: HashMap::new(),
            asks: Vec::new(),
            frame_tx,
            frame_rx,
            texture: None,
            busy: false,
            last_frame: None,
            behind: false,
            note: String::new(),
            small: false,
            big: false,
            started: false,
            form: Default::default(),
            profiles_tx,
            profiles_rx,
            profiles_asked: false,
            ask_risky: None,
        }
    }
}

impl BrowserWatch {
    /// Start the hub once; its events wake the UI.
    pub fn start(&mut self, ctx: &egui::Context, ask_risky: bool) {
        // Tests never publish a hub file or register a host.
        if self.started || cfg!(test) {
            return;
        }
        self.started = true;
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        match browser_hub::start(Box::new(move |ev| {
            let _ = tx.send(ev);
            ctx.request_repaint();
        })) {
            Ok(hub) => {
                hub.set_ask_risky(ask_risky);
                self.hub = Some(hub);
                self.events = Some(rx);
            }
            Err(e) => self.error = Some(format!("The browser hub could not start: {e}")),
        }
    }
}

impl BrowserWatch {
    /// Keep Settings > Browser current: profiles read off the UI thread when
    /// it opens, the connected browsers, a profile window to open, and the
    /// ask switch passed on to the hub.
    fn settings(&mut self, open: bool, ask_risky: bool) {
        if let Some(hub) = &self.hub {
            if self.ask_risky != Some(ask_risky) {
                hub.set_ask_risky(ask_risky);
                self.ask_risky = Some(ask_risky);
            }
        }
        while let Ok(rows) = self.profiles_rx.try_recv() {
            self.form.profiles = rows;
        }
        if !open {
            self.profiles_asked = false;
            return;
        }
        if !self.profiles_asked {
            self.profiles_asked = true;
            self.form.ext_dir = browser_hub::extension_dir();
            let tx = self.profiles_tx.clone();
            std::thread::spawn(move || {
                let _ = tx.send(browser_hub::profiles::all_profiles());
            });
        }
        self.form.connected = self.browsers.clone();
        self.form.hub_error = self.error.clone();
        if let Some(p) = self.form.want_open.take() {
            match browser_hub::profiles::browser_exe(&p.browser) {
                None => self.form.error = Some(format!("{} isn't installed where Maestro looked.", p.browser)),
                Some(exe) => {
                    self.form.error = std::process::Command::new(exe).arg(format!("--profile-directory={}", p.dir)).spawn().err().map(|e| e.to_string());
                }
            }
        }
    }
}

/// Decode a peek answer's JPEG. Runs off the UI thread.
fn decode(answer: Result<serde_json::Value, String>) -> Result<(egui::ColorImage, bool), String> {
    let answer = answer?;
    let (data, front) = frame_of(&answer);
    let bytes = base64::engine::general_purpose::STANDARD.decode(data.ok_or("No picture right now")?).map_err(|e| e.to_string())?;
    let img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    Ok((egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()), front))
}

impl ShellApp {
    /// The name of the agent on stage, whose browser the live view shows.
    fn stage_agent(&self) -> Option<String> {
        let id = self.stage.as_ref().or(self.focus_pane.as_ref())?;
        self.panes.iter().find(|p| &p.id == id && p.cli.is_some()).map(|p| p.name.clone())
    }

    pub(crate) fn poll_browser(&mut self, ctx: &egui::Context) {
        let ask_risky = self.prefs.browser_ask;
        let settings_open = self.settings.open;
        let w = &mut self.extras.browser;
        w.start(ctx, ask_risky);
        w.settings(settings_open, ask_risky);
        let mut blockers = Vec::new();
        if let Some(rx) = &w.events {
            while let Ok(ev) = rx.try_recv() {
                match ev {
                    HubEvent::Browsers(list) => w.browsers = list,
                    HubEvent::Activity(list) => w.activity = list.into_iter().map(|a| (a.agent.clone(), a)).collect(),
                    HubEvent::Asks(list) => w.asks = list,
                    HubEvent::Blocker(b) => blockers.push(b),
                }
            }
        }
        while let Ok((agent, frame)) = w.frame_rx.try_recv() {
            w.busy = false;
            w.last_frame = Some(Instant::now());
            match frame {
                Ok((img, front)) => {
                    w.behind = !front;
                    w.note = if front { String::new() } else { "Behind another tab in Chrome, so it updates every few seconds".into() };
                    w.texture = Some((agent, ctx.load_texture("browser-frame", img, egui::TextureOptions::LINEAR)));
                }
                Err(e) => {
                    w.behind = true;
                    w.note = if e.contains("isn't showing") { "Behind another tab in Chrome, so it updates every few seconds".into() } else { "No picture right now".into() };
                }
            }
        }
        for b in blockers {
            let what = if b.kind == "captcha" { "a CAPTCHA" } else { "a login page" };
            let at = host(&b.url);
            let where_ = if at.is_empty() { String::new() } else { format!(" on {at}") };
            self.notice = Some(format!("{} reached {what}{where_}. Handle it in Chrome, then tell it to carry on.", b.agent));
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Informational));
        }
        if !self.extras.browser.asks.is_empty() && !ctx.input(|i| i.viewport().focused.unwrap_or(true)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Critical));
        }
        self.browser_card(ctx);
        self.browser_asks(ctx);
    }

    fn browser_card(&mut self, ctx: &egui::Context) {
        let now = now_ms();
        let Some(agent) = self.stage_agent() else { return };
        let w = &mut self.extras.browser;
        let Some(a) = w.activity.get(&agent).cloned().filter(|a| shows(Some(a), now)) else { return };
        ctx.request_repaint_after(Duration::from_secs(1));
        // A frame about once a second, fewer while its tab is behind another.
        let every = Duration::from_millis(if w.behind { BACK_FRAME_MS } else { FRAME_MS });
        if let Some(hub) = w.hub.clone().filter(|_| !w.small && !w.busy && w.last_frame.is_none_or(|t| t.elapsed() >= every)) {
            w.busy = true;
            let (tx, who, big) = (w.frame_tx.clone(), agent.clone(), w.big);
            let args = serde_json::json!({ "maxW": if big { 1100 } else { 560 }, "quality": if big { 78 } else { 62 } });
            std::thread::spawn(move || {
                let _ = tx.send((who.clone(), decode(hub.app_call(&who, "peek", args, Duration::from_secs(6)))));
            });
        }
        let width = if w.big { 760.0 } else { 300.0 };
        let frame = egui::Frame::new().fill(SHEET).corner_radius(14.0).stroke(Stroke::new(1.0, Color32::from_white_alpha(24))).inner_margin(egui::Margin::same(10));
        egui::Area::new(egui::Id::unique("browser-live")).anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0)).order(egui::Order::Foreground).show(ctx, |ui| {
            frame.show(ui, |ui| {
                ui.set_width(width);
                ui.horizontal(|ui| {
                    let dot = if a.paused { DEL } else { AMBER };
                    ui.label(RichText::new("\u{25CF}").size(10.0).color(dot));
                    ui.label(RichText::new(&agent).strong().color(TEXT));
                    if !a.label.is_empty() {
                        ui.label(RichText::new(format!("\u{00B7} {}", a.label)).size(12.0).color(INK3));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new(RichText::new(if w.small { "Show" } else { "Fold" }).size(12.0).color(INK3)).frame(false)).clicked() {
                            w.small = !w.small;
                        }
                    });
                });
                if !w.small {
                    let tex = w.texture.as_ref().filter(|(who, _)| *who == agent).map(|(_, t)| t.clone());
                    let resp = match tex {
                        Some(t) => {
                            let s = t.size_vec2();
                            let h = width * s.y / s.x.max(1.0);
                            ui.add(egui::Image::new(&t).fit_to_exact_size(egui::vec2(width, h)).corner_radius(8.0).sense(egui::Sense::click()))
                        }
                        None => ui.add(egui::Label::new(RichText::new("Waiting for a picture of its tab\u{2026}").size(12.0).color(INK3)).sense(egui::Sense::click())),
                    };
                    if resp.on_hover_text(if w.big { "Make it small" } else { "Make it large" }).clicked() {
                        w.big = !w.big;
                        w.last_frame = None;
                    }
                    if !w.note.is_empty() {
                        ui.label(RichText::new(&w.note).size(11.5).color(INK3));
                    }
                }
                ui.horizontal(|ui| {
                    ui.label(RichText::new(what_line(&a, now)).size(12.0).color(INK3));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let label = if a.paused { "Let it go on" } else { "Stop" };
                        let b = egui::Button::new(RichText::new(label).size(12.5).strong().color(if a.paused { PRIMARY_INK } else { TEXT }))
                            .fill(if a.paused { PRIMARY } else { DEL.gamma_multiply(0.45) })
                            .corner_radius(999.0);
                        if ui.add(b).clicked() {
                            if let Some(hub) = &w.hub {
                                hub.set_paused(&a.agent, !a.paused);
                            }
                        }
                    });
                });
            });
        });
    }

    /// Clicks held for your OK, bottom centre, newest last.
    fn browser_asks(&mut self, ctx: &egui::Context) {
        let now = now_ms();
        let w = &mut self.extras.browser;
        let live: Vec<Ask> = w.asks.iter().filter(|a| a.at == 0 || now.saturating_sub(a.at) < HOLD_MS).cloned().collect();
        if live.is_empty() {
            return;
        }
        let mut answer = None;
        egui::Area::new(egui::Id::unique("browser-asks")).anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -24.0)).order(egui::Order::Foreground).show(ctx, |ui| {
            for a in &live {
                egui::Frame::new().fill(SHEET).corner_radius(14.0).stroke(Stroke::new(1.0, AMBER.gamma_multiply(0.6))).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                    ui.set_width(460.0);
                    ui.label(RichText::new(ask_line(a)).strong().color(TEXT));
                    let at = host(&a.url);
                    ui.label(RichText::new(if at.is_empty() { "in the browser".to_string() } else { format!("on {at}") }).size(12.0).color(INK3));
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(egui::Button::new(RichText::new("Allow").strong().color(PRIMARY_INK)).fill(PRIMARY).corner_radius(999.0)).clicked() {
                                answer = Some((a.id, true));
                            }
                            if ui.add(egui::Button::new(RichText::new("Don't allow").color(TEXT)).corner_radius(999.0)).clicked() {
                                answer = Some((a.id, false));
                            }
                        });
                    });
                });
                ui.add_space(6.0);
            }
        });
        if let (Some((id, ok)), Some(hub)) = (answer, &w.hub) {
            hub.answer(id, ok);
            w.asks.retain(|a| a.id != id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bad_frame_says_so() {
        assert_eq!(decode(Err("tab isn't showing".into())).unwrap_err(), "tab isn't showing");
        assert!(decode(Ok(serde_json::json!({}))).is_err());
        let not_jpeg = serde_json::json!({ "content": [{ "type": "image", "data": "AAAA" }] });
        assert!(decode(Ok(not_jpeg)).is_err());
    }
}
