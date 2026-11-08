//! Feeding text into an agent's PTY the way a human would. Ported from the
//! web app's `src/typing.ts`, same constants and verdicts.
//!
//! One write of `message + "\r"` loses against a TUI that does its own line
//! editing (Claude Code): the head of a long blob is dropped, and a CR inside
//! the same burst reads as a newline in the text instead of Enter. So the body
//! goes in small chunks (or as one bracketed paste when the program asked for
//! that mode), the CR follows on its own after a settle delay, and when the
//! text is still sitting in the composer afterwards Enter is pressed again.
//! Retries stop while a live question is on screen, so a match inside a
//! permission prompt can never approve anything.
//!
//! [`Typer`] is a step machine instead of a sleeping loop: the UI thread calls
//! [`Typer::step`] every frame with the clock and the pane's screen, and
//! writes whatever bytes it hands back.

use std::sync::LazyLock;

use regex::Regex;

use crate::ask::parse_ask;

/// Code points per write: ConPTY never has to swallow a blob.
pub const TYPE_CHUNK: usize = 120;
/// Breathing room between chunks.
pub const TYPE_GAP_MS: u64 = 12;
/// Gap before Enter: the burst has to be over for it to count as a keypress.
pub const SUBMIT_DELAY_MS: u64 = 220;
/// Waits before each check for a message still in the box, growing because a
/// busy (or still booting) TUI redraws late. Reaches past 30 s on purpose.
pub const RETRY_WAITS_MS: [u64; 11] = [400, 700, 1000, 1500, 2000, 3000, 4000, 5000, 6000, 8000, 8000];

/// DECSET 2004 wrappers, sent only to a program that turned the mode on.
pub const PASTE_START: &str = "\u{1b}[200~";
pub const PASTE_END: &str = "\u{1b}[201~";

/// Hints and status lines the TUI prints inside its input box; not typed text.
static HINT_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)press up to edit queued|bypass permissions|shift\+tab to cycle|esc to interrupt|for shortcuts").unwrap()
});
/// A horizontal rule: box edges around Claude Code's composer.
static RULE: LazyLock<Regex> = LazyLock::new(|| Regex::new("^[\u{2500}\u{2014}-]+$").unwrap());
/// A composer line: a known prompt glyph, then the typed text.
static PROMPT: LazyLock<Regex> = LazyLock::new(|| Regex::new("^\\s*[>\u{203A}\u{276F}]\\s?(.*)$").unwrap());
/// An empty composer line.
static EMPTY_PROMPT: LazyLock<Regex> = LazyLock::new(|| Regex::new("^\\s*[>\u{203A}\u{276F}]\\s*$").unwrap());

/// Split by code point, never mid-character.
pub fn chunk_text(text: &str, size: usize) -> Vec<String> {
    if size == 0 {
        return if text.is_empty() { Vec::new() } else { vec![text.to_string()] };
    }
    let cps: Vec<char> = text.chars().collect();
    cps.chunks(size).map(|c| c.iter().collect()).collect()
}

fn squash(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).flat_map(|c| c.to_lowercase()).collect()
}

fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// What sits in the pane's input box, read from a plain-text screen. Claude
/// Code draws it between two rules with `>` on the first line; Codex draws a
/// bare `›` line near the bottom; Muse a `❯` line. "" when the box is empty.
///
/// The screen has no colour, so a dimmed suggestion in an EMPTY box reads like
/// typed text: only ever act on this when it matches text you just typed.
pub fn composer_text(screen: &str) -> String {
    let lines: Vec<&str> = screen.lines().collect();
    let rules: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| {
            let t = l.trim();
            t.chars().count() > 10 && RULE.is_match(t)
        })
        .map(|(i, _)| i)
        .collect();
    if rules.len() < 2 {
        // Boxless: only the bottom of the screen, so a transcript echo of an
        // already-sent message higher up is never taken for the box.
        let tail = &lines[lines.len().saturating_sub(6)..];
        for l in tail.iter().rev() {
            if HINT_LINE.is_match(l) {
                continue;
            }
            if let Some(m) = PROMPT.captures(l) {
                return collapse(&m[1]);
            }
        }
        return String::new();
    }
    let region = &lines[rules[rules.len() - 2] + 1..rules[rules.len() - 1]];
    let mut body: Vec<&str> = Vec::new();
    let mut seen = false;
    for l in region {
        if HINT_LINE.is_match(l) {
            continue;
        }
        if let Some(m) = PROMPT.captures(l) {
            seen = true;
            body.push(m.get(1).map_or("", |g| g.as_str()));
        } else if seen {
            body.push(l);
        }
    }
    collapse(&body.join(" "))
}

