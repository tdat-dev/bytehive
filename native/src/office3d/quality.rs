//! How much the office draws (quality.ts). Auto starts with everything on and
//! gives one thing up per level change that fell under 30 frames a second:
//! shadows first, then glow, then ambient motion. Low is the bottom step, and
//! battery means Low. Where the setting is kept and how the battery is read
//! come with the Company page (P6).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quality {
    Auto,
    Low,
    High,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub shadows: bool,
    pub glow: bool,
    pub ambient: bool,
    pub max_dpr: f32,
}

const STEPS: [Look; 4] = [
    Look { shadows: true, glow: true, ambient: true, max_dpr: 1.5 },
    Look { shadows: false, glow: true, ambient: true, max_dpr: 1.5 },
    Look { shadows: false, glow: false, ambient: true, max_dpr: 1.25 },
    Look { shadows: false, glow: false, ambient: false, max_dpr: 1.0 },
];

pub const LOW_TIER: usize = STEPS.len() - 1;
/// Slower than this at the middle of a level change, and it is not 30 fps.
const SLOW_MS: f64 = 40.0;
const HITCH_MS: f64 = 100.0;
const MIN_FRAMES: usize = 6;

pub fn look_of(tier: usize) -> Look {
    STEPS[tier.min(LOW_TIER)]
}

pub fn start_tier(q: Quality, battery: bool) -> usize {
    match q {
        Quality::High => 0,
        Quality::Low => LOW_TIER,
        Quality::Auto if battery => LOW_TIER,
        Quality::Auto => 0,
    }
}

/// After a level change: the tier to go on with, from the gaps between its frames.
pub fn stepped_down(tier: usize, q: Quality, gaps: &[f64]) -> usize {
    if q != Quality::Auto || tier >= LOW_TIER || gaps.len() < MIN_FRAMES {
        return tier;
    }
    let mut sorted = gaps.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let middle = sorted[gaps.len() / 2];
    let hitches = gaps.iter().filter(|g| **g > HITCH_MS).count();
    if middle > SLOW_MS || hitches > 1 { tier + 1 } else { tier }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gives_up_shadows_then_glow_then_motion() {
        assert!(look_of(0).shadows && look_of(0).glow && look_of(0).ambient);
        assert!(!look_of(1).shadows && look_of(1).glow);
        assert!(!look_of(2).glow && look_of(2).ambient);
        assert!(!look_of(3).ambient);
        assert_eq!(look_of(99), look_of(LOW_TIER));
    }

    #[test]
    fn starts_low_on_battery_unless_asked_for_high() {
        assert_eq!(start_tier(Quality::Auto, false), 0);
        assert_eq!(start_tier(Quality::Auto, true), LOW_TIER);
        assert_eq!(start_tier(Quality::Low, false), LOW_TIER);
        assert_eq!(start_tier(Quality::High, true), 0);
    }

    #[test]
    fn steps_down_one_for_a_slow_or_hitching_change_only_on_auto() {
        let smooth = [16.0; 10];
        let slow = [45.0; 10];
        let hitchy = [16.0, 120.0, 16.0, 130.0, 16.0, 16.0, 16.0];
        assert_eq!(stepped_down(0, Quality::Auto, &smooth), 0);
        assert_eq!(stepped_down(0, Quality::Auto, &slow), 1);
        assert_eq!(stepped_down(1, Quality::Auto, &hitchy), 2);
        assert_eq!(stepped_down(0, Quality::High, &slow), 0);
        assert_eq!(stepped_down(LOW_TIER, Quality::Auto, &slow), LOW_TIER);
        assert_eq!(stepped_down(0, Quality::Auto, &slow[..3]), 0, "too few frames to judge");
    }
}
