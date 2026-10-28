//! ADR 0134 (wave 48E), integration part: the panel checks E1, E2, E3, E4 (panel part), E6, E7, E10 and E11 with the
//! replay double `fake-claude` (never the real CLI). Set-ups: ADR 0134 Amendment 3.

mod chat_common;

use chat_common::*;
use rcfd_claude::allow::AllowRule;
use rcfd_claude::discover::{live_pids, OpenState};
use rcfd_claude::stream::parse_line;
use rcfd_claude::AllowList;
use rcfd_studio::chat::{Folder, ItemKind, TurnMsg, TurnState, Update};
use serde_json::{json, Value};
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

/// Registered limits (ADR 0134 table).
const CANCEL_LIMIT: Duration = Duration::from_secs(5);
const MIRROR_LIMIT: Duration = Duration::from_millis(2000);
const FRAME_LIMIT_MS: f64 = 8.0;

/// The D7 fixture home of ADR 0126: S2 open (this process), S1 closed (dead pid), S3 open without a transcript, an id
/// that is not a UUID, and S4's transcript of another folder whose directory name collides.
fn d7(name: &str, stream: Option<&str>) -> ChatSetup {
    let me = std::process::id();
    let dead = dead_pid();
    let s = ChatSetup::new(name, stream, Some(json!([])), json!({}));
    let p = s.project.clone();
    s.configure(
        stream,
        Some(listing(&[
            (S2, me, &p),
            (S1, dead, &p),
            (S3, me, &p),
            ("not-a-uuid", me, &p),
        ])),
        json!({}),
    );
    let dir = s.transcript_dir();
    std::fs::write(
        dir.join(format!("{S1}.jsonl")),
        retarget("transcript_resumed_denied.jsonl", &p),
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(50));
    std::fs::write(
        dir.join(format!("{S2}.jsonl")),
        retarget("transcript_allowlist.jsonl", &p),
    )
    .unwrap();
    let other = p.with_file_name("my_proj"); // "my-proj" and "my_proj" share the directory name
    std::fs::write(
        dir.join(format!("{S4}.jsonl")),
        retarget("transcript_allowlist.jsonl", &other),
    )
    .unwrap();
    s
}

fn rows(st: &rcfd_studio::Studio) -> Vec<(String, bool)> {
    st.chat
        .session_rows()
        .into_iter()
        .map(|r| (r.id, r.input_enabled))
        .collect()
}

#[test]
fn e1_session_list_open_closed_unknown() {
    let s = d7("e1", None);
    let mut st = s.studio();
    let me = std::process::id();
    let got = rows(&st);
    println!("E1 rows: {:?}", st.chat.session_rows());
    assert_eq!(
        got,
        vec![(S2.to_string(), false), (S1.to_string(), true), (S3.to_string(), false)]
    );
    assert_eq!(st.chat.sessions[0].state, OpenState::Open { pid: me });
    assert_eq!(st.chat.sessions[2].state, OpenState::Open { pid: me });
    assert!(st.chat.sessions[2].transcript.is_none());
    assert!(st.chat.warnings.is_empty(), "{:?}", st.chat.warnings);
    // negative control: S4's transcript is in the same directory, and it is not listed
    assert!(s.transcript_dir().join(format!("{S4}.jsonl")).is_file());
    assert!(!got.iter().any(|(id, _)| id == S4));
    // the panel shows the states
    let mut app = app(st);
    let ctx = egui::Context::default();
    let text = shown(&mut app, &ctx);
    assert!(text.contains(&format!("open in process {me} (mirror only)")), "{text}");
    assert!(text.contains("closed"), "{text}");
    st = app.st;
    // the listing fails: unknown, disabled, a warning
    s.configure(None, None, json!({}));
    let n = st.chat.discoveries;
    st.chat.refresh();
    assert!(st.drain_until(WAIT, |s| s.chat.discoveries > n));
    let got = rows(&st);
    println!(
        "E1 failing listing: {:?}; warnings {:?}",
        st.chat.session_rows(),
        st.chat.warnings
    );
    assert_eq!(got, vec![(S2.to_string(), false), (S1.to_string(), false)]);
    assert!(st.chat.sessions.iter().all(|x| x.state == OpenState::Unknown));
    assert!(
        st.chat.warnings.iter().any(|w| w.contains("agents")),
        "{:?}",
        st.chat.warnings
    );
    st.chat.select(S1).unwrap();
    assert!(st.chat.can_send().is_err());
    let mut app = app_of(st);
    let text = shown(&mut app, &ctx);
    assert!(text.contains("state unknown"), "{text}");
    assert!(text.contains("claude agents --json"), "{text}");
}

