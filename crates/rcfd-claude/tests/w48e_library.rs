//! ADR 0134 (wave 48E), library part: checks E4 (the file part), E5 and E9 against the replay double `fake-claude`
//! (never the real CLI).

use rcfd_claude::allow::{check_args, check_args_in, pinned_mcp_config, suggest, PINNED_SERVER, PROJECT_READ_TOOLS};
use rcfd_claude::stream::StreamEvent;
use rcfd_claude::{AllowList, AllowRule, Claude, Error};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

const EVENT_TIMEOUT: Duration = Duration::from_secs(30);
const S1: &str = "00000000-0000-4000-8000-000000000001";
const PERMISSION: [&str; 4] = ["--permission-mode", "dontAsk", "--permission-prompts", "none"];

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
    root: PathBuf,
    project: PathBuf,
    claude: Claude,
}

/// A project folder `<tmp>/my-proj` whose `.fake-claude.json` replays `stream` and lists no open session.
fn setup(name: &str, stream: &str) -> Setup {
    let root = tmp(name);
    let project = root.join("my-proj");
    std::fs::create_dir_all(&project).unwrap();
    let s = root.join("stream.jsonl");
    std::fs::write(&s, retarget(stream, &project)).unwrap();
    let a = root.join("agents.json");
    std::fs::write(&a, "[]").unwrap();
    std::fs::write(
        project.join(".fake-claude.json"),
        json!({"stream": s.to_string_lossy(), "agents": a.to_string_lossy()}).to_string(),
    )
    .unwrap();
    let claude = Claude::new(env!("CARGO_BIN_EXE_fake-claude"), root.join("home"));
    Setup { root, project, claude }
}

