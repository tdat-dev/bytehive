//! The prototype palette: ANSI 16 + 256-table + truecolor to egui colors.

use alacritty_terminal::vte::ansi::{Color, NamedColor};
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

pub fn rgb_of(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Spec(rgb) => (rgb.r, rgb.g, rgb.b),
        Color::Indexed(i) => indexed(i),
        Color::Named(n) => named(n),
    }
}

pub fn named(n: NamedColor) -> (u8, u8, u8) {
    use NamedColor::*;
    match n {
        Foreground => FG_DEFAULT,
        Background => BG_DEFAULT,
        Cursor => FG_DEFAULT,
        Black => ANSI16[0],
        Red => ANSI16[1],
        Green => ANSI16[2],
        Yellow => ANSI16[3],
        Blue => ANSI16[4],
        Magenta => ANSI16[5],
        Cyan => ANSI16[6],
        White => ANSI16[7],
        BrightBlack => ANSI16[8],
        BrightRed => ANSI16[9],
        BrightGreen => ANSI16[10],
        BrightYellow => ANSI16[11],
        BrightBlue => ANSI16[12],
        BrightMagenta => ANSI16[13],
        BrightCyan => ANSI16[14],
        BrightWhite => ANSI16[15],
        BrightForeground => (255, 255, 255),
        DimBlack => ANSI16[0],
        DimRed => ANSI16[1],
        DimGreen => ANSI16[2],
        DimYellow => ANSI16[3],
        DimBlue => ANSI16[4],
        DimMagenta => ANSI16[5],
        DimCyan => ANSI16[6],
        DimWhite => ANSI16[7],
        DimForeground => FG_DEFAULT,
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::vte::ansi::Rgb;

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
    fn named_colors_cover_the_ansi_16() {
        assert_eq!(named(NamedColor::Red), ANSI16[1]);
        assert_eq!(named(NamedColor::BrightWhite), ANSI16[15]);
        assert_eq!(named(NamedColor::Foreground), FG_DEFAULT);
        assert_eq!(named(NamedColor::Background), BG_DEFAULT);
        assert_eq!(rgb_of(Color::Spec(Rgb { r: 1, g: 2, b: 3 })), (1, 2, 3));
    }
}
