//! The MCP server every member of the Company runs (mcp/, bundled into one file
//! that plain node runs), and the Node it needs.

use super::launch::{NodeRun, NodeVerdict, node_verdict};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const FILE: &str = "bytehive-mcp.mjs";

/// How long `node --version` gets (engine.ts NODE_WAIT_MS).
const NODE_WAIT: Duration = Duration::from_secs(30);

/// The server file: beside the app in mcp-server\, where the installer puts it (and
/// the Tauri app kept it), else in a checkout's mcp\dist, which
/// `npm --prefix mcp run bundle -- dist/bytehive-mcp.mjs` writes for a dev run.
pub fn mcp_server() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok();
    server_in(exe.as_deref().and_then(Path::parent), Path::new(env!("CARGO_MANIFEST_DIR")))
}

fn server_in(exe_dir: Option<&Path>, crate_dir: &Path) -> Option<PathBuf> {
    let beside = exe_dir.map(|d| d.join("mcp-server").join(FILE));
    let checkout = crate_dir.parent().map(|repo| repo.join("mcp").join("dist").join(FILE));
    beside.into_iter().chain(checkout).find(|p| p.is_file())
}

/// Whether this machine's Node runs the server: an error is the words the owner
/// reads. A check that could not tell is logged and lets the job through, as the
/// Tauri app does.
pub fn node_check() -> Result<(), String> {
    match node_verdict(&run_node()) {
        NodeVerdict::Ok => Ok(()),
        NodeVerdict::Refuse(text) => Err(text),
        NodeVerdict::Log(text) => {
            eprintln!("[bytehive] {text}");
            Ok(())
        }
    }
}

/// `node --version`, through cmd /c as every launch here, with no window.
fn run_node() -> NodeRun {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let spec = agent_core::launch::launch_spec("node", &["--version".to_string()]);
    let child = Command::new(&spec.program)
        .args(&spec.args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => return NodeRun::Failed(e.to_string()),
    };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() < NODE_WAIT => std::thread::sleep(Duration::from_millis(50)),
            Ok(None) => {
                let _ = child.kill();
                return NodeRun::Failed(format!("node took longer than {} s", NODE_WAIT.as_secs()));
            }
            Err(e) => return NodeRun::Failed(e.to_string()),
        }
    }
    match child.wait_with_output() {
        Ok(out) => NodeRun::Printed { printed: String::from_utf8_lossy(&out.stdout).into_owned(), code: out.status.code() },
        Err(e) => NodeRun::Failed(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "// server").unwrap();
    }

    #[test]
    fn mcp_server_takes_the_file_beside_the_app_first() {
        let t = tempfile::tempdir().unwrap();
        let (app, repo) = (t.path().join("app"), t.path().join("repo"));
        put(&app.join("mcp-server").join(FILE));
        put(&repo.join("mcp").join("dist").join(FILE));
        assert_eq!(server_in(Some(&app), &repo.join("native")), Some(app.join("mcp-server").join(FILE)));
    }

    #[test]
    fn mcp_server_falls_back_to_the_checkouts_bundle_in_a_dev_run() {
        let t = tempfile::tempdir().unwrap();
        let repo = t.path().join("repo");
        put(&repo.join("mcp").join("dist").join(FILE));
        assert_eq!(server_in(Some(&t.path().join("target")), &repo.join("native")), Some(repo.join("mcp").join("dist").join(FILE)));
    }

    #[test]
    fn mcp_server_is_none_when_neither_is_there() {
        let t = tempfile::tempdir().unwrap();
        assert_eq!(server_in(Some(t.path()), &t.path().join("native")), None);
        assert_eq!(server_in(None, &t.path().join("native")), None);
    }
}
