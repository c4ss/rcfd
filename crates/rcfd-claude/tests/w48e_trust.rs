//! ADR 0134 E8 (set-up of Amendment 2): no chat process in a folder whose Claude Code control files the user has not
//! confirmed, against the replay double `fake-claude` (never the real CLI).

use rcfd_claude::stream::StreamEvent;
use rcfd_claude::trust::{scan, Pending, TrustState, TrustStore};
use rcfd_claude::{AllowList, AllowRule, Claude, Error};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

const EVENT_TIMEOUT: Duration = Duration::from_secs(30);
const S1: &str = "00000000-0000-4000-8000-000000000001";

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("rcfd-claude-{name}-{}", rcfd_claude::new_uuid()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

struct Setup {
    root: PathBuf,
    project: PathBuf,
    claude: Claude,
    store: TrustStore,
}

/// `<tmp>/<dir>` replaying the recorded allow-list stream, no open session; the record in `<tmp>/studio-config`.
fn setup(name: &str, dir: &str) -> Setup {
    let root = tmp(name);
    let project = root.join(dir);
    std::fs::create_dir_all(&project).unwrap();
    let esc = serde_json::to_string(&project.to_string_lossy()).unwrap();
    let stream = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v2.1.310/stream_allowlist_dontask.jsonl"),
    )
    .unwrap()
    .replace("C:\\\\proj\\\\demo", &esc[1..esc.len() - 1]);
    let s = root.join("stream.jsonl");
    std::fs::write(&s, stream).unwrap();
    let a = root.join("agents.json");
    std::fs::write(&a, "[]").unwrap();
    std::fs::write(
        project.join(".fake-claude.json"),
        json!({"stream": s.to_string_lossy(), "agents": a.to_string_lossy()}).to_string(),
    )
    .unwrap();
    let store_path = root.join("studio-config").join("claude-trust.json");
    let mut claude = Claude::new(env!("CARGO_BIN_EXE_fake-claude"), root.join("home"));
    claude.trust_store = Some(store_path.clone());
    Setup {
        root,
        project,
        claude,
        store: TrustStore::new(store_path),
    }
}

