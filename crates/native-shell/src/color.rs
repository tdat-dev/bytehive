//! The prototype palette: ANSI 16 + 256-table + truecolor to egui colors.

use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::Color;
use eframe::egui::Color32;

/// Classic 16 ANSI colors, in SGR order.
pub const ANSI16: [(u8, u8, u8); 16] = [
    (0, 0, 0),       // black
    (205, 49, 49),   // red
    (13, 188, 121),  // green
    (229, 229, 16),  // yellow
    (36, 114, 200),  // blue
    (188, 63, 188),  // magenta
    (17, 168, 205),  // cyan
    (229, 229, 229), // white
    (102, 102, 102), // bright black
    (241, 76, 76),   // bright red
    (35, 209, 139),  // bright green
    (245, 245, 67),  // bright yellow
    (59, 142, 234),  // bright blue
    (214, 112, 214), // bright magenta
    (41, 184, 219),  // bright cyan
    (255, 255, 255), // bright white
];

pub const FG_DEFAULT: (u8, u8, u8) = (229, 229, 229);
pub const BG_DEFAULT: (u8, u8, u8) = (11, 13, 18);

/// The 256-color table: 0-15 reuse ANSI, 16-231 the cube, 232-255 the ramp.
pub fn indexed(i: u8) -> (u8, u8, u8) {
    if i < 16 {
        return ANSI16[i as usize];
    }
    if i < 232 {
        let v = [0, 95, 135, 175, 215, 255];
        let n = i - 16;
        return (v[(n / 36) as usize], v[((n / 6) % 6) as usize], v[(n % 6) as usize]);
    }
    let g = 8 + 10 * (i - 232);
    (g, g, g)
}

pub fn c32(t: (u8, u8, u8)) -> Color32 {
    Color32::from_rgb(t.0, t.1, t.2)
}

/// How much faint text (SGR 2) is dimmed, as alacritty dims it.
const DIM: f32 = 0.66;

fn scale(c: (u8, u8, u8), k: f32) -> (u8, u8, u8) {
    ((c.0 as f32 * k) as u8, (c.1 as f32 * k) as u8, (c.2 as f32 * k) as u8)
}

/// Palette slot `i` before a program changes it, numbered as OSC 4/10/11
/// ask for it: 0-255 the table, 256 foreground, 257 background, 258 cursor,
/// 259-266 the dim colours.
pub fn default_rgb(i: usize) -> (u8, u8, u8) {
    match i {
        0..=255 => indexed(i as u8),
        257 => BG_DEFAULT,
        259..=266 => scale(ANSI16[i - 259], DIM),
        267 => (255, 255, 255),
        268 => scale(FG_DEFAULT, DIM),
        _ => FG_DEFAULT,
    }
}

/// A colour with the program's own palette (set with OSC 4/10/11) over ours.
pub fn resolve(c: Color, colors: &Colors) -> (u8, u8, u8) {
    let slot = |i: usize| colors[i].map(|c| (c.r, c.g, c.b)).unwrap_or_else(|| default_rgb(i));
    match c {
        Color::Spec(rgb) => (rgb.r, rgb.g, rgb.b),
        Color::Indexed(i) => slot(i as usize),
        Color::Named(n) => slot(n as usize),
    }
}

/// A cell's text colour as Windows Terminal draws it: bold turns the eight
/// base colours bright, faint dims.
pub fn cell_fg(c: Color, flags: Flags, colors: &Colors) -> (u8, u8, u8) {
    let bold = flags.contains(Flags::BOLD);
    let c = match c {
        Color::Indexed(i) if bold && i < 8 => Color::Indexed(i + 8),
        Color::Named(n) if bold && (n as usize) < 8 => Color::Indexed(n as u8 + 8),
        c => c,
    };
    let rgb = resolve(c, colors);
    if flags.contains(Flags::DIM) { scale(rgb, DIM) } else { rgb }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::vte::ansi::{NamedColor, Rgb};

    #[test]
    fn indexed_colors_follow_the_256_table() {
        assert_eq!(indexed(0), ANSI16[0]);
        assert_eq!(indexed(15), ANSI16[15]);
        assert_eq!(indexed(16), (0, 0, 0));
        assert_eq!(indexed(231), (255, 255, 255));
        assert_eq!(indexed(232), (8, 8, 8));
        assert_eq!(indexed(255), (238, 238, 238));
    }

    #[test]
    fn bold_brightens_faint_dims_and_the_program_palette_wins() {
        let mut colors = Colors::default();
        let red = Color::Named(NamedColor::Red);
        assert_eq!(cell_fg(red, Flags::empty(), &colors), ANSI16[1]);
        assert_eq!(cell_fg(red, Flags::BOLD, &colors), ANSI16[9]);
        assert_eq!(cell_fg(Color::Indexed(2), Flags::BOLD, &colors), ANSI16[10]);
        let spec = Color::Spec(Rgb { r: 200, g: 100, b: 50 });
        assert_eq!(cell_fg(spec, Flags::DIM, &colors), (132, 66, 33));
        assert_eq!(cell_fg(spec, Flags::BOLD, &colors), (200, 100, 50), "true colour stays as sent");
        colors[1] = Some(Rgb { r: 1, g: 2, b: 3 });
        assert_eq!(cell_fg(red, Flags::empty(), &colors), (1, 2, 3), "OSC 4 changed red");
        assert_eq!(default_rgb(257), BG_DEFAULT);
    }

    #[test]
    fn named_colors_cover_the_ansi_16() {
        let colors = Colors::default();
        let named = |n: NamedColor| resolve(Color::Named(n), &colors);
        assert_eq!(named(NamedColor::Red), ANSI16[1]);
        assert_eq!(named(NamedColor::BrightWhite), ANSI16[15]);
        assert_eq!(named(NamedColor::Foreground), FG_DEFAULT);
        assert_eq!(named(NamedColor::Background), BG_DEFAULT);
        assert_eq!(resolve(Color::Spec(Rgb { r: 1, g: 2, b: 3 }), &colors), (1, 2, 3));
    }
}
