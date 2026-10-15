//! F-203 (ADR 0015, checks C1 to C3): the near-wall catalogue on flat plates.
//!
//! Registered limits (ADR 0015, written before the first run):
//! * Smooth plate, average Cf over [0, L] (L = 1 and 2, Re_L 5e6 and 1e7) within 5 % of the Prandtl-Schlichting law
//!   0.455/(log10 Re_L)^2.58 (3.365e-3 and 3.003e-3). A run is applicable only if the realised first-cell y+ for
//!   x in [0.5, 2] lies in [20, 60] (y+ ~ 30 meshes) or [0.3, 2] (y+ ~ 1 meshes); otherwise it is reported "not applicable".
//! * C1: SST automatic at y+ ~ 30 and y+ ~ 1.
//! * C2: k-epsilon (k = 1.5e-4, eps = 1e-3) Standard at y+ ~ 30; Scalable at ~30 and ~1; Enhanced at ~1 and ~30; the run
//!   is "turbulent" only if Cf(0.5) >= 0.5 * 0.0576 Re_x^-0.2.
//! * C3: rough plate (nu 1e-7, U 1, L 2, ks 1e-3, wall-cell centre at 2 ks): local Cf at x = 0.5, 1, 1.5 within 10 % of
//!   (2.87 + 1.58 log10(x/ks))^-2.5 for SST automatic and k-epsilon Standard with roughness; the smooth plate on the same
//!   mesh must give lower Cf.
//!
//! Heavy (minutes per run): `--ignored`, release.

mod common;
use common::plate::*;
use rcfd_turb::kepsilon::WallFunction;
use rcfd_turb::sst::{SstVariant, SstWallMode};
use rcfd_turb::KEpsilonVariant;

fn sst() -> PlModel {
    PlModel::Sst {
        variant: SstVariant::Sst,
        mode: SstWallMode::Automatic,
    }
}

fn ke(wf: WallFunction) -> PlModel {
    PlModel::Ke {
        variant: KEpsilonVariant::Standard,
        wf,
    }
}

fn cfg_for(model: PlModel, y_plus: f64) -> PlateCfg {
    let (layers, ny) = if y_plus > 10.0 { (40, 60) } else { (70, 110) };
    let mut c = PlateCfg::tmr(model, y_plus, layers, ny);
    if matches!(model, PlModel::Ke { .. }) {
        c.k_in = 1.5e-4;
        c.second_in = 1e-3;
    }
    c
}

/// Realised first-cell y+ range for x in [0.5, 2].
fn y_plus_range(o: &Plate) -> (f64, f64) {
    let v: Vec<f64> = o.cf.iter().filter(|c| c.0 >= 0.5).map(|c| c.2).collect();
    (
        v.iter().cloned().fold(f64::INFINITY, f64::min),
        v.iter().cloned().fold(0.0, f64::max),
    )
}

fn smooth(model: PlModel, y_plus: f64, tag: &str) {
    let cfg = cfg_for(model, y_plus);
    let o = cached_plate(tag, &cfg);
    let (lo, hi) = y_plus_range(&o);
    let window = if y_plus > 10.0 { (20.0, 60.0) } else { (0.3, 2.0) };
    println!(
        "{tag}: realised y+ in [{lo:.2}, {hi:.2}] (applicable window {window:?}), converged {}",
        o.converged()
    );
    if matches!(model, PlModel::Ke { .. }) {
        let cf05 = o.at(0.5).0;
        let lim = 0.5 * power(0.5 * 5.0e6);
        println!("{tag}: Cf(0.5) {cf05:.6} (turbulent if >= {lim:.6})");
        assert!(cf05 >= lim, "{tag}: laminarised (Cf(0.5) {cf05} < {lim})");
    }
    assert!(
        lo >= window.0 && hi <= window.1,
        "{tag}: NOT APPLICABLE, y+ [{lo:.2}, {hi:.2}] outside {window:?}"
    );
    assert!(
        o.converged(),
        "{tag}: not converged (du {:.2e} dnut {:.2e})",
        o.du,
        o.change
    );
    let mut worst: f64 = 0.0;
    for l in [1.0, 2.0] {
        let cf = o.cf_avg(l);
        let r = prandtl_schlichting(5.0e6 * l);
        println!(
            "{tag}: L {l}: <Cf> {cf:.6} vs Prandtl-Schlichting {r:.6}: {:+.2} %",
            100.0 * (cf / r - 1.0)
        );
        worst = worst.max((cf / r - 1.0).abs());
    }
    assert!(worst < 0.05, "{tag}: worst deviation {:.2} %", 100.0 * worst);
}

