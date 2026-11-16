//! The office's colours (palette.ts): the meshes' from the approved look
//! (style-a.webp), a project's accent, skin beads, and WCAG contrast for the
//! words drawn over the scene. The page's own colours are the Inbox's
//! (inbox/company/look.rs), so palette.ts's APP set is not here.

pub mod office {
    pub const SKY: &str = "#191d27";
    pub const GROUND: &str = "#23262e";
    pub const SHELL_OUT: &str = "#3a3e4a";
    pub const SHELL_EDGE: &str = "#2d303a";
    pub const SLAB_TOP: &str = "#4c505c";
    pub const WALL_LIT: &str = "#efb877";
    pub const WALL_DIM: &str = "#6d4632";
    pub const WALL_DARK: &str = "#475a90";
    pub const WALL_NIGHT_LOW: &str = "#272f54";
    pub const FLOOR_LIT: &str = "#8a6e52";
    pub const FLOOR_DARK: &str = "#3a4670";
    pub const GROUT: &str = "#65493a";
    pub const GROUT_NIGHT: &str = "#222a4a";
    pub const HONEY: &str = "#f5b83d";
    pub const GLOW: &str = "#ffe2ae";
    pub const GLASS: &str = "#9fc3d8";
    pub const STEEL: &str = "#6a7180";
    pub const DESK_TOP: &str = "#b07a45";
    pub const DESK_BODY: &str = "#2f3240";
    pub const CHAIR: &str = "#262935";
    pub const SCREEN_ON: &str = "#7cc4ff";
    pub const SCREEN_OFF: &str = "#1b1e24";
    pub const LAMP_NEEDS: &str = "#f5b83d";
    pub const BEAD: &str = "#efc48a";
    pub const PLANT: &str = "#4f8f4a";
    pub const PLANT_DEEP: &str = "#2f6a4a";
    pub const POT: &str = "#b5734a";
    pub const PAPER: &str = "#f2ece0";
    pub const CORK: &str = "#b98d58";
    pub const NOTE_TODO: &str = "#f2d15e";
    pub const NOTE_DOING: &str = "#8fc7f2";
    pub const NOTE_DONE: &str = "#8fd9a4";
    pub const GRASS: &str = "#2f5048";
    pub const GRASS_LIT: &str = "#4a6a4e";
    pub const STONE: &str = "#4a4f5e";
    pub const KERB: &str = "#3a3e4d";
    pub const LEAF: &str = "#2f5e57";
    pub const LEAF_DEEP: &str = "#1f4347";
    pub const BARK: &str = "#3a2f33";
    pub const SKY_TOP: &str = "#0a1030";
    pub const SKY_HIGH: &str = "#101a48";
    pub const SKY_MID: &str = "#1c2c6a";
    pub const SKY_LOW: &str = "#34418a";
    pub const SKY_GLOW: &str = "#a86f7c";
    pub const HAZE: &str = "#54508a";
    pub const MOON: &str = "#c4c6d0";
}

/// A project's colour, for rugs, cushions and art. None is honey: that means your turn.
pub const ACCENTS: [&str; 8] = ["#3f8f8a", "#c4605a", "#7a5fa8", "#5d8a4e", "#4f7fc0", "#b0587f", "#3b6f96", "#a8664a"];
/// Skin, for a toy: warm beads, light to deep.
pub const BEADS: [&str; 5] = ["#f3cfa0", "#efc48a", "#e0a873", "#c98a5b", "#a8693f"];
pub const CHIP_BG: &str = "#14161c";

fn luminance(hex: &str) -> f64 {
    let n = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
    let ch = |v: u32| {
        let c = v as f64 / 255.0;
        if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * ch(n >> 16) + 0.7152 * ch((n >> 8) & 255) + 0.0722 * ch(n & 255)
}

/// WCAG contrast between two colours.
pub fn contrast(a: &str, b: &str) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_accent_is_honey() {
        assert!(ACCENTS.iter().all(|a| !a.eq_ignore_ascii_case(office::HONEY)));
    }

    #[test]
    fn words_over_the_scene_read_at_wcag_aa() {
        // The pairs from palette.ts that the native page still draws over the office.
        for (what, ink, bg, least) in [("needs tag on a chip", office::HONEY, CHIP_BG, 4.5), ("bubble mark on honey", "#14161c", office::HONEY, 4.5)] {
            assert!(contrast(ink, bg) >= least, "{what}: {}", contrast(ink, bg));
        }
    }

    #[test]
    fn contrast_is_the_wcag_ratio() {
        assert!((contrast("#000000", "#ffffff") - 21.0).abs() < 1e-9);
        assert!((contrast("#777777", "#777777") - 1.0).abs() < 1e-9);
    }
}
