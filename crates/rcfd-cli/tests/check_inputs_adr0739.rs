//! ADR 0739 S10: `rcfd check <case> --inputs --json` lists every file a case reads, each existing, with the hash and
//! size `rcfd_project::filehash` gives it (the names of the site's content-addressed objects). Every shipped case under
//! `cases/`, a case with a table, a `.wasm` user function and a mesh file, and a case whose input is missing (exit 3,
//! naming it).

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn rcfd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rcfd"))
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd-check-inputs-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The JSON of `rcfd check <case> --inputs --json` and its exit code.
fn inputs(case: &Path) -> (Value, i32) {
    let o = rcfd()
        .arg("check")
        .arg(case)
        .args(["--inputs", "--json"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    let v: Value = serde_json::from_str(&text).unwrap_or_else(|e| {
        panic!(
            "{}: not JSON ({e}): {text}\nstderr: {}",
            case.display(),
            String::from_utf8_lossy(&o.stderr)
        )
    });
    (v, o.status.code().unwrap_or(-1))
}

/// Each listed input exists, with the hash and size `filehash` gives it, and each of its files too.
fn check_listed(case: &Path, v: &Value) -> usize {
    let list = v["inputs"].as_array().unwrap();
    for i in list {
        let p = PathBuf::from(i["path"].as_str().unwrap());
        assert!(p.exists(), "{}: {} is listed but missing", case.display(), p.display());
        let h = rcfd_project::filehash::hash_path(&p).unwrap();
        assert_eq!(i["hash"], h.hash.as_str(), "{}: {}", case.display(), i["key"]);
        assert_eq!(i["bytes"], h.bytes, "{}: {}", case.display(), i["key"]);
        for f in i["files"].as_array().unwrap() {
            let rel = f["path"].as_str().unwrap();
            let fp = if rel.is_empty() { p.clone() } else { p.join(rel) };
            let fh = rcfd_project::filehash::hash_path(&fp).unwrap();
            assert_eq!(f["hash"], fh.hash.as_str(), "{}", fp.display());
            assert_eq!(f["bytes"], fh.bytes, "{}", fp.display());
        }
    }
    list.len()
}

#[test]
fn s10_every_shipped_case_lists_existing_inputs_with_their_hashes() {
    let mut cases = Vec::new();
    let mut stack = vec![repo().join("cases")];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "toml") {
                cases.push(p);
            }
        }
    }
    cases.sort();
    assert!(cases.len() >= 10, "the shipped cases: {}", cases.len());
    let mut listed = 0;
    for c in &cases {
        let (v, code) = inputs(c);
        assert_eq!(code, 0, "{}: {v}", c.display());
        assert_eq!(v["ok"], true);
        listed += check_listed(c, &v);
    }
    // profile_inlet reads a CSV table: at least that one
    assert!(listed >= 1);
    eprintln!(
        "S10: {} shipped cases, {listed} inputs, every one present with its filehash hash",
        cases.len()
    );
}

#[test]
fn s10_a_table_a_wasm_function_and_a_mesh_file_are_listed() {
    let d = scratch("udf");
    std::fs::create_dir_all(d.join("meshes")).unwrap();
    std::fs::write(d.join("t.csv"), "y,u\n0,0\n1,1\n").unwrap();
    let m = format!(
        "(module (func (export \"rcfd_abi_version\") (result i32) i32.const {}) (func (export \"momentum_source\") \
         (param f64 f64 f64 f64 f64 f64 f64 i32) (result f64) f64.const 0.0))",
        rcfd_api::udf::ABI_VERSION
    );
    std::fs::write(d.join("f.wasm"), wat::parse_str(m).unwrap()).unwrap();
    std::fs::write(d.join("meshes/m.msh"), "(0 \"not read by check --inputs\")\n").unwrap();
    let case = d.join("c.toml");
    std::fs::write(
        &case,
        "title = \"s10\"\n[mesh]\nsource = \"file\"\nfile = \"meshes/m.msh\"\n[physics]\nnu = 0.01\n\
         [tables.t1]\nfile = \"t.csv\"\ninputs = [\"y\"]\n[udf.f]\nfile = \"f.wasm\"\nmomentum_source = true\n",
    )
    .unwrap();
    let (v, code) = inputs(&case);
    assert_eq!(code, 0, "{v}");
    assert_eq!(check_listed(&case, &v), 3);
    let keys: Vec<&str> = v["inputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["key"].as_str().unwrap())
        .collect();
    assert_eq!(keys, ["mesh.file", "tables.t1.file", "udf.f.file"]);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn s10_a_missing_input_is_exit_3_and_named() {
    let d = scratch("missing");
    // a mesh file the case loads without reading: listed with its error
    let case = d.join("c.toml");
    std::fs::write(
        &case,
        "title = \"s10\"
[mesh]
source = \"file\"
file = \"gone.msh\"
[physics]
nu = 0.01
",
    )
    .unwrap();
    let o = rcfd()
        .arg("check")
        .arg(&case)
        .args(["--inputs", "--json"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(3));
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["inputs"][0]["key"], "mesh.file");
    assert!(v["inputs"][0]["error"].is_string());
    assert!(String::from_utf8_lossy(&o.stderr).contains("mesh.file"));
    // a user function the case reads when it loads: exit 3 too, named on stderr
    std::fs::write(
        &case,
        "title = \"s10\"
[mesh]
generator = \"cavity_2d\"
nx = 4
ny = 4
[physics]
nu = 0.01
         [udf.f]
file = \"gone.wasm\"
momentum_source = true
",
    )
    .unwrap();
    let o = rcfd()
        .arg("check")
        .arg(&case)
        .args(["--inputs", "--json"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&o.stderr).contains("udf.f.file"));
    let _ = std::fs::remove_dir_all(&d);
}
