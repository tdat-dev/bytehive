use super::*;
use std::fs;
use std::io;

const NOW: i64 = 1_000_000;
const MIN: i64 = 60_000;

fn held(silent: i64) -> LockRead {
    LockRead::Held(EngineLock { pid: 9, at: NOW - silent })
}

fn at(at: i64, pid: u64) -> LockRead {
    LockRead::Held(EngineLock { pid, at })
}

// lockMove

#[test]
fn runs_when_nobody_holds_the_lock() {
    assert_eq!(lock_move(&LockRead::Free, 7, NOW, false, 0), LockMove::Run);
}

#[test]
fn stays_out_while_another_apps_heartbeat_is_fresh_alive_or_not() {
    assert_eq!(lock_move(&at(NOW, 9), 7, NOW, true, 0), LockMove::StayOut);
    assert_eq!(lock_move(&held(LOCK_STALE_MS - 1), 7, NOW, true, 0), LockMove::StayOut);
    assert_eq!(lock_move(&held(LOCK_STALE_MS - 1), 7, NOW, false, 0), LockMove::StayOut);
}

#[test]
fn takes_the_lock_of_a_dead_holder_after_15_seconds_with_no_heartbeat() {
    assert_eq!(LOCK_STALE_MS, 15_000);
    assert_eq!(lock_move(&held(LOCK_STALE_MS), 7, NOW, false, 0), LockMove::Run);
}

#[test]
fn stays_out_for_a_live_holder_silent_1_minute_its_panes_are_still_running() {
    assert_eq!(lock_move(&held(MIN), 7, NOW, true, 0), LockMove::StayOut);
    assert_eq!(lock_move(&held(LOCK_ABANDONED_MS - 1), 7, NOW, true, 0), LockMove::StayOut);
}

#[test]
fn takes_over_from_a_live_holder_silent_11_minutes_and_says_it_did() {
    assert_eq!(LOCK_ABANDONED_MS, 10 * MIN);
    assert_eq!(lock_move(&held(LOCK_ABANDONED_MS), 7, NOW, true, 0), LockMove::TakeOver);
    assert_eq!(lock_move(&held(11 * MIN), 7, NOW, true, 0), LockMove::TakeOver);
}

#[test]
fn does_not_count_a_sleep_as_silence_the_holder_gets_to_wake_up_too() {
    assert_eq!(lock_move(&held(480 * MIN), 7, NOW, true, NOW - 1500), LockMove::StayOut);
    assert_eq!(lock_move(&held(480 * MIN), 7, NOW, true, NOW - LOCK_ABANDONED_MS), LockMove::TakeOver);
    assert_eq!(lock_move(&held(480 * MIN), 7, NOW, false, NOW - 1500), LockMove::Run);
    assert_eq!(lock_move(&at(NOW + LOCK_ABANDONED_MS, 9), 7, NOW, true, NOW - 1500), LockMove::StayOut);
}

#[test]
fn skips_the_beat_on_a_lock_it_could_not_read() {
    assert_eq!(lock_move(&LockRead::Unknown, 7, NOW, false, 0), LockMove::Skip);
    assert_eq!(lock_move(&LockRead::Unknown, 7, NOW, true, 0), LockMove::Skip);
}

#[test]
fn knows_its_own_lock_however_fresh_that_is_a_window_that_reloaded() {
    assert_eq!(lock_move(&at(NOW, 7), 7, NOW, true, 0), LockMove::Run);
    assert_eq!(lock_move(&at(NOW - 11 * MIN, 7), 7, NOW, true, 0), LockMove::Run);
}

#[test]
fn does_not_wait_forever_on_a_heartbeat_from_far_ahead_of_the_clock() {
    assert_eq!(lock_move(&at(NOW + LOCK_STALE_MS, 9), 7, NOW, false, 0), LockMove::Run);
    assert_eq!(lock_move(&at(NOW + LOCK_ABANDONED_MS, 9), 7, NOW, true, 0), LockMove::TakeOver);
    assert_eq!(lock_move(&at(NOW + 1000, 9), 7, NOW, false, 0), LockMove::StayOut);
}

// lockSilent

#[test]
fn is_another_apps_lock_with_no_heartbeat_for_15_seconds_only_then_is_the_process_looked_up() {
    assert!(lock_silent(&EngineLock { pid: 9, at: NOW - LOCK_STALE_MS }, 7, NOW));
    assert!(!lock_silent(&EngineLock { pid: 9, at: NOW - 1 }, 7, NOW));
    assert!(!lock_silent(&EngineLock { pid: 7, at: NOW - LOCK_STALE_MS }, 7, NOW));
}

