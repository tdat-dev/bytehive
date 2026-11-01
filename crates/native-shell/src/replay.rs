//! Session replay: an agent's terminal output recorded to a JSONL file of
//! timestamped frames (`agent_core::Agent::record_start` writes them), and a
//! player that writes those frames into a throwaway emulator on a virtual
//! clock. Seeking back resets the emulator and replays from the top, since
//! ANSI is stateful. Same files and folder as the web app's `replay.ts`.

use std::path::{Path, PathBuf};

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;
use base64::Engine;

use crate::term_pane::GridSize;

/// Where a project keeps its recordings.
pub const REC_DIR: &str = ".maestro/recordings";

pub struct Frame {
    pub t: u64,
    pub d: Vec<u8>,
}

/// The frames of a recording; the header and a cut-off last line are skipped.
pub fn parse(text: &str) -> Vec<Frame> {
    let b64 = base64::engine::general_purpose::STANDARD;
    text.lines()
        .filter(|l| l.trim_start().starts_with('{'))
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter_map(|v| Some(Frame { t: v["t"].as_u64()?, d: b64.decode(v["d"].as_str()?).ok()? }))
        .collect()
}

/// The agent and start time a recording's file name carries: `Ana-1793...jsonl`.
pub fn rec_name(file: &str) -> (String, u64) {
    let base = file.strip_suffix(".jsonl").unwrap_or(file);
    match base.rsplit_once('-') {
        Some((agent, at)) if at.len() >= 10 && at.bytes().all(|b| b.is_ascii_digit()) => (agent.to_string(), at.parse().unwrap_or(0)),
        _ => (base.to_string(), 0),
    }
}

/// A new recording's path for agent `name` in `project`.
pub fn rec_path(project: &str, name: &str, now: u64) -> PathBuf {
    let safe: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || "._-".contains(c) { c } else { '-' }).take(40).collect();
    Path::new(project).join(REC_DIR).join(format!("{safe}-{now}.jsonl"))
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecMeta {
    pub path: PathBuf,
    pub agent: String,
    pub at: u64,
    pub size: u64,
}

/// A project's recordings, newest first.
pub fn list(project: &str) -> Vec<RecMeta> {
    let Ok(rd) = std::fs::read_dir(Path::new(project).join(REC_DIR)) else { return Vec::new() };
    let mut v: Vec<RecMeta> = rd
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.to_ascii_lowercase().ends_with(".jsonl").then(|| {
                let (agent, at) = rec_name(&name);
                RecMeta { path: e.path(), agent, at, size: e.metadata().map(|m| m.len()).unwrap_or(0) }
            })
        })
        .collect();
    v.sort_by_key(|r| std::cmp::Reverse(r.at));
    v
}

/// "2:05" from ms.
pub fn clock(ms: u64) -> String {
    let s = (ms + 500) / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}

