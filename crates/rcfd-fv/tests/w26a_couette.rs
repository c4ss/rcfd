//! Wave 26A diagnosis: impulsively started plane Couette flow with the transient PISO / PIMPLE algorithms at a range of
//! diffusion numbers d = nu dt / dy^2 (ADR 0046). Prints the maximum |u| after a fixed number of steps; the exact solution is
//! bounded by the wall speed 1.
use rcfd_fv::bc::FixedValue;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::box_2d;
use std::sync::Arc;

fn couette(nx: usize, ny: usize, per_x: bool, set: impl FnOnce(&mut PimpleSettings)) -> PimpleSolver {
    let mesh = box_2d(nx, ny, 1.0, 1.0, per_x, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "ymin", Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    u.set_bc(&geom, "ymax", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = PimpleSettings::piso(1.0);
    s.momentum_solver.tol_rel = 1e-12;
    s.pressure_solver.tol_rel = 1e-12;
    set(&mut s);
    PimpleSolver::new(geom, u, p, s)
}

fn umax(s: &PimpleSolver) -> f64 {
    s.u.values.iter().fold(0.0f64, |m, v| m.max(v[0].abs()))
}

fn run(label: &str, nx: usize, ny: usize, per_x: bool, d: f64, steps: usize, set: impl FnOnce(&mut PimpleSettings)) {
    let dy = 1.0 / ny as f64;
    let dt = d * dy * dy;
    let mut s = couette(nx, ny, per_x, set);
    let mut peak = 0.0f64;
    for _ in 0..steps {
        s.step(dt);
        peak = peak.max(umax(&s));
        if !peak.is_finite() || peak > 1e6 {
            break;
        }
    }
    eprintln!(
        "W26A {label} nx {nx} ny {ny} per {per_x} d {d}: peak |u| {peak:.4e} after {} steps, final {:.4e}",
        s.history.len(),
        umax(&s)
    );
}

#[test]
#[ignore = "diagnosis"]
fn couette_scan() {
    for d in [0.5, 1.6, 3.0, 6.0, 16.0, 100.0] {
        run("piso", 4, 40, false, d, 60, |_| {});
        run("piso", 40, 40, false, d, 60, |_| {});
    }
}

#[test]
#[ignore = "diagnosis"]
fn couette_trace() {
    let ny = 40;
    let dt = 6.0 / (ny * ny) as f64;
    let per = std::env::var("PER").is_ok();
    let mut s = couette(4, ny, per, |_| {});
    for i in 0..14 {
        s.step(dt);
        let vmax = s.u.values.iter().fold(0.0f64, |m, v| m.max(v[1].abs()));
        let pmax = s.p.values.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        let xvar = (0..ny)
            .map(|j| (s.u.values[4 * j][0] - s.u.values[4 * j + 1][0]).abs())
            .fold(0.0f64, f64::max);
        eprintln!(
            "W26A step {i}: umax {:.3e} vmax {:.3e} pmax {:.3e} xvar {:.3e}",
            umax(&s),
            vmax,
            pmax,
            xvar
        );
    }
}
