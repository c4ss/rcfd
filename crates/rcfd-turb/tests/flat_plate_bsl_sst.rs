//! F-043 (ADR 0015): SST and BSL on the TMR zero-pressure-gradient flat plate at registered stations, y+~1
//! stability on three meshes, component-diagonal check.
//!
//! Registered limits (ADR 0015, written before the first run; none changed afterwards):
//! * B1: SST (automatic) Cf at x = 0.5, 0.97, 1.5 within 3 % of the TMR CFL3D SST-V file values.
//! * B2: SST (automatic) Cf at the same stations within 5 % of each TMR theory curve ("6-112a" = 0.0592 Re_x^-0.2,
//!   "6-121", "6-134" = White 0.455/ln^2(0.06 Re_x)).
//! * B3: BSL at the same stations within 5 % of the CFL3D values and within 5 % of SST.
//! * B4: first-cell y+ 2.0, 1.0, 0.5: each run converges (max update < 1e-9, nu_t change < 1e-6 in 8000 iterations) and
//!   the Cf(0.97) spread (max - min)/mean <= 3 %, for SST automatic, BSL and SST low_re.
//! * B6: `component_diagonal = corrected` changes Cf(0.97) of SST (y+ 0.5) by less than 1 %.
//!
//! Heavy (minutes per run): `--ignored`, release. Cached runs are shared between tests of one process; run single
//! tests in parallel processes to use several cores.

mod common;
use common::plate::*;
use rcfd_turb::sst::{SstVariant, SstWallMode};
use std::sync::Arc;

const STATIONS: [f64; 3] = [0.5, 0.97, 1.5];
/// TMR `cf_plate_sstv.dat`, zone CFL3D, interpolated at the stations.
const CFL3D: [f64; 3] = [2.96543e-3, 2.69089e-3, 2.53108e-3];
/// TMR `cf_as_function_of_x.dat`, zones 1-3 interpolated at the stations.
const THEORY: [(&str, [f64; 3]); 3] = [
    ("6-112a", [3.10981e-3, 2.72379e-3, 2.49637e-3]),
    ("6-121", [3.16930e-3, 2.88302e-3, 2.70896e-3]),
    ("6-134", [3.20314e-3, 2.87459e-3, 2.68528e-3]),
];

fn plate(variant: SstVariant, mode: SstWallMode, y_plus: f64) -> Arc<Plate> {
    let key = format!("{variant:?}-{mode:?}-{y_plus}");
    let cfg = PlateCfg::tmr(
        PlModel::Sst { variant, mode },
        y_plus,
        env("LAYERS", 70),
        env("NY", 110),
    );
    cached_plate(&key, &cfg)
}

fn sst05() -> Arc<Plate> {
    plate(SstVariant::Sst, SstWallMode::Automatic, 0.5)
}

fn bsl05() -> Arc<Plate> {
    plate(SstVariant::Bsl, SstWallMode::Automatic, 0.5)
}

#[test]
#[ignore = "heavy: wall-resolved flat plate, release"]
fn b1_sst_cf_vs_cfl3d_stations() {
    let o = sst05();
    assert!(o.converged(), "not converged: du {:.2e} dnut {:.2e}", o.du, o.change);
    for (i, &x) in STATIONS.iter().enumerate() {
        let cf = o.at(x).0;
        println!(
            "B1 x {x}: Cf {cf:.6} vs CFL3D {:.6}: {:+.2} %",
            CFL3D[i],
            100.0 * (cf / CFL3D[i] - 1.0)
        );
    }
    for (i, &x) in STATIONS.iter().enumerate() {
        let cf = o.at(x).0;
        assert!((cf / CFL3D[i] - 1.0).abs() < 0.03, "x {x}: Cf {cf} vs {}", CFL3D[i]);
    }
}

fn theory_check(k: usize) {
    let o = sst05();
    assert!(o.converged());
    let (name, vals) = THEORY[k];
    let mut worst: f64 = 0.0;
    for (i, &x) in STATIONS.iter().enumerate() {
        let cf = o.at(x).0;
        let dev = cf / vals[i] - 1.0;
        println!("B2 {name} x {x}: Cf {cf:.6} vs {:.6}: {:+.2} %", vals[i], 100.0 * dev);
        worst = worst.max(dev.abs());
    }
    assert!(
        worst < 0.05,
        "SST vs theory {name}: worst deviation {:.2} %",
        100.0 * worst
    );
}

