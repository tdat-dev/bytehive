//! Today's terminal palette (src/termtheme.ts) and how a cell's colour resolves.

use super::*;

pub(super) fn from_hex(hex: u32) -> Rgb {
    Rgb { r: (hex >> 16) as u8, g: (hex >> 8) as u8, b: hex as u8 }
}

pub(super) fn hsla(c: Rgb) -> Hsla {
    rgb(((c.r as u32) << 16) | ((c.g as u32) << 8) | c.b as u32).into()
}

pub(super) fn dimmed(c: Rgb) -> Rgb {
    Rgb { r: (c.r as f32 * 0.66) as u8, g: (c.g as f32 * 0.66) as u8, b: (c.b as f32 * 0.66) as u8 }
}

/// The 256 colour cube and grey ramp above the ANSI 16.
pub(super) fn indexed(i: u8) -> Rgb {
    match i {
        0..=15 => from_hex(ANSI[i as usize]),
        16..=231 => {
            let step = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
            let n = i - 16;
            Rgb { r: step(n / 36), g: step(n / 6 % 6), b: step(n % 6) }
        }
        _ => {
            let v = 8 + 10 * (i - 232);
            Rgb { r: v, g: v, b: v }
        }
    }
}

pub(super) fn resolve(color: Color, set: &Colors) -> Rgb {
    match color {
        Color::Spec(c) => c,
        Color::Indexed(i) => set[i as usize].unwrap_or_else(|| indexed(i)),
        Color::Named(n) => set[n].unwrap_or_else(|| named(n)),
    }
}

pub(super) fn named(n: NamedColor) -> Rgb {
    let i = n as usize;
    match n {
        NamedColor::Background => BACKGROUND,
        NamedColor::Cursor => CURSOR,
        NamedColor::Foreground | NamedColor::BrightForeground => FOREGROUND,
        NamedColor::DimForeground => dimmed(FOREGROUND),
        _ if i < 16 => from_hex(ANSI[i]),
        _ => dimmed(from_hex(ANSI[(i - NamedColor::DimBlack as usize) % 8])),
    }
}
