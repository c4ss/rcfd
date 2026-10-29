//! ADR 0155 (job 48U) H1: every case file of the repository is unchanged by `[mesh.transform]`. For each case file of
//! `cases/`, `example-projects/*/cases/`, `example-projects/*/meshes/` (the recipe), the test fixtures and each TOML
//! block of `docs/` that parses: the settings content hash, the bytes `Case::to_toml` and `Case::to_json_string` write
//! and the content hash of the mesh `build_mesh` builds equal the record R0, made by this test on the base code 6867e40
//! before any code change (`RCFD_W48U_RECORD=1` writes `tests/fixtures/case_hashes_6867e40.txt`). A file that does not
//! parse is recorded with the paths of its diagnostics (their texts list the known keys, which a new key extends).
//! The record also holds the base code's answer to a case with `[mesh.transform]` (R0's control: the key was unknown).
//! ADR 0163: the text columns are compared after removing `solver.corrector_control` when it holds its default `"fixed"`
//! (ADR 0156 added the key after R0; the settings hash omits it at its default, the written text lists it), with controls
//! that the test's serialisers equal the library's (C1), every recorded valid case holds the default (C2), the raw lines
//! differ from R0 in the text columns only (C3), and a non-default value is not normalised away (C4).

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

/// The text `Case::to_toml` writes, for a settings value.
fn toml_text(v: &serde_json::Value) -> String {
    toml::to_string_pretty(v).unwrap_or_else(|e| format!("# serialisation failed: {e}\n"))
}

/// The text `Case::to_json_string` writes, for a settings value.
fn json_text(v: &serde_json::Value) -> String {
    serde_json::to_string_pretty(v).unwrap()
}

/// ADR 0163 S1: the case's settings value without `solver.corrector_control` when (and only when) it holds its default
/// `"fixed"`. ADR 0156 added that key after R0 (6867e40); the settings hash omits it at its default, the written text lists
/// it. This is the one normalisation of the text columns; a non-default value is kept and so differs from R0.
fn without_default_corrector_control(c: &Case) -> serde_json::Value {
    let mut v = c.tree.to_json();
    if let Some(s) = v.get_mut("solver").and_then(|s| s.as_object_mut()) {
        if s.get("corrector_control").and_then(|x| x.as_str()) == Some("fixed") {
            s.remove("corrector_control");
        }
    }
    v
}

/// One source of the current code: its record line with the text columns normalised (ADR 0163 S1), the same line with
/// the raw text columns (as R0's recorder wrote them) and its `solver.corrector_control` value.
struct Entry {
    norm: String,
    raw: String,
    corrector_control: Option<String>,
}

fn src_of(l: &str) -> &str {
    l.split('\t').next().unwrap()
}

/// One record line of a parsed (or refused) case.
fn line(src: &str, parsed: Result<Case, Vec<rcfd_api::case::Diagnostic>>) -> Entry {
    match parsed {
        Ok(c) => {
            let raw = c.tree.to_json();
            // ADR 0163 C1: the test's serialisers write what the library writes.
            assert_eq!(toml_text(&raw), c.to_toml(), "C1 toml {src}");
            assert_eq!(json_text(&raw), c.to_json_string(), "C1 json {src}");
            let norm = without_default_corrector_control(&c);
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
            let mk = |v: &serde_json::Value| {
                format!(
                    "{src}\tvalid\tsettings {:016x}\ttoml {}\tjson {}\t{mesh}",
                    c.tree.content_hash(),
                    fnv(toml_text(v).as_bytes()),
                    fnv(json_text(v).as_bytes())
                )
            };
            Entry {
                norm: mk(&norm),
                raw: mk(&raw),
                corrector_control: c
                    .get("solver.corrector_control")
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string)),
            }
        }
        Err(d) => {
            let l = format!(
                "{src}\tinvalid at {}",
                d.iter().map(|x| x.path.clone()).collect::<Vec<_>>().join(",")
            );
            Entry {
                norm: l.clone(),
                raw: l,
                corrector_control: None,
            }
        }
    }
}

/// Every source (sorted by source).
fn current() -> Vec<Entry> {
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
    out.sort_by(|a, b| a.norm.cmp(&b.norm));
    out
}

