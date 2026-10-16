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

/// Registered checks C2 and C3 of ADR 0046 (limits fixed before the fix): the periodic Couette start-up is bounded by the wall
/// speed, stays x-uniform and has no cross flow, for PISO and PIMPLE at d = 0.5 ... 100.
fn c2_case(nx: usize, d: f64, pimple: bool) -> (f64, f64, f64) {
    let ny = 40;
    let dt = d / (ny * ny) as f64;
    let mut s = couette(nx, ny, true, |st| {
        if pimple {
            st.n_outer = 3;
            st.n_correctors = 1;
            st.alpha_u = 0.8;
            st.alpha_p = 0.5;
        }
    });
    let (mut peak, mut vmax, mut xvar) = (0.0f64, 0.0f64, 0.0f64);
    for _ in 0..60 {
        s.step(dt);
        peak = peak.max(umax(&s));
        vmax = s.u.values.iter().fold(vmax, |m, v| m.max(v[1].abs()));
        for j in 0..ny {
            for i in 1..nx {
                xvar = xvar.max((s.u.values[nx * j + i][0] - s.u.values[nx * j][0]).abs());
            }
        }
        if !peak.is_finite() || peak > 1e6 {
            break;
        }
    }
    eprintln!("W26A C2 nx {nx} d {d} pimple {pimple}: peak {peak:.6e} (limit 1+1e-9), max|v| {vmax:.2e}, x variation {xvar:.2e} (limits 1e-10)");
    (peak, vmax, xvar)
}

