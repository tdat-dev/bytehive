//! Settings > Browser (browserprefs.ts, index.html's browser section, .bp-*
//! in modal.css): which of your browser profiles agents can use, the three
//! steps to add "ByteHive for Chrome" to one, and whether risky clicks wait
//! for your OK. A profile turns to Connected by itself when its extension
//! reaches the hub, so setup needs no "check again" button.

use browser_hub::profiles::{self, ProfileRow};

use super::rows::{text, toggle};
use super::*;
use crate::inbox::browser::{same, short};

/// How long a Copy button says Copied.
const COPIED_MS: u64 = 1400;

/// The style of one stretch of a step's sentence.
#[derive(Clone, Copy)]
enum Run {
    Plain,
    Bold,
    Code,
}

/// A sentence with bold words and code in it, wrapping as one paragraph.
fn sentence(parts: &[(&str, Run)]) -> StyledText {
    let text: String = parts.iter().map(|(s, _)| *s).collect();
    let runs = parts
        .iter()
        .map(|(s, run)| {
            let (family, weight, tone, bg) = match run {
                Run::Plain => ("Geist", FontWeight::NORMAL, color(INK_3), None),
                Run::Bold => ("Geist", FontWeight::SEMIBOLD, color(INK), None),
                Run::Code => ("Geist Mono", FontWeight::NORMAL, color(INK), Some(white(0.06))),
            };
            TextRun { len: s.len(), font: Font { weight, ..font(family) }, color: tone, background_color: bg, underline: None, strikethrough: None }
        })
        .collect();
    StyledText::new(text).with_runs(runs)
}

/// .bp-copy: a small outlined button.
fn small_button(id: (&'static str, usize), label: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .py(px(4.0))
        .px(px(10.0))
        .rounded(px(7.0))
        .border_1()
        .border_color(white(0.12))
        .text_size(px(11.5))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color(INK_3))
        .hover(|b| b.text_color(color(INK)).border_color(white(0.24)))
        .child(label)
}

/// .sbtn, a little tighter in a profile row (.bp-row .sbtn).
fn setup_button(id: (&'static str, usize)) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .py(px(6.0))
        .px(px(13.0))
        .rounded_full()
        .bg(white(0.08))
        .text_size(px(12.5))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color(INK))
        .hover(|b| b.bg(white(0.14)))
        .child("Set up")
}

/// A green dot and Connected.
fn connected() -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(7.0))
        .text_size(px(12.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color(INK))
        .child(div().size(px(7.0)).rounded_full().bg(color(WORK)).shadow(vec![BoxShadow { color: tint(WORK, 0.22), offset: point(px(0.0), px(0.0)), blur_radius: px(0.0), spread_radius: px(3.0), inset: false }]))
        .child("Connected")
}

