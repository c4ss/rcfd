//! ADR 0155 (job 48U) E2 and E3 on the committed example runs of `example-projects/wind-tunnel`: the NACA 0012 at 4 and
//! 2 degrees with the angle set by rotating the mesh (`cases/naca0012-aoa*-rotated.toml`, flow along +x, lift +y) against
//! the same angles set by rotating the flow (`cases/naca0012-aoa*.toml`). E2 (amendment 1): `cl` and `cd` at iteration
//! 2000 within 1e-6 relative, and at every iteration of 1001-2000 within 1e-5; the largest difference over all
//! iterations is printed. Control: the 4-degree rotated run against the 2-degree flow run fails the comparison. E3: the
//! rotated runs lift upward (+y) and more at 4 than at 2 degrees; control: the two swapped fail.

use serde_json::Value;
use std::path::{Path, PathBuf};

fn wind_tunnel() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("example-projects")
        .join("wind-tunnel")
}

fn read_json(p: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))).unwrap()
}

/// The committed run of `case` (from `example-projects/manifest.json`).
fn run_of(case: &str) -> PathBuf {
    let m = read_json(&wind_tunnel().join("..").join("manifest.json"));
    let id = m["projects"]["wind-tunnel"]["runs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["case"] == case)
        .unwrap_or_else(|| panic!("no committed run of {case}"))["run"]
        .as_str()
        .unwrap()
        .to_string();
    wind_tunnel().join("runs").join(id)
}

/// The monitor series `name` of a run: (iterations, values).
fn series(run: &Path, name: &str) -> (Vec<f64>, Vec<f64>) {
    let m = read_json(&run.join("output").join("monitors.json"));
    let s = &m["series"][name];
    (
        serde_json::from_value(s["x"].clone()).unwrap(),
        serde_json::from_value(s["y"].clone()).unwrap(),
    )
}

/// The comparison of E2 for `cl` and `cd`: relative difference of the final values, largest relative difference over
/// iterations 1001-2000, largest absolute difference over all iterations with its iteration.
struct Cmp {
    name: &'static str,
    final_rel: f64,
    late_rel: f64,
    worst_abs: f64,
    worst_at: f64,
    rot_final: f64,
    flow_final: f64,
}

fn compare(rot: &str, flow: &str) -> Vec<Cmp> {
    let (a, b) = (run_of(rot), run_of(flow));
    let (ma, mb) = (read_json(&a.join("metrics.json")), read_json(&b.join("metrics.json")));
    assert_eq!(ma["steps"], 2000, "{rot}");
    assert_eq!(mb["steps"], 2000, "{flow}");
    ["cl", "cd"]
        .into_iter()
        .map(|name| {
            let (xa, ya) = series(&a, name);
            let (xb, yb) = series(&b, name);
            assert_eq!(xa, xb, "{name}: the two runs report at the same iterations");
            let (fa, fb) = (ma["reports"][name].as_f64().unwrap(), mb["reports"][name].as_f64().unwrap());
            assert_eq!(fa, *ya.last().unwrap(), "{rot}: {name} metric is the last monitor value");
            let late = xa
                .iter()
                .zip(ya.iter().zip(&yb))
                .filter(|(x, _)| **x > 1000.0)
                .map(|(_, (p, q))| (p - q).abs() / q.abs())
                .fold(0.0, f64::max);
            let (mut worst_abs, mut worst_at) = (0.0, 0.0);
            for (x, (p, q)) in xa.iter().zip(ya.iter().zip(&yb)) {
                if (p - q).abs() > worst_abs {
                    worst_abs = (p - q).abs();
                    worst_at = *x;
                }
            }
            Cmp {
                name,
                final_rel: (fa - fb).abs() / fb.abs(),
                late_rel: late,
                worst_abs,
                worst_at,
                rot_final: fa,
                flow_final: fb,
            }
        })
        .collect()
}

const PAIRS: [(&str, &str); 2] = [
    ("cases/naca0012-aoa4-rotated.toml", "cases/naca0012-aoa4.toml"),
    ("cases/naca0012-aoa2-rotated.toml", "cases/naca0012-aoa2.toml"),
];

/// E2 with its control.
#[test]
fn e2_rotated_mesh_matches_rotated_flow() {
    for (rot, flow) in PAIRS {
        for c in compare(rot, flow) {
            println!(
                "E2 {rot} vs {flow}: {} {:.10} vs {:.10}, final {:.3e} (limit 1e-6), iterations 1001-2000 {:.3e} (limit 1e-5), largest {:.3e} at iteration {}",
                c.name, c.rot_final, c.flow_final, c.final_rel, c.late_rel, c.worst_abs, c.worst_at
            );
            assert!(c.final_rel <= 1e-6, "{rot}: {} final {:e}", c.name, c.final_rel);
            assert!(c.late_rel <= 1e-5, "{rot}: {} late {:e}", c.name, c.late_rel);
        }
    }
    let control = compare(PAIRS[0].0, PAIRS[1].1);
    println!(
        "E2 control (4 deg rotated against 2 deg flow): cl {:.3e}, cd {:.3e}",
        control[0].final_rel, control[1].final_rel
    );
    assert!(control.iter().all(|c| c.final_rel > 1e-6), "the control must fail the limit");
}

/// E3: the rotated runs lift along +y, more at 4 than at 2 degrees; control: swapped, the order fails.
#[test]
fn e3_rotated_runs_lift_upward_more_at_the_larger_angle() {
    let cl = |case: &str| read_json(&run_of(case).join("metrics.json"))["reports"]["cl"].as_f64().unwrap();
    let (c4, c2) = (cl(PAIRS[0].0), cl(PAIRS[1].0));
    println!("E3 cl: 4 deg (rotated mesh) {c4}, 2 deg (rotated mesh) {c2}");
    assert!(0.0 < c2 && c2 < c4, "{c2} {c4}");
    assert!(!(0.0 < c4 && c4 < c2), "control");
}
