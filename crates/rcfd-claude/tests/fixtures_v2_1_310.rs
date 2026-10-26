//! ADR 0126 checks P1 (fields from every observed fixture), P2 (mutated fixtures never panic and are reported as
//! unrecognised), P3 (tail and incremental reading) and P4 (stream parser throughput). Fixtures: Claude Code 2.1.310,
//! recorded 2026-10-26 and scrubbed (paths, account and environment data removed; structure kept).

use rcfd_claude::stream::{parse_line, StreamEvent};
use rcfd_claude::transcript::{parse_entry, read_new, tail, Role, TranscriptEntry};
use std::io::Write;
use std::path::PathBuf;

const VERSION: &str = "2.1.310";

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/v2.1.310")
        .join(name)
}

fn read(name: &str) -> String {
    std::fs::read_to_string(fixture(name)).unwrap()
}

fn events(name: &str) -> Vec<StreamEvent> {
    read(name).lines().flat_map(parse_line).collect()
}

fn text_of(ev: &[StreamEvent]) -> String {
    ev.iter()
        .filter_map(|e| {
            if let StreamEvent::TextDelta(t) = e {
                Some(t.as_str())
            } else {
                None
            }
        })
        .collect()
}

fn result_of(ev: &[StreamEvent]) -> (String, bool, String, Option<String>, Option<f64>, Vec<String>) {
    ev.iter()
        .find_map(|e| match e {
            StreamEvent::Result {
                session_id,
                is_error,
                subtype,
                text,
                cost_usd,
                denials,
            } => Some((
                session_id.clone(),
                *is_error,
                subtype.clone(),
                text.clone(),
                *cost_usd,
                denials.iter().map(|d| d.tool_name.clone()).collect(),
            )),
            _ => None,
        })
        .expect("result event")
}

fn init_of(ev: &[StreamEvent]) -> (String, Option<String>) {
    match &ev[0] {
        StreamEvent::Init {
            session_id,
            permission_mode,
            version,
            model,
        } => {
            assert_eq!(version.as_deref(), Some(VERSION), "fixtures pin the observed version");
            assert_eq!(model.as_deref(), Some("claude-haiku-5-5"));
            (session_id.clone(), permission_mode.clone())
        }
        e => panic!("first event is not init: {e:?}"),
    }
}

