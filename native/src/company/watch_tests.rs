use super::*;
use crate::board::{Card, List};
use crate::company::alerts::Level;
use crate::company::types::{DEFAULT_LIMITS, Job, JobStatus, Limits};
use serde_json::Map;
use std::collections::HashMap;

const MIN: i64 = 60_000;

fn watched(quiet_since: Option<i64>, returns: u32) -> WatchedCard {
    WatchedCard { id: "c1".into(), title: "Login".into(), job: "J-1".into(), assignee_id: "d1".into(), quiet_since, returns, list: WatchList::Doing }
}

fn run(cards: &[WatchedCard], nudges: &HashMap<String, u32>, now: i64) -> WorkOut {
    watch_work(&WorkIn { cards, nudges, limits: &DEFAULT_LIMITS, boss_of: &|_| Some("l1".into()), now })
}

fn none() -> HashMap<String, u32> {
    HashMap::new()
}

// watchWork

#[test]
fn leaves_a_working_person_alone() {
    assert!(run(&[watched(None, 0)], &none(), 100 * MIN).delivers.is_empty());
}

#[test]
fn leaves_someone_quiet_for_under_10_minutes_alone() {
    assert!(run(&[watched(Some(91 * MIN), 0)], &none(), 100 * MIN).delivers.is_empty());
}

#[test]
fn nudges_the_boss_the_first_time() {
    let r = run(&[watched(Some(80 * MIN), 0)], &none(), 100 * MIN);
    assert_eq!(r.delivers.len(), 1);
    assert_eq!(r.delivers[0].person_id, "l1");
    assert_eq!(r.delivers[0].text, "\"Login\" has gone quiet for 10 minutes. Check on it.");
    assert!(r.alerts.is_empty());
    assert_eq!(r.nudges.get("c1"), Some(&1));
}

#[test]
fn does_not_nudge_again_in_the_same_quiet_stretch_until_another_10_minutes_pass() {
    let first = run(&[watched(Some(85 * MIN), 0)], &none(), 100 * MIN);
    assert!(run(&[watched(Some(85 * MIN), 0)], &first.nudges, 101 * MIN).delivers.is_empty());
}

#[test]
fn asks_you_the_second_time() {
    let r = run(&[watched(Some(70 * MIN), 0)], &HashMap::from([("c1".into(), 1)]), 100 * MIN);
    let a = &r.alerts[0];
    assert_eq!((a.key.as_str(), a.job.as_deref(), a.person_id.as_deref(), a.level), ("stall-c1", Some("J-1"), Some("d1"), Level::Warn));
    assert_eq!(a.text, "\"Login\" is stuck.");
}

#[test]
fn asks_you_the_first_time_when_nobody_is_over_them() {
    let r = watch_work(&WorkIn { cards: &[watched(Some(80 * MIN), 0)], nudges: &none(), limits: &DEFAULT_LIMITS, boss_of: &|_| None, now: 100 * MIN });
    assert!(r.delivers.is_empty());
    assert_eq!(r.alerts[0].key, "stall-c1");
}

#[test]
fn forgets_the_nudges_of_a_card_that_started_moving() {
    assert!(!run(&[watched(None, 0)], &HashMap::from([("c1".into(), 1)]), 100 * MIN).nudges.contains_key("c1"));
}

#[test]
fn stops_a_card_returned_3_times() {
    let r = run(&[watched(None, 3)], &none(), 100 * MIN);
    assert_eq!(r.alerts[0].key, "returns-c1");
    assert_eq!(r.alerts[0].text, "\"Login\" went back 3 times and needs you.");
}

// watchBudget

fn spent(used: u64, cap: u64) -> Job {
    Job {
        id: "J-1".into(),
        title: "t".into(),
        ask: "a".into(),
        status: JobStatus::Running,
        plan: None,
        token_cap: cap,
        tokens_used: used,
        token_base: Default::default(),
        people: Vec::new(),
        summary: None,
        created_at: 1,
        extra: Map::new(),
    }
}

#[test]
fn is_quiet_under_80_percent() {
    assert_eq!(watch_budget(&spent(799, 1000)), Budget { alert: None, pause: false });
}

