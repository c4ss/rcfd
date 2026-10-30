//! ADR 0167 (job 49D): the registered limits of the NACA 0012 angle-of-attack sweep with ADR 0164's two opt-in fixes
//! (the `total_effective` force component and the far-field `turbulence = "freestream"` role) on a 500-chord far
//! field, checked on the committed results (`docs/investigations/job49d-aoa-sweep-fixed/results.json`, written by
//! `scripts/w49d_aoa_sweep_fixed.py collect`). No solver runs here: the runs are made on demand by the driver. The slope
//! (L2), the stall rule (L3) and the TMR comparisons (LT) are recomputed from the committed curve, independently of the
//! driver's evaluation, and compared with it.

use std::path::PathBuf;

use serde_json::Value;

fn folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/investigations/job49d-aoa-sweep-fixed")
}

fn results() -> Value {
    let p = folder().join("results.json");
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    serde_json::from_str(&text).expect("results.json parses")
}

/// The sweep curve C(alpha) of D7: (alpha, cl, cd, steady).
fn curve(r: &Value) -> Vec<(f64, f64, f64, bool)> {
    r["evaluation"]["curve"]
        .as_array()
        .expect("curve")
        .iter()
        .map(|p| {
            (
                p["alpha"].as_f64().unwrap(),
                p["cl"].as_f64().unwrap(),
                p["cd"].as_f64().unwrap(),
                p["steady"].as_bool().unwrap(),
            )
        })
        .collect()
}

fn at(c: &[(f64, f64, f64, bool)], a: f64) -> (f64, f64) {
    let p = c
        .iter()
        .find(|p| p.0 == a)
        .unwrap_or_else(|| panic!("no curve value at {a} deg"));
    (p.1, p.2)
}

/// Least squares cl = a + b alpha (alpha in radians): (b, a).
fn slope_fit(points: &[(f64, f64)]) -> (f64, f64) {
    let n = points.len() as f64;
    let xs: Vec<f64> = points.iter().map(|p| p.0.to_radians()).collect();
    let mx = xs.iter().sum::<f64>() / n;
    let my = points.iter().map(|p| p.1).sum::<f64>() / n;
    let sxx: f64 = xs.iter().map(|x| (x - mx) * (x - mx)).sum();
    let sxy: f64 = xs.iter().zip(points).map(|(x, p)| (x - mx) * (p.1 - my)).sum();
    let b = sxy / sxx;
    (b, my - b * mx)
}

const SLOPE_ALPHAS: [f64; 5] = [0.0, 2.0, 4.0, 6.0, 8.0];
const ALPHAS: [f64; 16] = [
    0.0, 2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 20.0, 22.0, 24.0,
];
/// TMR NACA 0012 SA, CFL3D (`validation-data/reference/w49a_tmr_naca0012_sa_cfl3d_clcd.csv`)
const TMR: [(f64, f64, f64); 3] = [
    (0.0, -0.68553224298e-05, 0.81924676041e-02),
    (10.0, 0.10909146672e+01, 0.12310544747e-01),
    (15.0, 0.15460981214e+01, 0.21236647158e-01),
];

