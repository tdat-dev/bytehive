//! Backpressure between a PTY and the window that shows it.
//!
//! The reader thread counts every byte it sends (`sent`), and the window says
//! how far it has parsed (`ack`), both as running totals since the window took
//! the output over. While too much is sent and not yet parsed, the reader waits
//! before its next read, so the child's own write blocks on a full pipe instead
//! of the data piling up in the webview.
//!
//! The window's totals are tagged with an epoch it chose when it took the
//! output (spawn or attach). An ack for another epoch is from a window that no
//! longer owns the output and counts for nothing. A total lower than one seen
//! before is late and changes nothing; a lost one is made good by the next.
//!
//! Three rules keep this from ever hurting the person using the terminal:
//!  - input never waits (it does not touch this): typing, resizing and Ctrl+C
//!    go straight to the pseudo-console. Any input also opens the gate for a
//!    moment and a byte budget (`open_for`), so the program's answer to it is
//!    not held back, and a program that keeps asking cannot turn this off;
//!  - a window that stops answering (minimized, hung) cannot freeze the child:
//!    after `stall` without an ack the gate lets `trickle` bytes more through,
//!    again and again, so the child slows down but never stops. A window that
//!    is gone turns the flow off (`off`);
//!  - closing the flow (the agent is going away) wakes the reader at once.

use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

/// Limits for one agent's flow. See the field notes for why these values.
#[derive(Clone, Copy, Debug)]
pub struct FlowConfig {
    /// Sent and not yet parsed bytes at which the reader stops reading.
    pub high: u64,
    /// ...and starts again once the window has parsed down to this.
    pub low: u64,
    /// How long a held reader waits for any ack before it lets `trickle` through.
    pub stall: Duration,
    /// Bytes let through per `stall` while no ack arrives.
    pub trickle: u64,
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
    epoch: u64,
    /// Bytes sent to the window of `epoch`, and how many of them it has parsed.
    sent: u64,
    parsed: u64,
    held: bool,
    closed: bool,
    /// A window took the output over (`begin`): the spawning one's `enable`,
    /// should it come late, no longer applies.
    begun: bool,
    /// After input the reader reads freely until this time, while `sent` is under `open_to`.
    open_until: Option<Instant>,
    open_to: u64,
    /// A stalled window was given its trickle: not held again before `sent` reaches this.
    trickle_to: u64,
    last_input: Option<Instant>,
}