/// True when what is in the box is part of the message just typed: the only
/// case where pressing Enter again is safe.
pub fn still_holding(typed: &str, screen: &str) -> bool {
    let bx = squash(&composer_text(screen));
    let want = squash(typed);
    if bx.chars().count() >= 12 && want.contains(&bx) {
        return true;
    }
    fallback_holding(&want, screen)
}

/// A CLI whose composer uses none of the known prompts: our own distinctive
/// text in the last few lines means the Enter did not take. An empty known
/// composer below the match makes it a transcript echo instead.
fn fallback_holding(want: &str, screen: &str) -> bool {
    if want.chars().count() < 12 {
        return false;
    }
    let lines: Vec<&str> = screen.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let lines = &lines[lines.len().saturating_sub(4)..];
    for (i, line) in lines.iter().enumerate() {
        if HINT_LINE.is_match(line) {
            continue;
        }
        // As drawn, and with a possible unknown prompt glyph stripped.
        let stripped = line.split_once(char::is_whitespace).map_or("", |(_, rest)| rest.trim_start());
        let hit = [*line, stripped].iter().any(|v| {
            let frag = squash(v);
            frag.chars().count() >= 12 && (want.contains(&frag) || frag.contains(want))
        });
        if !hit {
            continue;
        }
        return !lines[i + 1..].iter().any(|l| EMPTY_PROMPT.is_match(l));
    }
    false
}

/// What the caller does this frame.
#[derive(Debug, PartialEq, Eq)]
pub enum TypeStep {
    /// Write these bytes to the pane now.
    Write(Vec<u8>),
    /// Nothing yet; call again later.
    Wait,
    /// Finished: true when the message left the box (or a question took over).
    Done(bool),
}

enum Stage {
    Body(usize),
    Submit,
    Check(usize),
    Over,
}

/// Types one message into one pane, a step at a time.
pub struct Typer {
    text: String,
    parts: Vec<String>,
    stage: Stage,
    next_at: u64,
}

impl Typer {
    /// `bracketed_paste`: the program turned DECSET 2004 on, so the body goes
    /// as one marked block and the TUI assembles it itself.
    pub fn new(text: &str, bracketed_paste: bool, now_ms: u64) -> Typer {
        let parts = if bracketed_paste {
            vec![format!("{PASTE_START}{text}{PASTE_END}")]
        } else {
            chunk_text(text, TYPE_CHUNK)
        };
        Typer { text: text.to_string(), parts, stage: Stage::Body(0), next_at: now_ms }
    }

