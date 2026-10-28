//! ADR 0155 (job 48U) H1: every case file of the repository is unchanged by `[mesh.transform]`. For each case file of
//! `cases/`, `example-projects/*/cases/`, `example-projects/*/meshes/` (the recipe), the test fixtures and each TOML
//! block of `docs/` that parses: the settings content hash, the bytes `Case::to_toml` and `Case::to_json_string` write
//! and the content hash of the mesh `build_mesh` builds equal the record R0, made by this test on the base code 6867e40
//! before any code change (`RCFD_W48U_RECORD=1` writes `tests/fixtures/case_hashes_6867e40.txt`). A file that does not
//! parse is recorded with the paths of its diagnostics (their texts list the known keys, which a new key extends).
//! The record also holds the base code's answer to a case with `[mesh.transform]` (R0's control: the key was unknown).

use rcfd_api::case::Case;
use std::path::{Path, PathBuf};

const RECORD: &str = "tests/fixtures/case_hashes_6867e40.txt";

/// A case with the table, the text of R0's control.
const WITH_TABLE: &str = "[mesh]\ngenerator = \"cavity_2d\"\nnx = 4\nny = 4\n\n[mesh.transform]\nrotate_z_deg = -4.0\nrotate_origin = [0.25, 0.0, 0.0]\n";

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn fnv(b: &[u8]) -> String {
    format!("{:016x}", rcfd_core::hash::fnv64(b))
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut es: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    es.sort();
    for p in es {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        if p.is_dir() {
            if name != "runs" && name != "target" && name != "reference" {
                walk(&p, out);
            }
        } else if name.ends_with(".toml") || name.ends_with(".json") {
            out.push(p);
        }
    }
}

