//! The live view (browserview.ts, .bv in browserview.css): while the agent
//! on stage has browsed within the last minute and a half, or you stopped
//! it, a small card sits above the dock, bottom right, with a frame of its
//! tab about once a second (peek never brings the tab forward), what it just
//! did, and Stop. A click on the frame makes it large.

use serde_json::json;

use super::frame::{Peeked, decode};
use super::live::{self, BACK_FRAME_MS, FRAME_MS};
use super::*;

/// The card's width, and its frame's shape while small (16:10).
const CARD_W: f32 = 300.0;
const BIG_MAX_W: f32 = 1080.0;

/// The app's window is on screen (not minimised, not tucked into the tray):
/// no frames for a view nobody can see (visibilityState in browserview.ts).
fn window_shown() -> bool {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, IsIconic, IsWindowVisible};
    unsafe extern "system" fn each(hwnd: HWND, lparam: LPARAM) -> i32 {
        let found = lparam as *mut (u32, bool);
        let mut owner = 0u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut owner);
            if owner == (*found).0 && IsWindowVisible(hwnd) != 0 && IsIconic(hwnd) == 0 {
                (*found).1 = true;
                return 0;
            }
        }
        1
    }
    let mut found = (std::process::id(), false);
    unsafe {
        EnumWindows(Some(each), &mut found as *mut (u32, bool) as LPARAM);
    }
    found.1
}

impl Inbox {
    /// The name of the agent on stage, whose browser the live view shows.
    fn stage_name(&self) -> Option<String> {
        if self.home {
            return None;
        }
        self.fleet.focused.and_then(|i| self.fleet.seats.get(i)).map(|s| s.spec.name.clone())
    }

    /// Show the card for the agent on stage when it browses; a new agent
    /// starts with no picture, small, and a frame right away.
    pub(super) fn browser_follow(&mut self, cx: &mut Context<Self>) {
        let now = now_ms();
        let name = self.stage_name().filter(|n| live::shows(self.browser.activity.get(n), now)).unwrap_or_default();
        let w = &mut self.browser;
        if name == w.shown_for {
            return;
        }
        w.shown_for = name;
        if let Some(old) = w.shot.take() {
            cx.drop_image(old.image, None);
        }
        w.big = false;
        w.note = "";
        if !w.shown_for.is_empty() {
            self.browser_frame(true, cx);
        }
    }

    /// Ask the browser for a frame when one is due (or `now`), off the UI thread.
    pub(super) fn browser_frame(&mut self, now: bool, cx: &mut Context<Self>) {
        let w = &mut self.browser;
        if w.busy && now {
            w.again = true;
        }
        if w.shown_for.is_empty() || w.small || w.busy || !window_shown() {
            return;
        }
        if !now && now_ms().saturating_sub(w.last_frame) < if w.behind { BACK_FRAME_MS } else { FRAME_MS } {
            return;
        }
        let Some(hub) = w.hub.clone() else { return };
        w.busy = true;
        let agent = w.shown_for.clone();
        // Pixels actually on screen: the frame's width times the display's scale.
        let shown = w.shown_w.get();
        let args = json!({ "maxW": (if shown > 0.0 { shown } else { CARD_W }).ceil() as u32, "quality": if w.big { 78 } else { 62 } });
        let (tx, rx) = async_channel::bounded(1);
        let who = agent.clone();
        std::thread::spawn(move || {
            let _ = tx.send_blocking(decode(hub.app_call(&who, "peek", args, Duration::from_secs(6))));
        });
        cx.spawn(async move |this, cx| {
            let got = rx.recv().await.unwrap_or_else(|_| Err(String::new()));
            let _ = this.update(cx, |this, cx| this.browser_got_frame(agent, got, cx));
        })
        .detach();
    }

    fn browser_got_frame(&mut self, agent: String, got: Result<Peeked, String>, cx: &mut Context<Self>) {
        let w = &mut self.browser;
        w.busy = false;
        w.last_frame = now_ms();
        if agent == w.shown_for {
            match got {
                Ok(p) => {
                    w.behind = !p.front;
                    w.note = if w.behind { live::BEHIND } else { "" };
                    if let Some((image, size)) = p.image {
                        if let Some(old) = w.shot.replace(Shot { agent, image, size }) {
                            cx.drop_image(old.image, None);
                        }
                    }
                }
                Err(e) => {
                    w.behind = true;
                    w.note = live::failed_note(&e);
                }
            }
        }
        if std::mem::take(&mut self.browser.again) {
            self.browser_frame(true, cx);
        }
        cx.notify();
    }

    fn browser_toggle_stop(&mut self, cx: &mut Context<Self>) {
        let Some(a) = self.browser.activity.get(&self.browser.shown_for).cloned() else { return };
        let Some(hub) = self.hub_or_say(cx) else { return };
        let paused = !a.paused;
        hub.set_paused(&a.agent, paused);
        let note = if paused { format!("{} can't use the browser until you let it go on.", a.agent) } else { format!("{} can use the browser again.", a.agent) };
        self.toast(note, cx);
    }

    fn browser_fold(&mut self, cx: &mut Context<Self>) {
        self.browser.small = !self.browser.small;
        if !self.browser.small {
            self.browser_frame(true, cx);
        }
        cx.notify();
    }

