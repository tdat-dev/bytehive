use super::harness::*;

fn approved(t: &mut T) -> String {
    t.waiting_job(plan_for("a", 1));
    t.approve(&t.job(0).id);
    t.member()
}

// Stop

#[test]
fn stop_ends_the_turn_of_someone_working() {
    let mut t = T::new();
    let m = approved(&mut t);
    t.host.seat_mut(&m).last_output_at = t.host.now;
    t.e.stop_job(&mut t.host, "J-0001").unwrap();
    assert!(t.host.keys.contains(&m));
}

#[test]
fn stop_sends_no_key_when_the_owners_typing_is_all_that_came_out() {
    let mut t = T::new();
    let m = approved(&mut t);
    let now = t.host.now;
    let s = t.host.seat_mut(&m);
    s.last_input_at = now - 100;
    s.last_output_at = now - 95; // the echo
    t.e.stop_job(&mut t.host, "J-0001").unwrap();
    assert!(!t.host.keys.contains(&m));
}

#[test]
fn stop_still_ends_the_turn_of_someone_working_after_the_owner_typed() {
    let mut t = T::new();
    let m = approved(&mut t);
    let now = t.host.now;
    let s = t.host.seat_mut(&m);
    s.last_input_at = now - 2000;
    s.last_output_at = now;
    t.e.stop_job(&mut t.host, "J-0001").unwrap();
    assert!(t.host.keys.contains(&m));
}

#[test]
fn stop_sends_no_key_to_a_question_on_screen_esc_would_answer_it_no() {
    let mut t = T::new();
    let m = approved(&mut t);
    t.host.seat_mut(&m).last_output_at = t.host.now;
    t.host.asking = true;
    t.e.stop_job(&mut t.host, "J-0001").unwrap();
    assert!(!t.host.keys.contains(&m));
}

#[test]
fn stop_sends_no_key_to_someone_quiet_who_may_be_at_a_prompt() {
    let mut t = T::new();
    let m = approved(&mut t);
    t.host.seat_mut(&m).last_output_at = t.host.now - 60_000;
    t.e.stop_job(&mut t.host, "J-0001").unwrap();
    assert!(!t.host.keys.contains(&m));
}

#[test]
fn a_pause_does_not_end_a_turn() {
    let mut t = T::new();
    let m = approved(&mut t);
    t.host.seat_mut(&m).last_output_at = t.host.now;
    let j = &mut t.e.state_mut().jobs[0];
    j.token_cap = 1;
    j.tokens_used = 5;
    t.advance(30_000);
    assert_eq!(t.job(0).status, JobStatus::Paused);
    assert!(t.host.keys.is_empty());
}

#[test]
fn stop_marks_the_jobs_cards_on_the_boards() {
    let mut t = T::new();
    approved(&mut t);
    t.e.stop_job(&mut t.host, "J-0001").unwrap();
    assert!(t.cards(A).iter().all(|c| c.stopped));
    let written = crate::board::serialize(&t.host.boards[A]);
    assert!(written.contains("\"stopped\": true"));
    assert!(crate::board::parse(&written).unwrap().lists.iter().flat_map(|l| &l.cards).all(|c| c.stopped));
}

#[test]
fn stop_still_stops_when_a_board_cannot_be_written() {
    let mut t = T::new();
    approved(&mut t);
    t.host.write_fails.push_back(BoardWrite::Failed("disk full".into()));
    t.e.stop_job(&mut t.host, "J-0001").unwrap();
    assert_eq!(t.job(0).status, JobStatus::Stopped);
    assert!(t.host.logs.iter().any(|l| l.contains("disk full")));
}

// delivery

