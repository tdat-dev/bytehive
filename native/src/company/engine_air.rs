//! What the 3D office reads off the engine besides the company: the messages
//! in the air between two people (engine.ts companyFlights, for the paper
//! plane) and each floor's board as To do, Doing and Done counts (live.ts
//! readBoards, here off the watch's own read of the boards).

use std::collections::{BTreeMap, HashMap};

use crate::board::List;

/// How long a message is in the air, ms (layout.ts FLIGHT_MS).
pub const FLIGHT_MS: i64 = 2400;

/// A message on its way: from whom, to whom, and when it left.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flight {
    pub id: String,
    pub from: String,
    pub to: String,
    pub at: i64,
}

/// A board's cards: To do, Doing, Done.
pub type BoardCounts = (u32, u32, u32);

#[derive(Default)]
pub struct Air {
    flights: Vec<Flight>,
    boards: HashMap<String, BoardCounts>,
}

/// How many cards a list of this title holds (live.ts count).
fn count(lists: &[List], title: &str) -> u32 {
    lists.iter().find(|l| l.title.trim().to_lowercase() == title).map_or(0, |l| l.cards.len() as u32)
}

impl Air {
    /// A message left `from` for `to` at `now`. One to oneself does not fly.
    pub fn sent(&mut self, ts: u64, from: &str, to: &str, now: i64) {
        if from == to {
            return;
        }
        self.flights.retain(|f| now - f.at < FLIGHT_MS);
        let id = format!("{ts}-{}-{to}", self.flights.len());
        self.flights.push(Flight { id, from: from.into(), to: to.into(), at: now });
    }

    pub fn flights(&self) -> Vec<Flight> {
        self.flights.clone()
    }

    /// A floor's board as just read.
    pub fn board(&mut self, floor: &str, lists: &[List]) {
        self.boards.insert(floor.into(), (count(lists, "to do"), count(lists, "doing"), count(lists, "done")));
    }

    /// A floor with no board, or no people, has no count.
    pub fn no_board(&mut self, floor: &str) {
        self.boards.remove(floor);
    }

    pub fn keep_boards(&mut self, keep: impl Fn(&str) -> bool) {
        self.boards.retain(|id, _| keep(id));
    }

    pub fn boards(&self) -> BTreeMap<String, BoardCounts> {
        self.boards.iter().map(|(k, v)| (k.clone(), *v)).collect()
    }

    pub fn clear(&mut self) {
        self.flights.clear();
        self.boards.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_flies_for_a_while_and_not_to_oneself() {
        let mut air = Air::default();
        air.sent(1, "lena", "arun", 1000);
        air.sent(2, "arun", "arun", 1000);
        assert_eq!(air.flights().len(), 1);
        air.sent(3, "arun", "kai", 1000 + FLIGHT_MS);
        assert_eq!(air.flights().iter().map(|f| f.to.as_str()).collect::<Vec<_>>(), ["kai"], "the first one landed");
    }
}
