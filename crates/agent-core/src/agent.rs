//! One running agent: a ConPTY child (or, for agents that talk in JSON lines,
//! a child on plain pipes) assigned to a kill-on-close Job, with a reader
//! thread streaming output and a wait thread reporting exit.
//!
//! Field DECLARATION order IS the teardown order on drop, and it is non-blocking:
//!   1. job     -> CloseHandle => KILL_ON_JOB_CLOSE terminates the whole tree;
//!                 the wait thread's child.wait() then returns.
//!   2. session -> dropping the master closes the ConPTY; the reader thread's
//!                 blocked read() gets EOF and the thread ends.
//!   3. threads -> JoinHandles detach; both threads have already finished.

use std::collections::VecDeque;
use std::io::Read;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use anyhow::{anyhow, Result};
use portable_pty::PtySize;

use crate::command_spec::CommandSpec;
#[cfg(windows)]
use crate::job::Job;
#[cfg(windows)]
use crate::pipe_session::{self, PipeSession};
use crate::pty_session::PtySession;

/// How the agent is reached: a terminal, or pipes carrying JSON lines.
enum Session {
    Pty(PtySession),
    #[cfg(windows)]
    Pipe(PipeSession),
}

/// Where PTY output goes (e.g. a window's IPC channel). Swappable via `attach`.
pub type Sink = Box<dyn FnMut(&[u8]) + Send>;

/// Most chunks a live tap (a remote terminal) may lag: past this the tap is
/// dropped instead of queueing forever (it re-taps and replays on return).
const TAP_CAP: usize = 256;

fn tap_channel() -> (SyncSender<Vec<u8>>, Receiver<Vec<u8>>) {
    sync_channel(TAP_CAP)
}

/// Recent output kept for replay when a new window attaches (tab detach).ch).
const SCROLLBACK_CAP: usize = 512 * 1024;

struct Output {
    buf: VecDeque<u8>,
    sink: Sink,
    // Extra live subscribers (the remote web terminal). Each gets every output
    // chunk; a closed channel is pruned. The single `sink` above is the owning
    // window; taps are additional read-only mirrors.
    taps: Vec<SyncSender<Vec<u8>>>,
    // When Some, every output chunk is also appended to this recording.
    rec: Option<crate::record::Rec>,
    // A pipe agent's output is JSON lines: its window reads them as they are,
    // a remote terminal or a recording gets them as readable text.
    pipe: bool,
}

pub struct Agent {
    // NOTE: drop order = declaration order. Keep job first, session second.
    // job + threads are held purely for their drop side-effects / ownership.
    #[cfg(windows)]
    #[allow(dead_code)]
    job: Job,
    session: Session,
    output: Arc<Mutex<Output>>,
    #[allow(dead_code)]
    reader_thread: JoinHandle<()>,
    #[allow(dead_code)]
    wait_thread: JoinHandle<()>,
}

impl Agent {
    /// Spawn one agent. `on_bytes` receives output chunks; `on_exit` fires once
    /// with the real exit code when the child terminates (naturally or on kill).
    pub fn spawn(
        spec: &CommandSpec,
        size: PtySize,
        on_bytes: impl FnMut(&[u8]) + Send + 'static,
        on_exit: impl FnOnce(u32) + Send + 'static,
    ) -> Result<Self> {
        let (session, reader, child) = PtySession::spawn(spec, size)?;

        // Assign to a kill-on-close Job IMMEDIATELY after spawn.
        #[cfg(windows)]
        let job = {
            let raw = child
                .as_raw_handle()
                .ok_or_else(|| anyhow!("child raw handle unavailable"))?;
            let job = Job::new_kill_on_close()?;
            job.assign_raw(raw)?;
            job
        };

        let output = Arc::new(Mutex::new(Output {
            buf: VecDeque::new(),
            sink: Box::new(on_bytes),
            taps: Vec::new(),
            rec: None,
            pipe: false,
        }));
        let reader_thread = spawn_reader(reader, output.clone());
        let wait_thread = spawn_waiter(child, on_exit);

        Ok(Agent {
            #[cfg(windows)]
            job,
            session: Session::Pty(session),
            output,
            reader_thread,
            wait_thread,
        })
    }

