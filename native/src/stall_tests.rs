use super::{Probe, limit_from};
use std::time::Duration;

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[test]
fn a_wake_on_time_or_a_little_late_is_not_a_stall() {
    let p = Probe { tick: ms(16), limit: ms(50) };
    assert_eq!(p.late(ms(16), 1000), None);
    assert_eq!(p.late(ms(40), 1000), None);
    assert_eq!(p.late(ms(66), 1000), None);
}

#[test]
fn a_wake_past_the_limit_is_logged_with_how_late_it_came() {
    let p = Probe { tick: ms(16), limit: ms(50) };
    assert_eq!(p.late(ms(140), 1_700_000_000_123), Some("1700000000123 late 124 ms".to_string()));
}

#[test]
fn the_limit_comes_from_the_env_or_is_50_ms() {
    assert_eq!(limit_from(None), ms(50));
    assert_eq!(limit_from(Some("100")), ms(100));
    assert_eq!(limit_from(Some("soon")), ms(50));
}
