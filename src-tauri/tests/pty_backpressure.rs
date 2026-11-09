#![cfg(windows)]
//! The state a held reader leaves a console in: its output pipe full and nobody
//! reading it. The flow gate depends on the console still taking input and
//! resizes then, and on a close not waiting for a reader.

use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytehive_lib::core::command_spec::CommandSpec;
use bytehive_lib::core::pty_session::PtySession;
use portable_pty::PtySize;

fn size() -> PtySize {
    PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 }
}

/// Runs `f` on its own thread; the time it took, or None if it did not return in `limit`.
fn timed<T: Send + 'static>(limit: Duration, f: impl FnOnce() -> T + Send + 'static) -> Option<(Duration, T)> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let t = Instant::now();
        let v = f();
        let _ = tx.send((t.elapsed(), v));
    });
    rx.recv_timeout(limit).ok()
}

#[test]
#[serial_test::serial]
fn a_console_nobody_reads_can_still_be_typed_to_resized_and_closed() {
    let dir = tempfile::tempdir().expect("temp dir");
    let file = dir.path().join("flood.txt");
    let line = format!("{}\r\n", "x".repeat(78));
    std::fs::write(&file, line.repeat(6 * 1024 * 1024 / line.len())).expect("write the flood file");
    let spec = CommandSpec::new("cmd.exe").arg("/C").arg("type").arg(file.to_string_lossy().to_string());
    let (mut session, mut reader, mut child) = PtySession::spawn(&spec, size()).expect("spawn");
    let _ = session.write_input(b"\x1b[1;1R");
    std::thread::sleep(Duration::from_secs(3)); // the pipe is full and the child is stuck in its write

    let session = Arc::new(Mutex::new(session));
    let s = session.clone();
    let resize = timed(Duration::from_secs(5), move || s.lock().unwrap().resize(100, 30));
    let s = session.clone();
    let typing = timed(Duration::from_secs(5), move || s.lock().unwrap().write_input(b"x"));
    assert!(resize.is_some_and(|(d, r)| r.is_ok() && d < Duration::from_millis(500)), "an unread console must still resize");
    assert!(typing.is_some_and(|(d, r)| r.is_ok() && d < Duration::from_millis(500)), "an unread console must still take input");

    // Reading again gets the data that was waiting: the child was only blocked.
    let mut buf = [0u8; 65536];
    assert!(reader.read(&mut buf).unwrap_or(0) > 0);

    let s = session.clone();
    let close = timed(Duration::from_secs(8), move || {
        let spare = PtySession::spawn(&CommandSpec::new("cmd.exe").arg("/C").arg("exit"), size()).expect("spare").0;
        std::mem::replace(&mut *s.lock().unwrap(), spare).shutdown();
    });
    assert!(close.is_some(), "closing a console nobody reads must not hang");
    let _ = child.kill();
}