    /// Larger or back: the next frame is fetched at the new size.
    fn browser_grow(&mut self, cx: &mut Context<Self>) {
        self.browser.big = !self.browser.big;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(260)).await;
            let _ = this.update(cx, |this, cx| this.browser_frame(true, cx));
        })
        .detach();
    }

    pub(in crate::inbox) fn browser_card_view(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let w = &self.browser;
        let a = w.activity.get(&w.shown_for).filter(|_| !w.shown_for.is_empty())?;
        let view = window.viewport_size();
        let width = if w.big { (f32::from(view.width) * 0.72).min(BIG_MAX_W) } else { CARD_W };
        w.shown_w.set(width * window.scale_factor());
        let dot = if a.paused { NEEDS } else { WORK };
        let head_text = if a.label.is_empty() { a.agent.clone() } else { format!("{} \u{00B7} {}", a.agent, a.label) };
        let name_len = a.agent.len();
        let head = div()
            .flex()
            .items_center()
            .gap(px(9.0))
            .py(px(8.0))
            .pr(px(8.0))
            .pl(px(12.0))
            .min_w_0()
            .child(div().flex_none().size(px(7.0)).rounded_full().bg(color(dot)).shadow(vec![BoxShadow { color: tint(dot, 0.22), offset: point(px(0.0), px(0.0)), blur_radius: px(0.0), spread_radius: px(3.0), inset: false }]))
            .child(div().flex_1().min_w_0().truncate().text_size(px(12.0)).text_color(color(INK_3)).child(StyledText::new(head_text).with_highlights([(0..name_len, HighlightStyle { color: Some(color(INK)), font_weight: Some(FontWeight::SEMIBOLD), ..Default::default() })])))
            .child(
                div()
                    .id("bv-fold")
                    .tooltip(tip(if w.small { "Show" } else { "Fold" }))
                    .flex_none()
                    .size(px(26.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(7.0))
                    .text_color(color(INK_3))
                    .hover(|b| b.bg(white(0.06)).text_color(color(INK)))
                    .on_click(cx.listener(|this, _, _, cx| this.browser_fold(cx)))
                    .child(if w.small { icon("chevron-down", 14.0, color(INK_3)).with_transformation(Transformation::rotate(radians(std::f32::consts::PI))) } else { icon("minus", 14.0, color(INK_3)) }),
            );
        let mut card = div()
            .id("bv")
            .absolute()
            .right(px(24.0))
            .bottom(px(DOCK_H + 12.0))
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(12.0))
            .bg(color(PANEL))
            .border_1()
            .border_color(white(0.12))
            .shadow(vec![BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.55), offset: point(px(0.0), px(18.0)), blur_radius: px(50.0), spread_radius: px(0.0), inset: false }])
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(head);
        if w.small {
            return Some(card.max_w(px(CARD_W)).into_any_element());
        }
        card = card.w(px(width));
        // Small: 16:10. Large: the picture's own shape, never past the window.
        let shot = w.shot.as_ref().filter(|s| s.agent == w.shown_for);
        let fit = |s: &Shot| width * s.size.1 as f32 / s.size.0.max(1) as f32;
        let frame_h = if w.big {
            let most = (f32::from(view.height) - DOCK_H - 140.0).max(120.0);
            shot.map_or(240.0, |s| fit(s).min(most))
        } else {
            width * 10.0 / 16.0
        };
        let picture = shot.map(|s| img(ImageSource::Render(s.image.clone())).w_full().h(px(fit(s).min(frame_h))).object_fit(ObjectFit::Contain));
        let note = (!w.note.is_empty()).then(|| div().absolute().left(px(10.0)).bottom(px(10.0)).py(px(4.0)).px(px(9.0)).rounded(px(7.0)).text_size(px(11.5)).text_color(color(INK_2)).bg(hsla(240.0 / 360.0, 0.09, 0.02, 0.82)).child(w.note));
        let frame = div()
            .id("bv-frame")
            .relative()
            .w_full()
            .h(px(frame_h))
            .overflow_hidden()
            .bg(color(0x0a0a0c))
            .border_t_1()
            .border_b_1()
            .border_color(hairline())
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| this.browser_grow(cx)))
            .children(picture)
            .children(note);
        let stop = div()
            .id("bv-stop")
            .flex_none()
            .py(px(5.0))
            .px(px(12.0))
            .rounded(px(8.0))
            .text_size(px(12.0))
            .font_weight(FontWeight::SEMIBOLD)
            .border_1()
            .when(a.paused, |b| b.bg(color(INK)).text_color(color(GROUND)).border_color(transparent_black()).hover(|b| b.opacity(0.9)))
            .when(!a.paused, |b| b.bg(color(RAISE)).text_color(color(INK)).border_color(white(0.12)).hover(|b| b.border_color(tint(DANGER, 0.6)).text_color(color(DANGER))))
            .on_click(cx.listener(|this, _, _, cx| this.browser_toggle_stop(cx)))
            .child(if a.paused { "Let it go on" } else { "Stop" });
        let foot = div()
            .flex()
            .items_center()
            .gap(px(10.0))
            .py(px(8.0))
            .pr(px(8.0))
            .pl(px(12.0))
            .child(div().flex_1().min_w_0().truncate().text_size(px(12.0)).text_color(color(INK_2)).child(live::what_line(a, now_ms())))
            .child(stop);
        Some(card.child(frame).child(foot).into_any_element())
    }
}
