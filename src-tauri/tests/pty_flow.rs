#![cfg(windows)]
//! Output flow control on real ConPTY children. The exact byte counts are tested
//! on a synthetic reader (core/agent.rs flow_tests); the speed of a real console
//! varies too much for that. What only a real ConPTY can show is here: a child
//! held back can still be typed to, resized and torn down.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytehive_lib::core::command_spec::CommandSpec;
use bytehive_lib::core::registry::Registry;
use portable_pty::PtySize;

fn size() -> PtySize {
    PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 }
}

fn pause(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

/// Waits until `cond` holds; fails after `ceiling`, which is generous: these
/// run real consoles and must not flake on a box at 100% CPU.
fn until(what: &str, ceiling: Duration, cond: impl Fn() -> bool) {
    let t = Instant::now();
    while !cond() {
        assert!(t.elapsed() < ceiling, "{what} (after {:?})", t.elapsed());
        pause(20);
    }
}

/// A child that prints 24 MB as fast as the console takes it, in a big console
/// (ConPTY sends what it draws, so more cells reach the high mark sooner).
fn flood(dir: &std::path::Path) -> CommandSpec {
    let file = dir.join("flood.txt");
    let line = format!("{}\r\n", "x".repeat(240));
    std::fs::write(&file, line.repeat(24 * 1024 * 1024 / line.len())).expect("write the flood file");
    CommandSpec::new("cmd.exe").arg("/C").arg("type").arg(file.to_string_lossy().to_string())
}

fn big() -> PtySize {
    PtySize { rows: 100, cols: 250, pixel_width: 0, pixel_height: 0 }
}

/// A flooding agent whose window never acks, held: flow on from the start, and
/// the test only goes on once the reader is held.
fn held() -> (Registry, Arc<AtomicUsize>, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let count = Arc::new(AtomicUsize::new(0));
    let c = count.clone();
    let mut reg = Registry::new();
    reg.spawn("a".into(), &flood(dir.path()), big(), move |b| { c.fetch_add(b.len(), Ordering::SeqCst); }, |_| {}).expect("spawn");
    reg.enable_flow("a", 1).expect("flow on");
    reg.write_input("a", b"\x1b[1;1R").expect("answer the startup query");
    let flow = reg.flow_of("a").expect("flow");
    until("the flood is held back by a window that never acks", Duration::from_secs(90), || flow.is_held());
    (reg, count, dir)
}

#[test]
#[serial_test::serial]
fn typing_and_resizing_do_not_wait_for_a_held_child() {
    let (mut reg, _count, _dir) = held();
    assert!(reg.flow_of("a").unwrap().is_held());
    let t = Instant::now();
    reg.write_input("a", b"x").expect("typing");
    assert!(t.elapsed() < Duration::from_secs(5), "typing waited {:?}", t.elapsed());
    let t = Instant::now();
    reg.resize("a", 100, 30).expect("resize");
    assert!(t.elapsed() < Duration::from_secs(5), "resizing waited {:?}", t.elapsed());
}

#[test]
#[serial_test::serial]
fn an_agent_held_back_can_be_killed_without_waiting_for_the_window() {
    let (mut reg, count, _dir) = held();
    assert!(reg.flow_of("a").unwrap().is_held());
    let gone = reg.take("a").expect("agent");
    let t = Instant::now();
    drop(gone);
    assert!(t.elapsed() < Duration::from_secs(10), "teardown of a held agent took {:?}", t.elapsed());
    // Nothing keeps flowing from a killed agent: the count settles.
    let t = Instant::now();
    loop {
        let a = count.load(Ordering::SeqCst);
        pause(500);
        if count.load(Ordering::SeqCst) == a {
            break;
        }
        assert!(t.elapsed() < Duration::from_secs(15), "output kept flowing from a killed agent");
    }
}

/// A program that prints without end and stops when `q` is pressed, saying so.
/// Interrupting a printing program is what Ctrl+C is for; a raw 0x03 does not
/// reach a child of the test harness as a signal, so a key it reads stands in.
fn stoppable_printer() -> CommandSpec {
    let script = "$s = ('x' * 240 + [char]13 + [char]10) * 50; \
        while ($true) { if ([Console]::KeyAvailable -and [Console]::ReadKey($true).KeyChar -eq 'q') { [Console]::Out.Write('STOP' + 'PED'); break }; [Console]::Out.Write($s) }; \
        Start-Sleep 60";
    CommandSpec::new("powershell.exe").arg("-NoProfile").arg("-Command").arg(script)
}

/// Whether pressing `q` stops the printer and its answer arrives within `within`.
/// With `hold`, the window never acks, and the key is pressed once the reader is
/// held. The console is big because ConPTY sends what it draws: more cells
/// reach the 512 KB mark sooner.
fn interrupt_gets_through(hold: bool, within: Duration) -> Result<(), String> {
    let out = Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
    let o = out.clone();
    let mut reg = Registry::new();
    let big = PtySize { rows: 100, cols: 250, pixel_width: 0, pixel_height: 0 };
    reg.spawn("a".into(), &stoppable_printer(), big, move |b| o.lock().unwrap_or_else(|p| p.into_inner()).extend_from_slice(b), |_| {})
        .expect("spawn");
    let flow = reg.flow_of("a").expect("flow");
    if hold {
        reg.enable_flow("a", 1).expect("flow on");
    }
    reg.write_input("a", b"\x1b[1;1R").expect("answer the startup query");
    if hold {
        let t = Instant::now();
        while !flow.is_held() {
            if t.elapsed() > Duration::from_secs(90) {
                return Err(format!("never held (in flight: {})", flow.inflight()));
            }
            pause(100);
        }
        pause(2_000); // the moment and budget the startup answer opened are long over
    } else {
        pause(3_000); // printing for a while
    }
    reg.write_input("a", b"q").expect("the key");
    let t = Instant::now();
    let answered = loop {
        if String::from_utf8_lossy(&out.lock().unwrap()).contains("STOPPED") {
            break true;
        }
        if t.elapsed() > within {
            break false;
        }
        pause(50);
    };
    let held = flow.is_held();
    let _ = reg.take("a");
    if answered { Ok(()) } else { Err(format!("no answer in {within:?} (held: {held})")) }
}

#[test]
#[serial_test::serial]
fn interrupting_a_held_program_gets_its_answer_through() {
    // The probe works on a console nobody holds back...
    interrupt_gets_through(false, Duration::from_secs(20)).expect("without flow control");
    // ...and a window that never acks does not keep the answer from coming.
    interrupt_gets_through(true, Duration::from_secs(20)).expect("with the reader held back");
}

/// The window's coalescer can hold an agent's last output for its whole
/// interval. That must not let the exit event overtake it: whatever the child
/// printed is in the sink before on_exit runs.
#[test]
fn an_agents_last_output_reaches_the_window_before_its_exit() {
    use bytehive_lib::core::coalesce::Coalescer;
    enum Ev {
        Bytes(Vec<u8>),
        Exit,
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let out = tx.clone();
    // 30 s: without a flush on exit the last words would wait that long.
    let joined = Coalescer::new(Duration::from_secs(30), Duration::from_secs(30), 1 << 20, || false, move |b| {
        let _ = out.send(Ev::Bytes(b));
    });
    let sink = move |b: &[u8]| if b.is_empty() { joined.flush() } else { joined.push(b) };
    // Filler first and a pause, so the mark comes after the leading-edge message
    // and the coalescer holds it for the interval.
    let spec = CommandSpec::new("cmd.exe").arg("/C").arg("echo filler& ping -n 2 127.0.0.1 >nul& echo FINAL-MARK");
    let mut reg = Registry::new();
    reg.spawn("a".into(), &spec, size(), sink, move |_| {
        let _ = tx.send(Ev::Exit);
    })
    .expect("spawn");
    reg.write_input("a", b"\x1b[1;1R").expect("answer the startup query");
    let mut chunks: Vec<Vec<u8>> = Vec::new();
    loop {
        match rx.recv_timeout(Duration::from_secs(25)).expect("the exit is reported well before the 30 s interval") {
            Ev::Bytes(b) => chunks.push(b),
            Ev::Exit => break,
        }
    }
    let text = |c: &[Vec<u8>]| String::from_utf8_lossy(&c.concat()).into_owned();
    assert!(!text(&chunks[..1]).contains("FINAL-MARK"), "the mark must not ride in the leading-edge message: {:?}", text(&chunks[..1]));
    assert!(text(&chunks).contains("FINAL-MARK"), "the last output came before the exit: {:?}", text(&chunks));
}
