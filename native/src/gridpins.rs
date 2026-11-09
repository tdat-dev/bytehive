//! Who sits in the Grid (togglePin, fillPins, gridOrder and railGroups in
//! src/inbox.ts): the agents you pinned, topped up from the list to the size
//! Settings > Inbox > Agents in the Grid allows.

use std::collections::HashSet;

/// Pin or unpin an agent. A pin past `max` pushes out the oldest one, never
/// `keep` (the agent in the terminal).
pub fn toggle_pin(pins: &[String], id: &str, keep: Option<&str>, max: usize) -> Vec<String> {
    if pins.iter().any(|p| p == id) {
        return pins.iter().filter(|p| *p != id).cloned().collect();
    }
    let mut next: Vec<String> = pins.to_vec();
    next.push(id.to_string());
    while next.len() > max {
        let at = next.iter().position(|x| Some(x.as_str()) != keep && x != id).unwrap_or(0);
        next.remove(at);
    }
    next
}

/// Who is in the Grid: the agents already there (so nobody jumps around),
/// topped up from the list order to `max`, skipping the ones you closed.
pub fn fill_pins(pins: &[String], order: &[String], closed: &HashSet<String>, max: usize) -> Vec<String> {
    let mut out: Vec<String> = pins.iter().filter(|id| order.contains(id) && !closed.contains(*id)).cloned().collect();
    for id in order {
        if out.len() >= max {
            break;
        }
        if !out.contains(id) && !closed.contains(id) {
            out.push(id.clone());
        }
    }
    let from = out.len().saturating_sub(max);
    out.split_off(from)
}

/// Who may sit in the Grid, in list order: agents that are alive. A stopped
/// one leaves it and comes back once it is resumed. `list` is (id, alive).
pub fn grid_order<'a>(list: impl IntoIterator<Item = (&'a str, bool)>) -> Vec<String> {
    list.into_iter().filter(|(_, alive)| *alive).map(|(id, _)| id.to_string()).collect()
}

/// The cards by project, the terminal's own project first and the rest by
/// name, so agents of one project sit together. Within a project they keep
/// the order they came in.
pub fn rail_groups<T>(list: Vec<(String, T)>, home: Option<&str>) -> Vec<(String, Vec<T>)> {
    let mut groups: Vec<(String, Vec<T>)> = Vec::new();
    for (project, t) in list {
        match groups.iter_mut().find(|g| g.0 == project) {
            Some(g) => g.1.push(t),
            None => groups.push((project, vec![t])),
        }
    }
    groups.sort_by(|a, b| {
        let at_home = |g: &(String, Vec<T>)| Some(g.0.as_str()) == home;
        at_home(b).cmp(&at_home(a)).then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase())).then_with(|| a.0.cmp(&b.0))
    });
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn pins_up_to_the_size_and_never_push_out_the_terminal() {
        assert_eq!(toggle_pin(&ids(&["a", "b"]), "b", None, 9), ids(&["a"]));
        assert_eq!(toggle_pin(&ids(&["a", "b", "c", "d"]), "e", Some("a"), 4), ids(&["a", "c", "d", "e"]));
        assert_eq!(toggle_pin(&ids(&["a", "b", "c", "d"]), "e", Some("b"), 4), ids(&["b", "c", "d", "e"]));
        assert_eq!(toggle_pin(&ids(&["a", "b", "c", "d", "e"]), "f", None, 9), ids(&["a", "b", "c", "d", "e", "f"]));
    }

    #[test]
    fn fills_from_the_list_without_moving_who_is_there() {
        let none = HashSet::new();
        assert_eq!(fill_pins(&[], &ids(&["a", "b", "c", "d", "e"]), &none, 4), ids(&["a", "b", "c", "d"]));
        assert_eq!(fill_pins(&[], &ids(&["a", "b", "c", "d", "e", "f", "g"]), &none, 6), ids(&["a", "b", "c", "d", "e", "f"]));
        assert_eq!(fill_pins(&ids(&["c", "a"]), &ids(&["a", "b", "c"]), &none, 9), ids(&["c", "a", "b"]));
        assert_eq!(fill_pins(&ids(&["a", "gone"]), &ids(&["a", "b", "c"]), &HashSet::from(["b".to_string()]), 9), ids(&["a", "c"]));
    }

    #[test]
    fn stopped_agents_stay_out_until_resumed() {
        let order = grid_order([("a", true), ("b", false), ("c", true), ("m", false)]);
        assert_eq!(order, ids(&["a", "c"]));
        let none = HashSet::new();
        assert_eq!(fill_pins(&ids(&["b", "a"]), &order, &none, 9), ids(&["a", "c"]));
        let order = grid_order([("a", true), ("b", true), ("c", true), ("m", false)]);
        assert_eq!(fill_pins(&ids(&["a", "c"]), &order, &none, 9), ids(&["a", "c", "b"]));
    }

    #[test]
    fn cards_group_by_project_the_terminals_first() {
        let list = || vec![("zoldify".to_string(), "z1"), ("bytehive".to_string(), "m1"), ("app".to_string(), "a1"), ("zoldify".to_string(), "z2")];
        let got: Vec<(String, Vec<&str>)> = rail_groups(list(), Some("zoldify"));
        assert_eq!(got, vec![("zoldify".to_string(), vec!["z1", "z2"]), ("app".to_string(), vec!["a1"]), ("bytehive".to_string(), vec!["m1"])]);
        let names: Vec<String> = rail_groups(list(), None).into_iter().map(|g| g.0).collect();
        assert_eq!(names, ids(&["app", "bytehive", "zoldify"]));
    }
}
