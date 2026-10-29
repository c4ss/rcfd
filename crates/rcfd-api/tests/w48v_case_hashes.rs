//! ADR 0156 N3 (job 48V): the case hash (`SettingsTree::content_hash`, the `case_hash` of the `start` event) of every shipped
//! case file, printed one line per file (`<path> <hash>`), so that the hashes of the base commit and of this branch can be
//! compared. The file uses only the API of the base commit: it is copied unchanged into a build of that commit.

use rcfd_api::case::Case;
use std::path::{Path, PathBuf};

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().is_some_and(|x| x == "toml" || x == "json") {
            out.push(p);
        }
    }
}

/// `cases/**/*.{toml,json}` and `example-projects*/*/cases/*.{toml,json}`, as `rcfd run <file>` reads them (`Case::load`).
#[test]
#[ignore = "ADR 0156 N3 tool: prints the case hash of every shipped case file (W48V_N3_OUT=<file> also writes them)"]
fn w48v_n3_print_case_hashes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let mut files = Vec::new();
    walk(&root.join("cases"), &mut files);
    for tree in ["example-projects", "example-projects-linux"] {
        let Ok(rd) = std::fs::read_dir(root.join(tree)) else {
            continue;
        };
        for e in rd.flatten() {
            walk(&e.path().join("cases"), &mut files);
        }
    }
    let mut lines: Vec<String> = files
        .iter()
        .map(|f| {
            let rel = f.strip_prefix(&root).unwrap_or(f).to_string_lossy().replace('\\', "/");
            match Case::load(f) {
                Ok(c) => format!("{rel} {:016x}", c.tree.content_hash()),
                Err(d) => format!(
                    "{rel} ERROR {}",
                    d.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
                ),
            }
        })
        .collect();
    lines.sort();
    let text = lines.join("\n") + "\n";
    print!("{text}");
    println!("N3: {} case files", lines.len());
    if let Some(out) = std::env::var_os("W48V_N3_OUT") {
        std::fs::write(out, &text).unwrap();
    }
    assert!(lines.len() >= 20, "under-powered: {} files", lines.len());
}
