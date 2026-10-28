//! Shared set-up of ADR 0152 (job 48R) DV6: the unedited cases whose start plans are compared with the record made on
//! 6867e40, the saved control, and the text of the record. Only API that existed on 6867e40 is used here, so that the
//! recorder (`plans_record_r.rs`) compiled there. Test files declare `mod common; mod g4; mod dv;`.
#![allow(dead_code)]

use rcfd_api::case::Case;
use rcfd_studio::runs::StartPlan;
use std::path::{Path, PathBuf};

/// One case of the record: its label, the project-relative path the plan is made with, its text, and the folder its
/// relative files resolve against.
#[derive(Debug, Clone)]
pub struct Entry {
    pub label: String,
    pub rel: String,
    pub text: String,
    pub base: PathBuf,
}

/// The repository root.
pub fn repo() -> PathBuf {
    rcfd_project::paths::canonicalize(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap()
}

/// `.toml` files under `dir` (recursively), as entries labelled and planned by their repository-relative path, sorted.
fn toml_files(dir: &Path) -> Vec<Entry> {
    let root = repo();
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().map(|e| e.unwrap()) {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "toml") {
                let rel = p.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
                out.push(Entry {
                    label: rel.clone(),
                    rel,
                    text: std::fs::read_to_string(&p).unwrap(),
                    base: p.parent().unwrap().to_path_buf(),
                });
            }
        }
    }
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
}

/// The 22 unedited cases of DV6: the ten files under `cases/` (the case starter's ten examples are these files), the ten
/// cases of `example-projects/*/cases/`, `common::RUN_CASE` and `common::CAVITY`.
pub fn unedited() -> Vec<Entry> {
    let root = repo();
    let mut v = toml_files(&root.join("cases"));
    let mut projects: Vec<PathBuf> = std::fs::read_dir(root.join("example-projects"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("cases").is_dir())
        .collect();
    projects.sort();
    for p in projects {
        v.extend(toml_files(&p.join("cases")));
    }
    for (label, text) in [
        ("common::RUN_CASE", crate::common::RUN_CASE),
        ("common::CAVITY", crate::common::CAVITY),
    ] {
        v.push(Entry {
            label: label.into(),
            rel: "cases/cavity.toml".into(),
            text: text.into(),
            base: PathBuf::from("."),
        });
    }
    v
}

/// The text a save writes for the 30-iteration cavity with `physics.nu` 0.02: `rcfd-api`'s writer (`Case::to_toml`),
/// every key spelled out, the store keys at their defaults.
pub fn saved_text() -> String {
    let mut c = Case::parse_toml(&crate::g4::cavity(30)).unwrap();
    c.set("physics.nu", serde_json::json!(0.02)).unwrap();
    c.to_toml()
}

/// DV6's control: [`saved_text`] as `cases/s.toml`.
pub fn saved_control() -> Entry {
    Entry {
        label: "saved control: g4::cavity(30), physics.nu 0.02, Case::to_toml".into(),
        rel: "cases/s.toml".into(),
        text: saved_text(),
        base: PathBuf::from("."),
    }
}

/// The record's text of one plan: its label, the `Debug` text of the plan (or of the refusal) and of its `set()`.
pub fn plan_text(e: &Entry, plan: &Result<StartPlan, String>) -> String {
    format!(
        "== {}\nplan: {:?}\nset: {:?}\n",
        e.label,
        plan,
        plan.as_ref().map(|p| p.set())
    )
}

/// The record's text of `entries`, each planned by `StartPlan::new` (the app's route).
pub fn record(entries: &[Entry]) -> String {
    entries
        .iter()
        .map(|e| plan_text(e, &StartPlan::new(&e.rel, &e.text, &e.base)))
        .collect()
}

/// The record made on 6867e40 (`tests/fixtures/plans_6867e40.txt`), line endings as LF.
pub fn recorded() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plans_6867e40.txt"))
        .unwrap()
        .replace("\r\n", "\n")
}

/// The blocks of a record (each starts with a line `== <label>`), as (label, the block's text).
pub fn blocks(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in text.split_inclusive('\n') {
        match line.strip_prefix("== ") {
            Some(label) => out.push((label.trim_end().to_string(), line.to_string())),
            None => {
                if let Some((_, b)) = out.last_mut() {
                    b.push_str(line);
                }
            }
        }
    }
    out
}
