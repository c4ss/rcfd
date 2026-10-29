//! ADR 0164 (job 49A): the registered limits of the SA drag investigation, checked on the committed results
//! (`docs/investigations/job49a-sa-drag/results.json`, written by `scripts/w49a_sa_drag.py collect`). No solver runs here:
//! the runs are made on demand by the driver. Each limit is recomputed from the committed window means and wall data with
//! the registered band, independently of the driver's evaluation. Amendment 1: the limits of the far-field arm B
//! (defective set-up, `turbulence = "inlet"`) are evaluated on its corrected arm F (`turbulence = "freestream"`).

use std::path::PathBuf;

use serde_json::Value;

const TMR_CD0: f64 = 0.0081924676041; // TMR CFL3D SA, alpha 0 (validation-data/reference/w49a_..._clcd.csv)
const TMR_CL10: f64 = 1.0909146672;
const TMR_CD10: f64 = 0.012310544747;
const TMR_PLATE_CF097: f64 = 2.70565e-3; // TMR CFL3D SA flat plate (ADR 0014)

fn folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/investigations/job49a-sa-drag")
}

fn results() -> Value {
    let p = folder().join("results.json");
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    serde_json::from_str(&text).expect("results.json parses")
}

fn win(r: &Value, id: &str, key: &str) -> f64 {
    r["runs"][id]["window"][key]["mean"]
        .as_f64()
        .unwrap_or_else(|| panic!("{id}.{key}: no window mean"))
}

fn wall(r: &Value, id: &str, key: &str) -> f64 {
    r["runs"][id]["wall"][key]
        .as_f64()
        .unwrap_or_else(|| panic!("{id}.wall.{key}"))
}

/// Friction part of TMR's cd at alpha 0 from the committed cf file: 2 x integral cf dx over the upper surface.
fn tmr_friction() -> f64 {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../validation-data/reference/w49a_tmr_naca0012_sa_cfl3d_cf_alpha0_upper.csv");
    let text = std::fs::read_to_string(&p).unwrap();
    let mut pts: Vec<(f64, f64)> = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with('x') && !l.trim().is_empty())
        .map(|l| {
            let mut s = l.split(',');
            (s.next().unwrap().parse().unwrap(), s.next().unwrap().parse().unwrap())
        })
        .collect();
    pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    2.0 * pts
        .windows(2)
        .map(|w| 0.5 * (w[0].1 + w[1].1) * (w[1].0 - w[0].0))
        .sum::<f64>()
}

#[test]
fn w49a_l1_runs_complete() {
    let r = results();
    for id in ["A00", "A10", "F00", "F10", "P01", "P05", "P40"] {
        let e = r["runs"][id]["exit"].as_i64().unwrap_or_else(|| panic!("{id} not run"));
        assert!(e == 0 || e == 5, "{id} exit {e}");
    }
}

#[test]
fn w49a_lr_defaults_reproduce_adr0159_bit_for_bit() {
    let r = results();
    for id in ["A00", "A10"] {
        let b = &r["runs"][id]["bits_vs_adr0159"];
        let (n, d) = (b["compared"].as_u64().unwrap(), b["different"].as_u64().unwrap());
        println!("LR {id}: {n} values compared, {d} different");
        assert!(n > 0 && d == 0, "{id}: {b}");
    }
}

#[test]
fn w49a_lp1_wall_law_cf_on_the_plate() {
    let r = results();
    for (id, band) in [("P01", 0.05), ("P05", 0.10), ("P40", 0.10)] {
        let cf = wall(&r, id, "cf097_law");
        println!(
            "LP1 {id}: Cf(0.97) {cf:.6e} ({:+.2} %)",
            100.0 * (cf / TMR_PLATE_CF097 - 1.0)
        );
        assert!((cf / TMR_PLATE_CF097 - 1.0).abs() <= band, "{id}");
    }
}

#[test]
fn w49a_lp2_lp3_molecular_report_against_wall_law() {
    let r = results();
    let ratio = |id: &str| wall(&r, id, "cf097_molecular") / wall(&r, id, "cf097_law");
    println!(
        "LP2 P01 {:.4}, LP3 P40 {:.4} (P05 {:.4})",
        ratio("P01"),
        ratio("P40"),
        ratio("P05")
    );
    assert!((ratio("P01") - 1.0).abs() <= 0.03, "LP2");
    assert!(ratio("P40") <= 0.6, "LP3");
}

