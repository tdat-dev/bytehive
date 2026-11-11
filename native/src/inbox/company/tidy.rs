//! After Mark done, the people of a Job who had nothing to merge give their
//! worktree back: a Lead who only handed out cards, a QA who only reported. Their
//! folders and branches would sit on the disk for nothing. They get a fresh one
//! from the base at their next start (fleet_host::needs_worktree).

use std::time::Duration;

use crate::company::types::FOUNDER_ID;
use crate::review::land;

use super::*;

/// The people of a Job whose worktree can go: not the Founder, seated, and with
/// a branch a merge would bring nothing in from.
pub(super) fn empty_seats<'a>(people: &[String], seats: impl Iterator<Item = (&'a str, Option<&'a str>, Option<bool>)>) -> Vec<String> {
    seats
        .filter(|(person, worktree, empty)| *person != FOUNDER_ID && people.iter().any(|p| p == person) && worktree.is_some() && *empty == Some(true))
        .map(|(person, ..)| person.to_string())
        .collect()
}

impl Inbox {
    pub(super) fn co_tidy(&mut self, people: &[String], cx: &mut Context<Self>) {
        let seats = self.fleet.seats.iter().filter_map(|s| Some((s.spec.person.as_deref()?, s.spec.worktree.as_deref(), s.changes.empty)));
        for person in empty_seats(people, seats) {
            let Some(i) = self.co_seat(&person) else { continue };
            let Some(wt) = self.fleet.seats[i].spec.worktree.clone() else { continue };
            let id = self.fleet.seats[i].spec.id.clone();
            let ran = self.fleet.seats[i].terminal.as_ref().is_some_and(|t| t.read(cx).exit_code().is_none());
            if ran {
                self.fleet.stop(i);
            }
            cx.spawn(async move |this, cx| {
                // Its CLI lets go of the folder first.
                cx.background_executor().timer(Duration::from_millis(400)).await;
                let gone = cx
                    .background_spawn(async move {
                        // Looked at again here: work that came in since stays.
                        if land::nothing_to_merge(&wt) != Some(true) {
                            return Err(format!("{wt} has work in it now"));
                        }
                        land::discard_branch(&wt)
                    })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    let Some(i) = this.fleet.seats.iter().position(|s| s.spec.id == id) else { return };
                    match gone {
                        Ok(()) => {
                            let seat = &mut this.fleet.seats[i];
                            seat.spec.worktree = None;
                            seat.spec.branch = None;
                            seat.branch = None;
                            this.fleet.save();
                        }
                        Err(why) => {
                            eprintln!("[bytehive] a worktree was kept after Mark done: {why}");
                            if ran {
                                this.start_seat(i, cx);
                            }
                        }
                    }
                    cx.notify();
                });
            })
            .detach();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::empty_seats;

    #[test]
    fn mark_done_gives_back_only_the_empty_worktrees_of_the_jobs_own_people() {
        let people = ["lead".to_string(), "qa".to_string(), "dev".to_string(), "founder".to_string()];
        let seats = [
            ("lead", Some("D:/wt/lead"), Some(true)),
            ("qa", Some("D:/wt/qa"), Some(true)),
            ("dev", Some("D:/wt/dev"), Some(false)),
            ("founder", Some("D:/p"), Some(true)),
            ("other", Some("D:/wt/other"), Some(true)),
            ("unread", Some("D:/wt/unread"), None),
        ];
        let seats = seats.iter().map(|(p, w, e)| (*p, *w, *e));
        assert_eq!(empty_seats(&people, seats), ["lead", "qa"]);
    }
}