/// Case files: `cases/`, the example projects' `cases/` and `meshes/`, every crate's `tests/fixtures/`.
fn case_files() -> Vec<PathBuf> {
    let r = repo();
    let mut out = Vec::new();
    walk(&r.join("cases"), &mut out);
    for p in ["quick-start", "wind-tunnel"] {
        walk(&r.join("example-projects").join(p).join("cases"), &mut out);
        walk(&r.join("example-projects").join(p).join("meshes"), &mut out);
    }
    let mut crates: Vec<PathBuf> = std::fs::read_dir(r.join("crates"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    crates.sort();
    for c in crates {
        walk(&c.join("tests").join("fixtures"), &mut out);
    }
    out
}

/// The fenced TOML blocks of `docs/**/*.md`: (file, block text).
fn doc_blocks() -> Vec<(PathBuf, String)> {
    let mut files = Vec::new();
    fn md(dir: &Path, out: &mut Vec<PathBuf>) {
        let mut es: Vec<PathBuf> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .collect();
        es.sort();
        for p in es {
            if p.is_dir() {
                md(&p, out);
            } else if p.extension().is_some_and(|e| e == "md") {
                out.push(p);
            }
        }
    }
    md(&repo().join("docs"), &mut files);
    let mut out = Vec::new();
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        let mut block: Option<String> = None;
        for line in text.lines() {
            match &mut block {
                None if line.trim_start().starts_with("```toml") => block = Some(String::new()),
                Some(b) if line.trim_start().starts_with("```") => {
                    out.push((f.clone(), std::mem::take(b)));
                    block = None;
                }
                Some(b) => {
                    b.push_str(line);
                    b.push('\n');
                }
                None => {}
            }
        }
    }
    out
}

fn rel(p: &Path) -> String {
    let r = repo().canonicalize().unwrap();
    let c = p.canonicalize().unwrap();
    c.strip_prefix(&r).unwrap().to_string_lossy().replace('\\', "/")
}

/// Cells a case's generator counts give (`nx ny nz`, at least 1 each), to skip a mesh too large to build in a test.
fn est_cells(c: &Case) -> u64 {
    let n = |k: &str| c.get(k).ok().and_then(|v| v.as_u64()).unwrap_or(1).max(1);
    n("mesh.nx") * n("mesh.ny") * n("mesh.nz")
}

/// One record line of a parsed (or refused) case.
fn line(src: &str, parsed: Result<Case, Vec<rcfd_api::case::Diagnostic>>) -> String {
    match parsed {
        Ok(c) => {
            let mesh = if est_cells(&c) > 400_000 {
                "mesh skipped (large)".to_string()
            } else {
                match rcfd_api::sim::build_mesh(&c) {
                    Ok(m) => format!("mesh {:016x} cells {}", m.content_hash(), m.n_cells()),
                    Err(d) => format!(
                        "mesh refused at {}",
                        d.iter().map(|x| x.path.clone()).collect::<Vec<_>>().join(",")
                    ),
                }
            };
            format!(
                "{src}\tvalid\tsettings {:016x}\ttoml {}\tjson {}\t{mesh}",
                c.tree.content_hash(),
                fnv(c.to_toml().as_bytes()),
                fnv(c.to_json_string().as_bytes())
            )
        }
        Err(d) => format!(
            "{src}\tinvalid at {}",
            d.iter().map(|x| x.path.clone()).collect::<Vec<_>>().join(",")
        ),
    }
}

/// Every record line (sorted by source).
fn current() -> Vec<String> {
    let mut out = Vec::new();
    for f in case_files() {
        out.push(line(&rel(&f), Case::load(&f)));
    }
    for (f, text) in doc_blocks() {
        let parsed = Case::parse_toml_in(&text, f.parent().unwrap());
        if parsed.is_ok() {
            out.push(line(&format!("{} block {}", rel(&f), fnv(text.as_bytes())), parsed));
        }
    }
    out.sort();
    out
}

/// H1 against R0. With `RCFD_W48U_RECORD=1` the record is written instead (R0, on the base code only).
#[test]
fn h1_case_files_are_unchanged_against_the_base_record() {
    let lines = current();
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(RECORD);
    if std::env::var("RCFD_W48U_RECORD").as_deref() == Ok("1") {
        let control = match Case::parse_toml(WITH_TABLE) {
            Ok(c) => format!("accepted, settings {:016x}", c.tree.content_hash()),
            Err(d) => d
                .iter()
                .map(|x| format!("{}: {}", x.path, x.message))
                .collect::<Vec<_>>()
                .join(" | "),
        };
        let mut text = format!(
            "# ADR 0155 R0: written by case_hashes_w48u.rs on the base code (RCFD_W48U_RECORD=1), {} lines\n# control [mesh.transform]: {control}\n",
            lines.len()
        );
        for l in &lines {
            text.push_str(l);
            text.push('\n');
        }
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        println!("R0 written: {} lines to {}", lines.len(), path.display());
        return;
    }
    let rec = std::fs::read_to_string(&path).expect("the R0 record");
    let want: Vec<&str> = rec.lines().filter(|l| !l.starts_with('#')).collect();
    let mut problems = Vec::new();
    for w in &want {
        let src = w.split('\t').next().unwrap();
        match lines.iter().find(|l| l.split('\t').next() == Some(src)) {
            Some(l) if l == w => {}
            Some(l) => problems.push(format!("{src}: now\n  {l}\nrecorded\n  {w}")),
            None => problems.push(format!("{src}: recorded, not found now")),
        }
    }
    let valid = want.iter().filter(|l| l.contains("\tvalid\t")).count();
    let meshes = want.iter().filter(|l| l.contains("\tmesh ")).count();
    println!(
        "H1: {} recorded sources ({valid} valid, {meshes} meshes built), {} now, {} differ",
        want.len(),
        lines.len(),
        problems.len()
    );
    for l in lines
        .iter()
        .filter(|l| !want.iter().any(|w| w.split('\t').next() == l.split('\t').next()))
    {
        println!("H1 not in the record (new): {l}");
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    assert!(valid >= 20 && meshes >= 20, "set-up: {valid} valid, {meshes} meshes");
    let control = rec.lines().nth(1).unwrap_or_default();
    println!("H1 R0 {control}");
    assert!(
        control.contains("unknown setting 'transform'"),
        "R0 control: the base code refused the table: {control}"
    );
}