/// .bp-row: a letter, the profile and its account, and what to do.
fn profile_row(letter: String, name: String, sub: String, on: bool) -> Div {
    let av = div().size(px(28.0)).flex_none().rounded_full().flex().items_center().justify_center().bg(color(RAISE)).text_size(px(12.5)).font_weight(FontWeight::BOLD).text_color(color(if on { INK } else { INK_3 })).child(letter);
    let words = div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .gap(px(1.0))
        .child(div().truncate().text_size(px(13.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child(name))
        .child(div().truncate().text_size(px(12.0)).text_color(color(INK_3)).child(sub));
    div().flex().items_center().gap(px(12.0)).py(px(10.0)).px(px(12.0)).rounded(px(10.0)).bg(color(PANEL)).border_1().border_color(white(0.07)).child(av).child(words)
}

fn first_letter(s: &str) -> String {
    s.trim().chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_else(|| "?".to_string())
}

impl Inbox {
    pub(super) fn browser_section(&self, cx: &mut Context<Self>) -> Div {
        self.load_profiles(cx);
        let w = &self.browser;
        let mut list: Vec<AnyElement> = Vec::new();
        let mut matched = Vec::new();
        for (i, p) in w.profiles.iter().enumerate() {
            let c = w.browsers.iter().find(|c| same(p, c));
            if let Some(c) = c {
                matched.push(c.id);
            }
            let sub = format!("{} \u{00B7} {}", if p.email.is_empty() { "Not signed in" } else { &p.email }, short(&p.browser));
            let row = profile_row(first_letter(&p.name), p.name.clone(), sub, c.is_some());
            let right = if c.is_some() {
                connected().into_any_element()
            } else {
                let p = p.clone();
                setup_button(("bp-setup", i)).on_click(cx.listener(move |this, _, _, cx| this.set_up_profile(p.clone(), cx))).into_any_element()
            };
            list.push(row.child(right).into_any_element());
        }
        // Connected profiles we can't name (not signed in to the browser).
        for c in w.browsers.iter().filter(|c| !matched.contains(&c.id)) {
            let name = if c.profile.is_empty() { "A profile".to_string() } else { c.profile.clone() };
            let sub = format!("{} \u{00B7} {}", if c.email.is_empty() { "Not signed in" } else { &c.email }, short(&c.browser));
            list.push(profile_row(first_letter(short(&c.browser)), name, sub, true).child(connected()).into_any_element());
        }
        if list.is_empty() {
            list.push(div().py(px(6.0)).text_size(px(12.5)).text_color(color(INK_3)).child("No Chrome, Edge or Brave profiles on this computer.").into_any_element());
        }
        let yours = div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .py(px(13.0))
            .border_b_1()
            .border_color(hairline())
            .child(text("Your browsers, for your agents", "Add ByteHive for Chrome to a profile and your agents can open tabs there, read pages, click and type, signed in as you. Each agent works in its own tab group, so you can see what it touches."))
            .child(div().flex().flex_col().gap(px(6.0)).children(list))
            .children(w.setting_up.as_ref().map(|s| self.setup_steps(s, cx)));
        div()
            .mb(px(28.0))
            .child(div().mb(px(12.0)).text_size(px(12.5)).font_weight(FontWeight::MEDIUM).text_color(color(INK_3)).child("Browser"))
            .child(yours)
            .child(
                toggle("pref-browser-ask", w.ask_risky, "Ask me before an agent sends, posts, pays or deletes", "A click on a button like Send, Post, Pay or Delete waits for your OK here in ByteHive. Logins and CAPTCHAs are always left to you.")
                    .on_click(cx.listener(|this, _, _, cx| this.set_browser_ask(!this.browser.ask_risky, cx))),
            )
    }

    /// .bp-setup: the three steps for the profile being set up.
    fn setup_steps(&self, s: &ProfileRow, cx: &mut Context<Self>) -> Div {
        let b = short(&s.browser).to_string();
        let copied = |key: &str| self.browser.copied.is_some_and(|(k, at)| k == key && now_ms().saturating_sub(at) < COPIED_MS);
        let copy = |n: usize, key: &'static str, what: String| {
            small_button(("bp-copy", n), if copied(key) { "Copied" } else { "Copy" }).on_click(cx.listener(move |this, _, _, cx| this.copy_for_setup(key, what.clone(), cx)))
        };
        let step = |n: usize, body: StyledText| {
            div()
                .flex()
                .items_start()
                .gap(px(10.0))
                .text_size(px(12.5))
                .line_height(relative(1.5))
                .child(div().flex_none().mt(px(1.0)).size(px(20.0)).rounded_full().flex().items_center().justify_center().bg(white(0.08)).text_size(px(11.0)).font_weight(FontWeight::BOLD).text_color(color(INK)).child(n.to_string()))
                .child(div().flex_1().min_w_0().child(body))
        };
        let pulse = div().size(px(7.0)).rounded_full().bg(color(INK_3)).with_animation("bp-wait", Animation::new(Duration::from_millis(1600)).repeat(), |el, d| el.opacity(1.0 - 0.7 * (1.0 - (2.0 * d - 1.0).abs())));
        div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .py(px(14.0))
            .px(px(16.0))
            .rounded(px(10.0))
            .border_1()
            .border_color(white(0.12))
            .bg(color(RAISE))
            .child(div().text_size(px(13.0)).font_weight(FontWeight::SEMIBOLD).text_color(color(INK)).child(format!("Add ByteHive for Chrome to {}", s.name)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(step(1, sentence(&[(&format!("ByteHive opened this profile in {b}. In its address bar, go to "), Run::Plain), ("chrome://extensions", Run::Code), (".", Run::Plain)])).child(copy(1, "ext", "chrome://extensions".to_string())))
                    .child(step(2, sentence(&[("Turn on ", Run::Plain), ("Developer mode", Run::Bold), (", top right of that page.", Run::Plain)])))
                    .child(step(3, sentence(&[("Click ", Run::Plain), ("Load unpacked", Run::Bold), (" and choose this folder: ", Run::Plain), (&self.browser.ext_dir, Run::Code)])).child(copy(3, "dir", self.browser.ext_dir.clone()))),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .pt(px(2.0))
                    .child(div().flex().items_center().gap(px(8.0)).text_size(px(12.0)).text_color(color(INK_3)).child(pulse).child("This profile shows Connected by itself once it's in."))
                    .child(small_button(("bp-done", 0), "Hide steps").on_click(cx.listener(|this, _, _, cx| {
                        this.browser.setting_up = None;
                        cx.notify();
                    }))),
            )
    }

    /// Every profile, read off the UI thread the first time the section shows
    /// after Settings opens.
    fn load_profiles(&self, cx: &mut Context<Self>) {
        if self.browser.profiles_asked.replace(true) {
            return;
        }
        let read = cx.background_spawn(async { profiles::all_profiles() });
        cx.spawn(async move |this, cx| {
            let rows = read.await;
            let _ = this.update(cx, |this, cx| {
                this.browser.profiles = rows;
                cx.notify();
            });
        })
        .detach();
    }

    /// Set up: open a window of that profile, so you can add the extension there.
    fn set_up_profile(&mut self, p: ProfileRow, cx: &mut Context<Self>) {
        self.browser.setting_up = Some(p.clone());
        cx.notify();
        let open = cx.background_spawn(async move {
            let exe = profiles::browser_exe(&p.browser).ok_or_else(|| format!("{} isn't installed where ByteHive looked.", p.browser))?;
            std::process::Command::new(exe).arg(format!("--profile-directory={}", p.dir)).spawn().map(|_| ()).map_err(|e| e.to_string())
        });
        cx.spawn(async move |this, cx| {
            if let Err(e) = open.await {
                let _ = this.update(cx, |this, cx| this.toast(e, cx));
            }
        })
        .detach();
    }

    fn copy_for_setup(&mut self, key: &'static str, what: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(what));
        self.browser.copied = Some((key, now_ms()));
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(COPIED_MS)).await;
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }
}
