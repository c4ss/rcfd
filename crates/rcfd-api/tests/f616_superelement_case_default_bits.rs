//! F-616 (ADR 0785) D1 and S1: the bits of the solid case file's runs without the keys `superelements` and `reduction`,
//! and the JSON schema of the solid case file without those two tables, recorded on main fcc84a84 before any library line
//! of F-616 was written. Uses only the API of that commit.
//!
//! D1: six case files of `tests/solid_cases/` run by `run_solid_case_file`; per file the FNV-1a digests of the bytes of the
//! `.vtu`, of the report without its `outputs` (the paths of the run) and of the bits of the displacements (static) or of
//! the eigenvalues and modes (modal).
//! S1: the digest of `schema().to_json_schema()` after removing `superelements` and `reduction` from the top-level
//! `properties` and `required` (ADR 0785, amendment 1); the removal does nothing on main.

use rcfd_api::solid_case::{run_solid_case_file, schema, SolidOutcome};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// The FNV-1a hash of bytes.
fn fnv(bytes: impl IntoIterator<Item = u8>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn fnv_f64<'a>(values: impl IntoIterator<Item = &'a f64>) -> u64 {
    fnv(values.into_iter().flat_map(|x| x.to_bits().to_le_bytes()))
}

fn cases() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("solid_cases")
}

fn out_dir(tag: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("f616_default_bits")
        .join(tag);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The three digests of one case file: the `.vtu`, the report without `outputs`, the result's bits. For a refused case
/// file: the digest of its problems, then two zeros.
fn digests(name: &str) -> [u64; 3] {
    let dir = out_dir(name);
    let run = match run_solid_case_file(cases().join(format!("{name}.toml")), Some(&dir)) {
        Ok(run) => run,
        // A case file that is refused: one digest, of its problems as text (ADR 0785, amendment 2).
        Err(e) => {
            let text: Vec<String> = e
                .iter()
                .map(|d| format!("{} | {:?} | {}", d.path, d.line, d.message))
                .collect();
            println!("D1 {name} is refused: {text:?}");
            return [fnv(text.join("; ").bytes()), 0, 0];
        }
    };
    let vtu = std::fs::read(dir.join(&run.setup.vtu)).unwrap();
    let mut report = run.report.clone();
    report.as_object_mut().unwrap().remove("outputs");
    let report = serde_json::to_string_pretty(&report).unwrap();
    let bits = match &run.outcome {
        SolidOutcome::Static { result, .. } => fnv_f64(result.displacement_values()),
        SolidOutcome::Modal(m) => fnv_f64(m.eigenvalues.iter().chain(m.modes.iter().flatten())),
    };
    [fnv(vtu), fnv(report.bytes()), bits]
}

/// (case file, `.vtu`, report, bits) on main fcc84a84; `cantilever_tet10_amg` is refused there (its problems, 0, 0).
const D1: [(&str, [u64; 3]); 6] = [
    ("bar_hex8", [0x8768e683116457a9, 0x40ce1f2fb23e31b0, 0x8957dfc90f121d60]),
    (
        "cantilever_hex8",
        [0x03d6666f4abe98b2, 0x9f6c3d15208cdf87, 0x746d93c64ab28c07],
    ),
    (
        "cantilever_tet10",
        [0xcdf4cbb6f54e636b, 0xaeec65c16dd86b01, 0x3d04841997e4a436],
    ),
    ("cantilever_tet10_amg", [0xc375f1ca5c8249e0, 0, 0]),
    (
        "gmsh_uniaxial",
        [0xaa42e24872ef0ea8, 0x49c5c86505b7f4f4, 0xe47582961b5fcf59],
    ),
    (
        "box_exact_bounds",
        [0xe6eef6a7466fe599, 0x65780de0b7f260a1, 0x5e8b20ab69c8bcd5],
    ),
];

/// The schema digest on main fcc84a84.
const S1: u64 = 0xc611_735a_b888_57f7;

#[test]
fn d1_case_files_without_the_keys() {
    let mut failed = Vec::new();
    for (name, want) in D1 {
        let got = digests(name);
        println!(
            "D1 (\"{name}\", [{:#018x}, {:#018x}, {:#018x}]),",
            got[0], got[1], got[2]
        );
        if got != want {
            failed.push(name);
        }
    }
    assert!(failed.is_empty(), "D1 digests differ: {failed:?}");
}

/// The JSON schema without the two tables of F-616.
fn schema_without_the_keys() -> Value {
    let mut v = schema().to_json_schema();
    for key in ["superelements", "reduction"] {
        v["properties"].as_object_mut().unwrap().remove(key);
        v["required"].as_array_mut().unwrap().retain(|x| x != key);
    }
    v
}

#[test]
fn s1_schema_without_the_keys() {
    let text = serde_json::to_string(&schema_without_the_keys()).unwrap();
    let got = fnv(text.bytes());
    println!("S1 {got:#018x} ({} bytes)", text.len());
    // For the report of the additive diff: the whole schema as text, when a path is given.
    if let Ok(p) = std::env::var("F616_SCHEMA_OUT") {
        std::fs::write(p, serde_json::to_string_pretty(&schema().to_json_schema()).unwrap()).unwrap();
    }
    assert_eq!(got, S1, "S1 schema digest differs");
}
