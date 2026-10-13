//! `rcfd data` (F-125): offline listing, verification and provenance of the public data pack.

use std::process::Command;

fn rcfd(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd"))
        .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn data_list_verify_and_provenance_work_offline() {
    let o = rcfd(&["data", "list"]);
    assert_eq!(o.status.code(), Some(0));
    let t = String::from_utf8_lossy(&o.stdout);
    assert!(
        t.contains("flat-plate-tmr") && t.contains("CC0-1.0") && t.contains("unresolved"),
        "{t}"
    );
    // fetched files may be absent (exit 0, "not fetched"); in-repo tables must verify
    let o = rcfd(&["data", "verify", "--data", "no-such-dir"]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let t = String::from_utf8_lossy(&o.stdout);
    assert!(t.contains("[cavity-ghia1982] ok") && t.contains("not fetched"), "{t}");
    let o = rcfd(&["data", "provenance"]);
    let t = String::from_utf8_lossy(&o.stdout);
    assert!(t.contains("# Validation data provenance") && t.contains("GPL-3.0-only"));
    // the case registry shows the data behind each case
    let o = rcfd(&["cases", "list"]);
    assert!(String::from_utf8_lossy(&o.stdout).contains("data: naca0012-tmr"));
    // a broken manifest is rejected (exit 3), a missing one fails (exit 1)
    assert_eq!(
        rcfd(&["data", "list", "--manifest", "nope.json"]).status.code(),
        Some(1)
    );
}
