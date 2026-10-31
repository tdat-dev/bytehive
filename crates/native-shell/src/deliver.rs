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

/// How long a fresh CLI gets to reach its prompt (the web app's default).
pub const JOB_DELAY_MS: u64 = 4000;
/// A question nobody answers for this long drops the task; it is never
/// answered on your behalf.
pub const GIVE_UP_MS: u64 = 15 * 60_000;

pub struct Delivery {
    pub pane_id: String,
    text: String,
    start_ms: u64,
    /// Set once a question showed: when it went away (None while it shows).
    answered: Option<Option<u64>>,
    typer: Option<Typer>,
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
    pub fn new(pane_id: String, text: String, now_ms: u64) -> Delivery {
        Delivery { pane_id, text, start_ms: now_ms + JOB_DELAY_MS, answered: None, typer: None }
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
            if now_ms - at < JOB_DELAY_MS {
                return Tick::Wait;
            }
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
        let mut d = Delivery::new("p".into(), "fix it".into(), 0);
        let (at, b) = first_write(&mut d, 0, "> ");
        assert!(at >= JOB_DELAY_MS);
        assert_eq!(b, b"fix it");
    }

    #[test]
    fn never_types_into_an_open_question() {
        assert!(parse_ask(TRUST).is_some(), "fixture reads as a question");
        let mut d = Delivery::new("p".into(), "fix it".into(), 0);
        for now in (0..20_000).step_by(100) {
            assert_eq!(d.tick(now, true, TRUST, false), Tick::Wait);
        }
        // Answered at 20s: the CLI gets the head start again before typing.
        let (at, _) = first_write(&mut d, 20_000, "> ");
        assert!(at >= 20_000 + JOB_DELAY_MS, "typed at {at}");
    }

    #[test]
    fn a_stopped_pane_drops_the_task() {
        let mut d = Delivery::new("p".into(), "fix it".into(), 0);
        assert_eq!(d.tick(10_000, false, "", false), Tick::Finished);
    }
}
