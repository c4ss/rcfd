//! ADR 0049 check E (rising bubble, Hysing et al. 2009 case 1, coarse 40 x 80 algebraic VOF). The limits are those registered in
//! the ADR before any run; none was changed afterwards. Reference values read in Hysing et al. 2009 Table V (ADR 0096).

use rcfd_fv::vof::{indicator_fraction, VofSettings, VofSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, Symmetry, VectorField};
use rcfd_mesh::gen::box_2d;
use std::sync::Arc;

fn bubble(free_slip_sides: bool) -> VofSolver {
    let mesh = Arc::new(box_2d(40, 80, 1.0, 2.0, false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for p in ["ymin", "ymax"] {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    for p in ["xmin", "xmax"] {
        if free_slip_sides {
            u.set_bc(&geom, p, Box::new(Symmetry)).unwrap();
        } else {
            u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
        }
    }
    let p_rgh = ScalarField::uniform("p_rgh", &geom, 0.0);
    let mut alpha = ScalarField::uniform("alpha", &geom, 1.0);
    // phase 1 = the surrounding liquid; the bubble (phase 2) is alpha = 0
    alpha.values = indicator_fraction(&geom, 10, |x| (x[0] - 0.5).hypot(x[1] - 0.5) > 0.25);
    let mut s = VofSettings::new([1000.0, 100.0], [10.0, 1.0]);
    s.gravity = [0.0, -0.98, 0.0];
    s.sigma = 24.5;
    s.n_correctors = 3;
    VofSolver::new(geom, u, p_rgh, alpha, s).unwrap()
}

fn run(free_slip_sides: bool) -> (f64, f64, f64, f64) {
    let mut s = bubble(free_slip_sides);
    let dt = 2.5e-3;
    assert!(dt <= s.capillary_dt(), "capillary limit {}", s.capillary_dt());
    let vol = s.geom.mesh.cell_volumes().to_vec();
    let cc = s.geom.mesh.cell_centres().to_vec();
    let v0 = s.phase_volume();
    let (mut vmax, mut tmax, mut dv, mut worst_bound) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let t0 = std::time::Instant::now();
    while s.time < 3.0 - 1e-9 {
        let i = s.step(dt).unwrap();
        let a = &s.transport.alpha.values;
        let m: f64 = (0..a.len()).map(|c| (1.0 - a[c]) * vol[c]).sum();
        let vc: f64 = (0..a.len())
            .map(|c| (1.0 - a[c]) * vol[c] * s.u.values[c][1])
            .sum::<f64>()
            / m;
        if vc > vmax {
            vmax = vc;
            tmax = s.time;
        }
        dv = dv.max((s.phase_volume() - v0).abs() / v0);
        worst_bound = worst_bound.max((-i.alpha_min).max(i.alpha_max - 1.0));
    }
    let a = &s.transport.alpha.values;
    let m: f64 = (0..a.len()).map(|c| (1.0 - a[c]) * vol[c]).sum();
    let yc: f64 = (0..a.len()).map(|c| (1.0 - a[c]) * vol[c] * cc[c][1]).sum::<f64>() / m;
    println!(
        "E free_slip_sides={free_slip_sides}: y_c(3) = {yc:.4} (ref 1.0813, limit +-0.12), v_max = {vmax:.4} at t = {tmax:.3} (ref 0.2417 at 0.9213, limit 25 percent), dV {dv:.2e}, bound violation {worst_bound:.2e}, {:.1} s",
        t0.elapsed().as_secs_f64()
    );
    (yc, vmax, dv, worst_bound)
}

#[test]
fn e_rising_bubble_free_slip_sides() {
    let (yc, vmax, dv, wb) = run(true);
    assert!(dv <= 1e-12 && wb <= 1e-9, "Z1");
    assert!((yc - 1.0813).abs() <= 0.12, "y_c {yc}");
    assert!((vmax - 0.2417).abs() / 0.2417 <= 0.25, "v_max {vmax}");
}

#[test]
fn e_rising_bubble_no_slip_sides_labelled() {
    let (yc, vmax, dv, wb) = run(false);
    assert!(dv <= 1e-12 && wb <= 1e-9, "Z1");
    println!("E (no-slip sides, not the benchmark's wall condition): y_c {yc:.4}, v_max {vmax:.4}");
}
