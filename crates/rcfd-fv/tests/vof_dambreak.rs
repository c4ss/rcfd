//! ADR 0049 checks D1-D3 (dam break / liquid column collapse in the Martin and Moyce geometry, no-slip walls). The limits are
//! those registered in the ADR before any run; none was changed afterwards.

use rcfd_fv::vof::{indicator_fraction, VofSettings, VofSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::box_2d;
use std::sync::Arc;

const A: f64 = 0.05715;
const G: f64 = 9.81;
const NX: usize = 120;
const NY: usize = 40;

fn dam() -> VofSolver {
    let mesh = Arc::new(box_2d(NX, NY, 12.0 * A, 4.0 * A, false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p_rgh = ScalarField::uniform("p_rgh", &geom, 0.0);
    let mut alpha = ScalarField::uniform("alpha", &geom, 0.0);
    alpha.values = indicator_fraction(&geom, 10, |x| x[0] <= A && x[1] <= 2.0 * A);
    let mut s = VofSettings::new([1000.0, 1.0], [1e-3, 1.8e-5]);
    s.gravity = [0.0, -G, 0.0];
    s.n_correctors = 3;
    VofSolver::new(geom, u, p_rgh, alpha, s).unwrap()
}

/// Front position: the rightmost bottom-row cell with alpha > 0.5, interpolated linearly to alpha = 0.5 with the next cell.
fn front(s: &VofSolver) -> f64 {
    let dx = 12.0 * A / NX as f64;
    let a = &s.transport.alpha.values;
    let mut i = NX - 1;
    while i > 0 && a[i] <= 0.5 {
        i -= 1;
    }
    let xc = (i as f64 + 0.5) * dx;
    if i + 1 < NX && a[i] > a[i + 1] {
        xc + dx * (a[i] - 0.5) / (a[i] - a[i + 1])
    } else {
        xc
    }
}

#[test]
fn d_dam_break_front() {
    let h = 2.0 * A;
    let t_unit = (h / G).sqrt();
    let mut s = dam();
    let dt = 1.5e-3;
    let v0 = s.phase_volume();
    let (mut dv, mut lo, mut hi, mut co_max) = (0.0f64, 0.0f64, 1.0f64, 0.0f64);
    let mut track: Vec<(f64, f64)> = vec![(0.0, front(&s))];
    let t0 = std::time::Instant::now();
    while s.time < 3.6 * t_unit {
        let i = s.step(dt).unwrap();
        dv = dv.max((s.phase_volume() - v0).abs() / v0);
        lo = lo.min(i.alpha_min);
        hi = hi.max(i.alpha_max);
        co_max = co_max.max(i.courant);
        track.push((s.time / t_unit, front(&s)));
    }
    let at = |ts: f64| -> f64 {
        let k = track.iter().position(|p| p.0 >= ts).unwrap();
        let (p0, p1) = (track[k - 1], track[k]);
        p0.1 + (p1.1 - p0.1) * (ts - p0.0) / (p1.0 - p0.0)
    };
    let vref = 1.69;
    let norm = |x: f64| x / h; // front displacement in units of H (t* units of sqrt(H/g)): v/sqrt(gH) = d(x/H)/dt*
    let v13 = (norm(at(3.0)) - norm(at(1.0))) / 2.0;
    let rel = |ts: f64| {
        let pred = vref * (ts - 1.0);
        ((norm(at(ts)) - norm(at(1.0))) - pred).abs() / pred
    };
    let (e2, e3) = (rel(2.0), rel(3.0));
    let monotone = track
        .windows(2)
        .filter(|w| w[0].0 >= 0.5)
        .all(|w| w[1].1 >= w[0].1 - 1e-12);
    println!(
        "D1 v/sqrt(gH) = {v13:.3} (limit 1.44..1.94 and < 2); D2 relative deviation from the 1.69 line: t*=2 {e2:.3}, t*=3 {e3:.3} (limit 0.20), monotone {monotone}; x_f/H at t*=1,2,3: {:.3} {:.3} {:.3}; D3 dV {dv:.2e} (limit 1e-12), alpha in [{lo:.2e}, {hi:.9}] (limit 1e-9), max Co {co_max:.2} (limit 1.5); {} steps {:.1} s",
        norm(at(1.0)),
        norm(at(2.0)),
        norm(at(3.0)),
        track.len() - 1,
        t0.elapsed().as_secs_f64()
    );
    assert!((1.69 * 0.85..=1.69 * 1.15).contains(&v13) && v13 < 2.0, "D1 {v13}");
    assert!(e2 <= 0.20 && e3 <= 0.20, "D2 {e2} {e3}");
    assert!(monotone, "D2 monotone");
    assert!(dv <= 1e-12, "D3 volume {dv}");
    assert!(lo >= -1e-9 && hi <= 1.0 + 1e-9, "Z1 bounds");
    assert!(co_max <= 1.5, "D3 Courant {co_max}");
}
