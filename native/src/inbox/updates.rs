//! Updates in the app (updater.ts, #updToast in updater.css): a check a
//! little after start and every six hours, quiet when nothing is new; a card
//! sliding up in the bottom right when a version is out, with Later and
//! Update, and a progress bar while it downloads. The installer runs silently
//! and opens ByteHive again. A build run from target\ never checks by itself.

use std::sync::{Arc, Mutex};

use crate::updater::{self, Release};

use super::*;

const FIRST: Duration = Duration::from_secs(20);
const EVERY: Duration = Duration::from_secs(6 * 3600);

#[derive(Clone, PartialEq)]
enum Card {
    Hidden,
    Available,
    Downloading,
    Restarting,
    Failed,
}

pub(super) struct Updates {
    found: Option<Release>,
    card: Card,
    /// Bytes got and the whole size, while downloading.
    progress: Arc<Mutex<(u64, u64)>>,
    pub(super) checking: bool,
    _loop: Option<Task<()>>,
}

impl Default for Updates {
    fn default() -> Self {
        Self { found: None, card: Card::Hidden, progress: Arc::default(), checking: false, _loop: None }
    }
}

impl Inbox {
    /// Start the background checks once (from the first render).
    pub(super) fn start_updates(&mut self, cx: &mut Context<Self>) {
        if self.updates._loop.is_some() || updater::dev_build() {
            return;
        }
        self.updates._loop = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(FIRST).await;
            loop {
                if this.update(cx, |this, cx| this.check_updates(true, cx)).is_err() {
                    break;
                }
                cx.background_executor().timer(EVERY).await;
            }
        }));
    }

    /// `silent` (the background check) stays quiet on no update or a failed
    /// check; the Settings button says either.
    pub(super) fn check_updates(&mut self, silent: bool, cx: &mut Context<Self>) {
        if self.updates.checking {
            return;
        }
        self.updates.checking = true;
        let ask = cx.background_spawn(async { updater::fetch_latest() });
        cx.spawn(async move |this, cx| {
            let got = ask.await;
            let _ = this.update(cx, |this, cx| {
                this.updates.checking = false;
                match got {
                    Ok(r) if updater::newer(updater::current(), &r.version) => {
                        this.updates.found = Some(r);
                        if this.updates.card == Card::Hidden {
                            this.updates.card = Card::Available;
                        }
                    }
                    Ok(_) if !silent => this.toast(format!("You're on the latest version (v{}).", updater::current()), cx),
                    Err(e) if !silent => this.toast(format!("Couldn't check for updates: {e}"), cx),
                    _ => {}
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn install_update(&mut self, cx: &mut Context<Self>) {
        let Some(r) = self.updates.found.clone() else { return };
        self.updates.card = Card::Downloading;
        *self.updates.progress.lock().unwrap() = (0, 0);
        let progress = self.updates.progress.clone();
        let fetch = cx.background_spawn(async move { updater::download(&r, |got, total| *progress.lock().unwrap() = (got, total)) });
        // Redraw the bar while it downloads.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(250)).await;
                let Ok(going) = this.update(cx, |this, cx| {
                    cx.notify();
                    this.updates.card == Card::Downloading
                }) else {
                    break;
                };
                if !going {
                    break;
                }
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            let got = fetch.await.and_then(|path| updater::run_installer(&path));
            let _ = this.update(cx, |this, cx| match got {
                Ok(()) => {
                    this.updates.card = Card::Restarting;
                    cx.notify();
                    // The installer closes this one anyway; it opens ByteHive again.
                    cx.quit();
                }
                Err(e) => {
                    this.updates.card = Card::Failed;
                    this.toast(e, cx);
                }
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn update_card(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let card = self.updates.card.clone();
        if card == Card::Hidden {
            return None;
        }
        let version = self.updates.found.as_ref().map(|r| format!("ByteHive v{}", r.version)).unwrap_or_default();
        let title = match card {
            Card::Downloading => "Downloading update…",
            Card::Restarting => "Restarting…",
            Card::Failed => "Update failed, try again",
            _ => "New version available",
        };
        let (got, total) = *self.updates.progress.lock().unwrap();
        let mb = |b: u64| format!("{:.1}", b as f64 / (1024.0 * 1024.0));
        let size = if total > 0 { format!("{} / {} MB", mb(got), mb(total)) } else { format!("{} MB downloaded", mb(got)) };
        let share = if total > 0 { (got as f32 / total as f32).min(1.0) } else { 0.0 };
        let busy = matches!(card, Card::Downloading | Card::Restarting);
        let pill = |id: &'static str, white_: bool| {
            div()
                .id(id)
                .h(px(32.0))
                .px(px(14.0))
                .flex()
                .items_center()
                .gap(px(6.0))
                .rounded_full()
                .text_size(px(12.5))
                .font_weight(FontWeight::SEMIBOLD)
                .when(white_, |b| b.bg(color(0xffffff)).text_color(color(0x000000)).hover(|b| b.bg(color(0xe9e9ec))))
                .when(!white_, |b| b.bg(white(0.06)).text_color(color(INK_2)).hover(|b| b.bg(white(0.12)).text_color(color(INK))))
        };
        let hide = cx.listener(|this, _, _, cx| {
            this.updates.card = Card::Hidden;
            cx.notify();
        });
        Some(
            div()
                .absolute()
                .right(px(18.0))
                .bottom(px(18.0))
                .w(px(340.0))
                .flex()
                .gap(px(12.0))
                .p(px(16.0))
                .rounded(px(18.0))
                .bg(color(RAISE))
                .text_color(color(INK))
                .shadow(vec![BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.65), offset: point(px(0.0), px(30.0)), blur_radius: px(70.0), spread_radius: px(0.0), inset: false }, ring(white(0.1))])
                .child(div().flex_none().size(px(36.0)).flex().items_center().justify_center().child(img(super::logo::logo()).w(px(30.0)).h(px(30.0))))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(3.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .child(div().text_size(px(14.0)).font_weight(FontWeight::SEMIBOLD).child(title))
                                .child(div().flex_1())
                                .when(!busy, |h| h.child(div().id("upd-x").size(px(26.0)).flex().items_center().justify_center().rounded_full().hover(|b| b.bg(white(0.08))).on_click(cx.listener(|this, _, _, cx| {
                                    this.updates.card = Card::Hidden;
                                    cx.notify();
                                })).child(icon("x", 13.0, color(INK_3))))),
                        )
                        .child(mono().text_size(px(12.0)).text_color(color(INK_3)).child(version))
                        .when(busy, |b| {
                            b.child(div().mt(px(8.0)).mb(px(2.0)).h(px(4.0)).rounded(px(4.0)).bg(white(0.08)).overflow_hidden().child(div().h_full().w(relative(share)).rounded(px(4.0)).bg(color(WORK))))
                                .child(mono().mt(px(1.0)).text_size(px(11.0)).text_color(color(INK_3)).flex().justify_end().child(size))
                        })
                        .when(!busy, |b| {
                            b.child(
                                div()
                                    .mt(px(10.0))
                                    .flex()
                                    .gap(px(8.0))
                                    .child(pill("upd-later", false).on_click(hide).child("Later"))
                                    .child(div().flex_1())
                                    .child(pill("upd-go", true).on_click(cx.listener(|this, _, _, cx| this.install_update(cx))).child("Update").child(icon("chevron-right", 13.0, color(0x000000)))),
                            )
                        }),
                )
                .into_any_element(),
        )
    }
}
