#![cfg(windows)]
//! A child on plain pipes, no terminal: for agents that talk in JSON lines
//! (Claude Code's stream-json mode) instead of drawing a screen.
//!
//! It still gets a console of its own, hidden. Without one (CREATE_NO_WINDOW,
//! or a GUI parent's nothing) every console program it starts, such as the
//! user's hooks, opens a window of its own; with a hidden one they share it.
//! std::process can't hide a new console (wShowWindow is unstable), so this
//! calls CreateProcessW itself. The child starts suspended, so the caller can
//! put it in its kill-on-close job before it runs anything.

use std::ffi::{c_void, OsStr, OsString};
use std::fs::File;
use std::io::Write;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{FromRawHandle, RawHandle};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, SetHandleInformation, HANDLE, HANDLE_FLAGS, HANDLE_FLAG_INHERIT};
use windows::Win32::Security::SECURITY_ATTRIBUTES;
use windows::Win32::System::Pipes::CreatePipe;
use windows::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess, InitializeProcThreadAttributeList, ResumeThread,
    UpdateProcThreadAttribute, WaitForSingleObject, CREATE_NEW_CONSOLE, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT,
    EXTENDED_STARTUPINFO_PRESENT, INFINITE, LPPROC_THREAD_ATTRIBUTE_LIST, PROCESS_INFORMATION,
    PROC_THREAD_ATTRIBUTE_HANDLE_LIST, STARTF_USESHOWWINDOW, STARTF_USESTDHANDLES, STARTUPINFOEXW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

use crate::command_spec::CommandSpec;
use crate::pty_session::INHERITED_SESSION_MARKERS;

/// What the agent reads: each line written here is one message to it.
pub struct PipeSession {
    stdin: File,
}

impl PipeSession {
    pub fn write_input(&mut self, bytes: &[u8]) -> Result<()> {
        self.stdin.write_all(bytes)?;
        self.stdin.flush()?;
        Ok(())
    }
}

/// The started (suspended) process. Owns its handles.
pub struct PipeChild {
    process: HANDLE,
    thread: HANDLE,
}

// Kernel handles; moving them across threads is sound.
unsafe impl Send for PipeChild {}

impl PipeChild {
    pub fn raw_handle(&self) -> RawHandle {
        self.process.0 as RawHandle
    }

    /// Let it run (after it is in its job).
    pub fn resume(&mut self) {
        if !self.thread.is_invalid() {
            unsafe {
                ResumeThread(self.thread);
                let _ = CloseHandle(self.thread);
            }
            self.thread = HANDLE::default();
        }
    }

    /// Block until it exits; its exit code.
    pub fn wait(self) -> u32 {
        let mut code = 1u32;
        unsafe {
            WaitForSingleObject(self.process, INFINITE);
            let _ = GetExitCodeProcess(self.process, &mut code);
        }
        code
    }
}

impl Drop for PipeChild {
    fn drop(&mut self) {
        unsafe {
            if !self.thread.is_invalid() {
                // Never resumed (a failed job): it must not be left frozen.
                ResumeThread(self.thread);
                let _ = CloseHandle(self.thread);
            }
            let _ = CloseHandle(self.process);
        }
    }
}

fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

/// One argument quoted the way the C runtime splits a command line.
pub fn quote_arg(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '\n', '"']) {
        return arg.to_string();
    }
    let mut out = String::from('"');
    let mut slashes = 0usize;
    for c in arg.chars() {
        match c {
            '\\' => slashes += 1,
            '"' => {
                out.push_str(&"\\".repeat(slashes * 2 + 1));
                out.push('"');
                slashes = 0;
            }
            _ => {
                out.push_str(&"\\".repeat(slashes));
                slashes = 0;
                out.push(c);
            }
        }
    }
    out.push_str(&"\\".repeat(slashes * 2));
    out.push('"');
    out
}

/// Find `program` the way a shell would: a path as given, else the first
/// folder on PATH holding it with one of PATHEXT's endings.
pub fn resolve(program: &str, path_var: Option<&OsStr>, pathext: Option<&str>) -> Option<PathBuf> {
    let p = Path::new(program);
    if p.components().count() > 1 || p.is_absolute() {
        return Some(p.to_path_buf());
    }
    let exts: Vec<String> = pathext
        .unwrap_or(".COM;.EXE;.BAT;.CMD")
        .split(';')
        .filter(|e| !e.is_empty())
        .map(|e| e.to_ascii_lowercase())
        .collect();
    let has_ext = p.extension().is_some();
    for dir in std::env::split_paths(path_var?) {
        if has_ext {
            let c = dir.join(program);
            if c.is_file() {
                return Some(c);
            }
        }
        for e in &exts {
            let c = dir.join(format!("{program}{e}"));
            if c.is_file() {
                return Some(c);
            }
        }
    }
    None
}