fn tools(ev: &[StreamEvent]) -> Vec<String> {
    ev.iter()
        .filter_map(|e| {
            if let StreamEvent::ToolCall { name, .. } = e {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect()
}

fn denied(ev: &[StreamEvent]) -> Vec<String> {
    ev.iter()
        .filter_map(|e| {
            if let StreamEvent::PermissionDenied { tool_name, .. } = e {
                Some(tool_name.clone())
            } else {
                None
            }
        })
        .collect()
}

const S1: &str = "00000000-0000-4000-8000-000000000001";
const S2: &str = "00000000-0000-4000-8000-000000000002";

#[test]
fn p1_stream_fields_from_every_observed_fixture() {
    let all = [
        "stream_text.jsonl",
        "stream_resume.jsonl",
        "stream_denied_default.jsonl",
        "stream_allowlist_dontask.jsonl",
        "stream_denied_dontask.jsonl",
    ];
    for f in all {
        let ev = events(f);
        let unrec: Vec<_> = ev
            .iter()
            .filter(|e| matches!(e, StreamEvent::Unrecognised { .. }))
            .collect();
        assert!(unrec.is_empty(), "{f}: {unrec:?}");
        let (sid, _) = init_of(&ev);
        let (rsid, is_error, subtype, text, cost, _) = result_of(&ev);
        assert_eq!(sid, rsid, "{f}: result keeps the session id");
        assert!(!is_error && subtype == "success", "{f}");
        assert!(cost.is_some_and(|c| c > 0.0 && c < 0.05), "{f}: cost {cost:?}");
        if f != "stream_denied_dontask.jsonl" {
            // call 5 was recorded without --include-partial-messages (no deltas); the driver always passes it
            assert_eq!(Some(text_of(&ev)), text, "{f}: deltas rebuild the result text");
        }
    }
    // call 1: trivial prompt; call 2: --resume keeps the id
    let e1 = events("stream_text.jsonl");
    assert_eq!(init_of(&e1), (S1.into(), Some("default".into())));
    assert_eq!(text_of(&e1), "pong");
    assert_eq!(init_of(&events("stream_resume.jsonl")).0, S1);
    // call 3: default mode, Bash redirection and Write both denied
    let e3 = events("stream_denied_default.jsonl");
    assert_eq!(tools(&e3), ["Bash", "Write"]);
    assert_eq!(denied(&e3), ["Bash", "Write"]);
    assert_eq!(result_of(&e3).5, ["Bash", "Write"]);
    // call 4: --session-id, dontAsk, allow-list Edit(./note.txt) Bash(echo hello): nothing denied (echo bye was
    // approved by the CLI's own read-only classification)
    let e4 = events("stream_allowlist_dontask.jsonl");
    assert_eq!(init_of(&e4), (S2.into(), Some("dontAsk".into())));
    assert_eq!(tools(&e4), ["Write", "Bash", "Bash"]);
    assert!(denied(&e4).is_empty() && result_of(&e4).5.is_empty());
    // call 5: resume S2 in dontAsk, Write other.txt not allowed
    let e5 = events("stream_denied_dontask.jsonl");
    assert_eq!(init_of(&e5), (S2.into(), Some("dontAsk".into())));
    assert_eq!(denied(&e5), ["Write"]);
    assert_eq!(result_of(&e5).5, ["Write"]);
}

fn entries(name: &str) -> Vec<TranscriptEntry> {
    read(name).lines().filter_map(|l| parse_entry(l.as_bytes())).collect()
}

fn summary(e: &[TranscriptEntry]) -> Vec<(Role, String, Vec<String>)> {
    e.iter()
        .map(|x| match x {
            TranscriptEntry::Message {
                role,
                text,
                tools,
                timestamp,
            } => {
                assert!(
                    timestamp.as_deref().is_some_and(|t| t.starts_with("2026-10-26T")),
                    "{x:?}"
                );
                (*role, text.chars().take(24).collect(), tools.clone())
            }
            u => panic!("unrecognised in an observed fixture: {u:?}"),
        })
        .collect()
}

#[test]
fn p1_transcript_fields_from_every_observed_fixture() {
    use Role::*;
    let s = |r: Role, t: &str, tools: &[&str]| {
        (
            r,
            t.to_string(),
            tools.iter().map(|x| x.to_string()).collect::<Vec<_>>(),
        )
    };
    let a = summary(&entries("transcript_resumed_denied.jsonl"));
    assert_eq!(
        a,
        vec![
            s(User, "Reply with exactly: pong", &[]),
            s(Assistant, "pong", &[]),
            s(User, "What single word did you", &[]),
            s(Assistant, "pong", &[]),
            s(User, "Use the Bash tool to run", &[]),
            s(Assistant, "", &["Bash"]),
            s(ToolResult, "Output redirection to 'C", &[]),
            s(Assistant, "", &["Write"]),
            s(ToolResult, "Claude requested permiss", &[]),
            s(Assistant, "Both were refused, so I ", &[]),
        ]
    );
    let b = summary(&entries("transcript_allowlist.jsonl"));
    assert_eq!(b.iter().filter(|x| x.0 == User).count(), 2, "{b:?}");
    assert_eq!(
        b.iter().flat_map(|x| x.2.clone()).collect::<Vec<_>>(),
        ["Write", "Bash", "Bash", "Write"]
    );
    assert_eq!(b.iter().filter(|x| x.0 == ToolResult).count(), 4);
}

/// Deterministic generator for the random mutations (registered seed 0x47E).
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
}

fn mutate_json(line: &str, f: impl Fn(&mut serde_json::Value)) -> String {
    let mut v: serde_json::Value = serde_json::from_str(line).unwrap();
    f(&mut v);
    v.to_string()
}

fn rename(v: &mut serde_json::Value, from: &str, to: &str) {
    if let Some(o) = v.as_object_mut() {
        if let Some(x) = o.remove(from) {
            o.insert(to.into(), x);
        }
    }
}

#[test]
fn p2_mutated_transcripts_never_panic_and_are_unrecognised() {
    let (mut targeted, mut random) = (0usize, 0usize);
    let mut rng = Lcg(0x47E);
    for f in ["transcript_resumed_denied.jsonl", "transcript_allowlist.jsonl"] {
        for line in read(f).lines() {
            if !matches!(parse_entry(line.as_bytes()), Some(TranscriptEntry::Message { .. })) {
                continue;
            }
            let muts: Vec<String> = vec![
                line[..line.len() / 2].to_string(),                         // M1 truncated
                mutate_json(line, |v| rename(v, "type", "kind")),           // M2 no type
                mutate_json(line, |v| v["message"] = 42.into()),            // M3 message retyped
                mutate_json(line, |v| v["message"]["content"] = 42.into()), // M4 content retyped
                mutate_json(line, |v| rename(v, "message", "msg")),         // M5 message renamed
                mutate_json(line, |v| v["type"] = "assistant_v2".into()),   // M6 new type
            ];
            for m in muts {
                match parse_entry(m.as_bytes()) {
                    Some(TranscriptEntry::Unrecognised { .. }) => targeted += 1,
                    other => panic!("not reported as unrecognised: {other:?} from {m}"),
                }
            }
            // M7: random byte flips and invalid UTF-8; only "no panic" is required
            for _ in 0..200 {
                let mut b = line.as_bytes().to_vec();
                for _ in 0..1 + rng.next() % 4 {
                    let i = (rng.next() as usize) % b.len();
                    b[i] = (rng.next() & 0xff) as u8;
                }
                let _ = parse_entry(&b);
                random += 1;
            }
        }
    }
    assert_eq!(targeted, 6 * 24, "registered: 24 message lines x 6 targeted mutations");
    assert_eq!(random, 200 * 24);
}

#[test]
fn p2_mutated_streams_never_panic_and_are_unrecognised() {
    let mut targeted = 0usize;
    let mut rng = Lcg(0x47E);
    for f in [
        "stream_text.jsonl",
        "stream_denied_default.jsonl",
        "stream_denied_dontask.jsonl",
    ] {
        for line in read(f).lines() {
            let ev = parse_line(line);
            let Some(e) = ev.first() else { continue };
            // the field each event depends on, removed
            let key: Option<fn(&mut serde_json::Value)> = match e {
                StreamEvent::Init { .. } | StreamEvent::Result { .. } => Some(|v| rename(v, "session_id", "sid")),
                StreamEvent::PermissionDenied { .. } => Some(|v| rename(v, "tool_name", "tool")),
                StreamEvent::TextDelta(_) => Some(|v| v["event"]["delta"]["text"] = serde_json::Value::Null),
                StreamEvent::ToolCall { .. } => Some(|v| v["message"]["content"] = "x".into()),
                _ => None,
            };
            let Some(key) = key else { continue };
            for m in [
                line[..line.len() / 2].to_string(),
                mutate_json(line, |v| rename(v, "type", "kind")),
                mutate_json(line, key),
            ] {
                let out = parse_line(&m);
                assert!(
                    out.len() == 1 && matches!(out[0], StreamEvent::Unrecognised { .. }),
                    "not reported as unrecognised: {out:?} from {m}"
                );
                targeted += 1;
            }
            for _ in 0..200 {
                let mut b = line.as_bytes().to_vec();
                let i = (rng.next() as usize) % b.len();
                b[i] = (rng.next() & 0xff) as u8;
                let _ = parse_line(&String::from_utf8_lossy(&b));
            }
        }
    }
    assert!(targeted >= 60, "too few targeted mutations: {targeted}");
}

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("rcfd-claude-{name}-{}", rcfd_claude::new_uuid()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn p3_tail_and_incremental_read() {
    let d = tmp("tail");
    let src = read("transcript_resumed_denied.jsonl");
    // a big file: 2 MB of skipped bookkeeping lines after the messages forces the window to grow from 256 KiB
    let pad = format!("{{\"type\":\"api-request\",\"pad\":\"{}\"}}\n", "x".repeat(1000));
    let big = d.join("big.jsonl");
    std::fs::write(&big, format!("{src}{}", pad.repeat(2000))).unwrap();
    let t = tail(&big, 3).unwrap();
    assert_eq!(t, entries("transcript_resumed_denied.jsonl")[7..].to_vec());
    assert_eq!(tail(&big, 1000).unwrap().len(), 10);
    // an unterminated last line (writer mid-line) is not returned; read_new picks it up once complete
    let live = d.join("live.jsonl");
    let lines: Vec<&str> = src.lines().collect();
    let last = lines.iter().rposition(|l| parse_entry(l.as_bytes()).is_some()).unwrap();
    let head = lines[..last].join("\n") + "\n";
    let (a, b) = lines[last].split_at(lines[last].len() / 2);
    std::fs::write(&live, format!("{head}{a}")).unwrap();
    let mut off = 0;
    assert_eq!(read_new(&live, &mut off).unwrap().len(), 9);
    assert_eq!(tail(&live, 100).unwrap().len(), 9);
    assert!(read_new(&live, &mut off).unwrap().is_empty());
    std::fs::OpenOptions::new()
        .append(true)
        .open(&live)
        .unwrap()
        .write_all(format!("{b}\n").as_bytes())
        .unwrap();
    let new = read_new(&live, &mut off).unwrap();
    assert_eq!(new, entries("transcript_resumed_denied.jsonl")[9..].to_vec());
    let _ = std::fs::remove_dir_all(d);
}

/// P4: registered stream of 1000 copies of the observed call-3 stream (68 lines each, about 30 MB).
#[test]
fn p4_stream_parser_throughput() {
    let one = read("stream_denied_default.jsonl");
    let big = one.repeat(1000);
    let t0 = std::time::Instant::now();
    let mut n = 0usize;
    for l in big.lines() {
        n += parse_line(l).len();
    }
    let dt = t0.elapsed().as_secs_f64();
    let mb_s = big.len() as f64 / 1e6 / dt;
    println!("P4: {} bytes, {n} events, {dt:.3} s, {mb_s:.1} MB/s", big.len());
    assert_eq!(n, events("stream_denied_default.jsonl").len() * 1000);
    if !cfg!(debug_assertions) {
        assert!(mb_s >= P4_MIN_MB_S, "P4 throughput {mb_s:.1} MB/s below {P4_MIN_MB_S}");
    }
}

/// Registered in ADR 0126 (release builds only).
const P4_MIN_MB_S: f64 = 10.0;