#[test]
fn w49a_lp4_effective_report_equals_wall_law_integral() {
    let r = results();
    for id in ["P01", "P40"] {
        let depth = r["runs"][id]["depth"].as_f64().unwrap();
        let rep = win(&r, id, "cf_veff") / (2.0 * depth);
        let law = wall(&r, id, "CF_law");
        println!("LP4 {id}: report C_F {rep:.6e}, wall-law integral {law:.6e}");
        assert!((rep / law - 1.0).abs() <= 0.02, "{id}");
    }
}

#[test]
fn w49a_la1_la2_la3_alpha0() {
    let r = results();
    let cf_tmr = tmr_friction();
    assert!((cf_tmr - 0.006831).abs() < 1e-6, "{cf_tmr}");
    let la1 = win(&r, "A00", "cd_v") / win(&r, "A00", "cd_veff");
    println!("LA1 A00 cd_v / cd_veff {la1:.4}");
    assert!(la1 <= 0.6, "LA1");
    for id in ["A00", "F00"] {
        let (f, t) = (win(&r, id, "cd_veff"), win(&r, id, "cd_eff"));
        println!(
            "LA2/LA3 {id}: friction {f:.6} ({:+.2} %), cd {t:.6} ({:+.2} %)",
            100.0 * (f / cf_tmr - 1.0),
            100.0 * (t / TMR_CD0 - 1.0)
        );
        assert!((f / cf_tmr - 1.0).abs() <= 0.10, "LA2 {id}");
        assert!((t / TMR_CD0 - 1.0).abs() <= 0.12, "LA3 {id}");
    }
}

#[test]
fn w49a_la4_alpha10_far_field_fixed() {
    let r = results();
    let (cd, cl) = (win(&r, "F10", "cd_eff"), win(&r, "F10", "cl"));
    println!(
        "LA4 F10: cd {cd:.6} ({:+.2} %), cl {cl:.5} ({:+.2} %)",
        100.0 * (cd / TMR_CD10 - 1.0),
        100.0 * (cl / TMR_CL10 - 1.0)
    );
    assert!((cd / TMR_CD10 - 1.0).abs() <= 0.25, "LA4 cd");
    assert!((cl / TMR_CL10 - 1.0).abs() <= 0.05, "LA4 cl");
}

#[test]
fn w49a_la5_outer_nutilde() {
    let r = results();
    let m = |id: &str| r["runs"][id]["freestream_nutilde"]["median"].as_f64().unwrap();
    println!(
        "LA5 median nuTilda/nu: A10 {:.4e}, F00 {:.4}, F10 {:.4}",
        m("A10"),
        m("F00"),
        m("F10")
    );
    assert!(m("A10") >= 1000.0 && m("F00") <= 5.0 && m("F10") <= 5.0);
}

#[test]
fn w49a_la6_no_drift_with_the_far_field_fixed() {
    let r = results();
    let d = r["runs"]["F10"]["drift_cd_1k2k_to_last"].as_f64().unwrap();
    println!(
        "LA6 F10 drift {:+.3} % (A10 {:?})",
        100.0 * d,
        r["runs"]["A10"]["drift_cd_1k2k_to_last"]
    );
    assert!(d.abs() <= 0.02);
}

#[test]
fn w49a_b1_storage() {
    let mut total = 0;
    for e in std::fs::read_dir(folder()).unwrap() {
        let e = e.unwrap();
        let n = e.metadata().unwrap().len();
        total += n;
        if e.path().extension().is_some_and(|x| x == "png") {
            assert!(n <= 120_000, "{}", e.path().display());
        }
    }
    assert!(total <= 300_000, "{total}");
    let refs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../validation-data/reference");
    let r: u64 = [
        "w49a_tmr_naca0012_sa_cfl3d_clcd.csv",
        "w49a_tmr_naca0012_sa_cfl3d_cf_alpha0_upper.csv",
    ]
    .iter()
    .map(|f| std::fs::metadata(refs.join(f)).unwrap().len())
    .sum();
    assert!(r <= 30_000, "{r}");
}
