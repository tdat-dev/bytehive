//! A frame of an agent's tab, from the hub's peek answer to something GPUI
//! paints: base64 JPEG in, a BGRA RenderImage out. Runs off the UI thread.

use std::sync::Arc;

use base64::Engine as _;
use gpui_kit::RenderImage;

use super::live::frame_of;

/// What one peek gave: maybe a picture (with its size in pixels), and
/// whether the tab was in front.
pub struct Peeked {
    pub image: Option<(Arc<RenderImage>, (u32, u32))>,
    pub front: bool,
}

/// Turn a peek answer into a frame. An answer without a picture still says
/// whether its tab is in front; a picture that does not decode is an error.
pub fn decode(answer: Result<serde_json::Value, String>) -> Result<Peeked, String> {
    let answer = answer?;
    let (data, front) = frame_of(&answer);
    let Some(data) = data else { return Ok(Peeked { image: None, front }) };
    let bytes = base64::engine::general_purpose::STANDARD.decode(data.as_bytes()).map_err(|e| e.to_string())?;
    let mut rgba = ::image::load_from_memory(&bytes).map_err(|e| e.to_string())?.into_rgba8();
    let size = (rgba.width(), rgba.height());
    // GPUI paints BGRA.
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let image = Arc::new(RenderImage::new(vec![::image::Frame::new(rgba)]));
    Ok(Peeked { image: Some((image, size)), front })
}

#[cfg(test)]
mod tests {
    use base64::Engine as _;

    use super::decode;

    #[test]
    fn a_bad_frame_says_so() {
        assert_eq!(decode(Err("tab isn't showing".into())).err().unwrap(), "tab isn't showing");
        let none = decode(Ok(serde_json::json!({ "content": [], "tab": { "active": false } }))).unwrap();
        assert!(none.image.is_none() && !none.front);
        let not_jpeg = serde_json::json!({ "content": [{ "type": "image", "data": "AAAA" }] });
        assert!(decode(Ok(not_jpeg)).is_err());
    }

    #[test]
    fn a_jpeg_becomes_a_picture_of_its_size() {
        let mut jpeg = Vec::new();
        let img = ::image::RgbImage::from_pixel(8, 5, ::image::Rgb([200, 10, 10]));
        ::image::DynamicImage::ImageRgb8(img).write_to(&mut std::io::Cursor::new(&mut jpeg), ::image::ImageFormat::Jpeg).unwrap();
        let data = base64::engine::general_purpose::STANDARD.encode(&jpeg);
        let got = decode(Ok(serde_json::json!({ "content": [{ "type": "image", "data": data }], "tab": { "active": true } }))).unwrap();
        let (image, size) = got.image.unwrap();
        assert_eq!(size, (8, 5));
        assert!(got.front);
        let bgra = image.as_bytes(0).unwrap();
        assert!(bgra[2] > 150 && bgra[0] < 60, "red lands in the third byte: BGRA");
    }
}
