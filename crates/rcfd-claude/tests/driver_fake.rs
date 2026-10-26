//! ADR 0126 checks D1-D7: the driver against the replay double `fake-claude` (never the real CLI).

use rcfd_claude::discover::{live_pids, OpenState};
use rcfd_claude::stream::StreamEvent;
use rcfd_claude::{AllowList, AllowRule, Claude, Error};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// Registered limits (ADR 0126).
const CANCEL_LIMIT: Duration = Duration::from_secs(5);
const EVENT_TIMEOUT: Duration = Duration::from_secs(30);

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/v2.1.310")
        .join(name)
}

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("rcfd-claude-{name}-{}", rcfd_claude::new_uuid()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The fixture text with the scrubbed project folder replaced by `project` (JSON-escaped).
fn retarget(name: &str, project: &Path) -> String {
    let esc = serde_json::to_string(&project.to_string_lossy()).unwrap();
    std::fs::read_to_string(fixture(name))
        .unwrap()
        .replace("C:\\\\proj\\\\demo", &esc[1..esc.len() - 1])
}

struct Setup {
    project: PathBuf,
    claude: Claude,
}

fn setup(name: &str, stream: Option<&str>, agents: Option<Value>, extra: Value) -> Setup {
    let root = tmp(name);
    let project = root.join("my-proj");
    std::fs::create_dir_all(&project).unwrap();
    let mut cfg = json!({"agents_fail": agents.is_none()});
    if let Some(s) = stream {
        let p = root.join("stream.jsonl");
        std::fs::write(&p, retarget(s, &project)).unwrap();
        cfg["stream"] = p.to_string_lossy().into();
    }
    if let Some(a) = agents {
        let p = root.join("agents.json");
        std::fs::write(&p, a.to_string()).unwrap();
        cfg["agents"] = p.to_string_lossy().into();
    }
    for (k, v) in extra.as_object().unwrap() {
        cfg[k] = v.clone();
    }
    std::fs::write(project.join(".fake-claude.json"), cfg.to_string()).unwrap();
    let claude = Claude::new(env!("CARGO_BIN_EXE_fake-claude"), root.join("home"));
    Setup { project, claude }
}

fn calls(project: &Path) -> Vec<Value> {
    std::fs::read_to_string(project.join(".fake-claude-calls.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn collect(chat: &rcfd_claude::Chat) -> Vec<StreamEvent> {
    let mut v = Vec::new();
    loop {
        let e = chat.events.recv_timeout(EVENT_TIMEOUT).expect("event within timeout");
        let end = matches!(e, StreamEvent::Exited { .. });
        v.push(e);
        if end {
            return v;
        }
    }
}

fn dead_pid() -> u32 {
    let mut c = Command::new(env!("CARGO_BIN_EXE_fake-claude"))
        .current_dir(env!("CARGO_TARGET_TMPDIR"))
        .arg("agents")
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let pid = c.id();
    c.wait().unwrap();
    pid
}

fn alive(pid: u32) -> bool {
    !live_pids(&[pid]).unwrap().is_empty()
}

#[test]
fn d1_new_session_sets_its_id_and_streams_events() {
    let s = setup("d1", Some("stream_allowlist_dontask.jsonl"), Some(json!([])), json!({}));
    let mut allow = AllowList::new();
    allow.add(AllowRule::Edit("note.txt".into())).unwrap();
    allow.add(AllowRule::Bash("echo hello".into())).unwrap();
    let chat = s
        .claude
        .new_session(&s.project, &allow, "-p hello \"quoted\" --dangerously-skip-permissions")
        .unwrap();
    let ev = collect(&chat);
    assert!(rcfd_claude::is_uuid(&chat.session_id));
    let c = calls(&s.project);
    assert_eq!(c.len(), 1);
    let args: Vec<String> = serde_json::from_value(c[0]["args"].clone()).unwrap();
    assert_eq!(args, chat.args());
    let i = args.iter().position(|a| a == "--session-id").expect("--session-id");
    assert_eq!(args[i + 1], chat.session_id);
    assert!(!args.iter().any(|a| a == "--resume"));
    // the prompt went to stdin as one stream-json line, not on the command line
    let stdin: Value = serde_json::from_str(c[0]["stdin"].as_str().unwrap().trim()).unwrap();
    assert_eq!(
        stdin["message"]["content"],
        "-p hello \"quoted\" --dangerously-skip-permissions"
    );
    assert!(!args.iter().any(|a| a.contains("hello \"quoted")));
    let n = |f: fn(&StreamEvent) -> bool| ev.iter().filter(|e| f(e)).count();
    assert!(matches!(&ev[0], StreamEvent::Init { permission_mode: Some(m), .. } if m == "dontAsk"));
    assert_eq!(n(|e| matches!(e, StreamEvent::ToolCall { .. })), 3);
    assert_eq!(n(|e| matches!(e, StreamEvent::Result { is_error: false, .. })), 1);
    assert_eq!(n(|e| matches!(e, StreamEvent::Aborted(_))), 0);
    assert!(
        matches!(ev.last(), Some(StreamEvent::Exited { code: Some(0), stderr_tail }) if stderr_tail.contains("replayed"))
    );
}

fn listing(entries: &[(&str, u32, &Path)]) -> Value {
    Value::Array(
        entries
            .iter()
            .map(|(id, pid, cwd)| json!({"pid": pid, "cwd": cwd.to_string_lossy(), "kind": "interactive", "sessionId": id, "name": "x", "status": "busy"}))
            .collect(),
    )
}

const S1: &str = "00000000-0000-4000-8000-000000000001";

#[test]
fn d2_open_session_refused_closed_one_resumed() {
    let me = std::process::id();
    // open in another process (this test's pid is live), listed under another cwd: refused, no chat process started
    let other = PathBuf::from("C:\\elsewhere");
    let s = setup(
        "d2a",
        Some("stream_resume.jsonl"),
        Some(listing(&[(S1, me, &other)])),
        json!({}),
    );
    match s.claude.resume(&s.project, S1, &AllowList::new(), "hi") {
        Err(Error::SessionOpen { session_id, pid }) => assert_eq!((session_id.as_str(), pid), (S1, me)),
        r => panic!("not refused: {r:?}"),
    }
    let c = calls(&s.project);
    assert_eq!(c.len(), 1, "only the listing ran");
    assert_eq!(c[0]["args"], json!(["agents", "--json"]));
    // negative control 1: same listing but the pid is dead -> closed -> resumed
    let dead = dead_pid();
    assert!(!alive(dead));
    let s = setup(
        "d2b",
        Some("stream_resume.jsonl"),
        Some(listing(&[(S1, dead, &other)])),
        json!({}),
    );
    let chat = s.claude.resume(&s.project, S1, &AllowList::new(), "hi").unwrap();
    let ev = collect(&chat);
    assert!(matches!(ev.last(), Some(StreamEvent::Exited { code: Some(0), .. })));
    let args: Vec<String> = serde_json::from_value(calls(&s.project)[1]["args"].clone()).unwrap();
    assert_eq!(args[args.len() - 2..], ["--resume", S1]);
    // negative control 2: not listed -> resumed
    let s = setup("d2c", Some("stream_resume.jsonl"), Some(json!([])), json!({}));
    assert!(s.claude.resume(&s.project, S1, &AllowList::new(), "hi").is_ok());
    // listing failed -> state unknown -> refused
    let s = setup("d2d", Some("stream_resume.jsonl"), None, json!({}));
    assert!(matches!(
        s.claude.resume(&s.project, S1, &AllowList::new(), "hi"),
        Err(Error::OpenStateUnknown { .. })
    ));
    // an id that is not a UUID never reaches the command line
    let s = setup("d2e", Some("stream_resume.jsonl"), Some(json!([])), json!({}));
    assert!(matches!(
        s.claude
            .resume(&s.project, "--dangerously-skip-permissions", &AllowList::new(), "hi"),
        Err(Error::InvalidSessionId(_))
    ));
    assert!(calls(&s.project).is_empty());
}

fn denials(ev: &[StreamEvent]) -> Vec<(String, Option<AllowRule>)> {
    ev.iter()
        .filter_map(|e| match e {
            StreamEvent::PermissionDenied {
                tool_name, suggestion, ..
            } => Some((tool_name.clone(), suggestion.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn d3_denial_carries_an_exact_allow_suggestion_that_only_edits_the_list() {
    let s = setup("d3", Some("stream_denied_dontask.jsonl"), Some(json!([])), json!({}));
    let mut allow = AllowList::new();
    allow.add(AllowRule::Edit("note.txt".into())).unwrap();
    let ev = collect(&s.claude.resume(&s.project, S1, &allow, "write other.txt").unwrap());
    let d = denials(&ev);
    assert_eq!(
        d,
        vec![("Write".to_string(), Some(AllowRule::Edit("other.txt".into())))]
    );
    // "allow this": the list grows by exactly that rule; mode and prompts stay the same
    let before = s.claude.chat_args("--resume", S1, &allow);
    allow.add(d[0].1.clone().unwrap()).unwrap();
    let after = s.claude.chat_args("--resume", S1, &allow);
    assert_eq!(after.len(), before.len() + 1);
    assert!(after.contains(&"Edit(./other.txt)".to_string()));
    assert!(after.windows(2).any(|w| w == ["--permission-mode", "dontAsk"]));
    // the default-mode stream gives Bash and Write suggestions
    let s = setup("d3b", Some("stream_denied_default.jsonl"), Some(json!([])), json!({}));
    let ev = collect(&s.claude.resume(&s.project, S1, &AllowList::new(), "x").unwrap());
    assert_eq!(
        denials(&ev),
        vec![
            (
                "Bash".to_string(),
                Some(AllowRule::Bash("echo hello > made_by_bash.txt".into()))
            ),
            ("Write".to_string(), Some(AllowRule::Edit("note.txt".into())))
        ]
    );
}

#[test]
fn d4_a_cli_that_does_not_apply_dont_ask_is_aborted() {
    // the recorded call-3 stream reports permissionMode "default"
    let s = setup(
        "d4",
        Some("stream_denied_default.jsonl"),
        Some(json!([])),
        json!({"hang": true, "pidfile": "pids.json"}),
    );
    let chat = s.claude.new_session(&s.project, &AllowList::new(), "x").unwrap();
    let ev = collect(&chat);
    assert!(matches!(ev[1], StreamEvent::Aborted(_)), "{ev:?}");
    assert!(matches!(ev.last(), Some(StreamEvent::Exited { code, .. }) if *code != Some(0)));
    // negative control: the dontAsk stream is not aborted (d1)
}

fn read_pids(project: &Path) -> (u32, u32) {
    let t0 = Instant::now();
    loop {
        if let Some(v) = std::fs::read_to_string(project.join("pids.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        {
            return (
                v["child"].as_u64().unwrap() as u32,
                v["grandchild"].as_u64().unwrap() as u32,
            );
        }
        assert!(t0.elapsed() < EVENT_TIMEOUT, "no pid file");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn d5_cancel_leaves_no_child_process() {
    let s = setup(
        "d5",
        Some("stream_allowlist_dontask.jsonl"),
        Some(json!([])),
        json!({"hang": true, "pidfile": "pids.json"}),
    );
    let chat = s.claude.new_session(&s.project, &AllowList::new(), "x").unwrap();
    assert!(matches!(
        chat.events.recv_timeout(EVENT_TIMEOUT).unwrap(),
        StreamEvent::Init { .. }
    ));
    let (child, grandchild) = read_pids(&s.project);
    assert_eq!(child, chat.pid());
    // the liveness check discriminates: both are alive before cancel
    assert!(alive(child) && alive(grandchild));
    std::thread::sleep(Duration::from_millis(300));
    assert!(alive(child) && alive(grandchild), "the fake must hang until cancelled");
    let t0 = Instant::now();
    chat.cancel();
    let t_cancel = t0.elapsed();
    let gone = || live_pids(&[child, grandchild]).unwrap().is_empty();
    let mut checks = 1;
    while !gone() && t0.elapsed() < CANCEL_LIMIT {
        checks += 1;
        std::thread::sleep(Duration::from_millis(20));
    }
    let dt = t0.elapsed();
    println!("D5: cancel() returned after {t_cancel:?}; child {child} and grandchild {grandchild} gone after {dt:?} ({checks} checks)");
    assert!(gone(), "orphan after {dt:?}");
    assert!(dt < CANCEL_LIMIT, "{dt:?}");
    assert!(matches!(
        chat.events.recv_timeout(EVENT_TIMEOUT).unwrap(),
        StreamEvent::Exited { .. }
    ));
}

#[test]
fn d6_negative_control_plain_kill_orphans_the_grandchild() {
    let s = setup(
        "d6",
        Some("stream_allowlist_dontask.jsonl"),
        Some(json!([])),
        json!({"hang": true, "pidfile": "pids.json"}),
    );
    let mut c = Command::new(env!("CARGO_BIN_EXE_fake-claude"))
        .current_dir(&s.project)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let (_, grandchild) = read_pids(&s.project);
    c.kill().unwrap();
    c.wait().unwrap();
    std::thread::sleep(CANCEL_LIMIT.min(Duration::from_secs(1)));
    let orphaned = alive(grandchild);
    // clean up our own orphan by pid before asserting
    let _ = if cfg!(windows) {
        Command::new("taskkill")
            .args(["/PID", &grandchild.to_string(), "/F"])
            .output()
    } else {
        Command::new("kill").args(["-KILL", &grandchild.to_string()]).output()
    };
    assert!(
        orphaned,
        "Child::kill alone should leave the grandchild running (else D5 does not discriminate)"
    );
}

#[test]
fn d7_discovery_from_both_sources_and_when_either_fails() {
    let me = std::process::id();
    let dead = dead_pid();
    let s1 = "00000000-0000-4000-8000-000000000001";
    let s2 = "00000000-0000-4000-8000-000000000002";
    let s3 = "11111111-2222-4333-8444-555555555555"; // open, no transcript yet
    let s4 = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee"; // transcript of another folder with the same directory name
    let lst = |p: &Path| listing(&[(s2, me, p), (s1, dead, p), (s3, me, p), ("not-a-uuid", me, p)]);
    let s = setup("d7", None, Some(json!([])), json!({}));
    let lst = lst(&s.project);
    std::fs::write(
        s.project.join(".fake-claude.json"),
        json!({"agents": s.project.join("agents.json").to_string_lossy()}).to_string(),
    )
    .unwrap();
    std::fs::write(s.project.join("agents.json"), lst.to_string()).unwrap();
    let dir = s
        .claude
        .claude_home
        .join("projects")
        .join(rcfd_claude::discover::project_dir_name(&s.project));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join(format!("{s1}.jsonl")),
        retarget("transcript_resumed_denied.jsonl", &s.project),
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(50));
    std::fs::write(
        dir.join(format!("{s2}.jsonl")),
        retarget("transcript_allowlist.jsonl", &s.project),
    )
    .unwrap();
    let other = s.project.with_file_name("my_proj"); // "my-proj" and "my_proj" share the directory name
    std::fs::write(
        dir.join(format!("{s4}.jsonl")),
        retarget("transcript_allowlist.jsonl", &other),
    )
    .unwrap();
    std::fs::write(dir.join("notes.txt"), "x").unwrap();
    let d = s.claude.discover(&s.project);
    assert!(d.warnings.is_empty(), "{:?}", d.warnings);
    let got: Vec<(&str, OpenState, bool)> = d
        .sessions
        .iter()
        .map(|x| (x.session_id.as_str(), x.state.clone(), x.transcript.is_some()))
        .collect();
    assert_eq!(
        got,
        vec![
            (s2, OpenState::Open { pid: me }, true),
            (s1, OpenState::Closed, true),
            (s3, OpenState::Open { pid: me }, false)
        ]
    );
    // listing fails: transcripts still listed, state unknown, a warning says why
    std::fs::write(
        s.project.join(".fake-claude.json"),
        json!({"agents_fail": true}).to_string(),
    )
    .unwrap();
    let d = s.claude.discover(&s.project);
    assert_eq!(
        d.sessions.iter().map(|x| x.state.clone()).collect::<Vec<_>>(),
        vec![OpenState::Unknown, OpenState::Unknown]
    );
    assert!(d.warnings.iter().any(|w| w.contains("agents")), "{:?}", d.warnings);
    // transcript directory missing: open sessions from the listing only, a warning says why
    std::fs::write(
        s.project.join(".fake-claude.json"),
        json!({"agents": s.project.join("agents.json").to_string_lossy()}).to_string(),
    )
    .unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    let d = s.claude.discover(&s.project);
    assert_eq!(
        d.sessions.iter().map(|x| x.session_id.as_str()).collect::<Vec<_>>(),
        vec![s2, s3]
    );
    assert_eq!(d.warnings.len(), 1);
}
