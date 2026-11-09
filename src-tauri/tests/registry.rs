#![cfg(windows)]
//! Multi-agent registry: many agents run concurrently, each killed independently,
//! and kill (drop) tears each down promptly without hanging.

use std::time::{Duration, Instant};

use bytehive_lib::core::command_spec::CommandSpec;
use bytehive_lib::core::registry::{self, Registry};
use std::sync::{Arc, Mutex};
use portable_pty::PtySize;

fn size() -> PtySize {
    PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 }
}

// Bounded, long-ish process that also spawns a child (ping) so kill tears a tree.
fn long_spec() -> CommandSpec {
    CommandSpec::new("cmd.exe")
        .arg("/C")
        .arg("echo hi & ping -n 20 127.0.0.1")
}

#[test]
#[serial_test::serial]
fn many_agents_run_and_each_kills_independently() {
    let mut reg = Registry::new();

    reg.spawn("a".into(), &long_spec(), size(), |_| {}, |_| {})
        .expect("spawn a");
    reg.spawn("b".into(), &long_spec(), size(), |_| {}, |_| {})
        .expect("spawn b");
    reg.spawn("c".into(), &long_spec(), size(), |_| {}, |_| {})
        .expect("spawn c");
    assert_eq!(reg.count(), 3, "three agents should run concurrently");

    // Duplicate id is rejected.
    assert!(
        reg.spawn("a".into(), &long_spec(), size(), |_| {}, |_| {}).is_err(),
        "duplicate agent id must be rejected"
    );

    // Input routes to a specific agent.
    reg.write_input("b", b"echo routed\r\n").expect("input to b");

    // Killing one leaves the others running.
    let start = Instant::now();
    reg.kill("a");
    assert_eq!(reg.count(), 2, "killing one removes exactly one");
    assert!(reg.has("b") && reg.has("c"));

    reg.kill("b");
    reg.kill("c");
    assert_eq!(reg.count(), 0, "all agents killed");

    assert!(
        start.elapsed() < Duration::from_secs(10),
        "kills must not hang (took {:?})",
        start.elapsed()
    );

    // Killing an unknown id is a no-op.
    reg.kill("ghost");
    // Operating on a missing agent errors cleanly.
    assert!(reg.write_input("a", b"x").is_err());
}

#[test]
#[serial_test::serial]
fn shared_spawn_inserts_rejects_duplicates_and_takes_out() {
    let shared = Arc::new(Mutex::new(Registry::new()));
    Registry::spawn_shared(&shared, "a".into(), 1, &long_spec(), size(), false, |_| {}, |_| {})
        .expect("spawn a");
    assert!(registry::lock(&shared).has("a"));
    assert!(
        Registry::spawn_shared(&shared, "a".into(), 2, &long_spec(), size(), false, |_| {}, |_| {}).is_err(),
        "duplicate id must be rejected"
    );
    registry::lock(&shared).write_input("a", b"echo routed\r\n").expect("input to a");

    let start = Instant::now();
    let gone = registry::lock(&shared).take("a");
    assert!(gone.is_some() && !registry::lock(&shared).has("a"));
    drop(gone); // teardown outside the lock
    assert!(registry::lock(&shared).drain().is_empty());
    assert!(start.elapsed() < Duration::from_secs(10), "teardown must not hang");
}

#[test]
fn a_panic_under_the_lock_does_not_wedge_the_registry() {
    let shared = Arc::new(Mutex::new(Registry::new()));
    let s = shared.clone();
    let _ = std::thread::spawn(move || {
        let _g = s.lock().unwrap();
        panic!("boom while holding the registry");
    })
    .join();
    assert!(shared.is_poisoned());
    assert_eq!(registry::lock(&shared).count(), 0, "still usable after a poisoning panic");
}