    /// Spawn an agent on pipes instead of a terminal: its stdout goes out in
    /// whole lines only, and each line it writes to stderr goes out as one line
    /// of its own, `{"type":"bytehive_stderr","text":...}`, so the two never cut
    /// into each other's JSON.
    #[cfg(windows)]
    pub fn spawn_pipe(
        spec: &CommandSpec,
        on_bytes: impl FnMut(&[u8]) + Send + 'static,
        on_exit: impl FnOnce(u32) + Send + 'static,
    ) -> Result<Self> {
        let (session, stdout, stderr, mut child) = pipe_session::spawn(spec)?;
        // In the job before it runs a single instruction, children included.
        let job = Job::new_kill_on_close()?;
        job.assign_raw(child.raw_handle())?;
        child.resume();

        let output = Arc::new(Mutex::new(Output {
            buf: VecDeque::new(),
            sink: Box::new(on_bytes),
            taps: Vec::new(),
            rec: None,
            pipe: true,
        }));
        let reader_thread = spawn_line_reader(Box::new(stdout), output.clone(), false);
        // Detached: it ends with the process, and the reader above is the one
        // the teardown waits on.
        spawn_line_reader(Box::new(stderr), output.clone(), true);
        let wait_thread = thread::spawn(move || on_exit(child.wait()));

        Ok(Agent {
            job,
            session: Session::Pipe(session),
            output,
            reader_thread,
            wait_thread,
        })
    }

    /// Re-point output at a new sink (a freshly-detached window's channel).
    /// The buffered scrollback is replayed through the new sink first, under
    /// the same lock the reader thread uses, so the swap loses/dupes nothing.
    pub fn attach(&self, mut sink: Sink) {
        let mut o = self.output.lock().unwrap_or_else(|p| p.into_inner());
        let (a, b) = o.buf.as_slices();
        if !a.is_empty() {
            sink(a);
        }
        if !b.is_empty() {
            sink(b);
        }
        o.sink = sink;
    }

    /// Subscribe to this agent's live output. The current scrollback is replayed
    /// into the channel first (so a fresh terminal paints immediately), then
    /// every new chunk arrives until the receiver is dropped, lags past TAP_CAP, or the agent exits.
    pub fn tap(&self) -> Receiver<Vec<u8>> {
        let (tx, rx) = tap_channel();
        let mut o = self.output.lock().unwrap_or_else(|p| p.into_inner());
        let (a, b) = o.buf.as_slices();
        if !a.is_empty() || !b.is_empty() {
            let mut replay = Vec::with_capacity(a.len() + b.len());
            replay.extend_from_slice(a);
            replay.extend_from_slice(b);
            let _ = tx.send(if o.pipe { readable(&replay) } else { replay });
        }
        o.taps.push(tx);
        rx
    }

    /// Start recording this agent's output to `path` (a JSONL "cast" file). The
    /// current scrollback is written as the first frame (t=0) so the recording is
    /// self-contained — a player sees the screen state at record start. Replaces
    /// any recording already in progress.
    pub fn record_start(&self, path: &str) -> Result<()> {
        let mut o = self.output.lock().unwrap_or_else(|p| p.into_inner());
        let (a, b) = o.buf.as_slices();
        let snap = [a, b].concat();
        let snap = if o.pipe && !snap.is_empty() { readable(&snap) } else { snap };
        o.rec = Some(crate::record::Rec::create(path, &snap)?);
        Ok(())
    }

    /// Stop recording and flush the file. No-op if not recording.
    pub fn record_stop(&self) {
        let mut o = self.output.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(rec) = o.rec.take() {
            rec.finish();
        }
    }

    pub fn is_recording(&self) -> bool {
        self.output
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .rec
            .is_some()
    }

    /// It talks in JSON lines, not keys.
    pub fn is_pipe(&self) -> bool {
        #[cfg(windows)]
        if let Session::Pipe(_) = self.session {
            return true;
        }
        false
    }

    pub fn write_input(&mut self, bytes: &[u8]) -> Result<()> {
        match &mut self.session {
            Session::Pty(s) => s.write_input(bytes),
            #[cfg(windows)]
            Session::Pipe(s) => s.write_input(bytes),
        }
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        match &self.session {
            Session::Pty(s) => s.resize(cols, rows),
            // no screen to size
            #[cfg(windows)]
            Session::Pipe(_) => Ok(()),
        }
    }
}

