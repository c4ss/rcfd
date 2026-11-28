//! ADR 0846 (F-989): an over-limit `axial_fan_sector` C-grid case that builds, with a small build of the leading-edge fan
//! first. Only `n_span` and `n_gap` of the shipped case are raised; the count and the refusal are ADR 0844's code.
//!
//! s1, s2: the shipped planar controls (`n_nose` 4, `front_gap` 0.3) build, at the shipped layers and at 20 times them,
//! and the estimate equals the registered and the built count. n1: H11b's planar controls fold at the hub at the shipped
//! layers (negative control). Heavy, ignored: `rc_` the refusal alone, `hc_` the refusal and the build, timed (ADR 0812).

use rcfd_api::case::Case;
use rcfd_api::facts::{self, FactsError, InText};
use rcfd_api::mesh_preview::cell_estimate;
use rcfd_api::sim;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::Instant;

const FAN_C: &str = "cases/axial_fan_cgrid/axial_fan_cgrid.toml";

fn fan(v: &[(&str, i64)]) -> Case {
    let root: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let mut case = Case::load(root.join(FAN_C)).unwrap();
    for (k, x) in v {
        case.set(&format!("mesh.fan.{k}"), json!(x)).unwrap();
    }
    case
}

/// S1, S2, N1 and HC of ADR 0846 with their registered counts.
fn s1() -> (Case, u64) {
    (fan(&[]), 11_536)
}

fn s2() -> (Case, u64) {
    (fan(&[("n_span", 280), ("n_gap", 140)]), 230_720)
}

fn n1() -> (Case, u64) {
    let planar = [
        ("n_up", 40),
        ("n_chord", 60),
        ("n_down", 40),
        ("n_side", 70),
        ("n_core", 4),
    ];
    (fan(&planar), 302_400)
}

fn hc() -> (Case, u64) {
    (fan(&[("n_span", 2440), ("n_gap", 1220)]), 2_010_560)
}

fn two_routes(what: &str, case: &Case, registered: u64) -> u64 {
    let est = cell_estimate(case);
    let mesh = sim::build_mesh(case).unwrap_or_else(|e| panic!("{what}: the build failed: {e:?}"));
    let n = mesh.n_cells() as u64;
    eprintln!("{what}: estimate {est:?}, registered {registered}, built {n}");
    assert_eq!(est, Some(registered), "{what}: route B (registered count)");
    assert_eq!(est, Some(n), "{what}: route A (built mesh)");
    n
}

/// C1: the leading-edge fan of the over-limit case's planar controls, at the shipped layers.
#[test]
fn s1_leading_edge_fan_small() {
    let (case, n) = s1();
    assert_eq!(case.get("mesh.fan.n_nose").unwrap(), json!(4));
    assert_eq!(case.get("mesh.fan.front_gap").unwrap().as_f64(), Some(0.3));
    two_routes("S1", &case, n);
}

/// C2: the same planar controls at 20 times the layers (the over-limit case's grading branch); refused one below.
#[test]
fn s2_dense_layers_small() {
    let (case, reg) = s2();
    let n = two_routes("S2", &case, reg);
    match facts::case_facts(&case, InText::NotDefault, Some(n)) {
        Ok(f) => assert_eq!(f.cells, n, "S2 at the limit"),
        other => panic!("S2 at the limit: {other:?}"),
    }
    match facts::case_facts(&case, InText::NotDefault, Some(n - 1)) {
        Err(FactsError::TooLarge { cells, limit }) => assert_eq!((cells, limit), (n, n - 1), "S2 below"),
        other => panic!("S2 below the limit: {other:?}"),
    }
}

/// C3 (negative control): H11b's planar controls fold at the hub at the shipped layers too.
#[test]
fn n1_the_fold_is_planar() {
    let (case, reg) = n1();
    assert_eq!(cell_estimate(&case), Some(reg), "N1 estimate");
    match sim::build_mesh(&case) {
        Ok(m) => panic!("N1 built {} cells; a fold was expected", m.n_cells()),
        Err(d) => {
            eprintln!("N1: {d:?}");
            assert!(
                d.iter()
                    .any(|x| x.path == "mesh.fan.front_gap"
                        && x.message.contains("the line family is folded at layer 0")),
                "N1: {d:?}"
            );
        }
    }
}

// ------------------------------------------------------------------ heavy (ignored; run once each, ADR 0846)

const LIMIT: u64 = 2_000_000;

fn refuse(case: &Case, n: u64) -> f64 {
    let t = Instant::now();
    let r = facts::case_facts(case, InText::NotDefault, Some(LIMIT));
    let tr = t.elapsed().as_secs_f64();
    match r {
        Err(FactsError::TooLarge { cells, limit }) => assert_eq!((cells, limit), (n, LIMIT), "HC"),
        other => panic!("HC: {other:?}"),
    }
    tr
}

/// C4, C6 (P_r): the refusal alone.
#[test]
#[ignore = "ADR 0846 heavy: the refusal alone (run once, under the sampler)"]
fn rc_refusal_alone() {
    let (case, n) = hc();
    let tr = refuse(&case, n);
    eprintln!("HC refusal alone: T_r {tr:.6} s");
}

/// C4, C5, C6 (P_b): the refusal and the reference build, timed.
#[test]
#[ignore = "ADR 0846 heavy: the refusal and the build, timed (run once, under the sampler)"]
fn hc_refusal_and_build() {
    let (case, n) = hc();
    let tr = refuse(&case, n);
    let t = Instant::now();
    let mesh = sim::build_mesh(&case);
    let tb = t.elapsed().as_secs_f64();
    let mesh = mesh.unwrap_or_else(|e| panic!("HC: the reference build failed (not deciding): {e:?}"));
    eprintln!(
        "HC: T_r {tr:.6} s, T_b {tb:.3} s, ratio {:.7}, estimate {n}, built {}",
        tr / tb,
        mesh.n_cells()
    );
    assert_eq!(mesh.n_cells() as u64, n, "HC: built count");
    assert!(tr <= 1.0, "HC: T_r {tr} s > 1 s");
    assert!(tr / tb <= 0.5, "HC: T_r / T_b {} > 0.5", tr / tb);
}