#[test]
fn waits_out_the_boot_of_someone_it_just_started() {
    let mut t = T::new();
    t.give("do it").unwrap();
    assert!(t.host.typed.is_empty());
    assert_eq!(t.e.said.len(), 1);
    assert!(t.e.said[0].1.contains("JOB J-0001") && t.e.said[0].2);
    t.advance(6000);
    assert!(t.host.typed.is_empty());
    t.advance(3000);
    assert_eq!(t.host.typed.len(), 1);
    assert!(t.host.typed[0].1.contains("JOB J-0001"));
}

#[test]
fn types_straight_in_once_they_are_at_their_prompt() {
    let mut t = T::new();
    t.founder_up(A);
    t.give("do it").unwrap();
    assert_eq!(t.host.typed.len(), 1);
    assert_eq!(t.host.typed[0].0, "founder");
    assert!(t.e.waiting().is_empty());
}

#[test]
fn waits_while_a_question_is_on_screen_however_long_they_have_been_up() {
    let mut t = T::new();
    t.founder_up(A);
    t.host.asking = true;
    t.give("do it").unwrap();
    t.advance(30_000);
    assert!(t.host.typed.is_empty());
    t.host.asking = false;
    t.advance(6000);
    assert!(t.host.typed.is_empty(), "it waits for the screen to settle after the answer");
    t.advance(4500);
    assert_eq!(t.host.typed.len(), 1);
}

#[test]
fn waits_for_one_who_is_up_but_not_at_their_prompt_yet() {
    let mut t = T::new();
    t.founder_up(A);
    t.host.starting = true;
    t.give("do it").unwrap();
    t.advance(30_000);
    assert!(t.host.typed.is_empty());
    t.host.starting = false;
    t.advance(STEP);
    assert_eq!(t.host.typed.len(), 1);
}

#[test]
fn types_anyway_when_the_input_box_never_shows() {
    let mut t = T::new();
    t.founder_up(A);
    t.host.starting = true;
    t.give("do it").unwrap();
    t.advance(2 * 60_000 + 3000);
    assert_eq!(t.host.typed.len(), 1);
}

#[test]
fn wakes_a_stopped_person() {
    let mut t = T::new();
    t.host.add_seat("founder", "Founder", A, SeatView { at_prompt: true, ..SeatView::default() });
    t.give("do it").unwrap();
    assert_eq!(t.host.restarts, ["founder"]);
    assert!(t.e.said[0].2);
}

#[test]
fn does_not_boot_a_seat_that_is_already_booting() {
    let mut t = T::new();
    t.host.add_seat("founder", "Founder", A, SeatView { spawning: true, ..SeatView::default() });
    t.give("do it").unwrap();
    assert!(t.host.restarts.is_empty());
    assert!(t.e.said[0].2);
}

// where the Founder sits

#[test]
fn the_founder_sits_in_the_active_project() {
    let mut t = T::new();
    t.host.active = Some(B.into());
    t.give("do it").unwrap();
    assert_eq!(t.host.dir_of("founder"), Some(B));
}

#[test]
fn the_founder_takes_the_first_project_with_a_folder_when_the_active_one_has_none() {
    let mut t = T::new();
    t.host.projects.push(Project { name: "Project 1".into(), dir: None });
    t.host.active = Some("Project 1".into());
    t.give("do it").unwrap();
    assert_eq!(t.host.dir_of("founder"), Some(A));
}

#[test]
fn the_founder_takes_the_first_project_with_a_folder_from_home() {
    let mut t = T::new();
    t.give("do it").unwrap();
    assert_eq!(t.host.dir_of("founder"), Some(A));
}

#[test]
fn refuses_a_job_when_nothing_is_open_or_no_project_has_a_folder() {
    for loose in [false, true] {
        let mut t = T::new();
        t.host.projects.clear();
        if loose {
            t.host.projects.push(Project { name: "Project 1".into(), dir: None });
        }
        assert_eq!(t.give("do it").unwrap_err(), "Open a project first.");
        assert!(t.jobs().is_empty());
        assert!(t.host.spawned.is_empty());
    }
}