#[test]
#[ignore = "heavy: wall-resolved flat plate, release"]
fn b2_sst_cf_vs_theory_6_112a_known_miss() {
    theory_check(0);
}

#[test]
#[ignore = "heavy: wall-resolved flat plate, release"]
fn b2_sst_cf_vs_theory_6_121_known_miss() {
    theory_check(1);
}

#[test]
#[ignore = "heavy: wall-resolved flat plate, release"]
fn b2_sst_cf_vs_theory_6_134_known_miss() {
    theory_check(2);
}

#[test]
#[ignore = "heavy: two wall-resolved flat plates, release"]
fn b3_bsl_cf_vs_cfl3d_and_sst() {
    let (b, s) = (bsl05(), sst05());
    assert!(
        b.converged(),
        "BSL not converged: du {:.2e} dnut {:.2e}",
        b.du,
        b.change
    );
    let mut worst_c: f64 = 0.0;
    let mut worst_s: f64 = 0.0;
    for (i, &x) in STATIONS.iter().enumerate() {
        let (cb, cs) = (b.at(x).0, s.at(x).0);
        println!(
            "B3 x {x}: BSL {cb:.6} SST {cs:.6}: vs CFL3D {:+.2} %, vs SST {:+.2} %",
            100.0 * (cb / CFL3D[i] - 1.0),
            100.0 * (cb / cs - 1.0)
        );
        worst_c = worst_c.max((cb / CFL3D[i] - 1.0).abs());
        worst_s = worst_s.max((cb / cs - 1.0).abs());
    }
    assert!(worst_c < 0.05, "BSL vs CFL3D worst {:.2} %", 100.0 * worst_c);
    assert!(worst_s < 0.05, "BSL vs SST worst {:.2} %", 100.0 * worst_s);
}

fn stability(variant: SstVariant, mode: SstWallMode) {
    let mut cfs = Vec::new();
    for yp in [2.0, 1.0, 0.5] {
        let o = plate(variant, mode, yp);
        let (cf, y_real) = o.at(0.97);
        println!(
            "B4 {variant:?} {mode:?} target y+ {yp}: Cf(0.97) {cf:.6} (realised first-cell y+ {y_real:.2}), converged {} ({} its)",
            o.converged(),
            o.iterations
        );
        assert!(
            o.converged(),
            "y+ {yp}: not converged (du {:.2e} dnut {:.2e})",
            o.du,
            o.change
        );
        cfs.push(cf);
    }
    let (mx, mn) = (
        cfs.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        cfs.iter().cloned().fold(f64::INFINITY, f64::min),
    );
    let mean = cfs.iter().sum::<f64>() / 3.0;
    let spread = (mx - mn) / mean;
    println!(
        "B4 {variant:?} {mode:?}: Cf(0.97) spread over the three meshes {:.2} % (limit 3 %)",
        100.0 * spread
    );
    assert!(spread <= 0.03, "spread {:.2} %", 100.0 * spread);
}

#[test]
#[ignore = "heavy: three wall-resolved flat plates, release"]
fn b4_sst_automatic_y_plus_stability_known_miss() {
    stability(SstVariant::Sst, SstWallMode::Automatic);
}

#[test]
#[ignore = "heavy: three wall-resolved flat plates, release"]
fn b4_bsl_y_plus_stability_known_miss() {
    stability(SstVariant::Bsl, SstWallMode::Automatic);
}

#[test]
#[ignore = "heavy: three wall-resolved flat plates, release"]
fn b4_sst_low_re_y_plus_stability_known_miss() {
    stability(SstVariant::Sst, SstWallMode::LowRe);
}

#[test]
#[ignore = "heavy: two wall-resolved flat plates, release"]
fn b6_component_diagonal_changes_cf_less_than_1pct() {
    let a = sst05();
    let mut cfg = PlateCfg::tmr(
        PlModel::Sst {
            variant: SstVariant::Sst,
            mode: SstWallMode::Automatic,
        },
        0.5,
        env("LAYERS", 70),
        env("NY", 110),
    );
    cfg.comp_diag = true;
    let b = cached_plate("sst-compdiag", &cfg);
    let (ca, cb) = (a.at(0.97).0, b.at(0.97).0);
    println!(
        "B6 Cf(0.97): averaged {ca:.6} corrected {cb:.6}: {:+.3} %",
        100.0 * (cb / ca - 1.0)
    );
    assert!((cb / ca - 1.0).abs() < 0.01);
}