// parseLock

#[test]
fn reads_a_lock() {
    assert_eq!(parse_lock(r#"{"pid":9,"at":5}"#), Some(EngineLock { pid: 9, at: 5 }));
    assert_eq!(parse_lock("\u{FEFF}{\"pid\":9,\"at\":5}"), Some(EngineLock { pid: 9, at: 5 }));
}

#[test]
fn takes_bad_text_for_no_lock() {
    for text in ["", "{ half", "null", "{}", r#"{"pid":"9","at":5}"#, r#"{"pid":9}"#] {
        assert_eq!(parse_lock(text), None, "{text}");
    }
}

// The file (store.test.ts)

#[test]
fn round_trips_the_engine_lock_whole_and_reads_a_missing_one_as_none() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(read(dir.path()), LockRead::Free);
    write(dir.path(), &EngineLock { pid: 7, at: 5 }).unwrap();
    assert_eq!(read(dir.path()), LockRead::Held(EngineLock { pid: 7, at: 5 }));
    let names: Vec<String> = fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["engine.lock"]);
    assert_eq!(fs::read_to_string(dir.path().join("engine.lock")).unwrap(), r#"{"pid":7,"at":5}"#);
}

#[test]
fn reads_the_lock_as_unknown_not_as_none_when_the_file_is_being_swapped_in() {
    assert_eq!(read_result(Err(io::Error::from_raw_os_error(5))), LockRead::Unknown);
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("engine.lock")).unwrap();
    assert_eq!(read(dir.path()), LockRead::Unknown);
}

#[test]
fn reads_the_lock_as_unknown_not_as_none_when_something_has_it_open() {
    assert_eq!(read_result(Err(io::Error::from_raw_os_error(32))), LockRead::Unknown);
}

#[test]
fn reads_the_lock_as_unknown_not_as_none_when_the_read_failed_otherwise() {
    assert_eq!(read_result(Err(io::Error::other("gone"))), LockRead::Unknown);
}

#[test]
fn reads_a_missing_lock_as_none_whichever_part_of_the_path_is_missing() {
    assert_eq!(read_result(Err(io::Error::from_raw_os_error(2))), LockRead::Free);
    assert_eq!(read_result(Err(io::Error::from_raw_os_error(3))), LockRead::Free);
    assert_eq!(read(&std::path::Path::new("Z:\\no\\such\\company")), LockRead::Free);
}

#[test]
fn reads_a_lock_a_crash_filled_with_zeros_as_none_no_later_read_would_ever_do_better() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("engine.lock"), "\0\0\0\0").unwrap();
    assert_eq!(read(dir.path()), LockRead::Free);
}

// Values a newer or odder writer leaves

#[test]
fn never_panics_or_waits_forever_on_a_heartbeat_at_the_ends_of_time() {
    assert!(lock_silent(&EngineLock { pid: 9, at: i64::MIN }, 7, NOW));
    assert!(lock_silent(&EngineLock { pid: 9, at: i64::MAX }, 7, i64::MIN));
    assert_eq!(lock_move(&at(i64::MIN, 9), 7, NOW, false, 0), LockMove::Run);
    assert_eq!(lock_move(&at(i64::MAX, 9), 7, NOW, true, 0), LockMove::TakeOver);
    assert_eq!(lock_move(&at(i64::MIN, 9), 7, i64::MAX, true, i64::MIN), LockMove::TakeOver);
    assert_eq!(lock_move(&at(i64::MIN, 9), 7, NOW, true, i64::MAX), LockMove::StayOut);
}

#[test]
fn honours_a_lock_with_odd_numbers_as_held_by_another_app() {
    assert_eq!(parse_lock(r#"{"pid":9,"at":5.5}"#), Some(EngineLock { pid: 9, at: 5 }));
    assert_eq!(parse_lock(r#"{"pid":5000000000,"at":5}"#), Some(EngineLock { pid: 5_000_000_000, at: 5 }));
    let odd = parse_lock(r#"{"pid":7.5,"at":1000000}"#).unwrap();
    assert_ne!(odd.pid, 7);
    assert_eq!(lock_move(&LockRead::Held(odd), 7, NOW, false, 0), LockMove::StayOut);
    assert_eq!(parse_lock(r#"{"pid":9,"at":1e300}"#).map(|l| l.at), Some(i64::MAX));
}

#[test]
fn reads_a_lock_file_saved_with_a_byte_order_mark() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("engine.lock"), "\u{FEFF}{\"pid\":9,\"at\":5}").unwrap();
    assert_eq!(read(dir.path()), LockRead::Held(EngineLock { pid: 9, at: 5 }));
}