fn app_of(st: rcfd_studio::Studio) -> rcfd_studio::StudioApp {
    app(st)
}

/// The resume stream of E2's control and E3: the recorded dontAsk stream (session S2), its session id set to `id`.
fn dontask_stream(id: &str, project: &Path) -> String {
    retarget("stream_allowlist_dontask.jsonl", project).replace(S2, id)
}

#[test]
fn e2_typing_into_an_open_session_is_refused_without_a_process() {
    let s = d7("e2", None);
    s.configure(
        Some(&dontask_stream(S1, &s.project)),
        Some(serde_json::from_str(&std::fs::read_to_string(s.root.join("agents.json")).unwrap()).unwrap()),
        json!({}),
    );
    let mut st = s.studio();
    st.chat.select(S2).unwrap();
    let r = st.chat.send("hello");
    println!("E2: send to the open session: {r:?}");
    assert!(r.unwrap_err().contains("open in process"));
    assert!(st.chat.turn.is_none());
    std::thread::sleep(Duration::from_secs(1));
    st.drain();
    assert!(s.chat_argvs().is_empty(), "a chat process was started");
    let listings = s.calls().len();
    // negative control: the closed session S1 of the same fixture starts exactly one process
    st.chat.select(S1).unwrap();
    st.chat.send("hello").unwrap();
    let state = finish_turn(&mut st);
    println!(
        "E2 control: {state:?}; chat processes {}; calls {} -> {}",
        s.chat_argvs().len(),
        listings,
        s.calls().len()
    );
    assert_eq!(state, TurnState::Finished { code: Some(0) });
    assert_eq!(s.chat_argvs().len(), 1);
}

const E3_RESULT: &str = "All three steps succeeded: created `note.txt` containing `x`, `echo hello` printed `hello`, and `echo bye` printed `bye`.";

#[test]
fn e3_resume_a_closed_session() {
    let s = ChatSetup::new("e3", None, Some(json!([])), json!({}));
    s.configure(Some(&dontask_stream(S2, &s.project)), Some(json!([])), json!({}));
    std::fs::write(
        s.transcript_dir().join(format!("{S2}.jsonl")),
        retarget("transcript_allowlist.jsonl", &s.project),
    )
    .unwrap();
    let mut st = s.studio();
    assert_eq!(rows(&st), vec![(S2.to_string(), true)]);
    st.chat.select(S2).unwrap();
    st.chat.send("run the three steps").unwrap();
    let state = finish_turn(&mut st);
    let t = st.chat.turn.clone().unwrap();
    let argvs = s.chat_argvs();
    println!("E3: {state:?}; text {:?}; cost {:?}; argv {:?}", t.text, t.cost, argvs);
    assert_eq!(state, TurnState::Finished { code: Some(0) });
    assert_eq!(argvs.len(), 1);
    assert_eq!(argvs[0], t.args);
    assert_eq!(argvs[0][argvs[0].len() - 2..], ["--resume".to_string(), S2.to_string()]);
    let i = argvs[0].iter().position(|a| a == "--mcp-config").unwrap();
    assert_eq!(argvs[0][i - 1], "--strict-mcp-config");
    assert!(!Path::new(&argvs[0][i + 1]).starts_with(&s.project));
    assert_eq!(t.result.as_ref().unwrap().1.as_deref(), Some(E3_RESULT));
    assert_eq!(t.text, E3_RESULT);
    assert_eq!(t.cost, Some(0.00381852));
    assert_eq!(st.chat.session_cost(S2), Some(0.00381852));
    let mut app = app(st);
    let ctx = egui::Context::default();
    let text = shown(&mut app, &ctx);
    assert!(text.contains("cost $0.003819"), "{text}");
    assert!(text.contains("session total here $0.003819"), "{text}");
    assert!(text.contains("turn finished"), "{text}");
}

