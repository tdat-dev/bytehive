//! Moving through the office (nav.ts), the parts the 3D scene needs: one level
//! up, a level kept valid as people move and projects close, and how often to
//! draw. The crumbs and the rail's targets live with the Company page.

use super::layout::Level;

/// 30 frames a second, rounded up so a slow timer never makes it 31.
pub const MOVE_MS: u64 = 34;
/// Working people move at 8 frames a second: toy-like, cheap to leave on.
pub const AMBIENT_MS: u64 = 125;
/// The Founder sits on the roof, on no floor.
pub const FOUNDER_ID: &str = "founder";

pub fn nav_up(l: &Level) -> Level {
    match l {
        Level::Desk { floor: Some(f), .. } => Level::Floor(f.clone()),
        _ => Level::Building,
    }
}

/// A person's desk level: the Founder's is on no floor.
fn desk_of(person: &str, floor: Option<&str>) -> Level {
    Level::Desk { person: person.to_string(), floor: if person == FOUNDER_ID { None } else { floor.map(str::to_string) } }
}

/// The level, kept to what still exists. `people`: everyone with their floor;
/// `floors`: the floor ids there are.
pub fn clamp_level(l: &Level, people: &[(String, Option<String>)], floors: &[String]) -> Level {
    if let Level::Desk { person, floor } = l
        && let Some((id, at)) = people.iter().find(|(id, _)| id == person)
    {
        let here = desk_of(id, at.as_deref());
        return if matches!(&here, Level::Desk { floor: f, .. } if f == floor) { l.clone() } else { here };
    }
    let floor = match l {
        Level::Building => None,
        Level::Floor(f) => Some(f),
        Level::Desk { floor, .. } => floor.as_ref(),
    };
    match floor {
        Some(f) if floors.contains(f) => Level::Floor(f.clone()),
        _ => Level::Building,
    }
}

/// How long until the next frame, or None to stop drawing.
pub fn frame_delay(visible: bool, moving: bool, ambient: bool, reduced: bool) -> Option<u64> {
    if !visible {
        return None;
    }
    if moving {
        return Some(MOVE_MS);
    }
    (ambient && !reduced).then_some(AMBIENT_MS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floor() -> Level {
        Level::Floor("a".into())
    }
    fn desk() -> Level {
        Level::Desk { person: "ana".into(), floor: Some("a".into()) }
    }
    fn people() -> Vec<(String, Option<String>)> {
        vec![("founder".into(), None), ("ana".into(), Some("a".into()))]
    }

    #[test]
    fn nav_up_goes_one_level_up_and_stops_at_the_building() {
        assert_eq!(nav_up(&desk()), floor());
        assert_eq!(nav_up(&floor()), Level::Building);
        assert_eq!(nav_up(&Level::Building), Level::Building);
        assert_eq!(nav_up(&Level::Desk { person: "founder".into(), floor: None }), Level::Building);
    }

    #[test]
    fn clamp_level_keeps_follows_and_falls_back() {
        let floors = vec!["a".to_string()];
        assert_eq!(clamp_level(&desk(), &people(), &floors), desk());
        assert_eq!(clamp_level(&floor(), &people(), &floors), floor());
        let moved = vec![("ana".to_string(), Some("b".to_string()))];
        assert_eq!(clamp_level(&desk(), &moved, &["a".into(), "b".into()]), Level::Desk { person: "ana".into(), floor: Some("b".into()) });
        let top = Level::Desk { person: "founder".into(), floor: None };
        assert_eq!(clamp_level(&top, &people(), &floors), top);
        assert_eq!(clamp_level(&Level::Desk { person: "gone".into(), floor: Some("a".into()) }, &people(), &floors), floor());
        assert_eq!(clamp_level(&Level::Floor("z".into()), &people(), &floors), Level::Building);
    }

    #[test]
    fn frame_delay_draws_only_what_moves() {
        assert_eq!(frame_delay(true, false, false, false), None);
        assert_eq!(frame_delay(false, true, true, false), None);
        assert_eq!(frame_delay(true, true, false, false), Some(MOVE_MS));
        assert!(MOVE_MS as f64 >= 1000.0 / 30.0);
        assert_eq!(frame_delay(true, false, true, false), Some(AMBIENT_MS));
        assert_eq!(frame_delay(true, false, true, true), None);
        assert_eq!(frame_delay(true, true, false, true), Some(MOVE_MS));
    }
}