#[test]
#[ignore = "heavy: flat plate, release"]
fn c1_sst_smooth_y30() {
    smooth(sst(), 30.0, "c1-sst-30");
}

#[test]
#[ignore = "heavy: flat plate, release"]
fn c1_sst_smooth_y1_known_miss() {
    smooth(sst(), 1.0, "c1-sst-1");
}

#[test]
#[ignore = "heavy: flat plate, release"]
fn c2_kepsilon_standard_y30_known_miss() {
    smooth(ke(WallFunction::Standard), 30.0, "c2-std-30");
}

#[test]
#[ignore = "heavy: flat plate, release"]
fn c2_kepsilon_scalable_y30() {
    smooth(ke(WallFunction::Scalable), 30.0, "c2-scal-30");
}

#[test]
#[ignore = "heavy: flat plate, release"]
fn c2_kepsilon_scalable_y1() {
    smooth(ke(WallFunction::Scalable), 1.0, "c2-scal-1");
}

#[test]
#[ignore = "heavy: flat plate, release"]
fn c2_kepsilon_enhanced_y1_known_miss() {
    smooth(ke(WallFunction::Enhanced), 1.0, "c2-enh-1");
}

#[test]
#[ignore = "heavy: flat plate, release"]
fn c2_kepsilon_enhanced_y30_known_miss() {
    smooth(ke(WallFunction::Enhanced), 30.0, "c2-enh-30");
}

fn rough_cfg(model: PlModel, ks: f64) -> PlateCfg {
    let mut c = PlateCfg::tmr(model, 1.0, 30, 50);
    c.nu = 1.0e-7;
    c.y_plus = None;
    c.first = 4.0e-3;
    c.growth = 1.1;
    c.ks = ks;
    if matches!(model, PlModel::Ke { .. }) {
        c.k_in = 1.5e-4;
        c.second_in = 1e-3;
    }
    c
}

fn rough(model: PlModel, tag: &str) {
    let ks = 1.0e-3;
    let o = cached_plate(&format!("{tag}-rough"), &rough_cfg(model, ks));
    let s = cached_plate(&format!("{tag}-smooth"), &rough_cfg(model, 0.0));
    println!(
        "{tag}: rough converged {} ({} its), smooth converged {}",
        o.converged(),
        o.iterations,
        s.converged()
    );
    let mut worst: f64 = 0.0;
    for x in [0.5, 1.0, 1.5] {
        let (cf, _) = o.at(x);
        let cs = s.at(x).0;
        let r = fully_rough(x / ks);
        println!(
            "{tag}: x {x}: Cf rough {cf:.6} vs fully-rough correlation {r:.6}: {:+.2} %; smooth plate {cs:.6}, ratio rough/smooth {:.2}",
            100.0 * (cf / r - 1.0),
            cf / cs
        );
        assert!(cf > cs, "{tag}: rough Cf not above smooth at x {x}");
        worst = worst.max((cf / r - 1.0).abs());
    }
    assert!(
        o.converged(),
        "{tag}: rough run not converged (du {:.2e} dnut {:.2e})",
        o.du,
        o.change
    );
    assert!(worst < 0.10, "{tag}: worst deviation {:.2} %", 100.0 * worst);
}

#[test]
#[ignore = "heavy: two flat plates, release"]
fn c3_rough_plate_sst_known_miss() {
    rough(sst(), "c3-sst");
}

#[test]
#[ignore = "heavy: two flat plates, release"]
fn c3_rough_plate_kepsilon() {
    rough(ke(WallFunction::Standard), "c3-ke");
}