pub fn size_text(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{} KB", bytes / 1024)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

pub struct Player {
    pub label: String,
    frames: Vec<Frame>,
    pub dur: u64,
    /// The virtual time shown, ms.
    pub vt: u64,
    idx: usize,
    pub playing: bool,
    pub speed: f32,
    pub term: Term<VoidListener>,
    processor: Processor,
    pub size: (usize, usize),
}

impl Player {
    pub fn new(label: String, frames: Vec<Frame>, cols: usize, rows: usize) -> Player {
        let dur = frames.last().map_or(0, |f| f.t);
        let term = Term::new(Config::default(), &GridSize { rows, cols }, VoidListener);
        let mut p = Player { label, frames, dur, vt: 0, idx: 0, playing: true, speed: 1.0, term, processor: Processor::new(), size: (cols, rows) };
        p.seek(0);
        p
    }

    /// Show the screen as it was at `target` ms.
    pub fn seek(&mut self, target: u64) {
        let target = target.min(self.dur);
        if target < self.vt || self.idx == 0 {
            self.rewind();
        }
        while let Some(f) = self.frames.get(self.idx).filter(|f| f.t <= target) {
            self.processor.advance(&mut self.term, &f.d);
            self.idx += 1;
        }
        self.vt = target;
    }

    fn rewind(&mut self) {
        let (cols, rows) = self.size;
        self.term = Term::new(Config::default(), &GridSize { rows, cols }, VoidListener);
        self.processor = Processor::new();
        self.idx = 0;
        self.vt = 0;
    }

    /// Fit the grid to the box; the screen is rebuilt at the same moment.
    pub fn fit(&mut self, cols: usize, rows: usize) {
        if (cols, rows) != self.size {
            self.size = (cols, rows);
            let at = self.vt;
            self.rewind();
            self.seek(at);
        }
    }

    /// Move the clock on by `real_ms` of wall time; stops at the end.
    pub fn tick(&mut self, real_ms: u64) {
        if !self.playing {
            return;
        }
        let next = self.vt + (real_ms as f32 * self.speed) as u64;
        self.seek(next);
        if self.vt >= self.dur {
            self.playing = false;
        }
    }

    /// Play, pause, or play again from the top once it ended.
    pub fn toggle(&mut self) {
        if !self.playing && self.vt >= self.dur {
            self.rewind();
            self.seek(0);
        }
        self.playing = !self.playing;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::term_pane::grid_text;

    fn rec(frames: &[(u64, &str)]) -> String {
        let b64 = base64::engine::general_purpose::STANDARD;
        let mut s = "{\"v\":1}\n".to_string();
        for (t, d) in frames {
            s.push_str(&format!("{{\"t\":{t},\"d\":\"{}\"}}\n", b64.encode(d)));
        }
        s.push_str("{\"t\":99,\"d\":\"cut");
        s
    }

    #[test]
    fn reads_frames_and_names() {
        let f = parse(&rec(&[(0, "a"), (500, "b")]));
        assert_eq!(f.len(), 2);
        assert_eq!(f[1].t, 500);
        assert_eq!(f[1].d, b"b");
        assert_eq!(rec_name("Ana-Bob-1793000000000.jsonl"), ("Ana-Bob".to_string(), 1793000000000));
        assert_eq!(rec_name("odd.jsonl"), ("odd".to_string(), 0));
        let p = rec_path("D:/x", "Ana (codex)", 5);
        assert!(p.ends_with(".maestro/recordings/Ana--codex--5.jsonl"), "{p:?}");
        assert_eq!(clock(125_000), "2:05");
        assert_eq!(size_text(2048), "2 KB");
    }

    #[test]
    fn plays_seeks_back_and_ends() {
        let mut p = Player::new("x".into(), parse(&rec(&[(0, "one"), (1000, "\r\ntwo"), (2000, "\r\nthree")])), 40, 6);
        let screen = |p: &Player| grid_text(&p.term).join("\n");
        assert!(screen(&p).contains("one") && !screen(&p).contains("two"));
        p.tick(1500);
        assert!(screen(&p).contains("two") && !screen(&p).contains("three"));
        p.seek(200);
        assert!(!screen(&p).contains("two"), "seeking back rebuilds the screen");
        p.tick(5000);
        assert!(screen(&p).contains("three") && !p.playing && p.vt == 2000);
        p.toggle();
        assert!(p.playing && p.vt == 0, "plays again from the top");
        p.fit(30, 5);
        assert!(screen(&p).contains("one"));
    }

    #[test]
    fn lists_newest_first() {
        let dir = std::env::temp_dir().join(format!("maestro-rec-{}", std::process::id()));
        let recs = dir.join(REC_DIR);
        std::fs::create_dir_all(&recs).unwrap();
        std::fs::write(recs.join("Ana-1700000000000.jsonl"), "x").unwrap();
        std::fs::write(recs.join("Bob-1800000000000.jsonl"), "xy").unwrap();
        std::fs::write(recs.join("notes.txt"), "").unwrap();
        let l = list(&dir.to_string_lossy());
        assert_eq!(l.iter().map(|r| r.agent.as_str()).collect::<Vec<_>>(), ["Bob", "Ana"]);
        assert_eq!(l[0].size, 2);
        std::fs::remove_dir_all(&dir).ok();
    }
}