/// The program an npm shim (`claude.cmd`) runs, when it is an .exe next to it:
/// `"%dp0%\node_modules\...\claude.exe" %*`. Running that directly keeps the
/// arguments away from cmd.exe, which can't carry every character.
pub fn unshim(cmd: &Path, text: &str) -> Option<PathBuf> {
    let dir = cmd.parent()?;
    for line in text.lines() {
        let Some(start) = line.find("\"%dp0%") else { continue };
        let rest = &line[start + 1..];
        let end = rest.find('"')?;
        let rel = rest[..end].trim_start_matches("%dp0%").trim_start_matches(['\\', '/']);
        if !rel.to_ascii_lowercase().ends_with(".exe") || rel.contains('%') {
            continue;
        }
        let exe = dir.join(rel);
        if exe.is_file() {
            return Some(exe);
        }
    }
    None
}

/// The command line to start: an .exe directly, a .cmd/.bat (an npm shim)
/// through the .exe it wraps, else through cmd.exe.
fn command_line(exe: &Path, args: &[String]) -> (Option<PathBuf>, String) {
    let ext = exe.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if ext == "cmd" || ext == "bat" {
        if let Some(inner) = std::fs::read_to_string(exe).ok().and_then(|t| unshim(exe, &t)) {
            return command_line(&inner, args);
        }
    }
    let ext = exe.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let mut line = quote_arg(&exe.to_string_lossy());
    for a in args {
        line.push(' ');
        line.push_str(&quote_arg(a));
    }
    if ext == "cmd" || ext == "bat" {
        let comspec = std::env::var_os("ComSpec").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows\System32\cmd.exe"));
        // /s: the outer quotes go, the rest is run as written.
        return (Some(comspec), format!("cmd.exe /d /s /c \"{line}\""));
    }
    (Some(exe.to_path_buf()), line)
}

/// Our environment, minus the launching Claude session's identity, plus `extra`.
fn environment(extra: &[(String, String)]) -> Vec<u16> {
    let mut vars: Vec<(OsString, OsString)> = std::env::vars_os()
        .filter(|(k, _)| !INHERITED_SESSION_MARKERS.iter().any(|m| k.to_string_lossy().eq_ignore_ascii_case(m)))
        .collect();
    for (k, v) in extra {
        vars.retain(|(x, _)| !x.to_string_lossy().eq_ignore_ascii_case(k));
        vars.push((k.into(), v.into()));
    }
    vars.sort_by_key(|(k, _)| k.to_string_lossy().to_uppercase());
    let mut block: Vec<u16> = Vec::new();
    for (k, v) in vars {
        block.extend(k.encode_wide());
        block.push('=' as u16);
        block.extend(v.encode_wide());
        block.push(0);
    }
    block.push(0);
    block
}

/// An inheritable pipe: (read end, write end).
fn pipe() -> Result<(HANDLE, HANDLE)> {
    let sa = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: true.into(),
    };
    let (mut r, mut w) = (HANDLE::default(), HANDLE::default());
    unsafe { CreatePipe(&mut r, &mut w, Some(&sa), 0)? };
    Ok((r, w))
}

/// Our end of a pipe stays ours: not handed to the child.
fn keep(h: HANDLE) -> Result<()> {
    unsafe { SetHandleInformation(h, HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0))? };
    Ok(())
}