#[test]
fn warns_at_80_percent() {
    let b = watch_budget(&spent(800, 1000));
    assert!(!b.pause);
    let a = b.alert.unwrap();
    assert_eq!((a.key.as_str(), a.level, a.text.as_str()), ("cap-J-1", Level::Warn, "J-1 has used 80% of its tokens."));
}

#[test]
fn pauses_at_100_percent() {
    let b = watch_budget(&spent(1000, 1000));
    assert!(b.pause);
    let a = b.alert.unwrap();
    assert_eq!((a.level, a.text.as_str()), (Level::Stop, "J-1 reached its token cap and is paused."));
}

#[test]
fn ignores_a_job_that_is_not_running() {
    assert!(!watch_budget(&Job { status: JobStatus::Paused, ..spent(5000, 1000) }).pause);
}

#[test]
fn pauses_a_job_with_no_cap_once_it_spends_anything() {
    assert!(watch_budget(&spent(1, 0)).pause);
    assert_eq!(watch_budget(&spent(0, 0)), Budget { alert: None, pause: false });
}

// cardsToWatch

fn board_card(id: &str, job: Option<&str>, assignee: Option<&str>) -> Card {
    Card {
        id: id.into(),
        title: id.into(),
        desc: String::new(),
        labels: Vec::new(),
        due: None,
        checklist: Vec::new(),
        assignee: None,
        job: job.map(Into::into),
        assignee_id: assignee.map(Into::into),
        returns: None,
        stopped: false,
        done: None,
    }
}

fn c(id: &str) -> Card {
    board_card(id, Some("J1"), None)
}

fn by(id: &str, who: &str) -> Card {
    board_card(id, Some("J1"), Some(who))
}

fn board(todo: Vec<Card>, doing: Vec<Card>) -> Vec<List> {
    vec![List { id: "l1".into(), title: "To do".into(), cards: todo }, List { id: "l2".into(), title: "Doing".into(), cards: doing }]
}

fn jobs(status: JobStatus) -> Vec<Job> {
    vec![Job { id: "J1".into(), ..spent(0, 1) }].into_iter().map(|j| Job { status: status.clone(), ..j }).collect()
}

fn watch(lists: &[List], jobs: &[Job], contact: Option<&str>, working: bool, quiet: &mut HashMap<String, i64>, now: i64) -> Watched {
    cards_to_watch(&CardsIn { lists, jobs, contact, is_working: &|_| working, now }, quiet)
}

#[test]
fn watches_a_jobs_to_do_card_through_the_floors_contact() {
    let r = watch(&board(vec![c("c1")], vec![]), &jobs(JobStatus::Running), Some("lead"), false, &mut HashMap::new(), 100);
    let want = WatchedCard { id: "c1".into(), title: "c1".into(), job: "J1".into(), assignee_id: "lead".into(), quiet_since: Some(100), returns: 0, list: WatchList::Todo };
    assert_eq!(r.cards, [want]);
}

#[test]
fn uses_the_person_a_to_do_card_names() {
    let r = watch(&board(vec![by("c1", "ana")], vec![]), &jobs(JobStatus::Running), Some("lead"), false, &mut HashMap::new(), 100);
    assert_eq!(r.cards[0].assignee_id, "ana");
}

#[test]
fn leaves_to_do_alone_while_a_card_of_the_same_job_is_in_doing() {
    let mut quiet = HashMap::from([("todo:c1".to_string(), 5)]);
    let r = watch(&board(vec![c("c1")], vec![by("c2", "ana")]), &jobs(JobStatus::Running), Some("lead"), false, &mut quiet, 100);
    assert_eq!(r.cards.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["c2"]);
    assert!(!quiet.contains_key("todo:c1"));
}

#[test]
fn starts_a_cards_quiet_time_over_when_it_moves_to_doing() {
    let mut quiet = HashMap::from([("todo:c1".to_string(), 5)]);
    let r = watch(&board(vec![], vec![by("c1", "ana")]), &jobs(JobStatus::Running), Some("lead"), false, &mut quiet, 200);
    assert_eq!(r.cards[0].quiet_since, Some(200));
}