#[test]
fn e3_control_an_init_in_another_mode_is_shown_as_aborted() {
    let s = ChatSetup::new(
        "e3c",
        Some("stream_denied_default.jsonl"),
        Some(json!([])),
        json!({"hang": true, "pidfile": "pids.json"}),
    );
    let mut st = s.studio();
    st.chat.send("x").unwrap();
    let state = finish_turn(&mut st);
    println!("E3 control: {state:?}");
    assert!(
        matches!(state, TurnState::Aborted(ref w) if w.contains("dontAsk")),
        "{state:?}"
    );
    assert!(st
        .chat
        .feed
        .items()
        .iter()
        .any(|i| i.kind == ItemKind::Note && i.text.starts_with("-- aborted")));
    let (child, grandchild) = read_pids(&s.project);
    assert!(
        wait_for(CANCEL_LIMIT, || live_pids(&[child, grandchild]).unwrap().is_empty()).is_some(),
        "the aborted turn left a process"
    );
    let mut app = app(st);
    let ctx = egui::Context::default();
    let text = shown(&mut app, &ctx);
    assert!(
        text.contains("aborted: the CLI did not apply --permission-mode dontAsk"),
        "{text}"
    );
}

fn read_pids(project: &Path) -> (u32, u32) {
    let mut out = (0, 0);
    assert!(
        wait_for(WAIT, || {
            match std::fs::read_to_string(project.join("pids.json"))
                .ok()
                .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            {
                Some(v) => {
                    out = (
                        v["child"].as_u64().unwrap() as u32,
                        v["grandchild"].as_u64().unwrap() as u32,
                    );
                    true
                }
                None => false,
            }
        })
        .is_some(),
        "no pid file"
    );
    out
}

fn rules_in(args: &[String]) -> Vec<String> {
    let i = args.iter().position(|a| a == "--allowedTools").unwrap();
    args[i + 1..]
        .iter()
        .take_while(|a| !a.starts_with('-'))
        .cloned()
        .collect()
}

#[test]
fn e4_denial_is_shown_and_allow_grows_the_file_by_one_rule() {
    let s = ChatSetup::new("e4", Some("stream_denied_dontask.jsonl"), Some(json!([])), json!({}));
    let mut st = s.studio();
    assert!(!st.chat.allow_on_disk);
    st.chat.send("write other.txt").unwrap();
    finish_turn(&mut st);
    let t = st.chat.turn.clone().unwrap();
    assert_eq!(t.denials.len(), 1);
    let rule = t.denials[0].suggestion.clone();
    println!("E4 panel: denial {:?}", t.denials[0]);
    assert_eq!(rule, Some(AllowRule::Edit("other.txt".into())));
    assert!(t.denials[0].input.contains("other.txt"));
    let mut app = app(st);
    let ctx = egui::Context::default();
    let text = shown(&mut app, &ctx);
    assert!(text.contains("Edit(./other.txt)"), "{text}");
    assert!(text.contains("Allow\n"), "{text}");
    st = app.st;
    // "Allow" (the button's call)
    let file = AllowList::file_of(&s.project);
    assert!(!file.exists());
    st.chat.allow_rule(rule.unwrap()).unwrap();
    assert!(st.drain_until(WAIT, |s| s.chat.allow_on_disk
        && s.chat.allow.as_ref().is_some_and(|l| l
            .as_ref()
            .is_ok_and(|l| l.rules().len() == AllowList::project_initial().rules().len() + 1))
        && s.chat.folder == Folder::Confirmed));
    let on_disk = AllowList::from_file(&file).unwrap();
    let first = AllowList::project_initial().rules().len();
    println!("E4 panel: file rules {} -> {}", first, on_disk.rules().len());
    assert_eq!(on_disk.rules().len(), first + 1);
    assert_eq!(st.chat.folder, Folder::Confirmed);
    // the next message carries it, after the fixed permission flags
    st.chat.send("again").unwrap();
    finish_turn(&mut st);
    let argvs = s.chat_argvs();
    assert_eq!(argvs.len(), 2);
    let (a, b) = (rules_in(&argvs[0]), rules_in(&argvs[1]));
    println!("E4 panel: argv rules {} -> {}", a.len(), b.len());
    assert_eq!(b.len(), a.len() + 1);
    assert!(b.contains(&"Edit(./other.txt)".to_string()) && !a.contains(&"Edit(./other.txt)".to_string()));
    let w = argvs[1]
        .windows(4)
        .position(|w| w == ["--permission-mode", "dontAsk", "--permission-prompts", "none"])
        .unwrap();
    assert!(w < argvs[1].iter().position(|x| x == "--allowedTools").unwrap());
}

