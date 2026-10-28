//! ADR 0130 A2: every event kind the run service writes, plus the three kinds of this crate, in both tag forms
//! (`event`, written by the 1248a76 run service, and `kind`), parse; re-serialised lines parse to equal values.

use rcfd_project::records::EventKind;
use rcfd_project::{jsonl, RunEvent, RunState};
use serde::Deserialize;
use serde_json::{json, Value};

/// `EventKind` as it was before ADR 0130: closed, tagged by `kind`.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[allow(dead_code)]
enum OldKind {
    State { from: Option<RunState>, to: RunState },
    Warning { message: String },
    Stop { reason: String },
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OldEvent {
    t: String,
    #[serde(flatten)]
    kind: OldKind,
}

/// The eight run-service kinds other than `state`, with the fields `runner.rs` writes (1248a76 lines 364-677).
fn runner_bodies() -> Vec<(&'static str, Value)> {
    vec![
        (
            "launched",
            json!({"pid": 4242, "launch_seq": 1, "threads": 1, "mem_mb": 65}),
        ),
        (
            "adopted",
            json!({"pid": 4242, "note": "solver found by a new runner; probed for liveness"}),
        ),
        (
            "estimate",
            json!({"cells": 144, "mem_mb": 65.0, "rule": "64 MB + 4 KB per cell"}),
        ),
        (
            "solver",
            json!({"solver_seq": 3, "solver": {"event": "finish", "kind": "x", "exit_code": 5, "time": 0.0}}),
        ),
        ("cancel_requested", json!({})),
        ("killed", json!({"pid": 4242, "why": "cancel grace elapsed"})),
        (
            "runner_shutdown",
            json!({"pid": 4242, "note": "the solver keeps running; the next runner adopts it"}),
        ),
        ("skipped_line", json!({"offset": 4194304, "bytes": 4194304})),
    ]
}

fn line(tag_fields: &[(&str, &str)], body: &Value) -> Value {
    let mut v = body.clone();
    v["t"] = json!("2026-10-28T00:00:00.000000000Z");
    for (k, name) in tag_fields {
        v[*k] = json!(name);
    }
    v
}

fn parse(v: &Value) -> RunEvent {
    serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{v}: {e}"))
}

/// Re-serialise and parse again: the same value.
fn round_trips(ev: &RunEvent) {
    let text = serde_json::to_string(ev).unwrap();
    let back: RunEvent = serde_json::from_str(&text).unwrap();
    assert_eq!(&back, ev, "{text}");
}

#[test]
fn a2_every_event_kind_parses_in_both_tag_forms() {
    let mut lines = Vec::new();
    let mut count = 0;
    // the eight other run-service kinds: legacy (`event`), new run service (`kind` and `event`), `kind` only
    for (name, body) in runner_bodies() {
        for tags in [
            vec![("event", name)],
            vec![("kind", name), ("event", name)],
            vec![("kind", name)],
        ] {
            let v = line(&tags, &body);
            let ev = parse(&v);
            assert_eq!(
                ev.kind,
                EventKind::Other {
                    kind: name.into(),
                    body: body.clone()
                },
                "{v}"
            );
            round_trips(&ev);
            lines.push(v);
            count += 1;
        }
    }
    // run-service state lines: legacy form -> from None, reason kept; new form carries from
    let legacy = line(
        &[("event", "state")],
        &json!({"state": "running", "reason": "launched", "pid": 7}),
    );
    let ev = parse(&legacy);
    assert_eq!(
        ev.kind,
        EventKind::State {
            from: None,
            to: RunState::Running,
            reason: Some("launched".into())
        }
    );
    round_trips(&ev);
    let new = line(
        &[("kind", "state"), ("event", "state")],
        &json!({"from": "queued", "to": "running", "state": "running", "reason": "launched", "pid": 7}),
    );
    let ev = parse(&new);
    assert_eq!(
        ev.kind,
        EventKind::State {
            from: Some(RunState::Queued),
            to: RunState::Running,
            reason: Some("launched".into())
        }
    );
    round_trips(&ev);
    lines.extend([legacy, new]);
    count += 2;
    // the three kinds of this crate, in both tag forms
    for tag in ["kind", "event"] {
        let cases = [
            (
                json!({"from": "running", "to": "paused"}),
                "state",
                EventKind::State {
                    from: Some(RunState::Running),
                    to: RunState::Paused,
                    reason: None,
                },
            ),
            (
                json!({"message": "slow"}),
                "warning",
                EventKind::Warning { message: "slow".into() },
            ),
            (
                json!({"reason": "max steps"}),
                "stop",
                EventKind::Stop {
                    reason: "max steps".into(),
                },
            ),
        ];
        for (body, name, want) in cases {
            let v = line(&[(tag, name)], &body);
            let ev = parse(&v);
            assert_eq!(ev.kind, want, "{v}");
            round_trips(&ev);
            lines.push(v);
            count += 1;
        }
    }
    eprintln!("A2: {count} lines parsed and round-tripped");
    assert_eq!(count, 8 * 3 + 2 + 3 * 2);

    // the same lines through the JSONL reader
    let dir = std::env::temp_dir().join(format!("rcfd-project-a2-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("events.jsonl");
    let text: String = lines.iter().map(|v| format!("{v}\n")).collect();
    std::fs::write(&path, &text).unwrap();
    assert_eq!(jsonl::read::<RunEvent>(&path).unwrap().items.len(), count);

    // negative control: the closed pre-change reader fails on run-service lines
    let old_failures = lines
        .iter()
        .filter(|v| serde_json::from_value::<OldEvent>((*v).clone()).is_err())
        .count();
    eprintln!("A2 negative control: the closed reader refuses {old_failures} of {count} lines");
    assert!(old_failures >= 1);
    assert!(jsonl::read::<OldEvent>(&path).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