#[test]
fn the_founder_stays_where_it_is_for_later_jobs_whichever_project_is_active() {
    let mut t = T::new();
    t.host.active = Some(B.into());
    t.give("one").unwrap();
    t.host.active = Some(A.into());
    t.give("two").unwrap();
    assert_eq!(t.host.dir_of("founder"), Some(B));
    assert_eq!(t.host.spawned, ["Founder"]);
}

#[test]
fn the_founder_starts_blank_in_the_active_project_when_its_project_closed_without_calling_it_lost() {
    let mut t = T::new();
    t.host.active = Some(B.into());
    t.give("one").unwrap();
    t.e.state_mut().company.people[0].session = Some("sess-b".into());
    t.host.close("founder");
    t.host.active = Some(A.into());
    t.give("two").unwrap();
    assert_eq!(t.host.dir_of("founder"), Some(A));
    assert_eq!(t.host.seat("founder").unwrap().session, None);
    assert!(!t.alert_keys().contains(&"lost-founder".to_string()));
}

#[test]
fn the_founder_carries_on_its_conversation_when_it_comes_back_to_the_same_folder() {
    let mut t = T::new();
    t.give("one").unwrap();
    t.e.state_mut().company.people[0].session = Some("sess-a".into());
    t.host.close("founder");
    t.host.sessions.insert((A.into(), "sess-a".into()));
    t.give("two").unwrap();
    assert_eq!(t.host.seat("founder").unwrap().session.as_deref(), Some("sess-a"));
}

#[test]
fn the_founders_project_is_not_a_floor_with_people() {
    let mut t = T::new();
    t.give("do it").unwrap();
    assert!(!t.e.is_company_floor(A));
    let f = t.person(|p| p.id == "founder");
    assert_eq!((f.rank, f.floor, f.boss), (Rank::Founder, None, None));
}

#[test]
fn the_founder_starts_in_a_project_when_a_message_reaches_it_with_no_seat() {
    let mut t = T::new();
    t.give("do it").unwrap();
    t.host.close("founder");
    t.staff_floors(&plan_for("b", 0));
    t.message("r-b", "Founder", "done");
    assert_eq!(t.host.dir_of("founder"), Some(A));
    assert!(t.said().last().unwrap().contains("done"));
}

#[test]
fn raises_an_alert_for_a_founder_message_with_no_project_open_and_keeps_reading() {
    let mut t = T::new();
    t.give("do it").unwrap();
    t.staff_floors(&plan_for("b", 0));
    t.host.close("founder");
    let open = std::mem::take(&mut t.host.projects);
    t.message("r-b", "Founder", "done");
    assert!(t.alert_keys().contains(&"founder-no-project".to_string()));
    t.host.projects = open;
    t.message("r-b", "Founder", "again");
    assert_eq!(t.host.dir_of("founder"), Some(A));
    assert!(!t.alert_keys().contains(&"founder-no-project".to_string()));
}

// the owner talking to a person, and waking one

#[test]
fn types_the_owners_message_into_their_terminal_marked_as_the_owners() {
    let mut t = T::new();
    let m = approved(&mut t);
    t.advance(9000); // past the boot wait
    t.host.typed.clear();
    t.e.message_person(&mut t.host, &m, "Please\u{1b}[2J check the login page");
    assert_eq!(t.host.typed.last().unwrap(), &(m, "[From the owner] Please[2J check the login page".to_string()));
}

#[test]
fn sends_nothing_for_an_empty_message() {
    let mut t = T::new();
    t.e.message_person(&mut t.host, "founder", "   ");
    assert!(t.host.typed.is_empty() && t.e.said.is_empty());
}

#[test]
fn wakes_someone_with_no_seat() {
    let mut t = T::new();
    let m = approved(&mut t);
    t.host.close(&m);
    t.host.spawned.clear();
    t.e.wake_person(&mut t.host, &m).unwrap();
    assert_eq!(t.host.spawned.len(), 1);
}
