//! How each person looks in the 3D office (model.ts lookOf): a shirt in the
//! colour of their department, hair from their id, and the prop of their
//! role, unless the owner chose a look. A colour comes from a file on disk, so
//! it is only trusted once it is one.

use crate::company::types::{Person, Rank, RoleDef};

pub(super) const FOUNDER_SHIRT: &str = "#eef0f4";
const SHIRTS: [(&str, &str); 5] = [("Management", "#5ec2f0"), ("Engineering", "#5fd08a"), ("Design", "#ff6b6b"), ("Marketing", "#e0913a"), ("Research", "#b18cf0")];
const SPARE_SHIRTS: [&str; 4] = ["#7d8b99", "#c98fb4", "#9db36b", "#6f9fd8"];
const HAIRS: [&str; 6] = ["#2a2e3a", "#4a3226", "#7a5230", "#b08a55", "#55585f", "#7a2f2a"];

/// model.ts hash: `(n * 31 + charCode) >>> 0` from 7, over UTF-16 units as `[...s]` gives code points.
fn hash(s: &str) -> u32 {
    s.chars().fold(7u32, |n, c| {
        let mut units = [0u16; 2];
        let first = c.encode_utf16(&mut units)[0];
        ((n as u64 * 31 + first as u64) % (1u64 << 32)) as u32
    })
}

/// What everyone chosen by the app wears. A role from the Founder may name any department.
pub(super) fn department_shirt(department: &str) -> &'static str {
    SHIRTS.iter().find(|(d, _)| *d == department).map_or_else(|| SPARE_SHIRTS[hash(department) as usize % SPARE_SHIRTS.len()], |(_, c)| *c)
}

pub(super) fn hair_for(id: &str) -> &'static str {
    HAIRS[hash(id) as usize % HAIRS.len()]
}

/// `#rgb` or `#rrggbb`, or the fallback.
fn colour_or(c: &str, fallback: &str) -> String {
    let hex = c.strip_prefix('#').unwrap_or("");
    if (hex.len() == 3 || hex.len() == 6) && hex.chars().all(|ch| ch.is_ascii_hexdigit()) { c.to_string() } else { fallback.to_string() }
}

/// (shirt, hair, prop).
pub(super) fn look_of(p: &Person, role: Option<&RoleDef>) -> (String, String, String) {
    let shirt = if p.rank == Rank::Founder { FOUNDER_SHIRT } else { department_shirt(role.map_or("", |r| r.department.as_str())) };
    let hair = hair_for(&p.id);
    if p.look.edited == Some(true) {
        return (colour_or(&p.look.shirt, shirt), colour_or(&p.look.hair, hair), p.look.prop.clone());
    }
    let shirt = if role.is_some() || p.rank == Rank::Founder { shirt.to_string() } else { colour_or(&p.look.shirt, shirt) };
    (shirt, hair.to_string(), p.look.prop.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shirts_and_hair_follow_model_ts() {
        assert_eq!(department_shirt("Engineering"), "#5fd08a");
        // From model.ts, run in node.
        assert_eq!(department_shirt("Ops"), "#6f9fd8");
        assert_eq!(department_shirt("Sales and Growth"), "#6f9fd8");
        for (id, hair) in [("ana", "#7a2f2a"), ("founder", "#2a2e3a"), ("lead-web-app-1", "#2a2e3a"), ("dev-2", "#4a3226")] {
            assert_eq!(hair_for(id), hair, "{id}");
        }
        assert_eq!(colour_or("#abc", "#000"), "#abc");
        assert_eq!(colour_or("red", "#000"), "#000");
        assert_eq!(colour_or("#12345g", "#000"), "#000");
    }
}
