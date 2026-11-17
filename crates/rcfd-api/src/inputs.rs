//! The files a case reads (ADR 0739 P4): what a run on another machine must carry with it, and what
//! `rcfd check <case> --inputs` lists.
//!
//! The rule follows the readers of the engine: `mesh.file` when `mesh.source = "file"` (a mesh file or an OpenFOAM
//! folder), `chemistry.mechanism` and `chemistry.thermo`, the `file` of every `tables.<name>`, the `file` of every
//! `udf.<name>` (WebAssembly modules), and `restart.from` with `restart.source_case` when the case asks for a restart.
//! Each is resolved as its reader resolves it: an absolute path as it is, a relative one against the case's
//! `base_dir`. A key that is empty is not an input. Nothing is read here.

use crate::case::Case;
use std::path::{Path, PathBuf};

/// One file (or folder) a case reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseInput {
    /// The case key that names it (`mesh.file`, `tables.t1.file`, ...).
    pub key: String,
    /// The value as the case writes it.
    pub file: String,
    /// Where the engine reads it.
    pub path: PathBuf,
}

fn s(case: &Case, key: &str) -> String {
    case.tree
        .get(key)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

/// Every file `case` reads, in a fixed order (mesh, chemistry, tables, user functions, restart).
pub fn case_inputs(case: &Case) -> Vec<CaseInput> {
    let mut keys: Vec<String> = Vec::new();
    if s(case, "mesh.source") == "file" {
        keys.push("mesh.file".into());
    }
    keys.push("chemistry.mechanism".into());
    keys.push("chemistry.thermo".into());
    for n in case.tree.names("tables").unwrap_or_default() {
        keys.push(format!("tables.{n}.file"));
    }
    for n in case.tree.names("udf").unwrap_or_default() {
        keys.push(format!("udf.{n}.file"));
    }
    if crate::restart::requested(case) {
        keys.push("restart.from".into());
        keys.push("restart.source_case".into());
    }
    keys.into_iter()
        .filter_map(|key| {
            let file = s(case, &key);
            if file.is_empty() {
                return None;
            }
            let path = if Path::new(&file).is_absolute() {
                PathBuf::from(&file)
            } else {
                case.base_dir.join(&file)
            };
            Some(CaseInput { key, file, path })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "title = \"t\"\n[mesh]\ngenerator = \"cavity_2d\"\nnx = 4\nny = 4\n[physics]\nnu = 0.01\n\
        [boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n";

    #[test]
    fn a_generator_case_reads_nothing_and_each_file_key_is_listed() {
        let dir = Path::new("base");
        let c = Case::parse_toml_in(BASE, dir).unwrap();
        assert!(case_inputs(&c).is_empty());
        let mut c = c;
        c.set("mesh.source", serde_json::json!("file")).unwrap();
        c.set("mesh.file", serde_json::json!("../m/a.msh")).unwrap();
        c.set("tables.t1.file", serde_json::json!("t.csv")).unwrap();
        c.set("tables.t1.inputs", serde_json::json!(["y"])).unwrap();
        c.set("udf.f.file", serde_json::json!("f.wasm")).unwrap();
        let got: Vec<(String, String, PathBuf)> =
            case_inputs(&c).into_iter().map(|i| (i.key, i.file, i.path)).collect();
        assert_eq!(
            got,
            vec![
                ("mesh.file".into(), "../m/a.msh".into(), dir.join("../m/a.msh")),
                ("tables.t1.file".into(), "t.csv".into(), dir.join("t.csv")),
                ("udf.f.file".into(), "f.wasm".into(), dir.join("f.wasm")),
            ]
        );
    }
}