/// Start `spec` suspended on pipes. Returns its input, its stdout and stderr
/// (blocking readers, for threads of their own) and the child.
pub fn spawn(spec: &CommandSpec) -> Result<(PipeSession, File, File, PipeChild)> {
    let path_var = std::env::var_os("PATH");
    let pathext = std::env::var("PATHEXT").ok();
    let exe = resolve(&spec.program, path_var.as_deref(), pathext.as_deref())
        .ok_or_else(|| anyhow!("{} was not found. Is it installed and on PATH?", spec.program))?;
    let (app, line) = command_line(&exe, &spec.args);
    let mut line = wide(OsStr::new(&line));
    let app = app.map(|a| wide(a.as_os_str()));
    let env = environment(&spec.env);
    let cwd = spec.cwd.as_ref().map(|d| wide(OsStr::new(d)));

    let (in_r, in_w) = pipe()?;
    let (out_r, out_w) = pipe()?;
    let (err_r, err_w) = pipe()?;
    let close_all = |hs: &[HANDLE]| hs.iter().for_each(|h| unsafe { let _ = CloseHandle(*h); });
    let all = [in_r, in_w, out_r, out_w, err_r, err_w];
    if let Err(e) = keep(in_w).and(keep(out_r)).and(keep(err_r)) {
        close_all(&all);
        return Err(e);
    }

    // Only the child's three pipe ends are inherited, nothing else of ours.
    let handles = [in_r, out_w, err_w];
    let mut size = 0usize;
    unsafe {
        let _ = InitializeProcThreadAttributeList(None, 1, None, &mut size);
    }
    let mut attrs = vec![0u8; size];
    let list = LPPROC_THREAD_ATTRIBUTE_LIST(attrs.as_mut_ptr() as *mut c_void);
    let started = unsafe {
        InitializeProcThreadAttributeList(Some(list), 1, None, &mut size).and_then(|_| {
            let r = UpdateProcThreadAttribute(
                list,
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                Some(handles.as_ptr() as *const c_void),
                size_of::<[HANDLE; 3]>(),
                None,
                None,
            )
            .and_then(|_| {
                let mut si = STARTUPINFOEXW::default();
                si.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
                si.StartupInfo.dwFlags = STARTF_USESTDHANDLES | STARTF_USESHOWWINDOW;
                si.StartupInfo.wShowWindow = SW_HIDE.0 as u16;
                si.StartupInfo.hStdInput = in_r;
                si.StartupInfo.hStdOutput = out_w;
                si.StartupInfo.hStdError = err_w;
                si.lpAttributeList = list;
                let mut pi = PROCESS_INFORMATION::default();
                CreateProcessW(
                    app.as_ref().map_or(PCWSTR::null(), |a| PCWSTR(a.as_ptr())),
                    Some(PWSTR(line.as_mut_ptr())),
                    None,
                    None,
                    true,
                    EXTENDED_STARTUPINFO_PRESENT | CREATE_NEW_CONSOLE | CREATE_UNICODE_ENVIRONMENT | CREATE_SUSPENDED,
                    Some(env.as_ptr() as *const c_void),
                    cwd.as_ref().map_or(PCWSTR::null(), |d| PCWSTR(d.as_ptr())),
                    &si.StartupInfo,
                    &mut pi,
                )
                .map(|_| pi)
            });
            DeleteProcThreadAttributeList(list);
            r
        })
    };
    // The child's ends are the child's now (or nobody's).
    close_all(&[in_r, out_w, err_w]);
    let pi = match started {
        Ok(pi) => pi,
        Err(e) => {
            close_all(&[in_w, out_r, err_r]);
            return Err(anyhow!("couldn't start {}: {e}", spec.program));
        }
    };
    let file = |h: HANDLE| unsafe { File::from_raw_handle(h.0 as RawHandle) };
    Ok((
        PipeSession { stdin: file(in_w) },
        file(out_r),
        file(err_r),
        PipeChild { process: pi.hProcess, thread: pi.hThread },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};

    #[test]
    fn quotes_like_the_c_runtime_splits() {
        assert_eq!(quote_arg("plain"), "plain");
        assert_eq!(quote_arg("two words"), "\"two words\"");
        assert_eq!(quote_arg(""), "\"\"");
        assert_eq!(quote_arg(r#"{"a":"b c"}"#), r#""{\"a\":\"b c\"}""#);
        assert_eq!(quote_arg(r"C:\dir with space\"), r#""C:\dir with space\\""#);
    }

    #[test]
    fn finds_a_program_on_path_with_its_ending() {
        let dir = std::env::temp_dir().join(format!("bytehive-resolve-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("tool.cmd"), "@echo off").unwrap();
        let path = std::env::join_paths([dir.clone()]).unwrap();
        assert_eq!(resolve("tool", Some(&path), Some(".EXE;.CMD")), Some(dir.join("tool.cmd")));
        assert_eq!(resolve("nope", Some(&path), Some(".EXE;.CMD")), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_npm_shim_runs_the_exe_it_wraps() {
        let dir = std::env::temp_dir().join(format!("bytehive-unshim-{}", std::process::id()));
        let bin = dir.join("node_modules").join("pkg").join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("tool.exe"), "").unwrap();
        let shim = dir.join("tool.cmd");
        let text = "@ECHO off\r\nCALL :find_dp0\r\n\"%dp0%\\node_modules\\pkg\\bin\\tool.exe\"   %*\r\n";
        assert_eq!(unshim(&shim, text), Some(dir.join(r"node_modules\pkg\bin\tool.exe")));
        // a node script shim has no .exe to run
        assert_eq!(unshim(&shim, "\"%_prog%\"  \"%dp0%\\node_modules\\pkg\\cli.js\" %*"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn talks_over_pipes_and_reports_its_exit() {
        // findstr echoes every line it reads; closing its input ends it.
        let spec = CommandSpec::new("findstr").arg("x");
        let (mut s, out, _err, mut child) = spawn(&spec).unwrap();
        child.resume();
        s.write_input(b"x marks\n").unwrap();
        drop(s);
        let mut line = String::new();
        BufReader::new(out).read_line(&mut line).unwrap();
        assert_eq!(line.trim_end(), "x marks");
        assert_eq!(child.wait(), 0);
    }
}
