// Stuck detection: repeated failures, stalls, and the watch that times them.

use supervise::stuck::{loop_of, stall_steps, DiffPrint, StuckKind, StuckSnap, StuckWatch, LOOP_COUNT, LOOP_WINDOW, STALL_MS, STALL_STEPS};
use supervise::task_state::TaskState;
use supervise::types::{ChatItem, StepItem, UserItem};

static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn next() -> u32 {
    N.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1
}

fn base_step() -> StepItem {
    let n = next();
    StepItem {
        id: format!("s{n}"),
        tool: "Bash".to_string(),
        verb: "Ran".to_string(),
        target: "npm test".to_string(),
        full: None,
        code: false,
        output: None,
        error: false,
        done: true,
        diff: None,
        added: None,
        removed: None,
        todos: None,
        images: None,
        questions: None,
        at: 1000 + (n as u64) * 1000,
    }
}

fn user(text: &str) -> ChatItem {
    let n = next();
    ChatItem::User(UserItem { id: format!("u{n}"), text: text.to_string(), images: 0, pics: None, at: 500 })
}

fn fail(output: &str) -> ChatItem {
    let mut s = base_step();
    s.error = true;
    s.output = Some(output.to_string());
    ChatItem::Step(s)
}

#[test]
fn flags_same_failures_in_the_recent_window() {
    let items = vec![user("fix it"), fail("FAIL src/app.test.ts"), fail("FAIL src/app.test.ts"), fail("FAIL src/app.test.ts")];
    let found = loop_of(&items).unwrap();
    assert_eq!(found.count, LOOP_COUNT);
    assert_eq!(found.sample, "FAIL src/app.test.ts");
}

#[test]
fn ignores_different_failures() {
    let items = vec![user("fix it"), fail("FAIL a"), fail("FAIL b"), fail("FAIL c")];
    assert_eq!(loop_of(&items), None);
}

#[test]
fn ignores_repeats_that_did_not_fail() {
    let mut items = vec![user("fix it")];
    items.extend((0..5).map(|_| ChatItem::Step(base_step())));
    assert_eq!(loop_of(&items), None);
}

#[test]
fn needs_the_full_count() {
    let items = vec![user("fix it"), fail("FAIL x"), fail("FAIL x")];
    assert_eq!(loop_of(&items), None);
}

#[test]
fn restarts_counting_after_you_speak() {
    let items = vec![fail("FAIL x"), fail("FAIL x"), fail("FAIL x"), user("try another way"), fail("FAIL x")];
    assert_eq!(loop_of(&items), None);
}

#[test]
fn counts_only_finished_steps() {
    let mut open = base_step();
    open.error = true;
    open.output = Some("FAIL x".to_string());
    open.done = false;
    let items = vec![user("fix it"), fail("FAIL x"), fail("FAIL x"), ChatItem::Step(open)];
    assert_eq!(loop_of(&items), None);
}

#[test]
fn normalizes_numbers_in_the_error_line() {
    let items = vec![user("fix it"), fail("failed after 3 attempts"), fail("failed after 4 attempts"), fail("failed after 5 attempts")];
    assert_eq!(loop_of(&items).map(|l| l.count), Some(3));
}

#[test]
fn looks_only_at_the_recent_window() {
    let mut items = vec![user("fix it"), fail("FAIL old"), fail("FAIL old"), fail("FAIL old")];
    for i in 0..LOOP_WINDOW {
        let mut s = base_step();
        s.target = format!("cmd-{i}");
        items.push(ChatItem::Step(s));
    }
    assert_eq!(loop_of(&items), None);
}

fn reads(k: usize) -> Vec<ChatItem> {
    (0..k)
        .map(|i| {
            let mut s = base_step();
            s.tool = "Read".to_string();
            s.verb = "Read".to_string();
            s.target = format!("file-{i}.ts");
            ChatItem::Step(s)
        })
        .collect()
}

#[test]
fn keeps_finished_steps_that_touched_no_files() {
    let mut items = vec![user("fix it")];
    items.extend(reads(STALL_STEPS));
    assert_eq!(stall_steps(&items).map(|s| s.len()), Some(STALL_STEPS));
}

#[test]
fn an_open_step_means_running_not_stalled() {
    let mut items = vec![user("fix it")];
    items.extend(reads(STALL_STEPS));
    let mut open = base_step();
    open.done = false;
    items.push(ChatItem::Step(open));
    assert_eq!(stall_steps(&items), None);
}

