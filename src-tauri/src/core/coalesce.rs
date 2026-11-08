//! Joins a terminal's many tiny reads into fewer, larger messages for the window.
//!
//! A ConPTY hands output over in pieces of a few dozen bytes, and every piece
//! sent to the webview is a script it has to compile and run: 12 shells printing
//! 130 lines a second each meant about 2400 of those a second, a third of the
//! renderer's main thread and 50 times the memory of the text itself while the
//! window was minimized. Here a message goes out at once when the stream was
//! quiet (a keystroke's echo is not delayed), and while it is busy at most one
//! goes out per `interval`, carrying everything read since. While someone is
//! typing into it (`fast` says so) the interval is the short one, so an echo
//! amid a stream is not held for a whole interval.

use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

struct Buf {
    data: Vec<u8>,
    closed: bool,
    last_sent: Option<Instant>,
}

struct Shared {
    buf: Mutex<Buf>,
    cv: Condvar,
}

pub struct Coalescer {
    shared: Arc<Shared>,
}

impl Coalescer {
    /// `deliver` runs on a thread of its own, never on the caller of `push`, so a
    /// slow window cannot stall the reader. `big` bytes waiting go out at once.
    pub fn new(
        interval: Duration,
        typing_interval: Duration,
        big: usize,
        fast: impl Fn() -> bool + Send + 'static,
        deliver: impl FnMut(Vec<u8>) + Send + 'static,
    ) -> Self {
        let shared = Arc::new(Shared { buf: Mutex::new(Buf { data: Vec::new(), closed: false, last_sent: None }), cv: Condvar::new() });
        let s = shared.clone();
        thread::spawn(move || run(s, interval, typing_interval, big, fast, deliver));
        Coalescer { shared }
    }

    pub fn push(&self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let mut b = self.shared.buf.lock().unwrap_or_else(|p| p.into_inner());
        b.data.extend_from_slice(bytes);
        self.shared.cv.notify_one();
    }
}

impl Drop for Coalescer {
    /// What is waiting still goes out, then the thread ends.
    fn drop(&mut self) {
        let mut b = self.shared.buf.lock().unwrap_or_else(|p| p.into_inner());
        b.closed = true;
        self.shared.cv.notify_one();
    }
}

fn run(shared: Arc<Shared>, interval: Duration, typing_interval: Duration, big: usize, fast: impl Fn() -> bool, mut deliver: impl FnMut(Vec<u8>)) {
    loop {
        let batch = {
            let mut b = shared.buf.lock().unwrap_or_else(|p| p.into_inner());
            while b.data.is_empty() && !b.closed {
                b = shared.cv.wait(b).unwrap_or_else(|p| p.into_inner());
            }
            if b.data.is_empty() {
                return; // closed and nothing left
            }
            // Busy: wait out the rest of the interval, taking more in meanwhile.
            while !b.closed && b.data.len() < big {
                let due = b.last_sent.map(|t| t + if fast() { typing_interval } else { interval });
                let Some(left) = due.and_then(|d| d.checked_duration_since(Instant::now())) else { break };
                b = shared.cv.wait_timeout(b, left).unwrap_or_else(|p| p.into_inner()).0;
            }
            b.last_sent = Some(Instant::now());
            std::mem::take(&mut b.data)
        };
        deliver(batch);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn collect(interval_ms: u64, big: usize) -> (Coalescer, mpsc::Receiver<Vec<u8>>) {
        let (tx, rx) = mpsc::channel();
        (Coalescer::new(Duration::from_millis(interval_ms), Duration::from_millis(interval_ms), big, || false, move |v| { let _ = tx.send(v); }), rx)
    }

    #[test]
    fn a_quiet_stream_is_not_delayed() {
        // With a 10 s interval, anything short of seconds means it did not wait for it.
        let (c, rx) = collect(10_000, 1 << 20);
        let t = Instant::now();
        c.push(b"a");
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), b"a");
        assert!(t.elapsed() < Duration::from_secs(5), "the echo of one key waits for nothing");
    }

    #[test]
    fn a_burst_becomes_few_messages_in_the_same_order() {
        let (c, rx) = collect(100, 1 << 20);
        let mut sent = Vec::new();
        let t = Instant::now();
        for i in 0..200u8 {
            c.push(&[i]);
            sent.push(i);
            thread::sleep(Duration::from_millis(1));
        }
        // A 1 ms sleep takes 15 ms on a busy Windows box: the bound follows the real time taken.
        let most = t.elapsed().as_millis() as usize / 100 + 3;
        let mut got = Vec::new();
        let mut messages = 0;
        while got.len() < sent.len() {
            got.extend(rx.recv_timeout(Duration::from_secs(2)).unwrap());
            messages += 1;
        }
        assert_eq!(got, sent);
        assert!(messages <= most, "200 reads in {messages} messages, at most {most} for one per 100 ms");
    }

    #[test]
    fn while_someone_types_the_short_interval_applies() {
        let typing = Arc::new(Mutex::new(false));
        let t = typing.clone();
        let (tx, rx) = mpsc::channel();
        let c = Coalescer::new(Duration::from_secs(30), Duration::from_millis(20), 1 << 20, move || *t.lock().unwrap(), move |v| { let _ = tx.send(v); });
        c.push(b"first");
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        c.push(b"slow");
        assert!(rx.recv_timeout(Duration::from_millis(300)).is_err(), "the long interval holds it");
        *typing.lock().unwrap() = true;
        c.push(b"-echo");
        assert_eq!(rx.recv_timeout(Duration::from_secs(10)).unwrap(), b"slow-echo", "well before the 30 s interval");
    }

    #[test]
    fn a_big_pile_goes_out_without_waiting_for_the_interval() {
        let (c, rx) = collect(30_000, 1000);
        c.push(b"first");
        rx.recv_timeout(Duration::from_secs(5)).unwrap(); // leading edge
        c.push(&[7u8; 1500]);
        assert_eq!(rx.recv_timeout(Duration::from_secs(10)).unwrap().len(), 1500, "well before the 30 s interval");
    }

    #[test]
    fn dropping_it_sends_what_is_left_and_ends_the_thread() {
        let (tx, rx) = mpsc::channel();
        let ended = Arc::new(Mutex::new(false));
        let e = ended.clone();
        struct Mark(Arc<Mutex<bool>>);
        impl Drop for Mark {
            fn drop(&mut self) {
                *self.0.lock().unwrap() = true;
            }
        }
        let mark = Mark(e);
        let c = Coalescer::new(Duration::from_secs(30), Duration::from_secs(30), 1 << 20, || false, move |v| {
            let _keep = &mark; // lives as long as the thread's closure
            let _ = tx.send(v);
        });
        c.push(b"one");
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        c.push(b"two"); // would wait 30 s
        drop(c);
        assert_eq!(rx.recv_timeout(Duration::from_secs(10)).unwrap(), b"two");
        let t = Instant::now();
        while !*ended.lock().unwrap() && t.elapsed() < Duration::from_secs(10) {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(*ended.lock().unwrap(), "the sender thread must end with its owner");
    }
}
