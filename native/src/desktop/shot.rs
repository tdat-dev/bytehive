//! A picture of the agent's Chrome window, whole: its pixels are the
//! coordinates the desktop path clicks at (desktop/tree.rs uses the same origin).

use base64::Engine;
use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::Storage::Xps::PrintWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowRect, PW_RENDERFULLCONTENT};

pub fn rect(hwnd: isize) -> RECT {
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    unsafe { GetWindowRect(hwnd as HWND, &mut r) };
    r
}

/// The window as RGB pixels, drawn by the window itself (so it works when covered).
fn pixels(hwnd: isize) -> Result<(u32, u32, Vec<u8>), String> {
    let r = rect(hwnd);
    let (w, h) = ((r.right - r.left).max(0), (r.bottom - r.top).max(0));
    if w == 0 || h == 0 {
        return Err("Chrome's window has no size (is it minimised?).".into());
    }
    unsafe {
        let screen = GetDC(std::ptr::null_mut());
        let dc = CreateCompatibleDC(screen);
        let bmp = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(dc, bmp as HGDIOBJ);
        let drawn = PrintWindow(hwnd as HWND, dc, PW_RENDERFULLCONTENT) != 0;
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = w;
        info.bmiHeader.biHeight = -h; // top-down
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let mut bgra = vec![0u8; (w * h * 4) as usize];
        let lines = GetDIBits(dc, bmp, 0, h as u32, bgra.as_mut_ptr().cast(), &mut info, DIB_RGB_COLORS);
        SelectObject(dc, old);
        DeleteObject(bmp as HGDIOBJ);
        DeleteDC(dc);
        ReleaseDC(std::ptr::null_mut(), screen);
        if !drawn || lines == 0 {
            return Err("Windows would not draw Chrome's window.".into());
        }
        let rgb: Vec<u8> = bgra.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0]]).collect();
        Ok((w as u32, h as u32, rgb))
    }
}

/// The screenshot tool's answer: a JPEG of the whole window.
pub fn screenshot(hwnd: isize) -> Result<serde_json::Value, String> {
    let (w, h, rgb) = pixels(hwnd)?;
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 80)
        .encode(&rgb, w, h, image::ExtendedColorType::Rgb8)
        .map_err(|e| format!("The screenshot could not be made: {e}"))?;
    let data = base64::engine::general_purpose::STANDARD.encode(jpeg);
    Ok(serde_json::json!({
        "content": [
            { "type": "image", "data": data, "mimeType": "image/jpeg" },
            { "type": "text", "text": format!("The whole Chrome window, {w}x{h}: click coordinates are pixels of this picture.") },
        ]
    }))
}
