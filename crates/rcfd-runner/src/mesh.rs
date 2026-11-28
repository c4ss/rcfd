//! The mesh file a case reads (ADR 0734 P4, Q4 (a)): `mesh.file` of a case whose `mesh.source` is `"file"`, after the
//! run's `--set` entries, resolved against the case's folder as `rcfd run --case-dir` resolves it. A generator case has
//! none, so its `run.json` keeps `mesh` and `mesh_hash` absent.
//!
//! The case is read as TOML or JSON only for these two keys; whatever else it says is the solver's business.

use serde_json::Value;
use std::path::{Path, PathBuf};

/// The case text as JSON: by the case's extension (`.json` JSON, else TOML), `None` when it does not parse.
fn case_value(text: &[u8], name: &Path) -> Option<Value> {
    let text = std::str::from_utf8(text).ok()?;
    let json = name.extension().is_some_and(|e| e.eq_ignore_ascii_case("json"));
    if json {
        serde_json::from_str(text).ok()
    } else {
        let t: toml::Table = toml::from_str(text).ok()?;
        serde_json::to_value(t).ok()
    }
}

/// The value a `--set path=value` entry gives (`rcfd_api::chain::apply_set`: JSON, else the bare text).
fn set_value(v: &str) -> Value {
    let v = v.trim();
    serde_json::from_str(v).unwrap_or_else(|_| Value::String(v.to_string()))
}

/// The mesh file or folder of the case `text` (file name `name`, for its format) with the run's `set` entries, its
/// relative path joined to `case_dir`; `None` for a generator case or a case that does not read.
pub fn case_mesh(text: &[u8], name: &Path, case_dir: &Path, set: &[String]) -> Option<PathBuf> {
    let case = case_value(text, name)?;
    let mut source = case["mesh"]["source"].clone();
    let mut file = case["mesh"]["file"].clone();
    for e in set {
        match e.split_once('=') {
            Some((k, v)) if k.trim() == "mesh.source" => source = set_value(v),
            Some((k, v)) if k.trim() == "mesh.file" => file = set_value(v),
            _ => {}
        }
    }
    if source.as_str() != Some("file") {
        return None;
    }
    let file = file.as_str().filter(|f| !f.is_empty())?;
    let p = Path::new(file);
    Some(if p.is_absolute() {
        p.to_path_buf()
    } else {
        case_dir.join(p)
    })
}

/// ADR 0840 D1: whether case text `text` (file name `name`, for its format) is a solid case file: read as
/// [`case_value`] reads it, it has a top-level key `materials`, which the solid schema has and the flow schema has not
/// (so `rcfd run` refuses every such text as an unknown setting).
pub fn solid_case(text: &[u8], name: &Path) -> bool {
    case_value(text, name).is_some_and(|v| v.get("materials").is_some())
}

/// The program of the solid runs of ADR 0840 D2: `rcfd-solid-run` next to the current executable (as `rcfd` is).
pub const SOLID_DRIVER: &str = "rcfd-solid-run";

#[cfg(test)]
mod tests {
    use super::*;

    /// ADR 0840 R1: the 9 solid case files of the repository are solid; the 13 case files of example-projects are not.
    #[test]
    fn r1_f990_solid_case_files_are_told_from_flow_cases() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut solid = 0;
        for e in std::fs::read_dir(repo.join("crates/rcfd-api/tests/solid_cases"))
            .unwrap()
            .flatten()
        {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "toml") {
                assert!(solid_case(&std::fs::read(&p).unwrap(), &p), "{} is solid", p.display());
                solid += 1;
            }
        }
        let mut flow = 0;
        for proj in std::fs::read_dir(repo.join("example-projects")).unwrap().flatten() {
            let Ok(cases) = std::fs::read_dir(proj.path().join("cases")) else {
                continue;
            };
            for e in cases.flatten() {
                let p = e.path();
                assert!(
                    !solid_case(&std::fs::read(&p).unwrap(), &p),
                    "{} is not solid",
                    p.display()
                );
                flow += 1;
            }
        }
        println!("R1: {solid} solid, {flow} flow");
        assert_eq!((solid, flow), (9, 13), "R1 counts");
    }

    #[test]
    fn only_a_file_mesh_names_one_and_set_entries_apply() {
        let d = Path::new("/p/cases");
        let gen = b"[mesh]\nnx = 4\n";
        assert_eq!(case_mesh(gen, Path::new("a.toml"), d, &[]), None);
        let file = b"[mesh]\nsource = \"file\"\nfile = \"../meshes/a.msh\"\n";
        assert_eq!(
            case_mesh(file, Path::new("a.toml"), d, &[]),
            Some(d.join("../meshes/a.msh"))
        );
        assert_eq!(
            case_mesh(file, Path::new("a.toml"), d, &["mesh.file=\"b.msh\"".into()]),
            Some(d.join("b.msh"))
        );
        assert_eq!(
            case_mesh(
                gen,
                Path::new("a.toml"),
                d,
                &["mesh.source=file".into(), "mesh.file=c.msh".into()]
            ),
            Some(d.join("c.msh"))
        );
        let json = br#"{"mesh": {"source": "file", "file": "m"}}"#;
        assert_eq!(case_mesh(json, Path::new("a.json"), d, &[]), Some(d.join("m")));
        assert_eq!(case_mesh(b"not a case [", Path::new("a.toml"), d, &[]), None);
    }
}