#[test]
fn e4_control_a_suggestion_for_a_control_file_shows_no_button() {
    let stream = std::fs::read_to_string(fixture("stream_denied_dontask.jsonl"))
        .unwrap()
        .replace("other.txt", ".mcp.json");
    let s = ChatSetup::new("e4c", None, Some(json!([])), json!({}));
    s.configure(Some(&stream), Some(json!([])), json!({}));
    let mut st = s.studio();
    st.chat.send("write .mcp.json").unwrap();
    finish_turn(&mut st);
    let t = st.chat.turn.clone().unwrap();
    println!("E4 control: {:?}", t.denials);
    assert_eq!(t.denials.len(), 1);
    assert_eq!(t.denials[0].suggestion, None);
    let mut app = app(st);
    let ctx = egui::Context::default();
    let text = shown(&mut app, &ctx);
    assert!(text.contains("no rule can allow this call"), "{text}");
    assert!(!text.contains("Allow\n"), "{text}");
}

#[test]
fn e6_cancel_from_the_panel_leaves_no_process() {
    let s = ChatSetup::new(
        "e6",
        Some("stream_allowlist_dontask.jsonl"),
        Some(json!([])),
        json!({"hang": true, "pidfile": "pids.json"}),
    );
    let mut st = s.studio();
    st.chat.send("a long turn").unwrap();
    let (child, grandchild) = read_pids(&s.project);
    assert!(st.drain_until(WAIT, |s| matches!(
        s.chat.turn.as_ref().map(|t| &t.state),
        Some(TurnState::Running { .. })
    )));
    assert!(matches!(st.chat.turn.as_ref().unwrap().state, TurnState::Running { pid } if pid == child));
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        live_pids(&[child, grandchild]).unwrap().len(),
        2,
        "both must run before cancel"
    );
    let t0 = Instant::now();
    st.chat.cancel(); // what the Cancel button calls
    let gone = wait_for(CANCEL_LIMIT, || live_pids(&[child, grandchild]).unwrap().is_empty());
    println!("E6: child {child} and grandchild {grandchild} gone after {gone:?}");
    assert!(gone.is_some(), "a process is left after {:?}", t0.elapsed());
    assert_eq!(finish_turn(&mut st), TurnState::Cancelled);
}

fn marker(n: u32) -> String {
    json!({"type": "user", "message": {"role": "user", "content": format!("E7 marker {n}")}, "timestamp": "2026-10-28T12:00:00.000Z"})
        .to_string()
}

fn count_marker(st: &rcfd_studio::Studio, n: u32) -> usize {
    let m = format!("you> E7 marker {n}");
    st.chat.feed.items().iter().filter(|i| i.text == m).count()
}

fn unrecognised(st: &rcfd_studio::Studio) -> usize {
    st.chat
        .feed
        .items()
        .iter()
        .filter(|i| i.kind == ItemKind::Unrecognised)
        .count()
}

