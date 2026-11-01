//! Opening a file in your own editor, as the approved design asks ("Files
//! open in your editor. ByteHive shows the branch; it doesn't try to be a
//! second IDE"). VS Code first, then Cursor, else whatever Windows opens the
//! file with. Launched through `cmd /c` (the editors install as .cmd shims)
//! without flashing a console window.

use std::path::Path;
use std::process::Command;

/// Editors tried in order: name shown, program on PATH.
const EDITORS: [(&str, &str); 2] = [("VS Code", "code"), ("Cursor", "cursor")];

fn quiet(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console flash for the cmd.exe wrapper.
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}

/// Whether `program` resolves on PATH.
fn on_path(program: &str) -> bool {
    let finder = if cfg!(windows) { "where" } else { "which" };
    quiet(Command::new(finder).arg(program)).output().is_ok_and(|o| o.status.success())
}

/// The editor that will open files, for the drawer's note.
pub fn editor_name() -> &'static str {
    EDITORS.iter().find(|(_, p)| on_path(p)).map_or("the app Windows picks", |(n, _)| *n)
}

/// Open `path` (at its first line) in the first editor found. Returns which
/// one, or why nothing opened.
pub fn open_in_editor(path: &Path) -> Result<&'static str, String> {
    if !path.exists() {
        return Err(format!("{} is not on disk (deleted on this branch).", path.display()));
    }
    for (name, program) in EDITORS {
        if on_path(program) {
            let shown = path.to_string_lossy().into_owned();
            let spawned = if cfg!(windows) {
                quiet(Command::new("cmd").args(["/c", program, "-g", &shown])).spawn()
            } else {
                Command::new(program).args(["-g", &shown]).spawn()
            };
            return spawned.map(|_| name).map_err(|e| format!("Could not start {name}: {e}"));
        }
    }
    let opened = if cfg!(windows) {
        quiet(Command::new("cmd").args(["/c", "start", ""]).arg(path)).spawn()
    } else {
        Command::new("xdg-open").arg(path).spawn()
    };
    opened.map(|_| "the default app").map_err(|e| format!("Could not open {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_is_refused_before_launching_anything() {
        let err = open_in_editor(Path::new("Z:\\no\\such\\file.rs")).unwrap_err();
        assert!(err.contains("not on disk"), "{err}");
    }
}