#[test]
fn w49d_l1_every_run_completed() {
    let r = results();
    let l1 = &r["evaluation"]["L1"];
    let inc = l1["incomplete"].as_array().expect("L1");
    let not_run: Vec<&str> = l1["not_run"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    println!("L1: {} runs, incomplete {inc:?}, not run {not_run:?}", l1["counted"]);
    assert!(inc.is_empty(), "runs that did not complete: {inc:?}");
    // the SA and N arms are not droppable (only report-only SST runs may be)
    assert!(
        not_run.iter().all(|i| i.starts_with("SST-")),
        "SA or N runs not made: {not_run:?}"
    );
    let c = curve(&r);
    for a in ALPHAS {
        at(&c, a);
    }
}

fn slope_factor(r: &Value) -> f64 {
    let c = curve(r);
    let pts: Vec<(f64, f64)> = SLOPE_ALPHAS.iter().map(|&a| (a, at(&c, a).0)).collect();
    let (b, a) = slope_fit(&pts);
    let factor = b / (2.0 * std::f64::consts::PI);
    let py = r["evaluation"]["L2"]["factor"].as_f64().unwrap();
    println!(
        "L2: slope {b:.5} per rad ({:.5} per deg), factor {factor:.5}, intercept {a:.5}",
        b.to_radians()
    );
    assert!(
        (factor - py).abs() < 1e-12,
        "the driver's factor {py} differs from {factor}"
    );
    factor
}

#[test]
fn w49d_l2_lift_slope_within_the_band() {
    let f = slope_factor(&results());
    assert!((0.90..=1.05).contains(&f), "slope factor {f} outside [0.90, 1.05]");
}

#[test]
fn w49d_l3_stall_is_detected() {
    let r = results();
    let c = curve(&r);
    let k = (0..c.len())
        .max_by(|&i, &j| c[i].1.partial_cmp(&c[j].1).unwrap())
        .unwrap();
    let (a_peak, cl_max) = (c[k].0, c[k].1);
    let after = &c[k + 1..];
    let low = after.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let detected = !after.is_empty() && low <= 0.90 * cl_max;
    println!("L3: cl_max {cl_max:.4} at {a_peak} deg, lowest after {low:.4}, detected {detected}");
    assert_eq!(
        detected,
        r["evaluation"]["L3"]["detected"].as_bool().unwrap(),
        "differs from the driver"
    );
    assert!(
        detected,
        "no drop of 10 percent after the peak ({cl_max} at {a_peak} deg)"
    );
}

#[test]
fn w49d_l4_no_lift_at_zero_incidence() {
    let c = curve(&results());
    let (cl0, _) = at(&c, 0.0);
    println!("L4: cl(0) = {cl0:.3e}");
    assert!(cl0.abs() <= 0.005, "cl(0) = {cl0}");
}

/// LT: (cd relative error, cl relative error) against TMR at `alpha`, checked against the driver's numbers.
fn tmr_rel(r: &Value, alpha: f64) -> (f64, f64) {
    let c = curve(r);
    let (cl, cd) = at(&c, alpha);
    let t = TMR.iter().find(|t| t.0 == alpha).unwrap();
    let (cd_rel, cl_rel) = (cd / t.2 - 1.0, cl / t.1 - 1.0);
    let e = &r["evaluation"][format!("LT{}", alpha as i64)];
    assert!(
        (e["cd_rel"].as_f64().unwrap() - cd_rel).abs() < 1e-12,
        "driver's cd_rel differs"
    );
    println!(
        "LT{alpha}: cd {cd:.6} ({:+.2} %), cl {cl:.5} ({:+.2} %)",
        100.0 * cd_rel,
        100.0 * cl_rel
    );
    (cd_rel, cl_rel)
}

#[test]
fn w49d_lt0_drag_at_zero_incidence_against_tmr() {
    let (cd_rel, _) = tmr_rel(&results(), 0.0);
    assert!(cd_rel.abs() <= 0.12, "cd {cd_rel:+.4} outside 12 %");
}

#[test]
fn w49d_lt10_ten_degrees_against_tmr() {
    let (cd_rel, cl_rel) = tmr_rel(&results(), 10.0);
    assert!(cd_rel.abs() <= 0.15, "cd {cd_rel:+.4} outside 15 %");
    assert!(cl_rel.abs() <= 0.05, "cl {cl_rel:+.4} outside 5 %");
}

#[test]
fn w49d_lt15_fifteen_degrees_against_tmr() {
    let (cd_rel, cl_rel) = tmr_rel(&results(), 15.0);
    assert!(cd_rel.abs() <= 0.25, "cd {cd_rel:+.4} outside 25 %");
    assert!(cl_rel.abs() <= 0.05, "cl {cl_rel:+.4} outside 5 %");
}

#[test]
fn w49d_ld_drift_of_every_sa_run() {
    let r = results();
    let d = r["evaluation"]["LD"]["drift_cd_eff"].as_object().expect("LD");
    assert_eq!(d.len(), ALPHAS.len());
    let mut over = Vec::new();
    for (a, v) in d {
        let v = v.as_f64().unwrap_or(f64::NAN);
        println!("LD: {a} deg: D = {v:+.3e}");
        // a missing value (NaN) counts as over the limit
        if v.is_nan() || v.abs() > 0.005 {
            over.push((a.clone(), v));
        }
    }
    assert!(over.is_empty(), "drift over 0.5 %: {over:?}");
}

#[test]
fn w49d_p2_meshes_are_the_registered_ones() {
    let r = results();
    let m = r["evaluation"]["mesh"].as_object().expect("mesh");
    let want = [
        (
            "M1w",
            "bd8ef3c6da2dd3681d38c18914c8fbdcddab77bb1381c0688e1e2dce5f0c0299",
        ),
        (
            "M1w-far500",
            "59e707a866e34ec2fef6dd13a41de6411f844ceff99a2b6a644cb45f3a70c889",
        ),
    ];
    for (name, sha) in want {
        assert_eq!(m[name]["sha256"].as_str().unwrap(), sha, "{name}");
    }
}

#[test]
fn w49d_b1_storage_budget() {
    let mut total = 0u64;
    for e in std::fs::read_dir(folder()).expect("folder") {
        let e = e.unwrap();
        let len = e.metadata().unwrap().len();
        total += len;
        if e.path().extension().and_then(|x| x.to_str()) == Some("png") {
            assert!(len <= 120_000, "{}: {len} bytes", e.path().display());
        }
    }
    println!("B1: {total} bytes");
    assert!(total <= 400_000, "{total} bytes");
}
