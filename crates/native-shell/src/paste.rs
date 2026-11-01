//! Clipboard into a terminal. egui turns Ctrl+C, Ctrl+X and Ctrl+V into
//! Copy / Cut / Paste events instead of keys, so a terminal answers them
//! itself: Ctrl+C and Ctrl+X go to the program as the control bytes they are
//! (the terminal has no selection to copy), pasted text goes in as one paste
//! (bracketed when the program asked for that), and a pasted picture is
//! saved as a PNG whose path is typed in, the way agents take a dropped file.
//! The picture folder is the Tauri app's (`%TEMP%\bytehive-paste`).

use std::path::{Path, PathBuf};

use eframe::egui;

const PASTE_DIR: &str = "bytehive-paste";
/// Pictures pasted in earlier sessions go after a week.
const KEEP_SECS: u64 = 7 * 24 * 3600;

/// The bytes a paste sends: newlines as Enter, wrapped in the bracketed-paste
/// markers when the program turned that mode on.
pub fn paste_bytes(text: &str, bracketed: bool) -> Vec<u8> {
    let body = text.replace("\r\n", "\n").replace('\n', "\r");
    if bracketed {
        format!("\x1b[200~{body}\x1b[201~").into_bytes()
    } else {
        body.into_bytes()
    }
}

/// Save a pasted picture as a PNG in `dir`; its path.
pub fn save_image_in(dir: &Path, img: &egui::ColorImage, now_ms: u64) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|a| a.as_secs() > KEEP_SECS);
        if old {
            let _ = std::fs::remove_file(e.path());
        }
    }
    let [w, h] = img.size;
    let rgba: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
    let buf = image::RgbaImage::from_raw(w as u32, h as u32, rgba).ok_or("the picture is empty")?;
    let mut path = dir.join(format!("pasted-{now_ms}.png"));
    let mut n = 1;
    while path.exists() {
        path = dir.join(format!("pasted-{now_ms}-{n}.png"));
        n += 1;
    }
    buf.save_with_format(&path, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn save_image(img: &egui::ColorImage) -> Result<PathBuf, String> {
    save_image_in(&std::env::temp_dir().join(PASTE_DIR), img, crate::app::now_ms())
}

/// What a clipboard event sends to the terminal's program, if anything.
pub fn clipboard_bytes(ev: &egui::Event, bracketed: bool) -> Option<Vec<u8>> {
    match ev {
        egui::Event::Copy => Some(vec![0x03]),
        egui::Event::Cut => Some(vec![0x18]),
        egui::Event::Paste(t) => Some(paste_bytes(t, bracketed)),
        egui::Event::PasteImage(img) => save_image(img).ok().map(|p| paste_bytes(&p.to_string_lossy(), bracketed)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_and_pastes_reach_the_program() {
        assert_eq!(clipboard_bytes(&egui::Event::Copy, false), Some(vec![0x03]), "Ctrl+C interrupts");
        assert_eq!(clipboard_bytes(&egui::Event::Cut, false), Some(vec![0x18]));
        assert_eq!(paste_bytes("a\r\nb\n", false), b"a\rb\r");
        assert_eq!(paste_bytes("hi", true), b"\x1b[200~hi\x1b[201~");
        assert_eq!(clipboard_bytes(&egui::Event::Text("x".into()), false), None);
    }

    #[test]
    fn a_pasted_picture_becomes_a_png_file() {
        let dir = std::env::temp_dir().join(format!("bytehive-paste-test-{}", std::process::id()));
        let img = egui::ColorImage::new([3, 2], vec![egui::Color32::RED; 6]);
        let p = save_image_in(&dir, &img, 42).unwrap();
        assert!(p.ends_with("pasted-42.png"));
        let back = image::open(&p).unwrap();
        assert_eq!((back.width(), back.height()), (3, 2));
        let p2 = save_image_in(&dir, &img, 42).unwrap();
        assert!(p2.ends_with("pasted-42-1.png"), "never overwrites");
        std::fs::remove_dir_all(&dir).ok();
    }
}