impl State {
    fn inflight(&self) -> u64 {
        self.sent.saturating_sub(self.parsed)
    }
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
            state: Mutex::new(State {
                on: false,
                epoch: 0,
                sent: 0,
                parsed: 0,
                held: false,
                closed: false,
                begun: false,
                open_until: None,
                open_to: 0,
                trickle_to: 0,
                last_input: None,
            }),
            cv: Condvar::new(),
        }
    }

    fn st(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The window that has had every byte since the agent started acks from
    /// now on, as `epoch` (pty_spawn). The count of what was sent stays. Does
    /// nothing once another window took the output over: pty_spawn can finish
    /// after a pty_attach for the same agent.
    pub fn enable(&self, epoch: u64) {
        let mut s = self.st();
        if s.begun {
            return;
        }
        s.on = true;
        s.epoch = epoch;
        self.cv.notify_all();
    }

    /// A new window takes the output over (attach), as `epoch`, and counts from
    /// zero: call this before the first byte it is sent.
    pub fn begin(&self, epoch: u64) {
        let mut s = self.st();
        s.on = true;
        s.begun = true;
        s.epoch = epoch;
        s.sent = 0;
        s.parsed = 0;
        s.held = false;
        s.trickle_to = 0;
        self.cv.notify_all();
    }

    /// Nobody can ack any more (the window is gone or reloading): let
    /// everything through until a window takes the output over again.
    pub fn off(&self) {
        let mut s = self.st();
        s.on = false;
        s.held = false;
        self.cv.notify_all();
    }

    /// The window of `epoch` is gone: `off`, unless another window has taken
    /// the output over since. An old window's sink drops after the new one's
    /// `begin`, and its last send failing says nothing about the new window.
    pub fn off_if(&self, epoch: u64) {
        let mut s = self.st();
        if s.epoch != epoch {
            return;
        }
        s.on = false;
        s.held = false;
        self.cv.notify_all();
    }

    /// `n` bytes went to the window.
    pub fn sent(&self, n: usize) {
        let mut s = self.st();
        s.sent = s.sent.saturating_add(n as u64);
        if s.on && !s.held && s.inflight() >= self.cfg.high && s.sent >= s.trickle_to {
            s.held = true;
        }
    }

    /// The window of `epoch` has parsed `total` bytes in all.
    pub fn ack(&self, epoch: u64, total: u64) {
        let mut s = self.st();
        if epoch != s.epoch || total <= s.parsed {
            return; // another window's, or late
        }
        s.parsed = total;
        if s.held && s.inflight() <= self.cfg.low {
            s.held = false;
        }
        self.cv.notify_all();
    }

    /// Let output through for `d`, and at most `budget` bytes, whatever the
    /// count says: the program is about to answer something the person just
    /// did. Also when nothing is held yet, since the answer itself can be what
    /// takes the count over the high mark. Input while the gate is still open
    /// (moment not over, budget not spent) changes nothing, so a program that
    /// keeps querying the terminal gets one budget, not one per query.
    pub fn open_for(&self, d: Duration, budget: u64) {
        let mut s = self.st();
        let now = Instant::now();
        s.last_input = Some(now);
        if s.open_until.is_some_and(|t| now < t) && s.sent < s.open_to {
            return;
        }
        s.open_until = Some(now + d);
        s.open_to = s.sent.saturating_add(budget);
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

    pub fn is_on(&self) -> bool {
        self.st().on
    }

    pub fn inflight(&self) -> u64 {
        self.st().inflight()
    }

    /// Blocks while the window is behind. Returns when the reader may read:
    /// the window caught up, the gate was opened, the flow closed or went off,
    /// or a stalled window was given its trickle.
    pub fn wait_ready(&self) {
        let mut s = self.st();
        loop {
            if !s.on || s.closed || !s.held {
                return;
            }
            if s.open_until.is_some_and(|t| Instant::now() < t) && s.sent < s.open_to {
                return;
            }
            let before = s.parsed;
            let (guard, timeout) = self.cv.wait_timeout(s, self.cfg.stall).unwrap_or_else(|p| p.into_inner());
            s = guard;
            // Nobody acked for a whole `stall`: the window is stuck. Let
            // `trickle` bytes through so the child lives, then hold again. The
            // count itself stays what it is.
            if timeout.timed_out() && s.held && s.parsed == before {
                s.trickle_to = s.sent.saturating_add(self.cfg.trickle);
                s.held = false;
                return;
            }
        }
    }
}

/// Every agent's flow by id, apart from the registry: an ack comes in for
/// each terminal several times a second and must not queue behind the registry
/// lock, which spawning and tearing down a console holds for long. Holds weak
/// handles, so an agent that is gone takes its flow with it.
#[derive(Default)]
pub struct Flows {
    by_id: Mutex<std::collections::HashMap<String, std::sync::Weak<Flow>>>,
}

impl Flows {
    pub fn put(&self, id: &str, flow: &std::sync::Arc<Flow>) {
        let mut m = self.by_id.lock().unwrap_or_else(|p| p.into_inner());
        m.retain(|_, w| w.strong_count() > 0);
        m.insert(id.to_string(), std::sync::Arc::downgrade(flow));
    }

    pub fn get(&self, id: &str) -> Option<std::sync::Arc<Flow>> {
        self.by_id.lock().unwrap_or_else(|p| p.into_inner()).get(id).and_then(|w| w.upgrade())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;

    /// Long enough that no test ever reaches it by accident, even at 100% CPU.
    const NEVER: Duration = Duration::from_secs(60);
    /// A generous ceiling for something that should happen at once.
    const SOON: Duration = Duration::from_secs(5);

    fn small() -> FlowConfig {
        FlowConfig { high: 100, low: 40, stall: NEVER, trickle: 10 }
    }

    fn on(cfg: FlowConfig) -> Arc<Flow> {
        let f = Arc::new(Flow::new(cfg));
        f.enable(1);
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

    /// Joins `h` or fails after `SOON`.
    fn joined(h: thread::JoinHandle<()>, what: &str) {
        let t = Instant::now();
        while !h.is_finished() {
            assert!(t.elapsed() < SOON, "{what}");
            thread::sleep(Duration::from_millis(5));
        }
        h.join().unwrap();
    }

    /// Time for a reader that should stay blocked to show it does not.
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
        f.ack(1, 30); // 70 left: above the low mark, still held
        settle();
        assert!(!done.load(Ordering::SeqCst), "not released until the low mark");
        f.ack(1, 60); // 40 left
        joined(h, "released at the low mark");
        assert!(!f.is_held());
    }

    #[test]
    fn a_lost_ack_is_made_good_by_the_next_one() {
        let f = on(small());
        f.sent(100);
        assert!(f.is_held());
        // the ack for 50 never arrives (failed invoke, dropped callback)
        f.ack(1, 100);
        assert_eq!(f.inflight(), 0);
        assert!(!f.is_held());
    }

    #[test]
    fn a_late_ack_changes_nothing() {
        let f = on(small());
        f.sent(100);
        f.ack(1, 80);
        f.ack(1, 50); // overtaken on the way
        assert_eq!(f.inflight(), 20);
        f.ack(1, 1_000); // more than was ever sent cannot go below zero
        assert_eq!(f.inflight(), 0);
    }

    #[test]
    fn an_old_windows_acks_count_for_nothing() {
        let f = on(small());
        f.sent(80);
        f.begin(2); // a new window took over (attach)
        f.sent(100);
        assert!(f.is_held());
        f.ack(1, 100); // the old window catching up
        assert!(f.is_held());
        assert_eq!(f.inflight(), 100);
        f.ack(2, 100);
        assert!(!f.is_held());
    }

    #[test]
    fn a_window_that_stops_answering_still_gets_a_trickle_and_the_count_never_rises() {
        let f = on(FlowConfig { high: 100, low: 40, stall: Duration::from_millis(60), trickle: 30 });
        f.sent(90);
        f.ack(1, 50); // 40 in flight
        f.sent(70); // 110 in flight: held
        assert!(f.is_held());
        let t = Instant::now();
        f.wait_ready(); // no ack ever comes
        assert!(t.elapsed() >= Duration::from_millis(50), "waits a whole stall first, took {:?}", t.elapsed());
        assert!(!f.is_held());
        assert_eq!(f.inflight(), 110, "the trickle does not touch the count");
        f.sent(29);
        assert!(!f.is_held(), "within the trickle");
        f.sent(1); // the trickle is used up
        assert!(f.is_held());
        assert_eq!(f.inflight(), 140);
        let t = Instant::now();
        f.wait_ready();
        assert!(t.elapsed() >= Duration::from_millis(50), "and the next trickle takes another stall");
        f.ack(1, 190); // the window wakes up: one ack puts it right
        assert_eq!(f.inflight(), 0);
    }

    #[test]
    fn input_opens_the_gate_for_a_moment_then_it_closes_again() {
        let f = on(small());
        f.sent(100);
        f.open_for(Duration::from_millis(300), 1 << 20);
        let (done, h) = reader(&f);
        joined(h, "open for the answer to a keystroke");
        assert!(done.load(Ordering::SeqCst));
        thread::sleep(Duration::from_millis(350));
        let (done, h) = reader(&f);
        settle();
        assert!(!done.load(Ordering::SeqCst), "held again once the moment is over");
        f.close();
        joined(h, "close wakes it");
    }

    #[test]
    fn input_opens_the_gate_for_a_byte_budget_only() {
        let f = on(small());
        f.sent(100);
        f.open_for(NEVER, 50);
        f.wait_ready(); // open
        f.sent(49);
        f.wait_ready(); // still within the budget
        f.sent(1);
        let (done, h) = reader(&f);
        settle();
        assert!(!done.load(Ordering::SeqCst), "a program that answers with a flood is held after the budget");
        f.open_for(NEVER, 50); // the next key gives a new budget
        joined(h, "a new key opens it again");
    }

    #[test]
    fn input_just_before_the_window_falls_behind_still_lets_the_answer_through() {
        let f = on(small());
        f.open_for(NEVER, 1 << 20); // a key typed while the window keeps up
        f.sent(100); // the program's answer takes it over the high mark
        assert!(f.is_held());
        let (_, h) = reader(&f);
        joined(h, "the answer to a keystroke must not wait");
    }

    #[test]
    fn closing_wakes_a_held_reader_at_once() {
        let f = on(small());
        f.sent(100);
        let (done, h) = reader(&f);
        settle();
        assert!(!done.load(Ordering::SeqCst));
        f.close();
        joined(h, "shutdown must not wait for the stall timer");
    }

    #[test]
    fn a_window_that_is_gone_turns_the_flow_off_and_a_new_one_back_on() {
        let f = on(small());
        f.sent(100);
        let (_, h) = reader(&f);
        settle();
        f.off();
        joined(h, "off wakes a held reader");
        f.sent(10_000);
        assert!(!f.is_held(), "nothing is held while nobody can ack");
        f.begin(2);
        f.sent(100);
        assert!(f.is_held(), "the new window's count holds again");
    }

    #[test]
    fn a_dying_old_window_cannot_turn_the_new_ones_flow_off() {
        let f = on(small()); // window 1
        f.begin(2); // window 2 took over; window 1's sink drops and its last send fails
        f.off_if(1);
        assert!(f.is_on(), "the old window's failure is not the new one's");
        f.sent(100);
        assert!(f.is_held());
        f.off_if(2); // the new window's own failure does count
        assert!(!f.is_on());
        assert!(!f.is_held());
    }

    #[test]
    fn input_while_the_gate_is_open_does_not_renew_its_budget() {
        let f = on(small());
        f.sent(100);
        f.open_for(NEVER, 50);
        f.sent(40);
        f.open_for(NEVER, 50); // the gate is still open: no new budget
        f.sent(10);
        let (done, h) = reader(&f);
        settle();
        assert!(!done.load(Ordering::SeqCst), "a program queried 5 times a second still stops at one budget");
        f.open_for(NEVER, 50); // spent: this one opens it again
        joined(h, "a key after the budget is spent opens it");
    }

    #[test]
    fn input_while_the_gate_is_open_does_not_renew_its_moment() {
        let f = on(small());
        f.sent(100);
        f.open_for(Duration::from_millis(400), 1 << 20);
        thread::sleep(Duration::from_millis(250));
        f.open_for(Duration::from_millis(400), 1 << 20); // still open: the moment stays as it was
        thread::sleep(Duration::from_millis(250)); // 500 ms after the first key
        let (done, h) = reader(&f);
        settle();
        assert!(!done.load(Ordering::SeqCst), "closed 400 ms after the key that opened it");
        f.close();
        joined(h, "close wakes it");
    }

    #[test]
    fn enable_after_a_window_took_over_changes_nothing() {
        let f = Arc::new(Flow::new(small()));
        f.begin(2); // pty_attach got there first
        f.sent(100);
        f.enable(1); // pty_spawn finishing late
        f.ack(1, 100);
        assert!(f.is_held(), "the spawning window's epoch does not come back");
        f.ack(2, 100);
        assert!(!f.is_held());
        f.off_if(2);
        f.enable(1);
        assert!(!f.is_on(), "nor does it switch a gone window's flow back on");
    }

    #[test]
    fn the_table_finds_a_live_flow_and_forgets_a_gone_one() {
        let t = Flows::default();
        let a = on(small());
        t.put("a", &a);
        assert!(Arc::ptr_eq(&t.get("a").unwrap(), &a));
        let b = on(small());
        t.put("a", &b); // restarted in place
        assert!(Arc::ptr_eq(&t.get("a").unwrap(), &b));
        drop(b);
        assert!(t.get("a").is_none());
        assert!(t.get("nobody").is_none());
    }

    #[test]
    fn it_knows_whether_someone_just_typed() {
        let f = on(small());
        assert!(!f.typed_within(NEVER));
        f.open_for(Duration::from_millis(1), 1); // any input, held or not
        assert!(f.typed_within(NEVER));
        thread::sleep(Duration::from_millis(60));
        assert!(!f.typed_within(Duration::from_millis(30)));
    }
}