/// H1 against R0. With `RCFD_W48U_RECORD=1` the record is written instead (R0, on the base code only; raw text columns).
/// Otherwise the text columns are compared after ADR 0163's normalisation (S1) with its controls C1 to C4.
#[test]
fn h1_case_files_are_unchanged_against_the_base_record() {
    let entries = current();
    let lines: Vec<String> = entries.iter().map(|e| e.norm.clone()).collect();
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(RECORD);
    if std::env::var("RCFD_W48U_RECORD").as_deref() == Ok("1") {
        let lines: Vec<String> = entries.iter().map(|e| e.raw.clone()).collect();
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
    // R0 was recorded on Windows. A generator's mesh calls the platform's libm (sin, cos, ...), whose last bits differ
    // between platforms (ADR 0137), so elsewhere the mesh hashes are printed, not compared; every other field is.
    let compare_meshes = cfg!(windows);
    let without_mesh = |l: &str| -> String {
        l.split('\t')
            .filter(|f| !f.starts_with("mesh "))
            .collect::<Vec<_>>()
            .join("\t")
    };
    let mut problems = Vec::new();
    for w in &want {
        let src = w.split('\t').next().unwrap();
        match lines.iter().find(|l| l.split('\t').next() == Some(src)) {
            Some(l) if l == w => {}
            Some(l) if !compare_meshes && without_mesh(l) == without_mesh(w) => {
                println!("H1 {src}: mesh differs from the Windows record, not compared here (ADR 0137): {l}");
            }
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
    // ADR 0163 C2 and C3: every recorded valid source holds the default `"fixed"`; count the recorded lines whose raw text
    // columns differ (and check that settings and mesh columns are equal raw, i.e. only the text columns differ).
    let mut raw_text_differs = 0;
    let mut raw_other_differs = Vec::new();
    for w in &want {
        let Some(e) = entries.iter().find(|e| src_of(&e.norm) == src_of(w)) else {
            continue;
        };
        if w.contains("\tvalid\t") {
            assert_eq!(e.corrector_control.as_deref(), Some("fixed"), "C2 {}", src_of(w));
        }
        if e.raw != *w {
            raw_text_differs += 1;
            let keep = |l: &str| -> String {
                l.split('\t')
                    .filter(|f| {
                        !f.starts_with("toml ")
                            && !f.starts_with("json ")
                            && (compare_meshes || !f.starts_with("mesh "))
                    })
                    .collect::<Vec<_>>()
                    .join("\t")
            };
            if keep(&e.raw) != keep(w) {
                raw_other_differs.push(src_of(w).to_string());
            }
        }
    }
    println!("H1 C3: {raw_text_differs} of {valid} recorded valid sources differ raw (text columns only), 0 after normalisation");
    assert!(raw_other_differs.is_empty(), "C3: {raw_other_differs:?}");
    // ADR 0163 C4: a non-default value is not normalised away (one recorded valid case file, set to adaptive).
    let w = want
        .iter()
        .find(|l| l.contains("\tvalid\t") && !src_of(l).contains(" block "))
        .expect("a recorded valid case file");
    let mut c = Case::load(repo().join(src_of(w))).expect("C4 case loads");
    c.set("solver.corrector_control", serde_json::json!("adaptive"))
        .expect("C4 set");
    let n = without_default_corrector_control(&c);
    let toml_col = format!("toml {}", fnv(toml_text(&n).as_bytes()));
    let json_col = format!("json {}", fnv(json_text(&n).as_bytes()));
    assert!(
        !w.split('\t').any(|f| f == toml_col) && !w.split('\t').any(|f| f == json_col),
        "C4: an adaptive copy of {} still matches R0's text columns",
        src_of(w)
    );
    println!(
        "H1 C4: {} with corrector_control = adaptive differs from R0 ({toml_col}, {json_col})",
        src_of(w)
    );
    let control = rec.lines().nth(1).unwrap_or_default();
    println!("H1 R0 {control}");
    assert!(
        control.contains("unknown setting 'transform'"),
        "R0 control: the base code refused the table: {control}"
    );
}
