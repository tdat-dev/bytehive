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

/// A child that prints 6 MB as fast as the console takes it.
fn flood(dir: &std::path::Path) -> CommandSpec {
    let file = dir.join("flood.txt");
    let line = format!("{}\r\n", "x".repeat(78));
    std::fs::write(&file, line.repeat(6 * 1024 * 1024 / line.len())).expect("write the flood file");
    CommandSpec::new("cmd.exe").arg("/C").arg("type").arg(file.to_string_lossy().to_string())
}

/// A flooding agent whose window never acks, past the startup burst: held.
fn held() -> (Registry, Arc<AtomicUsize>, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let count = Arc::new(AtomicUsize::new(0));
    let c = count.clone();
    let mut reg = Registry::new();
    reg.spawn("a".into(), &flood(dir.path()), size(), move |b| { c.fetch_add(b.len(), Ordering::SeqCst); }, |_| {}).expect("spawn");
    reg.write_input("a", b"\x1b[1;1R").expect("answer the startup query");
    pause(1_800); // input opens the gate for 1.5 s; let that pass
    reg.enable_flow("a").expect("flow on");
    pause(2_500);
    (reg, count, dir)
}

#[test]
#[serial_test::serial]
fn typing_and_resizing_do_not_wait_for_a_held_child() {
    let (mut reg, _count, _dir) = held();
    let t = Instant::now();
    reg.write_input("a", b"x").expect("typing");
    assert!(t.elapsed() < Duration::from_millis(500), "typing waited {:?}", t.elapsed());
    let t = Instant::now();
    reg.resize("a", 100, 30).expect("resize");
    assert!(t.elapsed() < Duration::from_millis(500), "resizing waited {:?}", t.elapsed());
}

#[test]
#[serial_test::serial]
fn an_agent_held_back_can_be_killed_without_waiting_for_the_window() {
    let (mut reg, count, _dir) = held();
    let gone = reg.take("a").expect("agent");
    let t = Instant::now();
    drop(gone);
    assert!(t.elapsed() < Duration::from_secs(3), "teardown of a held agent took {:?}", t.elapsed());
    pause(1_500);
    let a = count.load(Ordering::SeqCst);
    pause(500);
    assert_eq!(count.load(Ordering::SeqCst), a, "nothing keeps flowing from a killed agent");
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
    reg.write_input("a", b"\x1b[1;1R").expect("answer the startup query");
    pause(1_800); // past the moment the typing opened
    let flow = reg.flow_of("a").expect("flow");
    if hold {
        reg.enable_flow("a").expect("flow on");
        let t = Instant::now();
        while !flow.is_held() {
            if t.elapsed() > Duration::from_secs(30) {
                return Err(format!("never held (in flight: {})", flow.inflight()));
            }
            pause(100);
        }
        pause(1_000);
    } else {
        pause(3_000);
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
    interrupt_gets_through(false, Duration::from_secs(5)).expect("without flow control");
    // ...and a window that never acks does not keep the answer from coming.
    interrupt_gets_through(true, Duration::from_secs(5)).expect("with the reader held back");
}