/// Hand one chunk of output to the window, the live taps, the recording, and
/// the scrollback a newly attached window replays.
fn emit(output: &Arc<Mutex<Output>>, bytes: &[u8]) {
    let mut o = output.lock().unwrap_or_else(|p| p.into_inner());
    o.buf.extend(bytes);
    let over = o.buf.len().saturating_sub(SCROLLBACK_CAP);
    if over > 0 {
        o.buf.drain(..over);
    }
    (o.sink)(bytes);
    if o.taps.is_empty() && o.rec.is_none() {
        return;
    }
    let shown = if o.pipe { readable(bytes) } else { bytes.to_vec() };
    if shown.is_empty() {
        return;
    }
    // Mirror to live taps (remote terminals); drop closed or lagging ones.
    if !o.taps.is_empty() {
        o.taps.retain(|t| t.try_send(shown.clone()).is_ok());
    }
    // Append to the session recording, if one is running.
    if let Some(rec) = o.rec.as_mut() {
        rec.frame(&shown);
    }
}

/// A pipe agent's JSON lines as a terminal would show the exchange: what you
/// sent, what it wrote, the tools it called, how each turn ended. Everything
/// else (pieces of a reply still being written, its own bookkeeping) is left
/// out; it all arrives again whole.
pub fn readable(bytes: &[u8]) -> Vec<u8> {
    use serde_json::Value;
    let mut out = String::new();
    let mut put = |color: &str, text: &str| {
        let text = text.trim_end().replace("\r\n", "\n").replace('\n', "\r\n");
        if text.is_empty() {
            return;
        }
        if color.is_empty() {
            out.push_str(&text);
        } else {
            out.push_str(&format!("\x1b[{color}m{text}\x1b[0m"));
        }
        out.push_str("\r\n");
    };
    let text_of = |c: &Value| -> String {
        match c {
            Value::String(s) => s.clone(),
            Value::Array(parts) => parts.iter().filter_map(|p| p.get("text").and_then(|t| t.as_str())).collect::<Vec<_>>().join("\n"),
            _ => String::new(),
        }
    };
    for line in String::from_utf8_lossy(bytes).lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        let content = v.get("message").and_then(|m| m.get("content"));
        match v.get("type").and_then(|t| t.as_str()) {
            Some("user") => {
                // a message you sent, not the results of its tools
                if v.get("isReplay").is_none() && content.map_or(false, |c| c.is_string() || c.as_array().map_or(false, |a| a.iter().any(|p| p.get("type").and_then(|t| t.as_str()) == Some("text")))) {
                    put("36", &format!("› {}", text_of(content.unwrap())));
                }
            }
            Some("assistant") => {
                for b in content.and_then(|c| c.as_array()).into_iter().flatten() {
                    match b.get("type").and_then(|t| t.as_str()) {
                        Some("text") => put("", b.get("text").and_then(|t| t.as_str()).unwrap_or("")),
                        Some("tool_use") => put("90", &format!("● {}", b.get("name").and_then(|n| n.as_str()).unwrap_or("tool"))),
                        _ => {}
                    }
                }
            }
            Some("result") => {
                let failed = v.get("is_error").and_then(|e| e.as_bool()).unwrap_or(false);
                put(if failed { "31" } else { "90" }, if failed { "Stopped." } else { "Done." });
            }
            Some("control_request") if v.pointer("/request/subtype").and_then(|s| s.as_str()) == Some("can_use_tool") => {
                put("33", &format!("? Wants to use {} (answer it in ByteHive)", v.pointer("/request/tool_name").and_then(|s| s.as_str()).unwrap_or("a tool")));
            }
            Some("bytehive_stderr") => put("31", v.get("text").and_then(|t| t.as_str()).unwrap_or("")),
            _ => {}
        }
    }
    out.into_bytes()
}

/// Wrap one stderr line as a JSON line of its own.
fn stderr_line(line: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(line);
    let mut out = serde_json::json!({ "type": "bytehive_stderr", "text": text.trim_end_matches('\r') }).to_string().into_bytes();
    out.push(b'\n');
    out
}

