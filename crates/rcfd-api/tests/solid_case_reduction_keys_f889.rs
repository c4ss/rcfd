//! F-889 (ADR 0797): the keys `cutoff`, `residual_vectors` and `load_patterns` of the table `[reduction]` of the solid case
//! file. The rows D1 and S1 are records of main 25311db5, made before any library line; the rest are the rows K1 to K5, E1,
//! R, R2, Z and N of the registration. The case files are texts of this file, written to `CARGO_TARGET_TMPDIR`.

use rcfd_api::solid_case::{run_solid_case_any, schema, CaseRun};
use serde_json::Value;
use std::path::{Path, PathBuf};

// ------------------------------------------------------------------------------------------------- the case texts

/// A fresh directory of this test under the target's temporary directory.
fn dir(tag: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("solid_case_reduction_keys_f889")
        .join(tag);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p
}

/// The case text of a half of the bar (`8 x 2 x 2` Hex8 on `0.5 x 0.1 x 0.08` from `x0`), clamped on `xmin` when asked, with
/// the node set `corner` at `(x0 + 0.5, 0.1, 0.08)` when asked, then `tail`.
fn half_text(x0: f64, clamp: bool, corner: bool, tail: &str) -> String {
    let mut t = format!(
        "[mesh]\nelement = \"hex8\"\norigin = [{x0:?}, 0.0, 0.0]\nlengths = [0.5, 0.1, 0.08]\ndivisions = [8, 2, 2]\n\n"
    );
    if corner {
        let x = x0 + 0.5;
        t.push_str(&format!(
            "[node_sets.corner]\nbox_min = [{x:?}, 0.1, 0.08]\nbox_max = [{x:?}, 0.1, 0.08]\n\n"
        ));
    }
    t.push_str("[materials.steel]\nyoungs_modulus = 2.1e11\npoisson_ratio = 0.3\ndensity = 7850.0\n\n");
    if clamp {
        t.push_str("[constraints.clamp]\nset = \"xmin\"\ncomponents = [\"x\", \"y\", \"z\"]\n\n");
    }
    t.push_str(tail);
    t
}

/// Part A of the registration with `tail` (loads, then `[reduction]`): clamped at `xmin`, interface `xmax`.
fn a_text(tail: &str) -> String {
    half_text(0.0, true, false, tail)
}

/// Part B with `tail`: free, interface `xmin`, the node set `corner`.
fn b_text(tail: &str) -> String {
    half_text(0.5, false, true, tail)
}

const TIP: &str = "[loads.tip]\nkind = \"nodal_force\"\nset = \"corner\"\nforce = [1.0, 1.0, 1.0]\n\n";
const M_LOADS: &str = "[loads.a_traction]\nkind = \"traction\"\nset = \"xmax\"\ntraction = [1.0e6, 2.0e6, -3.0e6]\n\n\
                       [loads.b_gravity]\nkind = \"gravity\"\nacceleration = [0.0, -9.81, -9.81]\n\n\
                       [loads.c_pressure]\nkind = \"pressure\"\nset = \"ymax\"\npressure = 2.0e5\n\n";

/// The `[reduction]` table with `keys` (lines) and the part file `file`.
fn reduction(interface: &str, keys: &str, file: &str) -> String {
    format!("[reduction]\ninterface = \"{interface}\"\n{keys}file = \"{file}\"\n")
}

fn any(path: &Path) -> CaseRun {
    run_solid_case_any(path, None).unwrap_or_else(|e| panic!("{}: {e:?}", path.display()))
}

/// Runs the part case `text` written as `name` in `d`; returns the bytes of its part file `file`.
fn part_bytes(d: &Path, name: &str, text: &str, file: &str) -> Vec<u8> {
    match any(&write(d, name, text)) {
        CaseRun::Reduction(_) => std::fs::read(d.join(file)).unwrap(),
        _ => panic!("{name} is not a reduction"),
    }
}

// ------------------------------------------------------------------------------------------------------ D1 and S1

