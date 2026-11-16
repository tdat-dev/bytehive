//! The office's repeatable randomness (geometry.ts): `seeded` is mulberry32, so
//! a seed always gives the same room, and `hash_of` turns a name into a seed.
//! Both in 32-bit integers as JavaScript's Math.imul and `>>> 0` work.

/// seeded(seed): numbers in 0..1, the same run for the same seed.
pub struct Seeded(u32);

impl Seeded {
    pub fn new(seed: u32) -> Self {
        Self(seed)
    }

    pub fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b_79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        (t ^ (t >> 14)) as f64 / 4_294_967_296.0
    }
}

/// hashOf(s): `[...s]` walks code points and `charCodeAt(0)` takes each one's
/// first UTF-16 unit, so a character past the BMP counts its high surrogate.
pub fn hash_of(s: &str) -> u32 {
    s.chars().fold(7u32, |n, c| {
        let mut units = [0u16; 2];
        let first = c.encode_utf16(&mut units)[0];
        n.wrapping_mul(31).wrapping_add(first as u32)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_javascript() {
        // From node: seeded(42) three times, then hashOf of three names.
        let mut r = Seeded::new(42);
        let got = [r.next(), r.next(), r.next()];
        let want = [0.6011037519201636, 0.44829055899754167, 0.8524657934904099];
        for (g, w) in got.iter().zip(want) {
            assert!((g - w).abs() < 1e-15, "{g} vs {w}");
        }
        assert_eq!(hash_of("tea-shop"), 2_800_930_714);
        assert_eq!(hash_of("V\u{103}n ph\u{f2}ng"), 2_377_480_909);
        assert_eq!(hash_of("a\u{1F600}"), 65_091);
    }
}
