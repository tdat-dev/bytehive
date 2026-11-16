//! Colours as three's Color handles them, in f64: a CSS hex turned linear on
//! the way in (colour management on), and back to an sRGB hex, rounded, by
//! getHexString. props.ts darkens and mixes colours this way (`dim`, `shift`),
//! and a rounding a step off would change a colour the office shows.

use glam::DVec3;

fn to_linear(c: f64) -> f64 {
    if c < 0.04045 { c * 0.0773993808 } else { (c * 0.9478672986 + 0.0521327014).powf(2.4) }
}

fn to_srgb(c: f64) -> f64 {
    if c < 0.0031308 { c * 12.92 } else { 1.055 * c.powf(0.41666) - 0.055 }
}

/// `new Color(hex)`: linear, f64.
pub fn linear(hex: &str) -> DVec3 {
    let n = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0xffffff);
    let ch = |shift: u32| to_linear(((n >> shift) & 0xff) as f64 / 255.0);
    DVec3::new(ch(16), ch(8), ch(0))
}

/// `#${color.getHexString()}`.
pub fn hex_of(c: DVec3) -> String {
    let ch = |v: f64| (to_srgb(v) * 255.0).clamp(0.0, 255.0).round() as u32;
    format!("#{:02x}{:02x}{:02x}", ch(c.x), ch(c.y), ch(c.z))
}

/// props.ts `dim(hex, k)`: darker (or lighter) by k, in linear light.
pub fn dim(hex: &str, k: f64) -> String {
    hex_of(linear(hex) * k)
}

/// props.ts `shift(hex, to, k)`: k of the way to `to`.
pub fn shift(hex: &str, to: &str, k: f64) -> String {
    hex_of(linear(hex).lerp(linear(to), k))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_as_three_does() {
        // From node with three 0.170: dim("#3f8f8a", 0.72), shift("#c4605a", "#f2e4cf", 0.18).
        assert_eq!(dim("#3f8f8a", 0.72), "#357b77");
        assert_eq!(shift("#c4605a", "#f2e4cf", 0.18), "#cd857a");
        assert_eq!(dim("#b5734a", 1.0), "#b5734a");
    }
}