fn e7_setup(name: &str, whole: bool) -> (ChatSetup, rcfd_studio::Studio, std::path::PathBuf) {
    let s = ChatSetup::new(name, None, Some(json!([])), json!({}));
    let p = s.project.clone();
    s.configure(None, Some(listing(&[(S2, std::process::id(), &p)])), json!({}));
    let tr = s.transcript_dir().join(format!("{S2}.jsonl"));
    std::fs::write(&tr, retarget("transcript_allowlist.jsonl", &p)).unwrap();
    let mut st = s.studio_with(|st| st.chat.switches.whole_file_mirror = whole);
    assert_eq!(rows(&st), vec![(S2.to_string(), false)]);
    st.chat.select(S2).unwrap();
    assert!(st.drain_until(WAIT, |s| !s.chat.feed.is_empty()));
    (s, st, tr)
}

fn append(path: &Path, text: &str) {
    let mut f = std::fs::OpenOptions::new().append(true).open(path).unwrap();
    f.write_all(text.as_bytes()).unwrap();
    f.flush().unwrap();
}

#[test]
fn e7_mirror_shows_appended_messages_and_a_partial_line_only_when_complete() {
    let (_s, mut st, tr) = e7_setup("e7", false);
    let before = st.chat.feed.len();
    assert_eq!(unrecognised(&st), 0);
    // a complete line
    let t0 = Instant::now();
    append(&tr, &format!("{}\n", marker(1)));
    assert!(st.drain_until(WAIT, |s| count_marker(s, 1) == 1));
    let dt1 = t0.elapsed();
    // a partial line: nothing new for 4 periods
    let m2 = marker(2);
    let (a, b) = m2.split_at(m2.len() / 2);
    append(&tr, a);
    let n = st.chat.feed.len();
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(2000) {
        st.drain();
        assert_eq!(st.chat.feed.len(), n, "the partial line was shown");
        std::thread::sleep(Duration::from_millis(20));
    }
    let t0 = Instant::now();
    append(&tr, &format!("{b}\n"));
    assert!(st.drain_until(WAIT, |s| count_marker(s, 2) == 1));
    let dt2 = t0.elapsed();
    std::thread::sleep(Duration::from_millis(1200));
    st.drain();
    println!(
        "E7: {before} items at first, marker 1 after {dt1:?}, marker 2 (completed) after {dt2:?}; items {}, unrecognised {}",
        st.chat.feed.len(),
        unrecognised(&st)
    );
    assert_eq!(count_marker(&st, 1), 1);
    assert_eq!(count_marker(&st, 2), 1);
    assert_eq!(unrecognised(&st), 0);
    assert!(dt1 <= MIRROR_LIMIT, "{dt1:?}");
    assert!(dt2 <= MIRROR_LIMIT, "{dt2:?}");
}

#[test]
fn e7_control_the_whole_file_reader_shows_the_partial_line() {
    let (_s, mut st, tr) = e7_setup("e7c", true);
    let m = marker(3);
    append(&tr, &m[..m.len() / 2]);
    let shown = wait_for(Duration::from_secs(5), || {
        st.drain();
        unrecognised(&st) > 0
    });
    println!("E7 control: partial line shown after {shown:?}");
    assert!(shown.is_some(), "the whole-file reader did not show the partial line");
}

/// The P4 stream of ADR 0126: 1000 copies of the recorded call-3 stream, parsed by `rcfd-claude`.
fn p4_events() -> Vec<rcfd_claude::StreamEvent> {
    let one = std::fs::read_to_string(fixture("stream_denied_default.jsonl")).unwrap();
    one.repeat(1000).lines().flat_map(parse_line).collect()
}

