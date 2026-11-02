//! Each agent's mark: its CLI's own logo (see brand.rs). A plain shell, or a
//! custom command without a known logo, gets a small pixel face worked out
//! from its name instead: four parts (head, top, eyes, mouth), 192 faces in
//! all, the same faces the web app had.

use eframe::egui::{Color32, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};

use crate::sessions_ui::Cli;

/// One half of an 8x8 face, left to right (the other half mirrors it): "#"
/// the head, "o" a dark cell, "." empty. Parts are laid over each other.
type Half = [&'static str; 8];

const HEADS: [Half; 3] = [
    ["....", "....", ".###", "####", "####", "####", "####", ".###"],
    ["....", "....", "####", "####", "####", "####", "####", "####"],
    ["....", ".###", ".###", ".###", ".###", ".###", ".###", ".###"],
];
const TOPS: [Half; 4] = [
    ["...#", "...#", "....", "....", "....", "....", "....", "...."],
    ["....", "#...", "#...", "....", "....", "....", "....", "...."],
    ["..##", ".###", "....", "....", "....", "....", "....", "...."],
    ["....", "....", "....", "....", "....", "....", "....", "...."],
];
const EYES: [Half; 4] = [
    ["....", "....", "....", "....", ".o..", "....", "....", "...."],
    ["....", "....", "....", "....", ".oo.", "....", "....", "...."],
    ["....", "....", "....", ".o..", ".o..", "....", "....", "...."],
    ["....", "....", "....", "....", ".ooo", "....", "....", "...."],
];
const MOUTHS: [Half; 4] = [
    ["....", "....", "....", "....", "....", "....", "...o", "...."],
    ["....", "....", "....", "....", "....", "....", "..oo", "...."],
    ["....", "....", "....", "....", "....", "..o.", "...o", "...."],
    ["....", "....", "....", "....", "....", "....", "....", "...."],
];

/// ByteHive's own agent names, each with a face of its own.
const PERSONAS: [&str; 20] = ["Ana", "Bob", "Cid", "Dot", "Eve", "Fin", "Gio", "Hux", "Ivy", "Jax", "Kim", "Lux", "Mei", "Nix", "Oz", "Pia", "Rue", "Sol", "Tao", "Uma"];

fn fnv(s: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for c in s.encode_utf16() {
        h ^= c as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// Which parts a name gets: (head, top, eyes, mouth).
pub fn parts(seed: &str) -> [usize; 4] {
    let s = seed.trim().to_lowercase();
    let all = (HEADS.len() * TOPS.len() * EYES.len() * MOUTHS.len()) as u32;
    // 97 shares no factor with 192: stepping by it visits every face once.
    let mut h = match PERSONAS.iter().position(|n| n.to_lowercase() == s) {
        Some(k) => (k as u32 * 97) % all,
        None => fnv(if s.is_empty() { "?" } else { &s }),
    };
    let mut pick = |n: usize| {
        let v = h % n as u32;
        h /= n as u32;
        v as usize
    };
    [pick(HEADS.len()), pick(TOPS.len()), pick(EYES.len()), pick(MOUTHS.len())]
}

/// The face as 8 rows of 8 cells.
pub fn grid(seed: &str) -> [[u8; 8]; 8] {
    let [h, t, e, m] = parts(seed);
    let mut g = [[b'.'; 8]; 8];
    let mut lay = |half: &Half, on_head: bool| {
        for (y, row) in half.iter().enumerate() {
            for (x, c) in row.bytes().enumerate() {
                if c == b'.' || (on_head && g[y][x] != b'#') {
                    continue;
                }
                g[y][x] = c;
                g[y][7 - x] = c;
            }
        }
    };
    lay(&HEADS[h], false);
    lay(&TOPS[t], false);
    lay(&EYES[e], true);
    lay(&MOUTHS[m], true);
    g
}

/// The CLI's own color, as the web app had it.
pub fn cli_color(cli: Option<Cli>) -> Color32 {
    match cli {
        Some(Cli::Claude) => Color32::from_rgb(0xd9, 0x77, 0x57),
        Some(Cli::Codex) => Color32::from_rgb(0x10, 0xa3, 0x7f),
        Some(Cli::Opencode) => Color32::from_rgb(0xf0, 0x88, 0x3e),
        Some(Cli::Muse) => Color32::from_rgb(0xf4, 0x72, 0xb6),
        None => Color32::from_rgb(0x5e, 0xc2, 0xf0),
    }
}

/// The avatar tile: the brand color mixed 22% into near-black and a ring of
/// it at 38%, with the CLI's own logo; a custom command nobody knows and a
/// plain shell get a pixel face. Dimmed for a stopped agent.
pub fn avatar(p: &Painter, at: Rect, cli: Option<Cli>, custom: Option<&str>, name: &str, radius: f32, dim: bool) {
    let brand = cli.map(crate::brand::of_cli).or_else(|| custom.and_then(crate::brand::of_command));
    let c = brand.map_or(cli_color(None), |b| b.color);
    let a = if dim { 0.55 } else { 1.0 };
    let base = Color32::from_rgb(17, 17, 20).lerp_to_gamma(c, 0.22);
    p.rect_filled(at, radius, base.gamma_multiply(a));
    p.rect_stroke(at, radius, Stroke::new(1.0, c.gamma_multiply(0.38 * a)), StrokeKind::Inside);
    if let Some(b) = brand {
        let r = Rect::from_center_size(at.center(), Vec2::splat((at.width() * 0.66).round()));
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        p.image(crate::brand::texture(p.ctx(), b), r, uv, Color32::WHITE.gamma_multiply(a));
        return;
    }
    let side = (at.width() * 0.66).round();
    let cell = side / 8.0;
    let o = (at.center() - Vec2::splat(side / 2.0)).round();
    let dark = Color32::from_rgb(11, 11, 14);
    for (y, row) in grid(name).iter().enumerate() {
        for (x, &k) in row.iter().enumerate() {
            let col = match k {
                b'#' => c,
                b'o' => dark,
                _ => continue,
            };
            let r = Rect::from_min_size(Pos2::new(o.x + x as f32 * cell, o.y + y as f32 * cell), Vec2::splat(cell));
            p.rect_filled(r.expand(0.25), 0.0, col.gamma_multiply(a));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn personas_get_their_own_faces_and_names_are_stable() {
        assert_eq!(parts("Ana"), [0, 0, 0, 0]);
        assert_eq!(parts("bob"), parts(" Bob "));
        assert_ne!(parts("Bob"), parts("Cid"));
        assert_eq!(grid("someone"), grid("Someone"));
        // Mirrored left to right.
        let g = grid("Eve");
        assert!(g.iter().all(|r| (0..4).all(|x| r[x] == r[7 - x])));
    }
}
