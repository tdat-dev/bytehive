//! Owns a ConPTY-backed PTY (master + writer). Tauri-free. Returns the output
//! reader and the child SEPARATELY so callers run the (blocking) reader on its
//! own thread and wait on the child on another.

use std::io::{Read, Write};

use anyhow::Result;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

use crate::core::command_spec::CommandSpec;

/// Session-identity markers Claude Code exports into the processes it launches.
/// `CommandBuilder` inherits our whole environment, so if ByteHive itself was
/// started from inside a Claude Code session (e.g. `npm run tauri:dev` run by an
/// agent) these leak into every agent we spawn — each one then believes it is a
/// *child* session and silently turns off transcript saving. Scrubbing them
/// restores the "launched from a plain terminal" baseline; when ByteHive is
/// started normally none of them are set and removing them is a no-op. These are
/// identity, not user config, so unsetting them never discards a real setting.
pub(crate) const INHERITED_SESSION_MARKERS: [&str; 4] = [
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_EXECPATH",
];

/// ByteHive sets this on itself so its own windows keep their timers while
/// hidden (lib.rs). It is for ByteHive's webviews only: an agent, and a WebView2
/// app it starts, gets what the variable held before ByteHive changed it.
pub const WEBVIEW_ARGS: &str = "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS";

static WEBVIEW_ARGS_BEFORE: std::sync::OnceLock<Option<std::ffi::OsString>> = std::sync::OnceLock::new();

/// Called once, right before ByteHive changes WEBVIEW_ARGS on itself. As an
/// OsString, so a value that is not valid Unicode comes back exactly.
pub fn remember_webview_args(before: Option<std::ffi::OsString>) {
    let _ = WEBVIEW_ARGS_BEFORE.set(before);
}

/// WEBVIEW_ARGS for an agent: `None` while ByteHive never changed it (inherit
/// it as it is), else the value from before, or `Some(None)` for unset.
pub(crate) fn agent_webview_args() -> Option<Option<&'static std::ffi::OsStr>> {
    WEBVIEW_ARGS_BEFORE.get().map(|b| b.as_deref())
}

/// The same for a child started with std::process::Command (a CLI run in the
/// background), so it too sees the variable as it was before ByteHive.
pub fn restore_webview_args(cmd: &mut std::process::Command) {
    match agent_webview_args() {
        Some(Some(before)) => {
            cmd.env(WEBVIEW_ARGS, before);
        }
        Some(None) => {
            cmd.env_remove(WEBVIEW_ARGS);
        }
        None => {}
    }
}

pub struct PtySession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
}

impl PtySession {
    /// Spawn `spec` under a new PTY of `size`. Returns the session, the output
    /// reader (BLOCKING — run on its own thread), and the child (wait + raw
    /// handle live on the child, used on another thread).
    pub fn spawn(
        spec: &CommandSpec,
        size: PtySize,
    ) -> Result<(Self, Box<dyn Read + Send>, Box<dyn Child + Send + Sync>)> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(size)?;

        let mut cmd = CommandBuilder::new(&spec.program);
        for a in &spec.args {
            cmd.arg(a);
        }
        if let Some(cwd) = &spec.cwd {
            cmd.cwd(cwd);
        }
        // Drop the launching session's identity before applying our own vars, so
        // an explicit spec.env entry could still set one deliberately.
        for k in INHERITED_SESSION_MARKERS {
            cmd.env_remove(k);
        }
        match agent_webview_args() {
            Some(Some(before)) => cmd.env(WEBVIEW_ARGS, before),
            Some(None) => cmd.env_remove(WEBVIEW_ARGS),
            None => {}
        }
        for (k, v) in &spec.env {
            cmd.env(k, v);
        }

        let child = pair.slave.spawn_command(cmd)?;
        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;

        // Let the child see its controlling-terminal end close.
        drop(pair.slave);

        Ok((PtySession { master: pair.master, writer }, reader, child))
    }

    pub fn write_input(&mut self, bytes: &[u8]) -> Result<()> {
        self.writer.write_all(bytes)?;
        self.writer.flush()?;
        Ok(())
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        // portable-pty's resize already returns anyhow::Result<()>.
        self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
    }

    /// Drops the master (closes the ConPTY), which makes a blocked reader's
    /// read() return Ok(0) so its thread can end.
    pub fn shutdown(self) {
        drop(self);
    }
}