fn e10_measure(whole_text: bool) -> (f64, Vec<f64>, usize, usize) {
    let s = ChatSetup::new(
        if whole_text { "e10c" } else { "e10" },
        None,
        Some(json!([])),
        json!({}),
    );
    let st = s.studio_with(|st| st.chat.switches.whole_text = whole_text);
    let mut app = app(st);
    let ctx = egui::Context::default();
    let events = p4_events();
    let n = events.len();
    app.st.chat.apply(Update::Turn(TurnMsg::Started {
        session_id: S1.into(),
        pid: 1,
        args: Vec::new(),
    }));
    for _ in 0..5 {
        let _ = frame(&mut app, &ctx);
    }
    let per = n.div_ceil(100);
    let mut it = events.into_iter();
    let mut times = Vec::with_capacity(100);
    for _ in 0..100 {
        let t = Instant::now();
        for ev in it.by_ref().take(per) {
            app.st.chat.apply(Update::Turn(TurnMsg::Event(ev)));
        }
        let _ = frame(&mut app, &ctx);
        times.push(t.elapsed().as_secs_f64() * 1e3);
    }
    assert!(it.next().is_none());
    let mut sorted = times.clone();
    sorted.sort_by(f64::total_cmp);
    (
        (sorted[49] + sorted[50]) / 2.0,
        sorted,
        n,
        app.st.chat.feed.rows().len(),
    )
}

#[test]
fn e10_frame_build_while_a_turn_streams_the_p4_stream() {
    if cfg!(debug_assertions) {
        eprintln!("SKIP e10 (crates/rcfd-studio/tests/chat_e.rs): the limit is registered for release builds");
        return;
    }
    let (median, sorted, n, rows) = e10_measure(false);
    println!(
        "E10: {n} events over 100 frames; median {median:.3} ms, min {:.3}, p90 {:.3}, max {:.3}; {rows} feed rows",
        sorted[0], sorted[89], sorted[99]
    );
    assert!(n >= 30_000);
    assert!(rows > 1000);
    assert!(median <= FRAME_LIMIT_MS, "median {median:.3} ms");
}

#[test]
fn e10_control_the_whole_transcript_as_one_text_exceeds_8_ms() {
    if cfg!(debug_assertions) {
        eprintln!("SKIP e10 control (crates/rcfd-studio/tests/chat_e.rs): release builds only");
        return;
    }
    let (median, sorted, n, _) = e10_measure(true);
    println!(
        "E10 control: {n} events; median {median:.3} ms, min {:.3}, max {:.3}",
        sorted[0], sorted[99]
    );
    assert!(median > FRAME_LIMIT_MS, "median {median:.3} ms");
}