/// Poll `cond` until it holds or the timeout elapses.
fn wait_for(mut cond: impl FnMut() -> bool, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

#[test]
#[serial_test::serial]
fn an_agent_whose_process_exits_leaves_the_registry() {
    let shared = Arc::new(Mutex::new(Registry::new()));
    let (tx, rx) = std::sync::mpsc::channel();
    let spec = CommandSpec::new("cmd.exe").arg("/C").arg("echo bye");
    Registry::spawn_shared(&shared, "a".into(), 1, &spec, size(), false, |_| {}, move |code| {
        let _ = tx.send(code);
    })
    .expect("spawn a");
    let _ = registry::lock(&shared).write_input("a", b"\x1b[1;1R"); // answer ConPTY's DSR

    assert_eq!(rx.recv_timeout(Duration::from_secs(10)).expect("exit reported"), 0);
    assert!(
        wait_for(|| !registry::lock(&shared).has("a"), Duration::from_secs(10)),
        "a dead agent must not keep its ConPTY, job and scrollback"
    );
    assert!(registry::lock(&shared).attach("a", Box::new(|_| {}), 1).is_err());
}

#[test]
#[serial_test::serial]
fn a_late_exit_of_a_replaced_run_leaves_the_new_run_alone() {
    let shared = Arc::new(Mutex::new(Registry::new()));
    let (tx, rx) = std::sync::mpsc::channel();
    Registry::spawn_shared(&shared, "a".into(), 1, &long_spec(), size(), false, |_| {}, move |code| {
        let _ = tx.send(code);
    })
    .expect("spawn run 1");
    // Restart in place: run 1 is taken out, run 2 takes the id, THEN run 1 dies.
    let old = registry::lock(&shared).take("a").expect("run 1");
    Registry::spawn_shared(&shared, "a".into(), 2, &long_spec(), size(), false, |_| {}, |_| {})
        .expect("spawn run 2");
    drop(old);
    rx.recv_timeout(Duration::from_secs(10)).expect("run 1 exit reported");
    std::thread::sleep(Duration::from_millis(1000)); // past run 1's exit cleanup
    assert!(registry::lock(&shared).has("a"), "run 2 must survive run 1's exit");
    let gone = registry::lock(&shared).drain();
    drop(gone);
}

/// A pipe agent (JSON lines, no terminal) goes through the same registry:
/// input reaches it, its output comes back as whole lines, its stderr as a
/// line of JSON, and taking it out ends it.
#[cfg(windows)]
#[test]
fn a_pipe_agent_talks_in_lines_and_dies_with_its_entry() {
    use std::sync::mpsc;
    let shared = Arc::new(Mutex::new(Registry::new()));
    let (out_tx, out_rx) = mpsc::channel::<Vec<u8>>();
    let (exit_tx, exit_rx) = mpsc::channel::<u32>();
    // cmd runs the lines it reads as they come (findstr would hold its output
    // until its input closes); findstr asked to read a missing file says so on
    // stderr.
    let (out2, exit2) = (out_tx.clone(), exit_tx.clone());
    let spec = CommandSpec::new("cmd").arg("/q");
    Registry::spawn_shared(&shared, "p".into(), 1, &spec, size(), true, move |b| { let _ = out_tx.send(b.to_vec()); }, move |c| {
        let _ = exit_tx.send(c);
    })
    .expect("spawn pipe agent");
    let missing = CommandSpec::new("findstr").arg("x").arg("no-such-file.txt");
    Registry::spawn_shared(&shared, "q".into(), 1, &missing, size(), true, move |b| { let _ = out2.send(b.to_vec()); }, move |c| {
        let _ = exit2.send(c);
    })
    .expect("spawn failing pipe agent");
    exit_rx.recv_timeout(Duration::from_secs(10)).expect("the failing one exits");
    registry::lock(&shared).write_input("p", b"echo hello there\r\n").expect("input");
    let mut seen = String::new();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !(seen.contains("hello there") && seen.contains("bytehive_stderr")) && Instant::now() < deadline {
        if let Ok(b) = out_rx.recv_timeout(Duration::from_millis(200)) {
            seen.push_str(&String::from_utf8_lossy(&b));
        }
    }
    assert!(seen.contains("hello there"), "stdout line came back: {seen:?}");
    assert!(seen.contains("\"type\":\"bytehive_stderr\""), "stderr came as a JSON line: {seen:?}");
    // keys from the remote terminal would break its JSON: refused
    assert!(registry::lock(&shared).write_keys("p", b"x\r").is_err());
    let gone = registry::lock(&shared).take("p");
    drop(gone);
    exit_rx.recv_timeout(Duration::from_secs(10)).expect("exit reported after take");
}