fn calls(project: &Path) -> Vec<Value> {
    std::fs::read_to_string(project.join(".fake-claude-calls.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// Argument lists of the chat processes the fake recorded (the `agents --json` listings left out).
fn chat_argvs(project: &Path) -> Vec<Vec<String>> {
    calls(project)
        .iter()
        .map(|c| serde_json::from_value::<Vec<String>>(c["args"].clone()).unwrap())
        .filter(|a| a.first().map(String::as_str) != Some("agents"))
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

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

/// The `--allowedTools` values of an argument list.
fn rules_in(args: &[String]) -> Vec<String> {
    let Some(i) = args.iter().position(|a| a == "--allowedTools") else {
        return Vec::new();
    };
    args[i + 1..]
        .iter()
        .take_while(|a| !a.starts_with('-'))
        .cloned()
        .collect()
}

fn has_window(args: &[String], w: &[&str]) -> usize {
    args.windows(w.len())
        .filter(|x| x.iter().zip(w).all(|(a, b)| a == b))
        .count()
}

/// Defaults unchanged: without `mcp_config` the argument list is exactly the ADR 0126 one.
#[test]
fn e0_chat_args_without_pinning_are_unchanged() {
    let mut allow = AllowList::new();
    allow.add(AllowRule::Edit("note.txt".into())).unwrap();
    let mut c = Claude::new("claude", "home");
    assert!(c.mcp_config.is_none());
    let mut want = strings(&[
        "-p",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--input-format",
        "stream-json",
        "--permission-mode",
        "dontAsk",
        "--permission-prompts",
        "none",
        "--allowedTools",
        "Edit(./note.txt)",
        "--session-id",
        S1,
    ]);
    assert_eq!(c.chat_args("--session-id", S1, &allow), want);
    c.model = Some("haiku".into());
    want.splice(7..7, strings(&["--model", "haiku"]));
    assert_eq!(c.chat_args("--session-id", S1, &allow), want);
    // the new checker gives the old verdict on lists without MCP flags
    assert!(check_args(&want).is_ok() && check_args_in(&want, Path::new("my-proj")).is_ok());
}

/// E4, the library part (the suggestion shown and the button are the panel's, ADR 0134 integration): the call-5
/// denial gives `Edit(other.txt)`; "allow" grows the allow-list file by exactly that rule; the next chat's argv has it
/// and keeps `--permission-mode dontAsk --permission-prompts none` right after the fixed flags.
#[test]
fn e4_allow_grows_the_file_by_one_rule_and_the_next_argv_has_it() {
    let s = setup("e4", "stream_denied_dontask.jsonl");
    let file = AllowList::file_of(&s.project);
    let mut first = AllowList::project_initial();
    first.add(AllowRule::Edit("note.txt".into())).unwrap();
    first.to_file(&file).unwrap();
    let before = AllowList::from_file(&file).unwrap();
    assert_eq!(before, first);
    let ev = collect(&s.claude.resume(&s.project, S1, &before, "write other.txt").unwrap());
    let suggestion = ev
        .iter()
        .find_map(|e| match e {
            StreamEvent::PermissionDenied {
                tool_name, suggestion, ..
            } if tool_name == "Write" => Some(suggestion.clone()),
            _ => None,
        })
        .expect("a Write denial");
    assert_eq!(suggestion, Some(AllowRule::Edit("other.txt".into())));
    // "allow": read the file, add exactly the suggested rule, write it back
    let mut l = AllowList::from_file(&file).unwrap();
    l.add(suggestion.clone().unwrap()).unwrap();
    l.to_file(&file).unwrap();
    let after = AllowList::from_file(&file).unwrap();
    assert_eq!(after.rules().len(), before.rules().len() + 1);
    assert_eq!(after.rules()[..before.rules().len()], *before.rules());
    assert_eq!(after.rules().last(), suggestion.as_ref());
    // the next message
    let ev = collect(&s.claude.resume(&s.project, S1, &after, "again").unwrap());
    assert!(matches!(ev.last(), Some(StreamEvent::Exited { code: Some(0), .. })));
    let argvs = chat_argvs(&s.project);
    assert_eq!(argvs.len(), 2, "{argvs:?}");
    let (a1, a2) = (&argvs[0], &argvs[1]);
    assert!(!a1.contains(&"Edit(./other.txt)".to_string()));
    assert!(a2.contains(&"Edit(./other.txt)".to_string()));
    assert_eq!(rules_in(a2).len(), rules_in(a1).len() + 1);
    assert_eq!(a2[7..11], PERMISSION, "{a2:?}");
    assert_eq!(has_window(a2, &PERMISSION), 1);
    println!(
        "E4: suggestion {:?}; file rules {} -> {}; argv rules {} -> {}",
        suggestion.as_ref().map(AllowRule::to_flag_value),
        before.rules().len(),
        after.rules().len(),
        rules_in(a1).len(),
        rules_in(a2).len()
    );
    // negative control: a denied write of .mcp.json has no suggestion, so no "allow" button
    let mcp = s.project.join(".mcp.json");
    assert_eq!(
        suggest("Write", Some(&json!({"file_path": mcp.to_string_lossy()})), &s.project),
        None
    );
}

/// The list E5 writes: the first project list plus one rule of every kind, in the normalized form.
fn e5_list() -> AllowList {
    let mut l = AllowList::project_initial();
    for r in [
        AllowRule::Tool("Grep".into()),
        AllowRule::Bash("echo hello".into()),
        AllowRule::Bash("cargo test *".into()),
        AllowRule::PowerShell("Get-ChildItem cases".into()),
        AllowRule::Edit("note.txt".into()),
        AllowRule::Edit("cases/**".into()),
        AllowRule::Read("runs/*/metrics.json".into()),
    ] {
        l.add(r).unwrap();
    }
    l
}

/// Inserts `text` as line `line` (1-based) of `bytes`.
fn insert_line(bytes: &[u8], line: usize, text: &str) -> Vec<u8> {
    let s = String::from_utf8(bytes.to_vec()).unwrap();
    let mut lines: Vec<&str> = s.lines().collect();
    lines.insert(line - 1, text);
    (lines.join("\n") + "\n").into_bytes()
}

/// E5: round trip exact; a tampered file with a bare `Bash`, a bypass token or `Edit(.rcfd/x)` is refused as a whole
/// with the line number; a valid file is accepted.
#[test]
fn e5_allow_list_file_round_trip_and_tampered_files() {
    let root = tmp("e5");
    let project = root.join("proj");
    std::fs::create_dir_all(&project).unwrap();
    let file = AllowList::file_of(&project);
    assert!(!file.parent().unwrap().exists(), ".rcfd/ is created by to_file");
    let l = e5_list();
    l.to_file(&file).unwrap();
    let bytes = std::fs::read(&file).unwrap();
    assert_eq!(bytes, l.to_file_bytes());
    let back = AllowList::from_file(&file).unwrap();
    assert_eq!(back, l, "round trip");
    assert_eq!(back.to_file_bytes(), bytes, "rewritten byte for byte");
    assert_eq!(back.permission_args(), l.permission_args());
    // a rule written with the './' anchor reads back as the same flag value
    let mut dotted = AllowList::new();
    dotted.add(AllowRule::Edit("./cases/**".into())).unwrap();
    let d = AllowList::from_file_bytes(&file, &dotted.to_file_bytes()).unwrap();
    assert_eq!(d.permission_args(), dotted.permission_args());
    // tampered files: one inserted line each, inside the rules array
    let text = String::from_utf8(bytes.clone()).unwrap();
    println!("E5 file:\n{text}");
    let line = 6;
    for bad in [
        "    \"Bash\",",
        "    \"--dangerously-skip-permissions\",",
        "    \"Edit(./.rcfd/x)\",",
    ] {
        let t = insert_line(&bytes, line, bad);
        std::fs::write(&file, &t).unwrap();
        match AllowList::from_file(&file) {
            Err(Error::AllowFile {
                line: l,
                column,
                reason,
                ..
            }) => {
                println!("E5: {bad:?} refused at line {l}, column {column}: {reason}");
                assert_eq!(l, line, "{bad}");
                assert!(!reason.is_empty());
            }
            r => panic!("{bad} not refused: {r:?}"),
        }
        let msg = AllowList::from_file(&file).unwrap_err().to_string();
        assert!(
            msg.contains(&format!("line {line}")) && msg.contains("not used"),
            "{msg}"
        );
    }
    // more tampering, not registered: two rules on one line, another version, an unknown key, broken JSON
    let mut more = vec![
        (
            insert_line(&bytes, line, "    \"Grep\", \"Bash(echo bypassPermissions)\","),
            line,
        ),
        (text.replacen("\"version\": 1", "\"version\": 2", 1).into_bytes(), 2),
        (insert_line(&bytes, 2, "  \"deny\": [],"), 2),
    ];
    let mut broken = bytes.clone();
    broken.truncate(bytes.len() - 4);
    more.push((broken, 0));
    for (t, want) in more {
        match AllowList::from_file_bytes(&file, &t) {
            Err(Error::AllowFile { line: l, .. }) => assert!(want == 0 || l == want, "line {l}, want {want}"),
            r => panic!("not refused: {r:?}"),
        }
    }
    // negative control: the untampered file is accepted
    std::fs::write(&file, &bytes).unwrap();
    assert_eq!(AllowList::from_file(&file).unwrap(), l);
    // a missing file is an I/O error of kind NotFound (the caller then starts from AllowList::project_initial)
    match AllowList::from_file(&project.join(".rcfd/none.json")) {
        Err(Error::Io { source, .. }) => assert_eq!(source.kind(), std::io::ErrorKind::NotFound),
        r => panic!("{r:?}"),
    }
}

/// The argument list of `c` for a new session with `allow`, with the MCP tokens `pin` placed before the permission
/// flags (where `chat_args` puts them).
fn with_pin(c: &Claude, allow: &AllowList, pin: &[&str]) -> Vec<String> {
    let mut plain = c.clone();
    plain.mcp_config = None;
    let mut a = plain.chat_args("--session-id", S1, allow);
    let p = a.iter().position(|x| x == "--permission-mode").unwrap();
    a.splice(p..p, strings(pin));
    a
}

/// The generated space of ADR 0126 T3 (14 adversarial fragments x 5 kinds, stepped 4-subsets).
fn t3_space() -> Vec<AllowList> {
    let frags = [
        "echo hi",
        "bypassPermissions",
        "--dangerously-skip-permissions",
        "auto",
        "acceptEdits",
        "*",
        "a,b",
        "x)",
        "note.txt",
        "cases/**",
        "..",
        "-p",
        "git status",
        "Grep",
    ];
    let mut cands = Vec::new();
    for f in frags {
        cands.extend([
            AllowRule::Tool(f.into()),
            AllowRule::Bash(f.into()),
            AllowRule::PowerShell(f.into()),
            AllowRule::Edit(f.into()),
            AllowRule::Read(f.into()),
        ]);
    }
    let n = cands.len();
    let mut out = Vec::new();
    for a in 0..n {
        for b in (a..n).step_by(3) {
            for c in (b..n).step_by(7) {
                for d in (c..n).step_by(11) {
                    let mut l = AllowList::new();
                    for &i in &[a, b, c, d] {
                        let _ = l.add(cands[i].clone());
                    }
                    out.push(l);
                }
            }
        }
    }
    out
}

/// E9: with the app-owned config the argv carries `--strict-mcp-config --mcp-config <file outside the project>`;
/// `check_args_in` refuses the flags with a file inside the project, without `--strict-mcp-config` and in the other
/// broken forms; the T3 space with pinning has no forbidden argument; `--mcp-config ./.mcp.json` is refused.
#[test]
fn e9_mcp_pinning_flags_and_checker() {
    let s = setup("e9", "stream_allowlist_dontask.jsonl");
    let cfg_dir = s.root.join("studio-config");
    std::fs::create_dir_all(&cfg_dir).unwrap();
    let cfg = cfg_dir.join("mcp.json");
    let rcfd_mcp = PathBuf::from(if cfg!(windows) {
        "C:\\tools\\rcfd-mcp.exe"
    } else {
        "/usr/local/bin/rcfd-mcp"
    });
    std::fs::write(&cfg, pinned_mcp_config(&rcfd_mcp, &s.project)).unwrap();
    let cfg_s = cfg.to_string_lossy().into_owned();
    let mut claude = s.claude.clone();
    claude.mcp_config = Some(cfg.clone());
    let allow = AllowList::project_initial();
    let chat = claude.new_session(&s.project, &allow, "hi").unwrap();
    let ev = collect(&chat);
    assert!(matches!(ev.last(), Some(StreamEvent::Exited { code: Some(0), .. })));
    let argv = &chat_argvs(&s.project)[0];
    assert_eq!(argv, chat.args());
    assert_eq!(has_window(argv, &["--strict-mcp-config", "--mcp-config", &cfg_s]), 1);
    assert_eq!(has_window(argv, &PERMISSION), 1);
    assert!(check_args_in(argv, &s.project).is_ok());
    // the config names exactly one server, without '-', started with the project as --project and --root
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
    let servers = v["mcpServers"].as_object().unwrap();
    assert_eq!(servers.keys().collect::<Vec<_>>(), [PINNED_SERVER]);
    assert!(!PINNED_SERVER.contains('-'));
    let p = s.project.to_string_lossy();
    assert_eq!(servers[PINNED_SERVER]["args"], json!(["--project", p, "--root", p]));
    assert_eq!(allow.rules().len(), PROJECT_READ_TOOLS.len());
    for r in allow.rules() {
        let name = r.to_flag_value();
        assert!(name.starts_with("mcp__rcfd_project__") && name.len() <= 128 && !name.contains('-'));
        assert!(r.validate().is_ok());
    }

    // files used by the refusals
    let inside = s.project.join(".mcp.json");
    std::fs::write(&inside, "{}").unwrap();
    std::fs::write(s.project.join("pinned.json"), "{}").unwrap();
    let lexical_inside = format!("{}{sep}sub{sep}..{sep}pinned.json", p, sep = std::path::MAIN_SEPARATOR);
    let sibling_dir = s.root.join("my-proj2");
    std::fs::create_dir_all(&sibling_dir).unwrap();
    let sibling = sibling_dir.join("pin.json");
    std::fs::write(&sibling, "{}").unwrap();
    let via_parent = s.project.join("..").join("studio-config").join("mcp.json");
    let missing = cfg_dir.join("missing.json");
    let inside_s = inside.to_string_lossy().into_owned();
    let mut refused: Vec<(&str, Vec<String>)> = vec![
        (
            "negative control ./.mcp.json",
            with_pin(&claude, &allow, &["--strict-mcp-config", "--mcp-config", "./.mcp.json"]),
        ),
        (
            "relative .mcp.json",
            with_pin(&claude, &allow, &["--strict-mcp-config", "--mcp-config", ".mcp.json"]),
        ),
        (
            "relative outside",
            with_pin(
                &claude,
                &allow,
                &["--strict-mcp-config", "--mcp-config", "../studio-config/mcp.json"],
            ),
        ),
        (
            "inside, absolute",
            with_pin(&claude, &allow, &["--strict-mcp-config", "--mcp-config", &inside_s]),
        ),
        (
            "inside, lexically",
            with_pin(
                &claude,
                &allow,
                &["--strict-mcp-config", "--mcp-config", &lexical_inside],
            ),
        ),
        (
            "without --strict-mcp-config",
            with_pin(&claude, &allow, &["--mcp-config", &cfg_s]),
        ),
        (
            "inline JSON",
            with_pin(
                &claude,
                &allow,
                &["--strict-mcp-config", "--mcp-config", "{\"mcpServers\":{}}"],
            ),
        ),
        (
            "= form of --mcp-config",
            with_pin(
                &claude,
                &allow,
                &["--strict-mcp-config", &format!("--mcp-config={cfg_s}")],
            ),
        ),
        (
            "= form of --strict-mcp-config",
            with_pin(&claude, &allow, &["--strict-mcp-config=true", "--mcp-config", &cfg_s]),
        ),
        (
            "two files",
            with_pin(
                &claude,
                &allow,
                &["--strict-mcp-config", "--mcp-config", &cfg_s, &inside_s],
            ),
        ),
        (
            "--mcp-config twice",
            with_pin(
                &claude,
                &allow,
                &["--strict-mcp-config", "--mcp-config", &cfg_s, "--mcp-config", &cfg_s],
            ),
        ),
        ("--mcp-config without a value", {
            let mut a = with_pin(&claude, &allow, &[]);
            a.extend(strings(&["--strict-mcp-config", "--mcp-config"]));
            a
        }),
        (
            "missing file",
            with_pin(
                &claude,
                &allow,
                &["--strict-mcp-config", "--mcp-config", &missing.to_string_lossy()],
            ),
        ),
    ];
    let upper_inside = format!("{}{}.mcp.json", p.to_ascii_uppercase(), std::path::MAIN_SEPARATOR);
    if cfg!(windows) {
        refused.push((
            "inside, other ASCII case (Windows)",
            with_pin(&claude, &allow, &["--strict-mcp-config", "--mcp-config", &upper_inside]),
        ));
    }
    for (what, args) in &refused {
        match check_args_in(args, &s.project) {
            Err(Error::ForbiddenArgument(why)) => println!("E9 refused ({what}): {why}"),
            r => panic!("E9 accepted ({what}): {r:?} {args:?}"),
        }
    }
    // accepted: a sibling folder whose name starts with the project's, and the config reached through '..'
    for ok in [
        sibling.to_string_lossy().into_owned(),
        via_parent.to_string_lossy().into_owned(),
    ] {
        let a = with_pin(&claude, &allow, &["--strict-mcp-config", "--mcp-config", &ok]);
        check_args_in(&a, &s.project).unwrap_or_else(|e| panic!("{ok}: {e}"));
    }
    // --strict-mcp-config alone (no server at all) is accepted
    assert!(check_args_in(&with_pin(&claude, &allow, &["--strict-mcp-config"]), &s.project).is_ok());
    // a config inside the project never reaches a process
    let s2 = setup("e9b", "stream_allowlist_dontask.jsonl");
    let inside2 = s2.project.join(".mcp.json");
    std::fs::write(&inside2, "{}").unwrap();
    let mut bad = s2.claude.clone();
    bad.mcp_config = Some(inside2);
    assert!(matches!(
        bad.new_session(&s2.project, &allow, "hi"),
        Err(Error::ForbiddenArgument(_))
    ));
    assert!(calls(&s2.project).is_empty(), "a process was started");

    // T3 of ADR 0126 with pinning: no generated list fails the new checker or carries a forbidden token
    let (mut lists, mut rules) = (0usize, 0usize);
    for l in t3_space() {
        let args = claude.chat_args("--session-id", S1, &l);
        check_args_in(&args, &s.project).unwrap_or_else(|e| panic!("{e}: {args:?}"));
        for x in &args {
            let low = x.to_ascii_lowercase();
            assert!(
                !low.contains("dangerously") && !low.contains("bypasspermissions"),
                "{x}"
            );
        }
        assert_eq!(has_window(&args, &PERMISSION), 1);
        assert_eq!(has_window(&args, &["--strict-mcp-config", "--mcp-config", &cfg_s]), 1);
        lists += 1;
        rules += l.rules().len();
    }
    println!("E9: T3 space with pinning: {lists} lists, {rules} rules, none refused");
    assert!(lists >= 5000, "{lists}");
    // and the 15 bad lists of T3 stay refused by the new checker
    let base = AllowList::new().permission_args();
    let w = |extra: &[&str]| {
        let mut v = base.clone();
        v.extend(strings(extra));
        v
    };
    let bad_lists = vec![
        w(&["--dangerously-skip-permissions"]),
        w(&["--allow-dangerously-skip-permissions"]),
        w(&["--permission-mode", "bypassPermissions"]),
        strings(&["--permission-mode", "auto", "--permission-prompts", "none"]),
        strings(&["--permission-mode", "acceptEdits", "--permission-prompts", "none"]),
        strings(&["--permission-mode=dontAsk", "--permission-prompts", "none"]),
        strings(&["--permission-mode", "dontAsk", "--permission-prompts", "host"]),
        strings(&["--permission-prompts", "none"]),
        w(&["--settings", "{}"]),
        w(&["--permission-prompt-tool", "mcp__x__y"]),
        w(&["--add-dir", "C:/"]),
        w(&["--allowedTools", "Bash"]),
        w(&["--allowedTools", "Edit(**)"]),
        w(&["--allowedTools", "Grep", "Bash(*)"]),
        w(&["--allowedTools=Grep,Bash"]),
    ];
    assert_eq!(bad_lists.len(), 15);
    for b in bad_lists {
        assert!(check_args_in(&b, &s.project).is_err(), "accepted {b:?}");
    }
}

/// Not registered: a config file outside the project that is a link to a file inside it is refused after resolving
/// links. Creating a link can need a privilege on Windows; the test then prints SKIP and passes.
#[test]
fn e9_link_into_the_project_is_refused() {
    let s = setup("e9l", "stream_allowlist_dontask.jsonl");
    let inside = s.project.join("x.json");
    std::fs::write(&inside, "{}").unwrap();
    let link = s.root.join("link.json");
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_file(&inside, &link);
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(&inside, &link);
    if let Err(e) = made {
        println!("SKIP e9_link_into_the_project_is_refused: cannot create a symbolic link here ({e})");
        return;
    }
    let mut c = s.claude.clone();
    c.mcp_config = Some(link);
    let args = c.chat_args("--session-id", S1, &AllowList::new());
    let r = check_args_in(&args, &s.project);
    assert!(
        matches!(&r, Err(Error::ForbiddenArgument(m)) if m.contains("resolves inside")),
        "{r:?}"
    );
}
