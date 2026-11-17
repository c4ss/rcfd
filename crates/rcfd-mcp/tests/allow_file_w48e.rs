//! ADR 0134 (wave 48E) check E5: the allow-list file's round trip and tampered files; and the rule checks T1, T2 and T5
//! of ADR 0126 and ADR 0128 finding 2. Moved here from `w48e_library.rs` when the chat driver and the other checks of
//! that file were removed with the local-model assistant (F-838). Its two comparisons of `permission_args`, which went
//! with the driver, compare the rules' flag values. E5, T1, T2 and T5 moved from `crates/rcfd-claude` (its
//! `tests/allow_file_w48e.rs` and the unit tests of `src/allow.rs`) when that crate was removed; the rules are now
//! `tests/claude_allow/`.

mod claude_allow;

use claude_allow::{AllowList, AllowRule, Error};
use std::path::PathBuf;

/// A fresh scratch folder for one test.
fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("rcfd-claude-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The rules' flag values, in order (the `--allowedTools` part of the removed `permission_args`).
fn flags(l: &AllowList) -> Vec<String> {
    l.rules().iter().map(AllowRule::to_flag_value).collect()
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
    assert_eq!(flags(&back), flags(&l));
    // a rule written with the './' anchor reads back as the same flag value
    let mut dotted = AllowList::new();
    dotted.add(AllowRule::Edit("./cases/**".into())).unwrap();
    let d = AllowList::from_file_bytes(&file, &dotted.to_file_bytes()).unwrap();
    assert_eq!(flags(&d), flags(&dotted));
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

/// Registered table T1 (ADR 0126): rule -> exact flag value.
fn table() -> Vec<(AllowRule, &'static str)> {
    vec![
        (AllowRule::Tool("Grep".into()), "Grep"),
        (AllowRule::Tool("mcp__rcfd__list_runs".into()), "mcp__rcfd__list_runs"),
        (AllowRule::Bash("echo hello".into()), "Bash(echo hello)"),
        (AllowRule::Bash("git status".into()), "Bash(git status)"),
        (AllowRule::Bash("cargo test *".into()), "Bash(cargo test *)"),
        (
            AllowRule::PowerShell("Get-ChildItem cases".into()),
            "PowerShell(Get-ChildItem cases)",
        ),
        (AllowRule::Edit("note.txt".into()), "Edit(./note.txt)"),
        (AllowRule::Edit("./cases/**".into()), "Edit(./cases/**)"),
        (
            AllowRule::Read("runs/*/metrics.json".into()),
            "Read(./runs/*/metrics.json)",
        ),
    ]
}

fn mismatches(map: fn(&AllowRule) -> String) -> usize {
    table().iter().filter(|(r, want)| map(r) != *want).count()
}

#[test]
fn t1_mapping_is_exact_and_a_wrong_mapping_is_caught() {
    assert_eq!(mismatches(AllowRule::to_flag_value), 0);
    // negative control: Edit rendered without the './' anchor (relative to an unknown base) must be caught
    fn wrong(r: &AllowRule) -> String {
        match r {
            AllowRule::Edit(p) => format!("Edit({p})"),
            o => o.to_flag_value(),
        }
    }
    assert_eq!(mismatches(wrong), 1); // Edit("note.txt"); Edit("./cases/**") already carries the anchor
    let mut l = AllowList::new();
    l.add(AllowRule::Bash("echo hello".into())).unwrap();
    l.add(AllowRule::Edit("note.txt".into())).unwrap();
    l.add(AllowRule::Edit("note.txt".into())).unwrap(); // duplicate ignored
    assert_eq!(
        l.rules().iter().map(AllowRule::to_flag_value).collect::<Vec<_>>(),
        ["Bash(echo hello)", "Edit(./note.txt)"]
    );
    for (r, s) in table() {
        assert_eq!(
            AllowRule::parse(s).map(|p| p.to_flag_value()),
            Some(r.to_flag_value()),
            "round trip {s}"
        );
    }
}

#[test]
fn t2_policy_rejects_broad_or_injecting_rules() {
    let bad = [
        AllowRule::Tool("Bash".into()),
        AllowRule::Tool("Edit".into()),
        AllowRule::Tool("Write".into()),
        AllowRule::Tool("Read".into()),
        AllowRule::Tool("*".into()),
        AllowRule::Tool("--dangerously-skip-permissions".into()),
        AllowRule::Bash("*".into()),
        AllowRule::Bash("* --help".into()),
        AllowRule::Bash("".into()),
        AllowRule::Bash("echo a),Bash(*".into()),
        AllowRule::Bash("echo a\nrm -rf x".into()),
        AllowRule::Bash(" echo".into()),
        AllowRule::Bash("-rf".into()),
        AllowRule::Edit("**".into()),
        AllowRule::Edit("./**/*".into()),
        AllowRule::Edit("/etc/passwd".into()),
        AllowRule::Edit("//c/x".into()),
        AllowRule::Edit("~/.ssh/x".into()),
        AllowRule::Edit("C:/x".into()),
        AllowRule::Edit("cases\\a.json".into()),
        AllowRule::Edit("../outside.txt".into()),
        AllowRule::Read("cases/../../x".into()),
    ];
    for r in bad {
        assert!(r.validate().is_err(), "accepted {r:?}");
        assert!(AllowList::new().add(r).is_err());
    }
}

/// T5 (ADR 0128 finding 2): rules never reach the files that configure Claude or the project. (Its part on
/// suggestions went with `suggest`, F-838.)
#[test]
fn t5_control_files_are_refused() {
    let names = [".mcp.json", "CLAUDE.md", ".claude", ".rcfd", "rcfd-project.json"];
    let mut forms = Vec::new();
    for n in names {
        for x in [n.to_string(), n.to_ascii_uppercase(), n.to_ascii_lowercase()] {
            forms.extend([
                x.clone(),
                format!("./{x}"),
                format!("{x}."),
                format!("cases/../{x}"),
                format!("a\\{x}"),
                format!("cases/{x}"),
            ]);
        }
    }
    for d in [".claude", ".rcfd", ".Claude", ".RCFD"] {
        forms.extend([
            format!("{d}/settings.json"),
            format!("{d}/**"),
            format!("./{d}/sub/f"),
            format!("{d}./x"),
        ]);
    }
    forms.extend(
        [
            "*.md",
            ".c*",
            "*.json",
            "rcfd-*",
            "**/CLAUDE.md",
            "CLAUDE~1.MD",
            ".//CLAUDE.md",
        ]
        .map(String::from),
    );
    let accepted: Vec<&String> = forms
        .iter()
        .filter(|f| {
            AllowRule::Edit((*f).clone()).validate().is_ok() || AllowRule::Read((*f).clone()).validate().is_ok()
        })
        .collect();
    assert!(accepted.is_empty(), "control-file rules accepted: {accepted:?}");
    // ordinary paths stay allowed
    for ok in [
        "cases/x.json",
        "notes/a.md",
        "./cases/**",
        "runs/*/metrics.json",
        "note.txt",
    ] {
        assert!(AllowRule::Edit(ok.into()).validate().is_ok(), "{ok}");
        assert!(AllowRule::Read(ok.into()).validate().is_ok(), "{ok}");
    }
    eprintln!("T5: {} control-file forms refused", forms.len());
}