fn write(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

fn calls(project: &Path) -> usize {
    std::fs::read_to_string(project.join(".fake-claude-calls.jsonl"))
        .unwrap_or_default()
        .lines()
        .count()
}

fn chat_calls(project: &Path) -> usize {
    std::fs::read_to_string(project.join(".fake-claude-calls.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter(|l| serde_json::from_str::<Value>(l).unwrap()["args"][0] != "agents")
        .count()
}

fn run(chat: rcfd_claude::Chat) {
    loop {
        let e = chat.events.recv_timeout(EVENT_TIMEOUT).expect("event within timeout");
        if let StreamEvent::Exited { code, .. } = e {
            assert_eq!(code, Some(0));
            return;
        }
    }
}

fn refused(r: Result<rcfd_claude::Chat, Error>) -> Pending {
    match r {
        Err(Error::FolderNotConfirmed { pending, .. }) => pending,
        Err(e) => panic!("refused for another reason: {e}"),
        Ok(c) => panic!("a chat started (pid {})", c.pid()),
    }
}

/// E8: a project with `.mcp.json`, `.claude/settings.json` or a nested `CLAUDE.md` starts no chat process until
/// confirmed, one after, and asks again after one byte changes; a project without those files starts at once.
#[test]
fn e8_control_files_need_confirmation_and_a_change_asks_again() {
    for rel in [".mcp.json", ".claude/settings.json", "cases/sub/CLAUDE.md"] {
        let s = setup("e8", "my-proj");
        write(&s.project, rel, "{\"x\": 1}");
        let allow = AllowList::new();
        let p = refused(s.claude.new_session(&s.project, &allow, "hi"));
        assert_eq!(p.files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), [rel]);
        assert_eq!(p.changed, [rel]);
        assert_eq!(p.allow_rules, None);
        let p2 = refused(s.claude.resume(&s.project, S1, &allow, "hi"));
        assert_eq!(p2, p);
        assert_eq!(
            calls(&s.project),
            0,
            "{rel}: a process was started before the confirmation"
        );
        // the user confirms what was shown
        s.store.confirm(&s.project, &p.files).unwrap();
        assert_eq!(s.claude.check_folder(&s.project).unwrap(), TrustState::Confirmed);
        run(s.claude.new_session(&s.project, &allow, "hi").unwrap());
        assert_eq!((calls(&s.project), chat_calls(&s.project)), (1, 1), "{rel}");
        // one byte changes: asks again, nothing starts
        write(&s.project, rel, "{\"x\": 2}");
        let p3 = refused(s.claude.new_session(&s.project, &allow, "hi"));
        assert_eq!(p3.changed, [rel]);
        let p4 = refused(s.claude.resume(&s.project, S1, &allow, "hi"));
        assert_eq!(p4.changed, [rel]);
        assert_eq!(calls(&s.project), 1, "{rel}");
        println!(
            "E8 {rel}: refused before confirmation (0 processes), 1 chat process after, refused again after a change (still 1)"
        );
    }
    // negative control: a project without control files starts at once, without a record and without a store
    let s = setup("e8plain", "my-proj");
    write(&s.project, "cases/a.json", "{}");
    write(&s.project, "notes/CLAUDE.txt", "not a control file");
    let mut no_store = s.claude.clone();
    no_store.trust_store = None;
    run(no_store.new_session(&s.project, &AllowList::new(), "hi").unwrap());
    run(s.claude.new_session(&s.project, &AllowList::new(), "hi").unwrap());
    assert_eq!(chat_calls(&s.project), 2);
    assert!(!s.store.path().exists(), "nothing was recorded");
    println!("E8 control: a project without control files started at once (2 processes, no record)");
}

/// Amendment 2 (c): the allow-list file is confirmed with its rules shown; the "allow" button does not ask again, a
/// direct write of the file does.
#[test]
fn e8_allow_file_is_confirmed_with_its_rules() {
    let s = setup("e8allow", "my-proj");
    let file = AllowList::file_of(&s.project);
    let mut l = AllowList::project_initial();
    l.add(AllowRule::Edit("note.txt".into())).unwrap();
    l.to_file(&file).unwrap();
    let want: Vec<String> = l.rules().iter().map(AllowRule::to_flag_value).collect();
    let p = refused(s.claude.new_session(&s.project, &l, "hi"));
    assert_eq!(p.changed, [".rcfd/claude-allow.json"]);
    assert_eq!(p.allow_rules, Some(Ok(want)));
    assert_eq!(calls(&s.project), 0);
    s.store.confirm(&s.project, &p.files).unwrap();
    run(s.claude.new_session(&s.project, &l, "hi").unwrap());
    // "allow" through the record: no new question
    l.add(AllowRule::Bash("echo hi".into())).unwrap();
    s.store.save_allow_list(&s.project, &l).unwrap();
    assert_eq!(s.claude.check_folder(&s.project).unwrap(), TrustState::Confirmed);
    run(s.claude.new_session(&s.project, &l, "hi").unwrap());
    assert_eq!(chat_calls(&s.project), 2);
    // a write that did not go through the record asks again
    l.add(AllowRule::Bash("echo bye".into())).unwrap();
    l.to_file(&file).unwrap();
    let p = refused(s.claude.new_session(&s.project, &l, "hi"));
    assert_eq!(p.changed, [".rcfd/claude-allow.json"]);
    assert_eq!(chat_calls(&s.project), 2);
    // a tampered allow file is shown with the reason it is refused
    std::fs::write(&file, "{\"version\": 1, \"rules\": [\"Bash\"]}\n").unwrap();
    let p = refused(s.claude.new_session(&s.project, &l, "hi"));
    let why = p.allow_rules.unwrap().unwrap_err();
    assert!(why.contains("line 1"), "{why}");
    assert_eq!(chat_calls(&s.project), 2);
    println!("E8 allow file: rules shown, allow button kept the folder confirmed, a direct write and a tampered file asked again ({why})");
}

/// A record shipped inside the received folder confirms nothing, and a record cannot be kept there; a confirmed folder
/// copied to another path is not confirmed there.
#[test]
fn e8_shipped_record_and_copied_folder_are_not_confirmed() {
    let s = setup("e8ship", "my-proj");
    write(
        &s.project,
        ".mcp.json",
        "{\"mcpServers\": {\"x\": {\"command\": \"calc\"}}}",
    );
    write(&s.project, ".claude/settings.json", "{\"hooks\": {}}");
    // the sender's record, in the store format, with the right hashes for this exact path
    let files = scan(&s.project).unwrap();
    let key = TrustStore::key(&s.project).unwrap();
    let shipped = s.project.join(".rcfd").join("claude-trust.json");
    write(
        &s.project,
        ".rcfd/claude-trust.json",
        &serde_json::to_string_pretty(&json!({"version": 1, "projects": {key: files}})).unwrap(),
    );
    let p = refused(s.claude.new_session(&s.project, &AllowList::new(), "hi"));
    assert_eq!(p.changed, [".claude/settings.json", ".mcp.json"]);
    // a store pointed into the project is refused as a store, and still nothing starts
    let mut inside = s.claude.clone();
    inside.trust_store = Some(shipped.clone());
    match inside.new_session(&s.project, &AllowList::new(), "hi") {
        Err(Error::Io { source, .. }) => assert!(source.to_string().contains("inside the project")),
        r => panic!("{r:?}"),
    }
    assert!(TrustStore::new(&shipped).confirm(&s.project, &files).is_err());
    assert_eq!(calls(&s.project), 0);
    // confirmed here, then the same files at another path
    s.store.confirm(&s.project, &p.files).unwrap();
    run(s.claude.new_session(&s.project, &AllowList::new(), "hi").unwrap());
    let copy = s.root.join("copy-of-my-proj");
    for rel in [".mcp.json", ".claude/settings.json", ".fake-claude.json"] {
        std::fs::create_dir_all(copy.join(rel).parent().unwrap()).unwrap();
        std::fs::copy(s.project.join(rel), copy.join(rel)).unwrap();
    }
    let p = refused(s.claude.new_session(&copy, &AllowList::new(), "hi"));
    assert_eq!(p.changed, [".claude/settings.json", ".mcp.json"]);
    assert_eq!(calls(&copy), 0);
    println!("E8 shipped record and copied folder: both refused, no process");
}
