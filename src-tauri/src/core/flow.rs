//! Backpressure between a PTY and the window that shows it.
//!
//! The window tells the backend how many bytes it has finished parsing (`ack`).
//! The reader thread counts what it has sent (`sent`); when too much is sent and
//! not yet parsed, the reader waits before its next read, so the child's own
//! write blocks on a full pipe instead of the data piling up in the webview.
//!
//! Three rules keep this from ever hurting the person using the terminal:
//!  - input never waits (it does not touch this): typing, resizing and Ctrl+C
//!    go straight to the pseudo-console. Any input also opens the gate for a
//!    moment (`open_for`), so the program's answer to it is not held back;
//!  - a window that stops answering (minimized, hung, gone) cannot freeze the
//!    child: after `stall` without an ack the gate lets `trickle` bytes more
//!    through, again and again, so the child slows down but never stops;
//!  - closing the flow (the agent is going away) wakes the reader at once.

use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

/// Limits for one agent's flow. See the field notes for why these values.
#[derive(Clone, Copy, Debug)]
pub struct FlowConfig {
    /// Sent and not yet parsed bytes at which the reader stops reading.
    pub high: usize,
    /// ...and starts again once the window has parsed down to this.
    pub low: usize,
    /// How long a held reader waits for any ack before it lets `trickle` through.
    pub stall: Duration,
    /// Bytes let through per `stall` while no ack arrives.
    pub trickle: usize,
}

impl Default for FlowConfig {
    fn default() -> Self {
        // xterm parses several MB a second, so 512 KB is under a tenth of a
        // second of work for a window that is keeping up, and 12 agents hold
        // at most 6 MB between them. 2 s and 32 KB leave a stalled window with
        // 16 KB a second per agent: slow, alive, and bounded.
        FlowConfig { high: 512 * 1024, low: 128 * 1024, stall: Duration::from_secs(2), trickle: 32 * 1024 }
    }
}

struct State {
    on: bool,
    inflight: usize,
    held: bool,
    closed: bool,
    open_until: Option<Instant>,
    last_input: Option<Instant>,
}

pub struct Flow {
    cfg: FlowConfig,
    state: Mutex<State>,
    cv: Condvar,
}

impl Flow {
    /// A flow that lets everything through until `enable` is called: a consumer
    /// that never acks (a test, a pipe agent) must not stall its child.
    pub fn new(cfg: FlowConfig) -> Self {
        Flow {
            cfg,
            state: Mutex::new(State { on: false, inflight: 0, held: false, closed: false, open_until: None, last_input: None }),
            cv: Condvar::new(),
        }
    }

