//! ADR 0128 finding 5: the driver aborts a session whose permission mode it could not check before anything else
//! arrives. The streams are built in the test from the recorded v2.1.310 allow-list stream; the replay double
//! `fake-claude` prints them.

use rcfd_claude::stream::StreamEvent;
use rcfd_claude::{AllowList, Claude};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Duration;

fn recorded() -> Vec<String> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v2.1.310/stream_allowlist_dontask.jsonl");
    std::fs::read_to_string(p).unwrap().lines().map(String::from).collect()
}

fn is_init(l: &str) -> bool {
    serde_json::from_str::<Value>(l).is_ok_and(|v| v["type"] == "system" && v["subtype"] == "init")
}

/// Runs the fake CLI on `lines` and returns every event up to `Exited`.
fn run(tag: &str, lines: &[String]) -> Vec<StreamEvent> {
    let root =
        PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("rcfd-claude-{tag}-{}", rcfd_claude::new_uuid()));
    let project = root.join("my-proj");
    std::fs::create_dir_all(&project).unwrap();
    let stream = root.join("stream.jsonl");
    std::fs::write(&stream, lines.join("\n") + "\n").unwrap();
    std::fs::write(
        project.join(".fake-claude.json"),
        json!({"stream": stream.to_string_lossy()}).to_string(),
    )
    .unwrap();
    let claude = Claude::new(env!("CARGO_BIN_EXE_fake-claude"), root.join("home"));
    let chat = claude.new_session(&project, &AllowList::new(), "x").unwrap();
    let mut ev = Vec::new();
    loop {
        let e = chat
            .events
            .recv_timeout(Duration::from_secs(30))
            .expect("event within 30 s");
        let end = matches!(e, StreamEvent::Exited { .. });
        ev.push(e);
        if end {
            return ev;
        }
    }
}

fn aborted(ev: &[StreamEvent]) -> Option<&str> {
    ev.iter().find_map(|e| match e {
        StreamEvent::Aborted(m) => Some(m.as_str()),
        _ => None,
    })
}

fn tool_calls(ev: &[StreamEvent]) -> usize {
    ev.iter().filter(|e| matches!(e, StreamEvent::ToolCall { .. })).count()
}

/// Tool calls forwarded before the `Aborted` event (all of them when there is none).
fn tool_calls_before_abort(ev: &[StreamEvent]) -> usize {
    let end = ev
        .iter()
        .position(|e| matches!(e, StreamEvent::Aborted(_)))
        .unwrap_or(ev.len());
    tool_calls(&ev[..end])
}

#[test]
fn d8_init_without_session_id_is_aborted() {
    let lines: Vec<String> = recorded()
        .into_iter()
        .map(|l| {
            if !is_init(&l) {
                return l;
            }
            let mut v: Value = serde_json::from_str(&l).unwrap();
            v.as_object_mut().unwrap().remove("session_id");
            v.to_string()
        })
        .collect();
    let ev = run("d8", &lines);
    eprintln!(
        "D8: aborted {:?}, tool calls forwarded {} (before the abort {})",
        aborted(&ev),
        tool_calls(&ev),
        tool_calls_before_abort(&ev)
    );
    let msg = aborted(&ev).expect("no Aborted event");
    assert!(msg.contains("system/init") && msg.contains("session_id"), "{msg}");
    assert_eq!(tool_calls_before_abort(&ev), 0);
    assert!(matches!(ev.last(), Some(StreamEvent::Exited { .. })));
    // control: the recorded stream itself is not aborted and forwards its three tool calls
    let ok = run("d8ok", &recorded());
    assert!(aborted(&ok).is_none() && tool_calls(&ok) == 3, "{ok:?}");
}

#[test]
fn d9_tool_call_before_init_is_aborted() {
    let mut lines = recorded();
    let k = lines
        .iter()
        .position(|l| l.contains("\"tool_use\"") && l.contains("\"type\": \"assistant\""))
        .expect("a tool line");
    let tool = lines.remove(k);
    lines.insert(0, tool);
    let ev = run("d9", &lines);
    eprintln!(
        "D9: aborted {:?}, tool calls forwarded {} (before the abort {})",
        aborted(&ev),
        tool_calls(&ev),
        tool_calls_before_abort(&ev)
    );
    assert!(aborted(&ev).is_some(), "{ev:?}");
    assert_eq!(tool_calls_before_abort(&ev), 0);
    // the offending call itself is never forwarded: only the two later ones may follow the Aborted event
    assert!(tool_calls(&ev) <= 2, "{ev:?}");
    assert!(matches!(ev.last(), Some(StreamEvent::Exited { .. })));
}
