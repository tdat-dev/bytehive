use super::*;
use crate::company::types::CompanyRequest;
use std::fs;

fn file() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("requests.jsonl");
    (dir, path)
}

fn append(path: &std::path::Path, text: &str) {
    let old = fs::read_to_string(path).unwrap_or_default();
    fs::write(path, format!("{old}{text}")).unwrap();
}

// drainJsonl

#[test]
fn starts_at_0_on_a_file_that_is_not_there_yet() {
    let (_dir, path) = file();
    let mut d = Drain::default();
    assert!(d.lines(&path).is_empty());
    append(&path, "{\"a\":1}\n");
    assert_eq!(d.lines(&path), ["{\"a\":1}"]);
}

#[test]
fn skips_the_backlog_on_first_sight() {
    let (_dir, path) = file();
    append(&path, "old\nolder\nhal");
    let mut d = Drain::default();
    assert!(d.lines(&path).is_empty());
    append(&path, "f\nnew\n");
    assert_eq!(d.lines(&path), ["half", "new"]);
}

#[test]
fn keeps_a_line_still_being_written_for_the_next_read() {
    let (_dir, path) = file();
    let mut d = Drain::default();
    d.lines(&path);
    append(&path, "{\"a\":1}\n{\"b\":");
    assert_eq!(d.lines(&path), ["{\"a\":1}"]);
    append(&path, "2}\n");
    assert_eq!(d.lines(&path), ["{\"b\":2}"]);
    assert!(d.lines(&path).is_empty());
}

#[test]
fn skips_blank_lines() {
    let (_dir, path) = file();
    let mut d = Drain::default();
    d.lines(&path);
    append(&path, "\n  \r\none\r\n");
    assert_eq!(d.lines(&path), ["one"]);
}

#[test]
fn starts_over_on_a_file_cut_shorter() {
    let (_dir, path) = file();
    let mut d = Drain::default();
    d.lines(&path);
    append(&path, "a long first line\n");
    d.lines(&path);
    fs::write(&path, "new\n").unwrap();
    assert_eq!(d.lines(&path), ["new"]);
}

#[test]
fn empties_a_big_file_once_it_is_all_read() {
    let (_dir, path) = file();
    let mut d = Drain::default();
    d.lines(&path);
    let line = format!("{}\n", "x".repeat(1023));
    append(&path, &line.repeat(1100));
    assert_eq!(d.lines(&path).len(), 1100);
    assert_eq!(fs::metadata(&path).unwrap().len(), 0);
    append(&path, "after\n");
    assert_eq!(d.lines(&path), ["after"]);
}

#[test]
fn reads_a_file_bigger_than_one_slice_in_one_go() {
    let (_dir, path) = file();
    let mut d = Drain::default();
    d.lines(&path);
    let line = format!("{}\n", "y".repeat(99));
    append(&path, &line.repeat(30_000));
    assert_eq!(d.lines(&path).len(), 30_000);
}

#[test]
fn forgets_where_it_was() {
    let (_dir, path) = file();
    let mut d = Drain::default();
    d.lines(&path);
    append(&path, "one\n");
    d.reset();
    assert!(d.lines(&path).is_empty());
    append(&path, "two\n");
    assert_eq!(d.lines(&path), ["two"]);
}

// A line that is not a request

#[test]
fn reads_a_request() {
    let r = parse(r#"{"type":"message","ts":1,"from":"founder","to":"Ana","message":"hi"}"#).unwrap();
    assert!(matches!(r, CompanyRequest::Message { ref to, .. } if to.as_deref() == Some("Ana")));
}

#[test]
fn names_the_sender_of_a_line_it_could_not_read() {
    let bad = parse(r#"{"type":"plan","ts":1,"from":"founder","job":"J-0001"}"#).unwrap_err();
    assert_eq!(bad.from.as_deref(), Some("founder"));
    assert!(bad.why.contains("plan"), "{}", bad.why);
    assert_eq!(parse(r#"{"type":"wave","from":"r-a"}"#).unwrap_err().from.as_deref(), Some("r-a"));
}

#[test]
fn has_nobody_to_tell_when_the_line_names_no_sender() {
    for line in ["{ half", "null", "[]", r#"{"type":"message","from":7}"#, r#"{"from":"  "}"#] {
        assert_eq!(parse(line).unwrap_err().from, None, "{line}");
    }
}

#[test]
fn tells_the_sender_what_was_not_understood() {
    let bad = parse(r#"{"type":"plan","ts":1,"from":"founder","job":"J-0001"}"#).unwrap_err();
    let text = not_understood(&bad);
    assert!(text.starts_with("Your request was not understood"), "{text}");
    assert!(text.contains(&bad.why));
}
