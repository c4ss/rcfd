//! ADR 0038 check P1: `unsafe` is confined to `rcfd-gpu/src/cuda/launch.rs`, and every block there has a SAFETY comment.

use std::path::{Path, PathBuf};

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                rs_files(&p, out);
            } else if p.extension().map(|x| x == "rs").unwrap_or(false) {
                out.push(p);
            }
        }
    }
}

/// Source with comments and string/char literals blanked out (newlines kept), good enough for a keyword scan.
fn strip(src: &str) -> String {
    let b: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == '/' && b.get(i + 1) == Some(&'/') {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && b.get(i + 1) == Some(&'*') {
            let mut depth = 1;
            i += 2;
            while i < b.len() && depth > 0 {
                if b[i] == '/' && b.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if b[i] == '*' && b.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                } else {
                    if b[i] == '\n' {
                        out.push('\n');
                    }
                    i += 1;
                }
            }
        } else if c == 'r'
            && (b.get(i + 1) == Some(&'"')
                || (b.get(i + 1) == Some(&'#') && matches!(b.get(i + 2), Some('"') | Some('#'))))
        {
            let mut j = i + 1;
            let mut hashes = 0;
            while b.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            j += 1; // opening quote
            loop {
                if j >= b.len() {
                    break;
                }
                if b[j] == '"' && (0..hashes).all(|k| b.get(j + 1 + k) == Some(&'#')) {
                    j += 1 + hashes;
                    break;
                }
                if b[j] == '\n' {
                    out.push('\n');
                }
                j += 1;
            }
            i = j;
        } else if c == '"' {
            i += 1;
            while i < b.len() && b[i] != '"' {
                if b[i] == '\\' {
                    i += 1;
                }
                if i < b.len() && b[i] == '\n' {
                    out.push('\n');
                }
                i += 1;
            }
            i += 1;
        } else if c == '\'' && b.get(i + 2) == Some(&'\'') {
            i += 3; // 'x'
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

fn unsafe_lines(src: &str) -> Vec<usize> {
    let s = strip(src);
    let mut lines = Vec::new();
    for (ln, line) in s.lines().enumerate() {
        let bytes = line.as_bytes();
        let mut from = 0;
        while let Some(pos) = line[from..].find("unsafe") {
            let at = from + pos;
            let before = at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
            let after_i = at + 6;
            let after = after_i >= bytes.len() || !(bytes[after_i].is_ascii_alphanumeric() || bytes[after_i] == b'_');
            if before && after {
                lines.push(ln + 1);
            }
            from = at + 6;
        }
    }
    lines
}

#[test]
fn p1_unsafe_is_confined_to_the_launch_module() {
    let crates_dir = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let launch = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("cuda")
        .join("launch.rs");
    let mut offenders = Vec::new();
    let mut n_files = 0;
    for entry in std::fs::read_dir(&crates_dir).unwrap().flatten() {
        let mut files = Vec::new();
        rs_files(&entry.path(), &mut files);
        for f in files {
            if f.components().any(|c| c.as_os_str() == "target") {
                continue;
            }
            n_files += 1;
            if f == launch {
                continue;
            }
            // this very file mentions the keyword only in strings and comments
            let src = std::fs::read_to_string(&f).unwrap();
            for ln in unsafe_lines(&src) {
                offenders.push(format!("{}:{ln}", f.display()));
            }
        }
    }
    println!("P1: scanned {n_files} Rust files under {}", crates_dir.display());
    assert!(offenders.is_empty(), "unsafe outside launch.rs: {offenders:?}");

    let src = std::fs::read_to_string(&launch).unwrap();
    let lines: Vec<&str> = src.lines().collect();
    let hits = unsafe_lines(&src);
    println!("P1: launch.rs has {} unsafe keyword occurrences", hits.len());
    assert!(!hits.is_empty());
    for ln in hits {
        // walk up over the comment block directly above the statement
        let mut k = ln - 1;
        let mut found = false;
        while k > 0 {
            let above = lines[k - 1].trim_start();
            if above.starts_with("//") {
                if above.contains("SAFETY:") {
                    found = true;
                }
                k -= 1;
            } else {
                break;
            }
        }
        assert!(
            found,
            "launch.rs line {ln}: unsafe without a SAFETY: comment directly above"
        );
    }
}