/// The FNV-1a hash of bytes.
fn fnv(bytes: impl IntoIterator<Item = u8>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn report_text(report: &Value) -> String {
    let mut r = report.clone();
    r.as_object_mut().unwrap().remove("outputs");
    serde_json::to_string_pretty(&r).unwrap()
}

/// Part A, B and Bm with the keys `keys` (lines of `[reduction]`), as the case texts of D1.
fn d1_cases() -> Vec<(String, String)> {
    let sets = [
        ("m0", "modes = 0\n"),
        ("m5", "modes = 5\n"),
        ("m5e1", "modes = 5\ninterface_enrichment = 1\n"),
    ];
    let mut out = Vec::new();
    for (tag, keys) in sets {
        out.push((format!("A-{tag}"), a_text(&reduction("xmax", keys, "part.cms"))));
        out.push((
            format!("B-{tag}"),
            b_text(&format!("{TIP}{}", reduction("xmin", keys, "part.cms"))),
        ));
        out.push((
            format!("Bm-{tag}"),
            b_text(&format!("{M_LOADS}{}", reduction("xmin", keys, "part.cms"))),
        ));
    }
    out
}

/// Two digests of a case of D1: the part file's bytes and the report without `outputs`.
fn d1_digests(text: &str, tag: &str) -> [u64; 2] {
    let d = dir(&format!("d1-{tag}"));
    let path = write(&d, "case.toml", text);
    match any(&path) {
        CaseRun::Reduction(r) => [
            fnv(std::fs::read(d.join("part.cms")).unwrap()),
            fnv(report_text(&r.report).bytes()),
        ],
        _ => panic!("{tag} is not a reduction"),
    }
}

/// The host of arrangement I: the half B with the traction and gravity of F-616, attached to A's part (modes = 5).
fn d1_host(modal: bool) -> u64 {
    let d = dir(if modal { "d1-host-modal" } else { "d1-host-static" });
    let part = a_text(&reduction("xmax", "modes = 5\n", "a.cms"));
    part_bytes(&d, "part.toml", &part, "a.cms");
    let mut host = half_text(
        0.5,
        false,
        false,
        "[loads.a_traction]\nkind = \"traction\"\nset = \"xmax\"\ntraction = [1.0e6, 2.0e6, -3.0e6]\n\n\
         [loads.b_gravity]\nkind = \"gravity\"\nacceleration = [0.0, -9.81, -9.81]\n\n\
         [superelements.half]\nfile = \"a.cms\"\nset = \"xmin\"\n\n",
    );
    if modal {
        host.push_str("[analysis]\ntype = \"modal\"\nmodes = 8\ntolerance = 1e-12\nresidual_test = \"backward\"\n");
    }
    match any(&write(&d, "host.toml", &host)) {
        CaseRun::Superelements(r) => fnv(report_text(&r.report).bytes()),
        _ => panic!("not a run with superelements"),
    }
}

/// The records of main 25311db5 (Windows x86_64 and Linux x86_64: IEEE operations and square roots only).
const D1: [(&str, [u64; 2]); 9] = [
    ("A-m0", [0x94d6a0a929b5dc94, 0xb25445f588201213]),
    ("B-m0", [0xb0b6bf16e6e9ed76, 0x54dbb9667397e230]),
    ("Bm-m0", [0x36775db86f61c38c, 0x54dbb9667397e230]),
    ("A-m5", [0xa70a3d9bd1e944b7, 0x08b193f3e583ac86]),
    ("B-m5", [0xa2f2aefb0b329699, 0xd51b94a6b55d66dd]),
    ("Bm-m5", [0x9dea17632dbdd196, 0xd51b94a6b55d66dd]),
    ("A-m5e1", [0xabb8aa47864d2456, 0xc6ded481d08de187]),
    ("B-m5e1", [0xae6fe5a9d8188478, 0x5d8592fddae66626]),
    ("Bm-m5e1", [0x91215a3e6ffaca0a, 0x5d8592fddae66626]),
];
const D1_HOST: [u64; 2] = [0xde0d5a2a00fe34bc, 0x7e4e75f630b4cff0];
/// The schema of main without the properties `cutoff`, `residual_vectors` and `load_patterns` of `[reduction]`.
const S1: u64 = 0x36594e64f4f9fb2d;

#[test]
fn d1_part_cases_without_the_new_keys() {
    let mut failed = Vec::new();
    for ((tag, text), want) in d1_cases().into_iter().zip(D1) {
        let got = d1_digests(&text, &tag);
        println!("    (\"{tag}\", [{:#018x}, {:#018x}]),", got[0], got[1]);
        if tag != want.0 || got != want.1 {
            failed.push(tag);
        }
    }
    let host = [d1_host(false), d1_host(true)];
    println!("HOST [{:#018x}, {:#018x}]", host[0], host[1]);
    if host != D1_HOST {
        failed.push("host".to_string());
    }
    assert!(failed.is_empty(), "D1 digests differ: {failed:?}");
}

#[test]
fn s1_schema_without_the_new_keys() {
    let mut v = schema().to_json_schema();
    // For the report of the additive diff: the whole schema as text, when a path is given.
    if let Ok(p) = std::env::var("F889_SCHEMA_OUT") {
        std::fs::write(p, serde_json::to_string_pretty(&v).unwrap()).unwrap();
    }
    let table = &mut v["properties"]["reduction"];
    for key in ["cutoff", "residual_vectors", "load_patterns"] {
        table["properties"].as_object_mut().unwrap().remove(key);
        if let Some(required) = table.get_mut("required").and_then(Value::as_array_mut) {
            required.retain(|x| x != key);
        }
    }
    let text = serde_json::to_string(&v).unwrap();
    let got = fnv(text.bytes());
    println!("S1 {got:#018x} ({} bytes)", text.len());
    assert_eq!(got, S1, "S1 schema digest differs");
}
