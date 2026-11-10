//! Debug builds only: notes when the UI thread could not keep up. With
//! BYTEHIVE_STALL_LOG set, a timer on the UI thread asks to wake every 16 ms and
//! each wake that comes more than BYTEHIVE_STALL_MS (50) late is written down:
//! for that long nothing on screen could change. Off, it costs nothing.

use std::io::Write;
use std::time::{Duration, Instant};

use gpui_kit::*;

const TICK: Duration = Duration::from_millis(16);

pub struct Probe {
    pub tick: Duration,
    pub limit: Duration,
}

impl Probe {
    /// The line to log when a wake asked for after `tick` came after `waited`, or None.
    pub fn late(&self, waited: Duration, at_ms: u64) -> Option<String> {
        let late = waited.saturating_sub(self.tick);
        (late > self.limit).then(|| format!("{at_ms} late {} ms", late.as_millis()))
    }
}

pub fn limit_from(value: Option<&str>) -> Duration {
    Duration::from_millis(value.and_then(|v| v.trim().parse().ok()).unwrap_or(50))
}

pub fn start(cx: &mut App) {
    if !cfg!(debug_assertions) {
        return;
    }
    let Some(path) = std::env::var_os("BYTEHIVE_STALL_LOG") else { return };
    let probe = Probe { tick: TICK, limit: limit_from(std::env::var("BYTEHIVE_STALL_MS").ok().as_deref()) };
    let write = move |line: String| {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(f, "{line}");
        }
    };
    write(format!("{} start limit {} ms", crate::fleet::now_ms(), probe.limit.as_millis()));
    cx.spawn(async move |cx| {
        loop {
            let asked = Instant::now();
            cx.background_executor().timer(probe.tick).await;
            if let Some(line) = probe.late(asked.elapsed(), crate::fleet::now_ms()) {
                write(line);
            }
        }
    })
    .detach();
}

#[cfg(test)]
#[path = "stall_tests.rs"]
mod tests;
