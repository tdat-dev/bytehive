#![cfg(windows)]
//! Multi-agent registry: many agents run concurrently, each killed independently,
//! and kill (drop) tears each down promptly without hanging.

use std::time::{Duration, Instant};

use maestro_lib::core::command_spec::CommandSpec;
use maestro_lib::core::registry::{self, Registry};
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
    Registry::spawn_shared(&shared, "a".into(), 1, &long_spec(), size(), |_| {}, |_| {})
        .expect("spawn a");
    assert!(registry::lock(&shared).has("a"));
    assert!(
        Registry::spawn_shared(&shared, "a".into(), 2, &long_spec(), size(), |_| {}, |_| {}).is_err(),
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