#[test]
fn c2_c3_periodic_couette_startup_is_stable_and_x_uniform() {
    let mut bad = Vec::new();
    for pimple in [false, true] {
        for nx in [4, 40] {
            for d in [0.5, 1.6, 3.0, 6.0, 16.0, 100.0] {
                let (p, v, x) = c2_case(nx, d, pimple);
                if !(p <= 1.0 + 1e-9 && v <= 1e-10 && x <= 1e-10) {
                    bad.push((pimple, nx, d, p, v, x));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
}

/// Residual observation of ADR 0046: non-periodic x (zero-gradient ends), nx = 4, large d.
#[test]
#[ignore = "diagnosis"]
fn nonperiodic_nx4_scan() {
    for d in [16.0, 30.0, 50.0, 100.0, 400.0] {
        run("base", 4, 40, false, d, 60, |_| {});
        run("noddt", 4, 40, false, d, 60, |s| s.ddt_correction = false);
        run("1corr", 4, 40, false, d, 60, |s| s.n_correctors = 1);
        run("4corr", 4, 40, false, d, 60, |s| s.n_correctors = 4);
        run("nx8", 8, 40, false, d, 60, |_| {});
    }
}

/// Closed lid-driven box (all walls fixed, lid speed 1, nu = 1): impulsive start at large d.
fn closed_box(lid: f64, d: f64, label: &str, set: impl FnOnce(&mut PimpleSettings)) -> f64 {
    let n = 20;
    let mesh = box_2d(n, n, 1.0, 1.0, false, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for w in ["xmin", "xmax", "ymin"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    u.set_bc(&geom, "ymax", Box::new(FixedValue::vector([lid, 0.0, 0.0])))
        .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut st = PimpleSettings::piso(1.0);
    st.momentum_solver.tol_rel = 1e-12;
    st.pressure_solver.tol_rel = 1e-12;
    set(&mut st);
    let mut s = PimpleSolver::new(geom, u, p, st);
    let dt = d / (n * n) as f64;
    let mut peak = 0.0f64;
    for _ in 0..40 {
        s.step(dt);
        let m = s.u.values.iter().fold(0.0f64, |m, v| m.max(v[0].abs()).max(v[1].abs()));
        peak = peak.max(m);
        if !peak.is_finite() || peak > 1e6 {
            break;
        }
    }
    eprintln!(
        "W26A closed box lid {lid:e} d {d} {label}: peak |u|/lid {:.4e}",
        peak / lid
    );
    peak / lid
}

#[test]
#[ignore = "diagnosis"]
fn closed_box_scan() {
    for (lid, d) in [1.0f64, 1e-3]
        .into_iter()
        .flat_map(|l| [1.0, 10.0, 100.0, 1000.0].map(move |d| (l, d)))
    {
        closed_box(lid, d, "piso", |_| {});
        closed_box(lid, d, "piso 4 corr", |s| s.n_correctors = 4);
        closed_box(lid, d, "piso 20 corr", |s| s.n_correctors = 20);
        closed_box(lid, d, "pimple 3 .8/.5", |s| {
            s.n_outer = 3;
            s.n_correctors = 1;
            s.alpha_u = 0.8;
            s.alpha_p = 0.5;
        });
        closed_box(lid, d, "pimple 40 .7/.3 1corr", |s| {
            s.n_outer = 40;
            s.n_correctors = 1;
            s.alpha_u = 0.7;
            s.alpha_p = 0.3;
        });
        closed_box(lid, d, "pimple 40 .7/.3 2corr", |s| {
            s.n_outer = 40;
            s.n_correctors = 2;
            s.alpha_u = 0.7;
            s.alpha_p = 0.3;
        });
    }
}

#[test]
#[ignore = "diagnosis"]
fn closed_box_locate() {
    let n = 20;
    let mesh = box_2d(n, n, 1.0, 1.0, false, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for w in ["xmin", "xmax", "ymin"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    u.set_bc(
        &geom,
        "ymax",
        Box::new(FixedValue::vector([
            std::env::var("LID").map_or(1e-3, |v| v.parse().unwrap()),
            0.0,
            0.0,
        ])),
    )
    .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut st = PimpleSettings::piso(1.0);
    st.momentum_solver.tol_rel = 1e-12;
    st.pressure_solver.tol_rel = 1e-12;
    let mut s = PimpleSolver::new(geom.clone(), u, p, st);
    let noise: f64 = std::env::var("NOISE").map_or(0.0, |v| v.parse().unwrap());
    let mut seed = 1u64;
    for v in s.u.values.iter_mut() {
        for k in 0..2 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            v[k] = noise * (((seed >> 11) as f64 / (1u64 << 53) as f64) - 0.5);
        }
    }
    let dt = 100.0 / (n * n) as f64;
    for i in 0..14 {
        s.step(dt);
        let (mut mu, mut ic) = (0.0f64, 0);
        for (c, v) in s.u.values.iter().enumerate() {
            let m = v[0].abs().max(v[1].abs());
            if m > mu {
                mu = m;
                ic = c;
            }
        }
        let pm = s.p.values.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        let cc = geom.mesh.cell_centres()[ic];
        eprintln!(
            "W26A step {i}: max|u| {mu:.3e} at cell ({:.3},{:.3}) u=({:.2e},{:.2e}) max|p| {pm:.2e} res_p {:.1e}",
            cc[0],
            cc[1],
            s.u.values[ic][0],
            s.u.values[ic][1],
            s.history.last().unwrap().res_p
        );
    }
}

#[test]
#[ignore = "diagnosis"]
fn closed_box_threshold() {
    for d in [1.0, 2.0, 3.0, 4.0, 6.0, 8.0, 10.0, 20.0] {
        for m in [2usize, 3] {
            closed_box(1e-3, d, &format!("piso {m} corr"), |s| s.n_correctors = m);
        }
    }
}

/// Characterisation (limit 1 = the lid speed, the physical bound of the Stokes box; not a registered criterion): with enough
/// pressure correctors the wall-bounded step is stable at any d, so the instability of the 2-corrector splitting at d > ~20 is the
/// non-converged corrector iteration (ADR 0046, D2), not a coding defect.
#[test]
fn closed_box_with_many_correctors_is_bounded_at_large_d() {
    for d in [10.0, 100.0, 1000.0] {
        let r = closed_box(1e-3, d, "piso 20 corr", |s| s.n_correctors = 20);
        assert!(r.is_finite() && r <= 1.0, "d {d}: {r}");
    }
}
