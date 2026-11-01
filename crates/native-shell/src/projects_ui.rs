//! What a project action asks (show one, open one, close one), and the
//! system dialog that picks a folder to open.

pub enum ProjectAction {
    Filter(Option<String>),
    Open(String),
    Close(String),
}

/// Pick a folder with the system dialog; none when cancelled.
pub fn pick_folder(start: &str) -> Option<String> {
    let mut d = rfd::FileDialog::new().set_title("Open a project folder");
    if !start.trim().is_empty() {
        d = d.set_directory(start);
    }
    d.pick_folder().map(|p| p.to_string_lossy().into_owned())
}