    fn st(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The consumer will ack from now on. Bytes it already holds are forgotten.
    pub fn enable(&self) {
        let mut s = self.st();
        s.on = true;
        s.inflight = 0;
        s.held = false;
        self.cv.notify_all();
    }

    /// A new consumer took over (window detach): start its count from zero.
    pub fn reset(&self) {
        let mut s = self.st();
        s.inflight = 0;
        s.held = false;
        self.cv.notify_all();
    }

    /// `n` bytes went to the window.
    pub fn sent(&self, n: usize) {
        let mut s = self.st();
        if !s.on {
            return;
        }
        s.inflight = s.inflight.saturating_add(n);
        if s.inflight >= self.cfg.high {
            s.held = true;
        }
    }

    /// The window has parsed `n` more bytes.
    pub fn ack(&self, n: usize) {
        let mut s = self.st();
        s.inflight = s.inflight.saturating_sub(n);
        if s.held && s.inflight <= self.cfg.low {
            s.held = false;
        }
        self.cv.notify_all();
    }

    /// Let output through for `d` whatever the count says: the program is about
    /// to answer something the person just did. Also when nothing is held yet,
    /// since the answer itself can be what takes the count over the high mark.
    pub fn open_for(&self, d: Duration) {
        let mut s = self.st();
        let now = Instant::now();
        s.last_input = Some(now);
        s.open_until = Some(now + d);
        self.cv.notify_all();
    }

    /// The agent is going away: nothing waits on this any more.
    pub fn close(&self) {
        let mut s = self.st();
        s.closed = true;
        self.cv.notify_all();
    }

    /// Whether anything was typed (or otherwise sent in) within the last `d`.
    pub fn typed_within(&self, d: Duration) -> bool {
        self.st().last_input.is_some_and(|t| t.elapsed() < d)
    }

    pub fn is_held(&self) -> bool {
        self.st().held
    }

    pub fn inflight(&self) -> usize {
        self.st().inflight
    }

    /// Blocks while the window is behind. Returns when the reader may read:
    /// the window caught up, the gate was opened, the flow closed, or a stalled
    /// window was given its trickle.
    pub fn wait_ready(&self) {
        let mut s = self.st();
        loop {
            if !s.on || s.closed || !s.held {
                return;
            }
            if s.open_until.is_some_and(|t| Instant::now() < t) {
                return;
            }
            s.open_until = None; // the moment is over
            let before = s.inflight;
            let (guard, timeout) = self.cv.wait_timeout(s, self.cfg.stall).unwrap_or_else(|p| p.into_inner());
            s = guard;
            // Nobody acked for a whole `stall`: the window is stuck or gone. Let
            // `trickle` bytes through so the child lives, then hold again.
            if timeout.timed_out() && s.held && s.inflight == before {
                s.inflight = self.cfg.high.saturating_sub(self.cfg.trickle);
                s.held = false;
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;

    fn small() -> FlowConfig {
        FlowConfig { high: 100, low: 40, stall: Duration::from_secs(30), trickle: 10 }
    }

    fn on(cfg: FlowConfig) -> Arc<Flow> {
        let f = Arc::new(Flow::new(cfg));
        f.enable();
        f
    }

    /// Starts a reader that waits for the gate and flags when it gets through.
    fn reader(f: &Arc<Flow>) -> (Arc<AtomicBool>, thread::JoinHandle<()>) {
        let done = Arc::new(AtomicBool::new(false));
        let (f, d) = (f.clone(), done.clone());
        let h = thread::spawn(move || {
            f.wait_ready();
            d.store(true, Ordering::SeqCst);
        });
        (done, h)
    }

    fn settle() {
        thread::sleep(Duration::from_millis(150));
    }

    #[test]
    fn nothing_is_held_until_the_consumer_opts_in() {
        let f = Flow::new(small());
        f.sent(10_000);
        assert!(!f.is_held());
        f.wait_ready(); // returns at once
    }

    #[test]
    fn the_reader_stops_at_the_high_mark_and_goes_on_at_the_low_one() {
        let f = on(small());
        f.sent(100);
        assert!(f.is_held());
        let (done, h) = reader(&f);
        settle();
        assert!(!done.load(Ordering::SeqCst), "held at the high mark");
        f.ack(30); // 70 left: above the low mark, still held
        settle();
        assert!(!done.load(Ordering::SeqCst), "not released until the low mark");
        f.ack(40); // 30 left
        h.join().unwrap();
        assert!(done.load(Ordering::SeqCst));
        assert!(!f.is_held());
    }

    #[test]
    fn a_late_or_doubled_ack_cannot_underflow_the_count() {
        let f = on(small());
        f.sent(10);
        f.ack(1_000);
        assert_eq!(f.inflight(), 0);
    }

    #[test]
    fn a_window_that_stops_answering_still_gets_a_trickle() {
        let f = on(FlowConfig { high: 100, low: 40, stall: Duration::from_millis(60), trickle: 30 });
        f.sent(120);
        let t = Instant::now();
        f.wait_ready(); // no ack ever comes
        let first = t.elapsed();
        assert!(first >= Duration::from_millis(50), "waits a whole stall first, took {first:?}");
        assert!(first < Duration::from_secs(3), "must not wait forever, took {first:?}");
        assert!(!f.is_held());
        f.sent(30); // the trickle is used up
        assert!(f.is_held());
        let t = Instant::now();
        f.wait_ready();
        assert!(t.elapsed() >= Duration::from_millis(50), "and the next trickle takes another stall");
    }

    #[test]
    fn input_opens_the_gate_for_a_moment_then_it_closes_again() {
        let f = on(small());
        f.sent(100);
        f.open_for(Duration::from_millis(400));
        let t = Instant::now();
        f.wait_ready();
        assert!(t.elapsed() < Duration::from_millis(100), "open for the answer to a keystroke");
        thread::sleep(Duration::from_millis(450));
        let (done, h) = reader(&f);
        settle();
        assert!(!done.load(Ordering::SeqCst), "held again once the moment is over");
        f.close();
        h.join().unwrap();
    }

    #[test]
    fn input_just_before_the_window_falls_behind_still_lets_the_answer_through() {
        let f = on(FlowConfig { stall: Duration::from_secs(2), ..small() });
        f.open_for(Duration::from_millis(400)); // a key typed while the window keeps up
        f.sent(100); // the program's answer takes it over the high mark
        let t = Instant::now();
        f.wait_ready();
        assert!(t.elapsed() < Duration::from_millis(100), "the answer to a keystroke waited {:?}", t.elapsed());
    }

    #[test]
    fn closing_wakes_a_held_reader_at_once() {
        let f = on(small());
        f.sent(100);
        let (done, h) = reader(&f);
        settle();
        assert!(!done.load(Ordering::SeqCst));
        let t = Instant::now();
        f.close();
        h.join().unwrap();
        assert!(t.elapsed() < Duration::from_millis(500), "shutdown must not wait for the stall timer");
        assert!(done.load(Ordering::SeqCst));
    }

    #[test]
    fn it_knows_whether_someone_just_typed() {
        let f = on(small());
        assert!(!f.typed_within(Duration::from_secs(5)));
        f.open_for(Duration::from_millis(1)); // any input, held or not
        assert!(f.typed_within(Duration::from_secs(5)));
        thread::sleep(Duration::from_millis(60));
        assert!(!f.typed_within(Duration::from_millis(30)));
    }

    #[test]
    fn a_new_window_starts_from_zero() {
        let f = on(small());
        f.sent(200);
        assert!(f.is_held());
        f.reset();
        assert!(!f.is_held());
        assert_eq!(f.inflight(), 0);
        f.wait_ready();
    }
}
