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
    let err = Case::parse_toml(&bad).expect_err("unknown wall mode accepted");
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

// Registered limits, evaluated on the registered runs (D1 as registered, binary rcfd-w49e.exe).

/// L1: every registered run ends with exit code 0 or 5.
#[test]
fn l1_registered_runs_complete() {
    assert!(met("L1"));
}

/// LR: D01, D05, D40 and AD40 equal ADR 0164's P01, P05, P40 and F00 bit for bit.
#[test]
fn lr_defaults_bit_identical() {
    assert!(met("LR"));
}

/// LH1: the default Cf error follows the 1-D model (shifted by D01's offset) within 3 points at all 10 y+.
#[test]
fn lh1_default_error_follows_the_model() {
    assert!(met("LH1"));
}

/// LH2: the default over-predicts by at least 10 % somewhere in y+ 5 to 15.
#[test]
fn lh2_default_buffer_layer_error() {
    assert!(met("LH2"));
}

/// LH3: the default friction of the NACA 0012 on M1w05 is at least 8 % above TMR's.
#[test]
fn lh3_aerofoil_sees_the_buffer_layer_error() {
    assert!(met("LH3"));
}

/// LP1 (registered D1): Cf(0.97) within 5 % of TMR at all 10 y+. Missed at y+ 5 (E05), the switch (amendment 1).
#[test]
#[ignore = "ADR 0168 LP1 known miss: E05 +7.49 % against TMR (band 5 %), the y+ 5 switch of D1 as registered"]
fn lp1_opt_in_against_tmr_known_miss() {
    assert!(met("LP1"));
}

/// LP2 (registered D1): Cf(0.97) within 3 % of E01's at all 10 y+. Missed at y+ 5.
#[test]
#[ignore = "ADR 0168 LP2 known miss: E05 +5.07 % against E01 (band 3 %)"]
fn lp2_opt_in_yplus_independence_known_miss() {
    assert!(met("LP2"));
}

/// LP3 (registered D1): the effective report equals the integral of the wall-law Cf within 2 %.
#[test]
fn lp3_opt_in_report_is_the_applied_shear() {
    assert!(met("LP3"));
}

/// LP4 (registered D1): mean C_F within 5 % of Karman-Schoenherr at all 10 y+. Missed at y+ 5.
#[test]
#[ignore = "ADR 0168 LP4 known miss: E05 +6.16 % against Karman-Schoenherr (band 5 %)"]
fn lp4_opt_in_mean_friction_known_miss() {
    assert!(met("LP4"));
}

/// LA1-LA3 (registered D1): the NACA 0012 at 0 degrees on M1w05, M1w15, M1w.
#[test]
fn la1_la2_la3_opt_in_aerofoil() {
    assert!(met("LA1") && met("LA2") && met("LA3"));
}

// Amendment 1 limits, evaluated on the amended runs (binary rcfd-w49e-am1.exe): F, AF and G arms.

/// L1_A1 and LR_A1: the amended runs complete; G05, G40, GA40 equal P05, P40, F00 bit for bit.
#[test]
fn a1_runs_complete_and_defaults_bit_identical() {
    assert!(met("L1_A1") && met("LR_A1"));
}

/// LP1_A1 to LP4_A1: the registered plate bands on the amended opt-in at all 10 y+.
#[test]
fn a1_lp1_to_lp4_opt_in_plate() {
    for id in ["LP1_A1", "LP2_A1", "LP3_A1", "LP4_A1"] {
        assert!(met(id), "{id}");
    }
}

/// LA1_A1 to LA3_A1: the registered aerofoil bands on the amended opt-in.
#[test]
fn a1_la1_to_la3_opt_in_aerofoil() {
    for id in ["LA1_A1", "LA2_A1", "LA3_A1"] {
        assert!(met(id), "{id}");
    }
}

/// M1_A1 to M3_A1: no pin at y+ 5, no second-cell over-production, the steady runs settle.
#[test]
fn a1_m1_to_m3_mechanism() {
    for id in ["M1_A1", "M2_A1", "M3_A1"] {
        assert!(met(id), "{id}");
    }
}

/// B1: the investigation folder and its PNG stay within the registered budget.
#[test]
fn b1_storage() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/investigations/job49e-sa-wall");
    let mut total = 0;
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        let n = std::fs::metadata(&p).unwrap().len();
        if p.extension().is_some_and(|x| x == "png") {
            assert!(n <= 100_000, "{p:?} {n}");
        }
        total += n;
    }
    assert!(total <= 250_000, "{total}");
}
