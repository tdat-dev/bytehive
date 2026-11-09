//! One-time tips (tips.ts): a line in the hint card over the agent list
//! (hint.ts, .hint) the first time something it explains becomes useful: the
//! welcome a moment after start, the right-click menu with the first agent,
//! the Grid with the second, Changes when an agent finishes. Each shows once,
//! never two within 20 s, never over Home. Settings > Inbox turns them off or
//! brings them back.

use super::*;

/// Never two tips closer than this.
const GAP_MS: u64 = 20_000;
/// The welcome waits this long after start.
const WELCOME_AFTER_MS: u64 = 2_600;
const SHOW_MS: u64 = 7_000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Tip {
    Welcome,
    Menu,
    Split,
    Review,
}

impl Tip {
    fn id(self) -> &'static str {
        match self {
            Tip::Welcome => "welcome",
            Tip::Menu => "menu",
            Tip::Split => "split",
            Tip::Review => "review",
        }
    }

    /// TIPS in tips.ts, markup and all.
    fn text(self) -> &'static str {
        match self {
            Tip::Welcome => "Press <kbd>?</kbd> to see everything ByteHive can do · <kbd>Ctrl</kbd> <kbd>K</kbd> jumps anywhere",
            Tip::Menu => "<b>Right-click</b> an agent in the list to rename it, add it to the Grid, stop or remove it",
            Tip::Split => "Several agents? <b>Grid</b> (<kbd>Alt</kbd> <kbd>S</kbd>) shows them side by side, each answering on its own card",
            Tip::Review => "<kbd>Alt</kbd> <kbd>R</kbd> shows what it changed, to merge or send back",
        }
    }
}

/// A word of a tip and how it looks.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) enum Word {
    Plain(String),
    Bold(String),
    Key(String),
}

/// A tip's markup (only <b> and <kbd>) as words, so the card can wrap it.
pub(super) fn words_of(markup: &str) -> Vec<Word> {
    let mut out = Vec::new();
    let mut rest = markup;
    while !rest.is_empty() {
        let (tag, close) = match (rest.find("<b>"), rest.find("<kbd>")) {
            (Some(b), Some(k)) if k < b => (k, "kbd"),
            (Some(b), _) => (b, "b"),
            (None, Some(k)) => (k, "kbd"),
            (None, None) => (rest.len(), ""),
        };
        out.extend(rest[..tag].split_whitespace().map(|w| Word::Plain(w.to_string())));
        if close.is_empty() {
            break;
        }
        let inner = &rest[tag + close.len() + 2..];
        let end = inner.find(&format!("</{close}>")).unwrap_or(inner.len());
        let body = &inner[..end];
        if close == "kbd" {
            out.push(Word::Key(body.to_string()));
        } else {
            out.extend(body.split_whitespace().map(|w| Word::Bold(w.to_string())));
        }
        rest = inner.get(end + close.len() + 3..).unwrap_or("");
    }
    out
}

pub(super) struct Tips {
    shown: Option<(Vec<Word>, u64)>,
    last_at: u64,
    started: u64,
    welcomed: bool,
}

impl Default for Tips {
    fn default() -> Self {
        Self { shown: None, last_at: 0, started: now_ms(), welcomed: false }
    }
}

impl Inbox {
    fn tip_seen(&self, tip: Tip) -> bool {
        self.prefs.tips_seen.iter().any(|s| s == tip.id())
    }

