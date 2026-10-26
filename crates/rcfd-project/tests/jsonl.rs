//! ADR 0122 J1: torn-line tolerance of the JSONL reader and repair by the appender.

mod common;

use rcfd_project::jsonl::{self, Appender, TornTail};
use rcfd_project::ProjectError;
use serde_json::{json, Value};
use std::io::Write;

#[test]
fn j1_torn_last_line_is_ignored_and_reported() {
    let dir = common::scratch("j1");
    let p = dir.join("events.jsonl");
    let committed = b"{\"a\":1}\n{\"a\":2}\r\n\n{\"a\":3}\n";
    let torn = b"{\"a\":4,\"b\":\"hal";
    std::fs::write(&p, [&committed[..], &torn[..]].concat()).unwrap();
    let l = jsonl::read::<Value>(&p).unwrap();
    assert_eq!(l.items, [json!({"a":1}), json!({"a":2}), json!({"a":3})]);
    assert_eq!(
        l.torn,
        Some(TornTail {
            offset: committed.len() as u64,
            len: torn.len() as u64
        })
    );
    // A last line that parses but has no newline is still uncommitted.
    std::fs::write(&p, b"{\"a\":1}\n{\"a\":2}").unwrap();
    let l = jsonl::read::<Value>(&p).unwrap();
    assert_eq!((l.items.len(), l.torn.map(|t| t.len)), (1, Some(7)));
    // Missing file reads as empty.
    assert!(jsonl::read::<Value>(&dir.join("none.jsonl")).unwrap().items.is_empty());
}

#[test]
fn j1_negative_control_strict_reader_fails_on_the_same_fixture() {
    let bytes = b"{\"a\":1}\n{\"a\":4,\"b\":\"hal";
    let strict: Result<Vec<Value>, _> = bytes
        .split(|&b| b == b'\n')
        .map(serde_json::from_slice::<Value>)
        .collect();
    assert!(strict.is_err());
}

#[test]
fn j1_appender_repairs_a_torn_tail_and_corrupt_middle_lines_are_located() {
    let dir = common::scratch("j1rep");
    let p = dir.join("residuals.jsonl");
    std::fs::write(&p, b"{\"a\":1}\n{\"a\":2,\"tor").unwrap();
    let (mut a, removed) = Appender::open(&p).unwrap();
    assert_eq!(removed, 11);
    a.append(&json!({"a": 3})).unwrap();
    a.sync().unwrap();
    drop(a);
    let l = jsonl::read::<Value>(&p).unwrap();
    assert_eq!((l.items, l.torn), (vec![json!({"a":1}), json!({"a":3})], None));

    // Negative control: appending without the repair glues the next record onto the fragment, and the
    // reader then reports a located error at that line.
    std::fs::write(&p, b"{\"a\":1}\n{\"a\":2,\"tor").unwrap();
    std::fs::OpenOptions::new()
        .append(true)
        .open(&p)
        .unwrap()
        .write_all(b"{\"a\":3}\n")
        .unwrap();
    match jsonl::read::<Value>(&p) {
        Err(ProjectError::Json {
            path, line: Some(2), ..
        }) => assert_eq!(path, p),
        other => panic!("expected a located error at line 2, got {other:?}"),
    }
}