#[test]
fn clears_a_doing_cards_quiet_time_while_its_person_works() {
    let mut quiet = HashMap::from([("doing:c1".to_string(), 5)]);
    let r = watch(&board(vec![], vec![by("c1", "ana")]), &jobs(JobStatus::Running), Some("lead"), true, &mut quiet, 100);
    assert_eq!(r.cards[0].quiet_since, None);
}

#[test]
fn ignores_a_paused_job_cards_with_no_job_and_a_to_do_card_with_nobody_to_ask() {
    let mut quiet = HashMap::new();
    assert!(watch(&board(vec![c("c1")], vec![]), &jobs(JobStatus::Paused), Some("lead"), false, &mut quiet, 100).cards.is_empty());
    assert!(watch(&board(vec![board_card("c1", None, None)], vec![]), &jobs(JobStatus::Running), Some("lead"), false, &mut quiet, 100).cards.is_empty());
    assert!(watch(&board(vec![c("c1")], vec![]), &jobs(JobStatus::Running), None, false, &mut quiet, 100).cards.is_empty());
}

#[test]
fn names_every_card_it_read_for_clearing_old_quiet_times() {
    let r = watch(&board(vec![c("c1")], vec![c("c2")]), &jobs(JobStatus::Running), Some("lead"), false, &mut HashMap::new(), 100);
    assert_eq!(r.seen, ["doing:c2", "todo:c1"]);
}

#[test]
fn finds_the_lists_by_name_whatever_the_case_and_spaces() {
    let lists = vec![List { id: "l1".into(), title: "  to DO ".into(), cards: vec![c("c1")] }];
    assert_eq!(watch(&lists, &jobs(JobStatus::Running), Some("lead"), false, &mut HashMap::new(), 100).cards.len(), 1);
}

#[test]
fn counts_a_cards_returns_from_the_board() {
    let card = Card { returns: Some(2.0), ..by("c1", "ana") };
    let r = watch(&board(vec![], vec![card]), &jobs(JobStatus::Running), Some("lead"), false, &mut HashMap::new(), 100);
    assert_eq!(r.cards[0].returns, 2);
}

// watchWork on a To do card

#[test]
fn nudges_the_boss_in_its_own_words_then_raises_a_row() {
    let limits = Limits { stall_minutes: 10, max_returns: 3, ..DEFAULT_LIMITS };
    let todo = WatchedCard { id: "c1".into(), title: "Fix it".into(), job: "J1".into(), assignee_id: "lead".into(), quiet_since: Some(0), returns: 0, list: WatchList::Todo };
    let cards = [todo];
    let boss = |_: &str| Some("founder".to_string());
    let first = watch_work(&WorkIn { cards: &cards, nudges: &none(), limits: &limits, boss_of: &boss, now: 600_000 });
    assert_eq!(first.delivers, [Deliver { person_id: "founder".into(), text: "\"Fix it\" has waited in To do for 10 minutes. Hand it out or start it.".into() }]);
    let second = watch_work(&WorkIn { cards: &cards, nudges: &first.nudges, limits: &limits, boss_of: &boss, now: 1_200_000 });
    assert_eq!(second.alerts[0].text, "\"Fix it\" has not been picked up.");
}

#[test]
fn takes_an_empty_job_or_person_on_a_card_for_none() {
    let r = watch(&board(vec![board_card("c1", Some(""), None)], vec![by("c2", "")]), &jobs(JobStatus::Running), Some("lead"), false, &mut HashMap::new(), 100);
    assert!(r.cards.is_empty());
}

#[test]
fn takes_an_empty_boss_for_no_boss_and_asks_the_owner() {
    let r = watch_work(&WorkIn { cards: &[watched(Some(80 * MIN), 0)], nudges: &none(), limits: &DEFAULT_LIMITS, boss_of: &|_| Some(String::new()), now: 100 * MIN });
    assert!(r.delivers.is_empty());
    assert_eq!(r.alerts[0].key, "stall-c1");
}

#[test]
fn does_not_hand_a_to_do_card_with_an_empty_assignee_to_the_contact() {
    let r = watch(&board(vec![board_card("c1", Some("J1"), Some(""))], vec![]), &jobs(JobStatus::Running), Some("lead"), false, &mut HashMap::new(), 100);
    assert!(r.cards.is_empty());
}
