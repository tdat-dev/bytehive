//! A short note in the bottom right that goes by itself (paneToast, .kb-toast
//! in dock.css): what happened that nobody was looking at.

use super::*;

/// How long a note stays.
const SHOW_MS: u64 = 5000;

pub(super) struct Toast {
    text: String,
    no: u64,
}

impl Inbox {
    pub(super) fn toast(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.toast_for(text, SHOW_MS, cx);
    }

    /// A note that stays `ms`, for one said while the window is still opening.
    pub(super) fn toast_for(&mut self, text: impl Into<String>, ms: u64, cx: &mut Context<Self>) {
        let no = self.toast.as_ref().map_or(0, |t| t.no) + 1;
        self.toast = Some(Toast { text: text.into(), no });
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(ms)).await;
            let _ = this.update(cx, |this, cx| {
                if this.toast.as_ref().is_some_and(|t| t.no == no) {
                    this.toast = None;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn toast_view(&self) -> Option<AnyElement> {
        let t = self.toast.as_ref()?;
        Some(
            div()
                .absolute()
                .right(px(16.0))
                .bottom(px(100.0))
                .max_w(px(360.0))
                .py(px(11.0))
                .px(px(16.0))
                .rounded(px(14.0))
                .bg(color(RAISE))
                .text_size(px(13.0))
                .line_height(relative(1.4))
                .text_color(color(INK))
                .shadow(vec![BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.65), offset: point(px(0.0), px(24.0)), blur_radius: px(60.0), spread_radius: px(0.0), inset: false }, ring(white(0.1))])
                .child(t.text.clone())
                .with_animation(("toast", t.no as usize), Animation::new(Duration::from_millis(250)).with_easing(ease_out_quint()), |el, d| el.opacity(d).bottom(px(92.0 + 8.0 * d)))
                .into_any_element(),
        )
    }
}
