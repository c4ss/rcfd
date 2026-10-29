//! ADR 0168 (job 49E): the opt-in SA wall mode `auto_sa_law`. T2 (wiring, registered before this file was run) and the
//! limits of the registered runs, read from the committed `docs/investigations/job49e-sa-wall/results.json`; a missed
//! limit is an ignored `*_known_miss` test with the limit intact.

use rcfd_api::case::Case;
use rcfd_api::sim::{ReportDef, Simulation};
use std::path::PathBuf;

/// Coarse flat plate, Re = 5e6 per m, wall cell at y+ about 10 (buffer layer).
const PLATE: &str = r#"
title = "plate, y+ 10"
[mesh]
generator = "box_2d"
nx = 30
ny = 30
lx = 2.0
ly = 1.0
wall_layers = 16
layer_walls = "lower"
layer_growth = 1.2
target_y_plus = 10.0
layer_reference_velocity = 1.0
layer_reference_length = 1.0
[physics]
nu = 2e-7
[initial]
velocity = [1.0, 0.0, 0.0]
[boundaries.xmin]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "symmetry"
[turbulence]
model = "spalart_allmaras"
[solver]
algorithm = "simplec"
max_iterations = 100000
[reports.cf_eff]
type = "force_coefficient"
patch = "ymin"
direction = [1.0, 0.0, 0.0]
a_ref = 0.02
component = "viscous_effective"
"#;

fn cf_after(c: &Case, n: usize) -> f64 {
    let mut s = Simulation::new(c).unwrap_or_else(|e| panic!("{e:?}"));
    for _ in 0..n {
        s.step();
    }
    let r = ReportDef::from_case(c);
    s.eval_report(&r[0]).unwrap()
}

/// T2: the key parses, the opt-in mode runs to a finite, positive effective friction and changes the result; an
/// unknown value is a located error at the key.
#[test]
fn t2_auto_sa_law_is_wired() {
    let base = Case::parse_toml(PLATE).unwrap();
    let mut opt = base.clone();
    opt.set("turbulence.sa.wall_mode", serde_json::json!("auto_sa_law"))
        .unwrap();
    let (d, e) = (cf_after(&base, 300), cf_after(&opt, 300));
    assert!(d.is_finite() && e.is_finite() && d > 0.0 && e > 0.0, "{d} {e}");
    assert!((d / e - 1.0).abs() > 1e-6, "opt-in equals default: {d} {e}");
    let bad = PLATE.replace(
        "[turbulence]\nmodel = \"spalart_allmaras\"",
        "[turbulence]\nmodel = \"spalart_allmaras\"\n[turbulence.sa]\nwall_mode = \"sa_law\"",
    );
    let err = Case::parse_toml(&bad).err().expect("unknown wall mode accepted");
    assert!(
        err.iter().any(|d| format!("{d:?}").contains("turbulence.sa.wall_mode")),
        "{err:?}"
    );
}

// ------------------------------------------------------------------------------------------- limits of the runs

fn results() -> Option<serde_json::Value> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/investigations/job49e-sa-wall/results.json");
    let t = std::fs::read_to_string(p).ok()?;
    serde_json::from_str(&t).ok()
}

fn met(id: &str) -> bool {
    let r = results().expect("results.json of ADR 0168 not committed");
    r["evaluation"][id]["met"]
        .as_bool()
        .unwrap_or_else(|| panic!("{id}: no evaluation"))
}