#[test]
fn a_step_that_changed_a_file_breaks_the_stall() {
    let mut items = vec![user("fix it")];
    items.extend(reads(STALL_STEPS - 1));
    let mut edited = base_step();
    edited.verb = "Edited".to_string();
    edited.target = "app.ts".to_string();
    edited.added = Some(3);
    edited.removed = Some(1);
    items.push(ChatItem::Step(edited));
    assert_eq!(stall_steps(&items), None);
}

#[test]
fn an_edit_verb_counts_without_counts() {
    let mut items = vec![user("fix it")];
    items.extend(reads(STALL_STEPS - 1));
    let mut wrote = base_step();
    wrote.verb = "Wrote".to_string();
    wrote.target = "new.ts".to_string();
    items.push(ChatItem::Step(wrote));
    assert_eq!(stall_steps(&items), None);
}

#[test]
fn needs_the_full_run_after_you_speak() {
    let mut items = reads(STALL_STEPS);
    items.push(user("go on"));
    items.extend(reads(2));
    assert_eq!(stall_steps(&items), None);
}

fn snap(items: Vec<ChatItem>, now: u64) -> StuckSnap {
    StuckSnap { run: 1, state: TaskState::Working, items, diff: Some(DiffPrint { n: 0, add: 0, del: 0 }), now }
}

fn looped() -> Vec<ChatItem> {
    vec![user("fix it"), fail("FAIL x"), fail("FAIL x"), fail("FAIL x")]
}

fn stalled() -> Vec<ChatItem> {
    let mut items = vec![user("fix it")];
    items.extend((0..STALL_STEPS).map(|i| {
        let mut s = base_step();
        s.tool = "Read".to_string();
        s.verb = "Read".to_string();
        s.target = format!("f-{i}");
        ChatItem::Step(s)
    }));
    items
}

#[test]
fn reports_a_loop_with_evidence() {
    let mut w = StuckWatch::new();
    let e = w.step("a", &snap(looped(), 0)).unwrap();
    assert_eq!(e.kind, StuckKind::Loop);
    assert_eq!(e.count, 3);
    assert!(e.text.contains("3 times"));
    assert!(e.text.contains("FAIL x"));
}

#[test]
fn stays_quiet_when_not_working() {
    let mut w = StuckWatch::new();
    let mut s = snap(looped(), 0);
    s.state = TaskState::Idle;
    assert_eq!(w.step("a", &s), None);
}

#[test]
fn reports_a_stall_after_the_diff_sits_still() {
    let mut w = StuckWatch::new();
    assert_eq!(w.step("a", &snap(stalled(), 0)), None);
    let e = w.step("a", &snap(stalled(), STALL_MS + 60_000)).unwrap();
    assert_eq!(e.kind, StuckKind::Stall);
    assert_eq!(e.count, STALL_STEPS);
    assert!(e.text.contains("11m"));
}

#[test]
fn a_moving_diff_resets_the_stall_clock() {
    let mut w = StuckWatch::new();
    w.step("a", &snap(stalled(), 0));
    let mut moved = snap(stalled(), STALL_MS + 60_000);
    moved.diff = Some(DiffPrint { n: 1, add: 5, del: 0 });
    w.step("a", &moved);
    let mut later = snap(stalled(), STALL_MS + 120_000);
    later.diff = Some(DiffPrint { n: 1, add: 5, del: 0 });
    assert_eq!(w.step("a", &later), None);
}

#[test]
fn an_unknown_diff_cannot_stall() {
    let mut w = StuckWatch::new();
    let mut s = snap(stalled(), 0);
    s.diff = None;
    w.step("a", &s);
    let mut later = snap(stalled(), STALL_MS + 60_000);
    later.diff = None;
    assert_eq!(w.step("a", &later), None);
}

#[test]
fn a_restart_resets_the_stall_clock() {
    let mut w = StuckWatch::new();
    w.step("a", &snap(stalled(), 0));
    let mut s = snap(stalled(), STALL_MS + 60_000);
    s.run = 2;
    assert_eq!(w.step("a", &s), None);
}

#[test]
fn sees_nothing_without_a_conversation() {
    let mut w = StuckWatch::new();
    assert_eq!(w.step("a", &snap(vec![], STALL_MS + 60_000)), None);
}
