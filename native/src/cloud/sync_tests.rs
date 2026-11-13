use super::*;
use crate::cloud::crypto::{derive, open};
use crate::transcript::Item;

fn keys() -> Keys {
    derive("pass", b"salt-salt-salt-1", 1000)
}

fn src(session: &str) -> Source {
    Source { cli: "claude".into(), session: session.into(), run_dir: "D:\\p".into(), agent: "Ana".into(), project: "bytehive".into(), started: 1 }
}

fn users(n: usize) -> Vec<Item> {
    (0..n).map(|i| Item::User { id: format!("c{i}"), text: format!("ask {i}") }).collect()
}

/// Runs a round and gives back what was put.
fn run(sources: &[Source], sent: &mut Sent, items: &dyn Fn(&Source) -> Option<Vec<Item>>) -> Vec<(String, Vec<usize>)> {
    let mut puts = Vec::new();
    round(&keys(), "m1", "DESK", sources, sent, 100, &mut |s| items(s), &mut |p| {
        puts.push((p.sid.clone(), p.segments.iter().map(|s| s.n).collect()));
        Ok(())
    })
    .unwrap();
    puts
}

#[test]
fn sends_a_new_session_whole_then_nothing_while_it_is_unchanged() {
    let mut sent = Sent::default();
    let first = run(&[src("s1")], &mut sent, &|_| Some(users(450)));
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].1, vec![0, 1, 2]);
    assert!(run(&[src("s1")], &mut sent, &|_| Some(users(450))).is_empty());
}

#[test]
fn sends_only_the_segment_that_grew() {
    let mut sent = Sent::default();
    run(&[src("s1")], &mut sent, &|_| Some(users(450)));
    let next = run(&[src("s1")], &mut sent, &|_| Some(users(451)));
    assert_eq!(next[0].1, vec![2]);
}

#[test]
fn skips_a_session_with_nothing_to_read() {
    let mut sent = Sent::default();
    assert!(run(&[src("s1")], &mut sent, &|_| None).is_empty());
    assert!(run(&[src("s1")], &mut sent, &|_| Some(vec![])).is_empty());
}

#[test]
fn seals_so_the_web_can_open_it_in_its_slot() {
    let k = keys();
    let mut got = None;
    round(&k, "m1", "DESK", &[src("s1")], &mut Sent::default(), 100, &mut |_| Some(users(2)), &mut |p| {
        got = Some((p.sid.clone(), p.meta.clone(), p.segments[0].sealed.clone()));
        Ok(())
    })
    .unwrap();
    let (id, meta, seg) = got.unwrap();
    let meta: serde_json::Value = serde_json::from_slice(&open(&k, &format!("{id}/meta"), &meta).unwrap()).unwrap();
    assert_eq!(meta["title"], "ask 0");
    assert_eq!(meta["machine"], "DESK");
    assert_eq!(meta["items"], 2);
    let seg: serde_json::Value = serde_json::from_slice(&open(&k, &format!("{id}/0"), &seg).unwrap()).unwrap();
    assert_eq!(seg[1]["text"], "ask 1");
}

#[test]
fn a_failed_put_is_not_counted_as_sent() {
    let mut sent = Sent::default();
    let r = round(&keys(), "m1", "DESK", &[src("s1")], &mut sent, 100, &mut |_| Some(users(3)), &mut |_| Err(Fail::Quota));
    assert_eq!(r, Err(Fail::Quota));
    assert_eq!(run(&[src("s1")], &mut sent, &|_| Some(users(3))).len(), 1, "sent again next round");
}

#[test]
fn sends_in_batches_of_twenty_segments() {
    let mut sent = Sent::default();
    let puts = run(&[src("s1")], &mut sent, &|_| Some(users(200 * 45)));
    assert_eq!(puts.iter().map(|p| p.1.len()).collect::<Vec<_>>(), vec![20, 20, 5]);
}
