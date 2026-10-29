//! ADR 0159 (job 48Y): the registered limits of the NACA 0012 angle-of-attack sweep, checked on the committed results
//! (`docs/investigations/job48y-aoa-sweep/results.json`, written by `scripts/w48y_aoa_sweep.py collect`). No solver runs
//! here: the runs are made on demand by the driver (ADR 0159 D6). The slope (L2) and the stall rule (L3) are recomputed
//! from the committed sweep curve, independently of the driver's evaluation, and compared with it.

use std::path::PathBuf;

use serde_json::Value;

fn folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/investigations/job48y-aoa-sweep")
}

fn results() -> Value {
    let p = folder().join("results.json");
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    serde_json::from_str(&text).expect("results.json parses")
}

fn curve(r: &Value) -> Vec<(f64, f64, String)> {
    r["evaluation"]["curve"]
        .as_array()
        .expect("curve")
        .iter()
        .map(|p| {
            (
                p["alpha"].as_f64().unwrap(),
                p["cl"].as_f64().unwrap(),
                p["source"].as_str().unwrap().to_string(),
            )
        })
        .collect()
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
const BASE_ALPHAS: [f64; 14] = [
    0.0, 2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 20.0,
];

#[test]
fn w48y_l1_every_counted_run_completed() {
    let r = results();
    let inc = r["evaluation"]["L1"]["incomplete"].as_array().expect("L1");
    let counted = r["evaluation"]["L1"]["counted"].as_u64().unwrap();
    println!("L1: {counted} counted runs, incomplete {inc:?}");
    assert!(counted > 0);
    assert!(inc.is_empty(), "runs that did not complete: {inc:?}");
    // every base angle has a value on the curve
    let c = curve(&r);
    for a in BASE_ALPHAS {
        assert!(c.iter().any(|p| p.0 == a), "no curve value at {a} deg");
    }
}

#[test]
fn w48y_l2_lift_slope_within_the_band() {
    let r = results();
    let c = curve(&r);
    let pts: Vec<(f64, f64)> = SLOPE_ALPHAS
        .iter()
        .map(|a| *c.iter().find(|p| p.0 == *a).map(|p| (p.0, p.1)).as_ref().unwrap())
        .collect();
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
    assert!(
        (0.90..=1.05).contains(&factor),
        "slope factor {factor} outside [0.90, 1.05]"
    );
}

#[test]
fn w48y_l3_stall_is_detected() {
    let r = results();
    let c = curve(&r);
    let k = (0..c.len())
        .max_by(|&i, &j| c[i].1.partial_cmp(&c[j].1).unwrap())
        .unwrap();
    let (a_peak, cl_max) = (c[k].0, c[k].1);
    let after: Vec<_> = c[k + 1..].iter().collect();
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
fn w48y_l4_no_lift_at_zero_incidence() {
    let r = results();
    let c = curve(&r);
    let z = c.iter().find(|p| p.0 == 0.0).expect("0 deg");
    println!("L4: cl(0) = {:.3e} ({})", z.1, z.2);
    assert!(z.1.abs() <= 0.005, "cl(0) = {}", z.1);
}

#[test]
fn w48y_b1_storage_budget() {
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
