//! ADR 0128 finding 17 (c): no console program is started in non-test code of any crate except through a no-window
//! helper (`rcfd_runner::sys::console`, rcfd-claude's `console`). Scans `crates/*/src/**/*.rs`, the text of each file
//! before its first `#[cfg(test)]`, for `Command::new("<program>")` of the programs that open a console window.

use std::path::{Path, PathBuf};

const PROGRAMS: [&str; 6] = ["tasklist", "taskkill", "cmd", "powershell", "typeperf", "where"];

/// Lines (1-based) of `text` before its first `#[cfg(test)]` that start one of [`PROGRAMS`] directly.
fn direct_spawns(text: &str) -> Vec<(usize, String)> {
    let end = text.find("#[cfg(test)]").unwrap_or(text.len());
    text[..end]
        .lines()
        .enumerate()
        .filter(|(_, l)| {
            let l = l.replace(' ', "");
            PROGRAMS.iter().any(|p| {
                l.contains(&format!("Command::new(\"{p}\")")) || l.contains(&format!("Command::new(\"{p}.exe\")"))
            })
        })
        .map(|(i, l)| (i + 1, l.trim().to_string()))
        .collect()
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            rs_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn no_console_program_is_started_without_the_no_window_helper() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let mut files = Vec::new();
    for c in std::fs::read_dir(&crates).unwrap().flatten() {
        let src = c.path().join("src");
        if src.is_dir() {
            rs_files(&src, &mut files);
        }
    }
    let mut found = Vec::new();
    for f in &files {
        for (line, text) in direct_spawns(&std::fs::read_to_string(f).unwrap()) {
            found.push(format!("{}:{line}: {text}", f.display()));
        }
    }
    eprintln!(
        "F17 scan: {} source files, direct console spawns: {found:#?}",
        files.len()
    );
    assert!(files.len() > 100, "set-up: too few files under {}", crates.display());
    assert!(found.is_empty(), "start these through the no-window helper: {found:#?}");
    // negative control: the scanner finds a direct spawn, also as `.exe` and with spaces, and ignores test code
    let bad = "fn f() {\n    let c = Command::new(\"tasklist\");\n    let d = Command::new( \"cmd.exe\" );\n}\n";
    assert_eq!(direct_spawns(bad).iter().map(|x| x.0).collect::<Vec<_>>(), [2, 3]);
    assert!(direct_spawns(&format!("#[cfg(test)]\n{bad}")).is_empty());
}
