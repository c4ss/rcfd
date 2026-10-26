//! ADR 0126 R1-R3: the three real-CLI paths (list, new session, resume). Ignored: they call the installed `claude`
//! and cost API usage. Run by hand, one at a time, in a scratch folder that is not a repository:
//!
//! ```text
//! RCFD_CLAUDE_REAL_DIR=<scratch folder> cargo test -p rcfd-claude --test real_cli -- --ignored --test-threads=1
//! ```
//!
//! Each run is one cheap `--model haiku` turn (R2 and R3); R1 makes no model call.

use rcfd_claude::discover::OpenState;
use rcfd_claude::stream::StreamEvent;
use rcfd_claude::{AllowList, AllowRule, Claude};
use std::path::PathBuf;
use std::time::Duration;

fn setup() -> (Claude, PathBuf) {
    let dir =
        PathBuf::from(std::env::var("RCFD_CLAUDE_REAL_DIR").expect("set RCFD_CLAUDE_REAL_DIR to a scratch folder"));
    let mut c = Claude::from_env().expect("home directory");
    c.model = Some("haiku".into());
    (c, dir)
}

fn run(chat: &rcfd_claude::Chat) -> Vec<StreamEvent> {
    let mut v = Vec::new();
    loop {
        let e = chat.events.recv_timeout(Duration::from_secs(180)).expect("event");
        println!("{e:?}");
        let end = matches!(e, StreamEvent::Exited { .. });
        v.push(e);
        if end {
            return v;
        }
    }
}

#[test]
#[ignore = "calls the installed claude CLI (no model call)"]
fn r1_real_list() {
    let (c, dir) = setup();
    let d = c.discover(&dir);
    for s in &d.sessions {
        println!("{} {:?} {:?}", s.session_id, s.state, s.transcript);
    }
    assert!(d.warnings.is_empty(), "{:?}", d.warnings);
    assert!(
        !d.sessions.is_empty(),
        "the scratch folder should hold the sessions of the recorded calls"
    );
    assert!(
        d.sessions.iter().all(|s| s.state == OpenState::Closed),
        "nothing else should have the scratch folder open"
    );
    // the session running this test's parent, if any, is open in its own folder: check the listing sees it
    if let Ok(id) = std::env::var("RCFD_CLAUDE_REAL_OPEN_SESSION") {
        let cwd = PathBuf::from(std::env::var("RCFD_CLAUDE_REAL_OPEN_CWD").expect("cwd of that session"));
        let d = c.discover(&cwd);
        assert!(
            d.sessions
                .iter()
                .any(|s| s.session_id == id && matches!(s.state, OpenState::Open { .. })),
            "{d:?}"
        );
    }
}

#[test]
#[ignore = "calls the installed claude CLI (one haiku turn)"]
fn r2_real_new_session() {
    let (c, dir) = setup();
    let chat = c
        .new_session(&dir, &AllowList::new(), "Reply with exactly: pong")
        .unwrap();
    let id = chat.session_id.clone();
    let ev = run(&chat);
    assert!(
        matches!(&ev[0], StreamEvent::Init { session_id, permission_mode: Some(m), .. } if *session_id == id && m == "dontAsk")
    );
    assert!(ev.iter().any(|e| matches!(e, StreamEvent::TextDelta(_))));
    assert!(ev.iter().any(|e| matches!(e, StreamEvent::Result { session_id, is_error: false, text: Some(t), .. } if *session_id == id && t.contains("pong"))));
    assert!(matches!(ev.last(), Some(StreamEvent::Exited { code: Some(0), .. })));
    println!("R2 session {id}");
}

#[test]
#[ignore = "calls the installed claude CLI (one haiku turn)"]
fn r3_real_resume_with_denial() {
    let (c, dir) = setup();
    let d = c.discover(&dir);
    let s = d
        .sessions
        .iter()
        .find(|s| s.state == OpenState::Closed)
        .expect("a closed session in the scratch folder");
    let mut allow = AllowList::new();
    allow.add(AllowRule::Bash("echo hello".into())).unwrap();
    let chat = c
        .resume(
            &dir,
            &s.session_id,
            &allow,
            "Use the Write tool once to create r3.txt containing z. Do not retry; reply in one line.",
        )
        .unwrap();
    let ev = run(&chat);
    assert!(matches!(&ev[0], StreamEvent::Init { session_id, .. } if *session_id == s.session_id));
    assert!(
        ev.iter().any(|e| matches!(e, StreamEvent::PermissionDenied { tool_name, suggestion: Some(AllowRule::Edit(p)), .. } if tool_name == "Write" && p == "r3.txt")),
        "denial with an Edit(r3.txt) suggestion expected"
    );
    assert!(!dir.join("r3.txt").exists());
    assert!(matches!(ev.last(), Some(StreamEvent::Exited { code: Some(0), .. })));
}