/// Whole lines out of `pending`: everything up to its last newline (or all of
/// it at the end), as is or each wrapped as a stderr line.
fn take_lines(pending: &mut Vec<u8>, all: bool, wrap: bool) -> Vec<u8> {
    let cut = if all { pending.len() } else { pending.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1) };
    let lines: Vec<u8> = pending.drain(..cut).collect();
    if !wrap {
        return lines;
    }
    lines
        .split(|&b| b == b'\n')
        .filter(|l| !l.iter().all(|b| b.is_ascii_whitespace()))
        .flat_map(stderr_line)
        .collect()
}

/// Read until EOF and pass on whole lines only (a JSON line cut between two
/// reads would be two broken ones); `wrap` turns each into a stderr line.
fn spawn_line_reader(mut reader: Box<dyn Read + Send>, output: Arc<Mutex<Output>>, wrap: bool) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut buf = [0u8; 16384];
        let mut pending: Vec<u8> = Vec::new();
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    pending.extend_from_slice(&buf[..n]);
                    let out = take_lines(&mut pending, false, wrap);
                    if !out.is_empty() {
                        emit(&output, &out);
                    }
                }
            }
        }
        // what was left without a newline at the end
        if !pending.is_empty() && !wrap {
            pending.push(b'\n');
        }
        let out = take_lines(&mut pending, true, wrap);
        if !out.is_empty() {
            emit(&output, &out);
        }
    })
}

fn spawn_reader(mut reader: Box<dyn Read + Send>, output: Arc<Mutex<Output>>) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut buf = [0u8; 16384];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => emit(&output, &buf[..n]),
                Err(_) => break,
            }
        }
    })
}

fn spawn_waiter(
    child: Box<dyn portable_pty::Child + Send + Sync>,
    on_exit: impl FnOnce(u32) + Send + 'static,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut child = child;
        let code = child.wait().map(|s| s.exit_code()).unwrap_or(1);
        on_exit(code);
    })
}

#[cfg(test)]
mod line_tests {
    use super::{readable, take_lines};

    #[test]
    fn a_pipe_agents_lines_read_like_a_terminal() {
        let lines = concat!(
            r#"{"type":"user","message":{"role":"user","content":"hi there"}}"#, "\n",
            r#"{"type":"stream_event","event":{"type":"content_block_delta"}}"#, "\n",
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello\nworld"},{"type":"tool_use","name":"Bash"}]}}"#, "\n",
            r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"ok"}]}}"#, "\n",
            r#"{"type":"result","is_error":false}"#, "\n",
        );
        let out = String::from_utf8(readable(lines.as_bytes())).unwrap();
        assert_eq!(out, "\x1b[36m› hi there\x1b[0m\r\nHello\r\nworld\r\n\x1b[90m● Bash\x1b[0m\r\n\x1b[90mDone.\x1b[0m\r\n");
    }

    #[test]
    fn only_whole_lines_leave_and_the_rest_waits() {
        let mut p = b"{\"a\":1}\n{\"b\"".to_vec();
        assert_eq!(take_lines(&mut p, false, false), b"{\"a\":1}\n".to_vec());
        assert_eq!(p, b"{\"b\"".to_vec());
        p.extend_from_slice(b":2}\n");
        assert_eq!(take_lines(&mut p, false, false), b"{\"b\":2}\n".to_vec());
        assert!(p.is_empty());
    }

    #[test]
    fn stderr_lines_become_json_lines() {
        let mut p = b"Error: \"bad\"\r\n\n".to_vec();
        let out = String::from_utf8(take_lines(&mut p, false, true)).unwrap();
        assert_eq!(out, "{\"text\":\"Error: \\\"bad\\\"\",\"type\":\"bytehive_stderr\"}\n");
    }
    #[test]
    fn a_tap_that_never_reads_is_dropped_instead_of_queueing_forever() {
        use super::{emit, tap_channel, Output, TAP_CAP};
        use std::collections::VecDeque;
        use std::sync::{Arc, Mutex};
        let (tx, _rx) = tap_channel(); // held open, never read: a stuck remote
        let output = Arc::new(Mutex::new(Output {
            buf: VecDeque::new(),
            sink: Box::new(|_: &[u8]| {}),
            taps: vec![tx],
            rec: None,
            pipe: false,
        }));
        for _ in 0..TAP_CAP + 10 {
            emit(&output, b"x");
        }
        assert!(output.lock().unwrap().taps.is_empty());
    }
}