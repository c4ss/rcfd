//! ADR 0134 (wave 48E) check E5: the allow-list file's round trip and tampered files. Moved here from
//! `w48e_library.rs` when the chat driver and the other checks of that file were removed with the local-model assistant
//! (F-838). Its two comparisons of `permission_args`, which went with the driver, compare the rules' flag values.
use rcfd_claude::{AllowList, AllowRule, Error};
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