    /// Show a tip if it is new, tips are on, and none showed in the last 20 s.
    /// `lead` goes before it (an agent's name). Whether it showed.
    fn show_tip(&mut self, tip: Tip, lead: &str, now: u64, cx: &mut Context<Self>) -> bool {
        // Not over Home: the tip would be about a screen you can't see.
        if !self.prefs.tips || self.home || self.tip_seen(tip) || now.saturating_sub(self.tips.last_at) < GAP_MS {
            return false;
        }
        self.prefs.tips_seen.push(tip.id().to_string());
        self.prefs.save();
        self.tips.last_at = now;
        let no = self.tips.shown.as_ref().map_or(0, |s| s.1) + 1;
        self.tips.shown = Some((words_of(&format!("{lead}{}", tip.text())), no));
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(SHOW_MS)).await;
            let _ = this.update(cx, |this, cx| {
                if this.tips.shown.as_ref().is_some_and(|s| s.1 == no) {
                    this.tips.shown = None;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
        true
    }

    /// Each tick: the moments a tip becomes useful. `moved` is (seat index,
    /// state before, state now).
    pub(super) fn tips_tick(&mut self, moved: &[(usize, State, State)], now: u64, cx: &mut Context<Self>) {
        if !self.tips.welcomed && now.saturating_sub(self.tips.started) >= WELCOME_AFTER_MS {
            self.tips.welcomed = true;
            self.show_tip(Tip::Welcome, "", now, cx);
        }
        let n = self.fleet.seats.len();
        if !self.grid.on {
            if n >= 1 && !self.tip_seen(Tip::Menu) && self.show_tip(Tip::Menu, "", now, cx) {
                return;
            }
            if n >= 2 && !self.tip_seen(Tip::Split) {
                self.show_tip(Tip::Split, "", now, cx);
            }
        }
        for &(index, before, after) in moved {
            if after == State::Review && before == State::Working && !self.tip_seen(Tip::Review) {
                let name = self.fleet.seats[index].spec.name.clone();
                self.show_tip(Tip::Review, &format!("<b>{name}</b> finished. "), now, cx);
            }
        }
    }

    /// Settings > Show tips again.
    pub(super) fn reset_tips(&mut self) {
        self.prefs.tips_seen.clear();
        self.prefs.tips = true;
        self.prefs.save();
        self.tips.last_at = 0;
    }

    /// .hint: the card over the agent list, bottom left.
    pub(super) fn tip_view(&self) -> Option<AnyElement> {
        let (words, _) = self.tips.shown.as_ref().filter(|_| !self.home)?;
        let words = words.iter().map(|w| match w {
            Word::Plain(t) => div().text_color(color(INK)).child(t.clone()),
            Word::Bold(t) => div().text_color(color(INK)).font_weight(FontWeight::BOLD).child(t.clone()),
            Word::Key(t) => mono().text_size(px(11.0)).px(px(6.0)).py(px(1.0)).rounded(px(6.0)).text_color(color(INK_3)).shadow(vec![BoxShadow { color: white(0.14), offset: point(px(0.0), px(0.0)), blur_radius: px(0.0), spread_radius: px(1.0), inset: true }]).child(t.clone()),
        });
        Some(
            div()
                .absolute()
                .left(px(24.0))
                .bottom(px(DOCK_H + 12.0))
                .max_w(px(QUEUE_W - 48.0))
                .py(px(10.0))
                .px(px(14.0))
                .rounded(px(14.0))
                .bg(color(RAISE))
                .text_size(px(13.0))
                .line_height(relative(1.5))
                .shadow(vec![BoxShadow { color: hsla(0.0, 0.0, 0.0, 0.55), offset: point(px(0.0), px(18.0)), blur_radius: px(40.0), spread_radius: px(0.0), inset: false }, ring(white(0.1))])
                .child(div().flex().flex_wrap().items_center().gap_x(px(3.5)).children(words))
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{Word, words_of};

    #[test]
    fn reads_bold_and_keys() {
        let w = words_of("Press <kbd>Ctrl</kbd> <kbd>K</kbd> or <b>Right-click</b> it");
        assert_eq!(w, vec![Word::Plain("Press".into()), Word::Key("Ctrl".into()), Word::Key("K".into()), Word::Plain("or".into()), Word::Bold("Right-click".into()), Word::Plain("it".into())]);
        assert_eq!(words_of("<b>Ana Bo</b> finished. x"), vec![Word::Bold("Ana".into()), Word::Bold("Bo".into()), Word::Plain("finished.".into()), Word::Plain("x".into())]);
    }
}
