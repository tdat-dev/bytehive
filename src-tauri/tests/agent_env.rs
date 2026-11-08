#![cfg(windows)]
//! ByteHive sets WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS on itself so its own
//! windows keep their timers while hidden (lib.rs). That is for ByteHive's
//! webviews only: an agent, and any WebView2 app it starts, gets the variable
//! as it was before ByteHive changed it. One test, since that "before" is
//! remembered once per process.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytehive_lib::core::command_spec::CommandSpec;
use bytehive_lib::core::pty_session::{remember_webview_args, WEBVIEW_ARGS};
use bytehive_lib::core::registry::{self, Registry};
use portable_pty::PtySize;

/// What `cmd /C echo WV[%VAR%]WV` prints in an agent, between the brackets.
fn seen_by_agent(pipe: bool) -> String {
    let reg = Arc::new(Mutex::new(Registry::new()));
    let out = Arc::new(Mutex::new(Vec::new()));
    let o = out.clone();
    let spec = CommandSpec::new("cmd.exe").arg("/C").arg(format!("echo WV[%{WEBVIEW_ARGS}%]WV"));
    let size = PtySize { rows: 24, cols: 120, pixel_width: 0, pixel_height: 0 };
    Registry::spawn_shared(&reg, "a".into(), 1, &spec, size, pipe, move |b| o.lock().unwrap().extend_from_slice(b), |_| {})
        .expect("spawn");
    if !pipe {
        let _ = registry::lock(&reg).write_input("a", b"\x1b[1;1R"); // ConPTY's startup query
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let text = String::from_utf8_lossy(&out.lock().unwrap()).to_string();
        if let Some((_, rest)) = text.split_once("WV[") {
            if let Some((value, _)) = rest.split_once("]WV") {
                return value.to_string();
            }
        }
        assert!(Instant::now() < deadline, "no output from the agent (pipe: {pipe}): {text:?}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn agents_see_the_variable_as_it_was_before_bytehive_set_it() {
    remember_webview_args(None); // nobody had set it
    std::env::set_var(WEBVIEW_ARGS, "--bytehive-own-switch");
    for pipe in [false, true] {
        // cmd leaves an unset variable as written.
        assert_eq!(seen_by_agent(pipe), format!("%{WEBVIEW_ARGS}%"), "the agent must see it unset, as before (pipe: {pipe})");
    }
}