#[test]
fn e11_confirmation_dialog_gates_the_first_chat() {
    let s = ChatSetup::new(
        "e11",
        Some("stream_allowlist_dontask.jsonl"),
        Some(json!([])),
        json!({}),
    );
    std::fs::write(s.project.join(".mcp.json"), "{\"mcpServers\": {}}\n").unwrap();
    std::fs::create_dir_all(s.project.join(".claude")).unwrap();
    std::fs::write(
        s.project.join(".claude/settings.json"),
        "{\"hooks\": {\"SessionStart\": []}}\n",
    )
    .unwrap();
    let mut first = AllowList::project_initial();
    first.add(AllowRule::Bash("echo hi".into())).unwrap();
    first.to_file(&AllowList::file_of(&s.project)).unwrap();
    let mut st = s.studio();
    let Folder::Pending { pending, previews } = st.chat.folder.clone() else {
        panic!("not pending: {:?}", st.chat.folder)
    };
    let paths: Vec<&str> = pending.files.iter().map(|f| f.path.as_str()).collect();
    println!("E11: pending {paths:?}, changed {:?}", pending.changed);
    assert_eq!(paths, [".claude/settings.json", ".mcp.json", ".rcfd/claude-allow.json"]);
    assert_eq!(pending.changed.len(), 3);
    let rules: Vec<String> = first.rules().iter().map(AllowRule::to_flag_value).collect();
    assert_eq!(pending.allow_rules, Some(Ok(rules.clone())));
    assert_eq!(previews.len(), 3);
    assert!(previews.iter().all(|p| p.matches));
    // refused in the model, no process, the dialog opens
    assert!(st.chat.can_send().is_err());
    assert!(st.chat.send("hi").is_err());
    assert!(st.chat.dialog_open);
    std::thread::sleep(Duration::from_millis(500));
    assert!(s.chat_argvs().is_empty());
    let mut app = app(st);
    let ctx = egui::Context::default();
    let text = shown(&mut app, &ctx);
    for want in [
        "Confirm Claude Code files in this project",
        ".mcp.json",
        ".claude/settings.json",
        ".rcfd/claude-allow.json",
        "new or changed",
        "Bash(echo hi)",
        &pending.files[1].hash,
    ] {
        assert!(text.contains(want), "{want:?} not shown:\n{text}");
    }
    st = app.st;
    // accept: recorded in the config folder, outside the project
    st.chat.confirm().unwrap();
    assert!(st.drain_until(WAIT, |s| s.chat.folder == Folder::Confirmed));
    let record = std::fs::read_to_string(s.config.join("claude-trust.json")).unwrap();
    assert!(record.contains(".mcp.json"));
    assert!(!s.project.join(".rcfd/claude-trust.json").exists());
    st.chat.send("hi").unwrap();
    assert_eq!(finish_turn(&mut st), TurnState::Finished { code: Some(0) });
    assert_eq!(s.chat_argvs().len(), 1);
    // a changed byte asks again: refused by the driver, the dialog opens again, no process
    std::fs::write(s.project.join(".mcp.json"), "{\"mcpServers\": {} }\n").unwrap();
    st.chat.send("again").unwrap(); // the model's state is stale; the driver checks the folder before any process
    assert!(matches!(finish_turn(&mut st), TurnState::Refused(_)));
    assert!(st.drain_until(
        WAIT,
        |s| matches!(&s.chat.folder, Folder::Pending { previews, .. } if !previews.is_empty())
    ));
    let Folder::Pending { pending, .. } = &st.chat.folder else {
        unreachable!()
    };
    println!("E11: after a change: changed {:?}", pending.changed);
    assert_eq!(pending.changed, [".mcp.json"]);
    assert!(st.chat.dialog_open);
    assert_eq!(s.chat_argvs().len(), 1);
}

#[test]
fn e11_control_a_folder_without_control_files_starts_at_once() {
    let s = ChatSetup::new(
        "e11c",
        Some("stream_allowlist_dontask.jsonl"),
        Some(json!([])),
        json!({}),
    );
    let mut st = s.studio();
    assert_eq!(st.chat.folder, Folder::Confirmed);
    assert!(!st.chat.dialog_open);
    st.chat.send("hi").unwrap();
    assert_eq!(finish_turn(&mut st), TurnState::Finished { code: Some(0) });
    assert_eq!(s.chat_argvs().len(), 1);
}

/// Not registered (added after the registered runs, ADR 0134 addendum 3): closing the window during a turn kills the
/// turn's process tree before the frame returns (threads are not unwound at process exit).
#[test]
fn closing_the_window_during_a_turn_leaves_no_process() {
    let s = ChatSetup::new(
        "close",
        Some("stream_allowlist_dontask.jsonl"),
        Some(json!([])),
        json!({"hang": true, "pidfile": "pids.json"}),
    );
    let mut st = s.studio();
    st.chat.send("a long turn").unwrap();
    let (child, grandchild) = read_pids(&s.project);
    assert!(st.drain_until(WAIT, |s| matches!(
        s.chat.turn.as_ref().map(|t| &t.state),
        Some(TurnState::Running { .. })
    )));
    assert_eq!(live_pids(&[child, grandchild]).unwrap().len(), 2);
    let mut app = app(st);
    let ctx = egui::Context::default();
    let mut input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))),
        ..Default::default()
    };
    input
        .viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .events
        .push(egui::ViewportEvent::Close);
    let t0 = Instant::now();
    let _ = ctx.run_ui(input, |ui| app.ui(ui));
    let dt = t0.elapsed();
    let left = live_pids(&[child, grandchild]).unwrap();
    let gone = wait_for(Duration::from_secs(1), || {
        live_pids(&[child, grandchild]).unwrap().is_empty()
    });
    println!(
        "window close during a turn: the frame took {dt:?}; listed right after it {left:?}; all gone after {gone:?}"
    );
    assert!(gone.is_some());
}