    pub fn step(&mut self, now_ms: u64, screen: &str) -> TypeStep {
        if now_ms < self.next_at {
            return TypeStep::Wait;
        }
        match self.stage {
            Stage::Body(i) => {
                let Some(part) = self.parts.get(i) else {
                    self.stage = Stage::Submit;
                    self.next_at = now_ms + SUBMIT_DELAY_MS;
                    return TypeStep::Wait;
                };
                let bytes = part.as_bytes().to_vec();
                if i + 1 < self.parts.len() {
                    self.stage = Stage::Body(i + 1);
                    self.next_at = now_ms + TYPE_GAP_MS;
                } else {
                    self.stage = Stage::Submit;
                    self.next_at = now_ms + SUBMIT_DELAY_MS;
                }
                TypeStep::Write(bytes)
            }
            Stage::Submit => {
                self.stage = Stage::Check(0);
                self.next_at = now_ms + RETRY_WAITS_MS[0];
                TypeStep::Write(b"\r".to_vec())
            }
            Stage::Check(n) => {
                // Not on screen at all yet: the TUI has not taken the text in
                // (a CLI still running its start-up hooks buffers it and drops
                // the Enter, or is between a clear and a redraw). Calling that
                // sent would leave the task sitting unsent; look again, and
                // press Enter once it shows up in the box.
                let head: String = squash(&self.text).chars().take(24).collect();
                let seen = !head.is_empty() && squash(screen).contains(&head);
                if !seen && n + 1 < RETRY_WAITS_MS.len() {
                    self.stage = Stage::Check(n + 1);
                    self.next_at = now_ms + RETRY_WAITS_MS[n + 1];
                    return TypeStep::Wait;
                }
                let holding = still_holding(&self.text, screen);
                // A live question means the message landed and the CLI asks
                // something: Enter there would answer it.
                if !holding || parse_ask(screen).is_some() {
                    self.stage = Stage::Over;
                    return TypeStep::Done(true);
                }
                if n + 1 >= RETRY_WAITS_MS.len() {
                    self.stage = Stage::Over;
                    return TypeStep::Done(false);
                }
                self.stage = Stage::Check(n + 1);
                self.next_at = now_ms + RETRY_WAITS_MS[n + 1];
                TypeStep::Write(b"\r".to_vec())
            }
            Stage::Over => TypeStep::Done(true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RULE_LINE: &str = "\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}";

    #[test]
    fn text_not_taken_in_yet_is_not_a_sent_message() {
        // Claude still booting: the box shows its own hint, not our text, and
        // the Enter is dropped. Wait; once the text shows in the box, Enter.
        let mut t = Typer::new("fix the login form please", false, 0);
        assert!(matches!(t.step(0, ""), TypeStep::Write(_)));
        let mut now = SUBMIT_DELAY_MS;
        assert_eq!(t.step(now, ""), TypeStep::Write(b"\r".to_vec()));
        now += RETRY_WAITS_MS[0];
        let booting = format!("{RULE_LINE}\n> Try \"edit <filepath> to...\"\n{RULE_LINE}\n");
        assert_eq!(t.step(now, &booting), TypeStep::Wait);
        assert_eq!(t.step(now + 100_000, "   \n  "), TypeStep::Wait, "a blank redraw is no answer either");
        let held = format!("{RULE_LINE}\n> fix the login form please\n{RULE_LINE}\n");
        assert_eq!(t.step(now + 200_000, &held), TypeStep::Write(b"\r".to_vec()));
    }

    /// A Claude Code screen: transcript above, the boxed composer holding `typed`.
    fn screen_with(typed: &str) -> String {
        ["\u{25CF} earlier answer", "", RULE_LINE, &format!("> {typed}"), RULE_LINE, "  ? for shortcuts"].join("\n")
    }

    /// Drive a typer with a fixed screen until it finishes; returns the writes.
    fn run(t: &mut Typer, screen: &str) -> (Vec<Vec<u8>>, bool) {
        let mut now = 0;
        let mut writes = Vec::new();
        loop {
            match t.step(now, screen) {
                TypeStep::Write(b) => writes.push(b),
                TypeStep::Wait => now += 10,
                TypeStep::Done(ok) => return (writes, ok),
            }
        }
    }

    #[test]
    fn chunks_by_code_point() {
        assert_eq!(chunk_text("abcde", 2), vec!["ab", "cd", "e"]);
        assert_eq!(chunk_text("\u{1F600}\u{1F600}x", 2), vec!["\u{1F600}\u{1F600}", "x"]);
        assert!(chunk_text("", 3).is_empty());
    }

    #[test]
    fn reads_the_boxed_composer() {
        assert_eq!(composer_text(&screen_with("viet plan di")), "viet plan di");
        assert_eq!(composer_text(&screen_with("")), "");
        let wrapped = [RULE_LINE, "> first half of the message", "  second half of the message", RULE_LINE].join("\n");
        assert_eq!(composer_text(&wrapped), "first half of the message second half of the message");
        let hint = [RULE_LINE, "> ", "  Press up to edit queued messages", RULE_LINE].join("\n");
        assert_eq!(composer_text(&hint), "");
        assert_eq!(composer_text("$ npm test\nall good\n"), "");
    }

    #[test]
    fn reads_boxless_prompts() {
        let codex = ["\u{2022} Working (52s \u{2022} esc to interrupt)", "", "\u{203A} land the archive fix and report back", "", "  gpt high"].join("\n");
        assert_eq!(composer_text(&codex), "land the archive fix and report back");
        let muse = [
            "\u{25C7} Thinking (5s \u{00B7} esc to interrupt)",
            "",
            "\u{2500}\u{2500} Voice input (Alt+V to start) \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
            "\u{276F} take the archive fix and report back",
            RULE_LINE,
            "  muse \u{00B7} YOLO",
        ]
        .join("\n");
        assert_eq!(composer_text(&muse), "take the archive fix and report back");
        let old = ["> a message sent ten minutes ago", "\u{25CF} and the answer", "", "line", "line", "line", "line", "line"].join("\n");
        assert_eq!(composer_text(&old), "");
    }

    #[test]
    fn holds_only_our_own_text() {
        let wrapped = [RULE_LINE, "> Director, the numbers are wrong because", "  the window moved", RULE_LINE].join("\n");
        assert!(still_holding("Director, the numbers are wrong because the window moved", &wrapped));
        let long = format!("{} and finally the last sentence.", "A".repeat(400));
        assert!(still_holding(&long, &screen_with("and finally the last sentence.")));
        assert!(!still_holding("send this now", &screen_with("explain this codebase")));
        assert!(!still_holding("go ahead now", &screen_with("go ahead")));
        let unknown = ["\u{25C7} Working", "", "\u{25C8} Director, land the archive fix and report back", "", "  newcli"].join("\n");
        assert!(still_holding("Director, land the archive fix and report back", &unknown));
        let echo = ["\u{203A} Director, land the archive fix now", "", "\u{203A} ", "", "  gpt high"].join("\n");
        assert!(!still_holding("Director, land the archive fix now", &echo));
        assert!(!still_holding("npm test run now please", "$ npm test\nall good\n"));
    }

    #[test]
    fn types_chunks_then_a_lone_enter() {
        let text = "x".repeat(250);
        let mut t = Typer::new(&text, false, 0);
        let (writes, ok) = run(&mut t, &screen_with(""));
        assert!(ok);
        assert_eq!(writes.len(), 4, "three chunks then Enter");
        assert_eq!(writes[3], b"\r");
        assert_eq!(writes[..3].concat(), text.as_bytes());
    }

    #[test]
    fn bracketed_paste_sends_one_marked_block() {
        let mut t = Typer::new("fix the login form", true, 0);
        let (writes, ok) = run(&mut t, &screen_with(""));
        assert!(ok);
        assert_eq!(writes[0], format!("{PASTE_START}fix the login form{PASTE_END}").into_bytes());
        assert_eq!(writes[1], b"\r");
    }

    #[test]
    fn presses_enter_again_while_the_text_sits_in_the_box() {
        let msg = "fix the flaky upload test please";
        let mut t = Typer::new(msg, false, 0);
        let (writes, ok) = run(&mut t, &screen_with(msg));
        assert!(!ok, "still in the box after every retry");
        let enters = writes.iter().filter(|w| w.as_slice() == b"\r").count();
        assert_eq!(enters, RETRY_WAITS_MS.len(), "first Enter plus one per retry but the last check");
    }
}
