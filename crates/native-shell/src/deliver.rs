//! Handing a new agent its task, like the web app's `deliverTask`: wait for
//! the CLI to reach its prompt, then type the task in with [`Typer`].
//!
//! Some CLIs open on a question first (Claude asks whether to trust a folder
//! it has not seen, like a new worktree, with "No, exit" selected), so the
//! task's Enter would answer it and close the agent. While a question is on
//! screen the task waits for you; once it is answered the CLI gets the same
//! head start again, because answering the trust question restarts Claude's
//! session and a task typed before it is ready is lost.

use supervise::ask::parse_ask;
use supervise::typing::{TypeStep, Typer};

/// How long a fresh CLI gets to reach its prompt (the web app's default;
/// Settings changes it).
pub const JOB_DELAY_MS: u64 = 4000;
/// A question nobody answers for this long drops the task; it is never
/// answered on your behalf.
pub const GIVE_UP_MS: u64 = 15 * 60_000;
/// The screen has to sit still this long before the task goes in: a CLI that
/// is still booting (start-up hooks, a slow first draw) keeps redrawing, and
/// a task typed into it then is lost.
pub const SETTLE_MS: u64 = 1500;
/// Past the head start, never wait longer than this for the screen to settle.
pub const SETTLE_CAP_MS: u64 = 30_000;

pub struct Delivery {
    pub pane_id: String,
    text: String,
    start_ms: u64,
    delay_ms: u64,
    /// Set once a question showed: when it went away (None while it shows).
    answered: Option<Option<u64>>,
    typer: Option<Typer>,
    /// The screen last seen (hashed) and when it last changed.
    seen: u64,
    changed_at: u64,
}

fn screen_hash(screen: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    screen.hash(&mut h);
    h.finish()
}

/// What the app does with a delivery this frame.
#[derive(Debug, PartialEq)]
pub enum Tick {
    Wait,
    Write(Vec<u8>),
    /// Typed (or dropped: the pane stopped, or a question sat too long).
    Finished,
}

impl Delivery {
    pub fn new(pane_id: String, text: String, now_ms: u64, delay_ms: u64) -> Delivery {
        Delivery { pane_id, text, start_ms: now_ms + delay_ms, delay_ms, answered: None, typer: None, seen: 0, changed_at: now_ms }
    }

    /// `paste`: the pane's program turned bracketed paste on.
    pub fn tick(&mut self, now_ms: u64, running: bool, screen: &str, paste: bool) -> Tick {
        if !running {
            return Tick::Finished;
        }
        if let Some(t) = self.typer.as_mut() {
            return match t.step(now_ms, screen) {
                TypeStep::Write(b) => Tick::Write(b),
                TypeStep::Wait => Tick::Wait,
                TypeStep::Done(_) => Tick::Finished,
            };
        }
        let h = screen_hash(screen);
        if h != self.seen {
            self.seen = h;
            self.changed_at = now_ms;
        }
        if now_ms < self.start_ms {
            return Tick::Wait;
        }
        if parse_ask(screen).is_some() {
            self.answered = Some(None);
            return if now_ms - self.start_ms > GIVE_UP_MS { Tick::Finished } else { Tick::Wait };
        }
        if let Some(at) = self.answered {
            let at = at.unwrap_or(now_ms);
            self.answered = Some(Some(at));
            if now_ms - at < self.delay_ms {
                return Tick::Wait;
            }
        }
        // Still drawing (booting): wait for it to settle, up to the cap. Only a
        // first task waits; a message to a busy agent (no head start) goes now.
        // A blank screen sits still too: a CLI that cleared it at boot has not
        // drawn its prompt yet.
        let unsettled = now_ms - self.changed_at < SETTLE_MS || screen.trim().is_empty();
        if self.delay_ms > 0 && unsettled && now_ms - self.start_ms < SETTLE_CAP_MS {
            return Tick::Wait;
        }
        self.typer = Some(Typer::new(&self.text, paste, now_ms));
        Tick::Wait
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRUST: &str = "\n Do you trust the files in this folder?\n\n\u{276F} 1. Yes, proceed\n  2. No, exit\n\n Enter to confirm \u{00B7} Esc to cancel\n";

    fn first_write(d: &mut Delivery, mut now: u64, screen: &str) -> (u64, Vec<u8>) {
        loop {
            if let Tick::Write(b) = d.tick(now, true, screen, false) {
                return (now, b);
            }
            now += 100;
            assert!(now < 60_000, "never typed");
        }
    }

    #[test]
    fn waits_for_the_cli_to_boot_then_types() {
        let mut d = Delivery::new("p".into(), "fix it".into(), 0, JOB_DELAY_MS);
        let (at, b) = first_write(&mut d, 0, "> ");
        assert!(at >= JOB_DELAY_MS);
        assert_eq!(b, b"fix it");
    }

    #[test]
    fn never_types_into_an_open_question() {
        assert!(parse_ask(TRUST).is_some(), "fixture reads as a question");
        let mut d = Delivery::new("p".into(), "fix it".into(), 0, JOB_DELAY_MS);
        for now in (0..20_000).step_by(100) {
            assert_eq!(d.tick(now, true, TRUST, false), Tick::Wait);
        }
        // Answered at 20s: the CLI gets the head start again before typing.
        let (at, _) = first_write(&mut d, 20_000, "> ");
        assert!(at >= 20_000 + JOB_DELAY_MS, "typed at {at}");
    }

    #[test]
    fn waits_for_a_slow_boot_to_settle() {
        // Start-up hooks keep printing until 9s: nothing is typed into that.
        let mut d = Delivery::new("p".into(), "fix it".into(), 0, JOB_DELAY_MS);
        for now in (0..9_000).step_by(100) {
            let screen = format!("booting {}", now / 500);
            assert_eq!(d.tick(now, true, &screen, false), Tick::Wait, "at {now}");
        }
        let (at, b) = first_write(&mut d, 9_000, "> ");
        assert!(at >= 9_000 + SETTLE_MS, "typed at {at}");
        assert_eq!(b, b"fix it");
    }

    #[test]
    fn a_screen_that_never_settles_still_gets_the_task() {
        let mut d = Delivery::new("p".into(), "fix it".into(), 0, JOB_DELAY_MS);
        let mut now = 0;
        loop {
            if let Tick::Write(_) = d.tick(now, true, &format!("spinner {now}"), false) {
                break;
            }
            now += 100;
            assert!(now < 60_000, "never typed");
        }
        assert!(now >= JOB_DELAY_MS + SETTLE_CAP_MS);
    }

    #[test]
    fn a_stopped_pane_drops_the_task() {
        let mut d = Delivery::new("p".into(), "fix it".into(), 0, JOB_DELAY_MS);
        assert_eq!(d.tick(10_000, false, "", false), Tick::Finished);
    }
}
