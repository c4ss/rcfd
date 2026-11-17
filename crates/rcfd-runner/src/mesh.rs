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

#[cfg(test)]
mod tests {
    use super::*;

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
