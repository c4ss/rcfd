//! Wave 27C (ADR 0051) registered checks P1-P3, P5: coupled viscoelastic plane channel flow (periodic in x, no-slip walls).
use rcfd_fv::bc::FixedValue;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::simple::BodyForce;
use rcfd_fv::viscoelastic::*;
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::box_2d;
use std::sync::Arc;

const NX: usize = 4;

fn no_slip(geom: &FvGeometry) -> VectorField {
    let mut u = VectorField::uniform("U", geom, [0.0; 3]);
    u.set_bc(geom, "ymin", Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    u.set_bc(geom, "ymax", Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    u
}

fn channel_geom(ny: usize) -> Arc<FvGeometry> {
    let mesh = box_2d(NX, ny, 1.0, 1.0, true, false).unwrap();
    Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap())
}

fn make_solver(ny: usize, nu_s: f64, g: f64, model: Model, form: Formulation) -> ViscoelasticSolver {
    let geom = channel_geom(ny);
    let u = no_slip(&geom);
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = PimpleSettings::piso(nu_s);
    s.n_correctors = 4;
    s.momentum_solver.tol_rel = 1e-12;
    s.pressure_solver.tol_rel = 1e-12;
    ViscoelasticSolver::new(geom, u, p, s, model, form, [g, 0.0, 0.0]).unwrap()
}

/// Mean x-velocity of row `j`.
fn row_u(s: &ViscoelasticSolver, j: usize) -> f64 {
    (0..NX).map(|i| s.flow.u.values[NX * j + i][0]).sum::<f64>() / NX as f64
}

fn centre_u(s: &ViscoelasticSolver, ny: usize) -> f64 {
    0.5 * (row_u(s, ny / 2 - 1) + row_u(s, ny / 2))
}

// ---- reference solutions -------------------------------------------------------------------------------------------

/// Modal series of the Oldroyd-B start-up under a constant acceleration `g` (derivation in ADR 0051 P1): velocity at height `y`
/// (channel 0..1, half-width h = 0.5), rho = 1.
fn modal_u(y: f64, t: f64, g: f64, eta_s: f64, eta_p: f64, lam: f64) -> f64 {
    let h = 0.5;
    let eta = eta_s + eta_p;
    let mut sum = 0.0;
    for n in 1..=400usize {
        let k = (2 * n - 1) as f64 * std::f64::consts::PI / (2.0 * h);
        let gn = g * 4.0 * if n % 2 == 1 { 1.0 } else { -1.0 } / ((2 * n - 1) as f64 * std::f64::consts::PI);
        let uss = gn / (eta * k * k);
        // lam s^2 + (1 + lam eta_s k^2) s + eta k^2 = 0
        let (a, b, c) = (lam, 1.0 + lam * eta_s * k * k, eta * k * k);
        let disc = b * b - 4.0 * a * c;
        let an = if disc >= 0.0 {
            let sq = disc.sqrt();
            let (s1, s2) = ((-b + sq) / (2.0 * a), (-b - sq) / (2.0 * a));
            // A + B = -uss ; A s1 + B s2 = gn
            let aa = (gn + s2 * uss) / (s1 - s2);
            let bb = -uss - aa;
            uss + aa * (s1 * t).exp() + bb * (s2 * t).exp()
        } else {
            let sigma = -b / (2.0 * a);
            let omega = (-disc).sqrt() / (2.0 * a);
            let cc = (gn + sigma * uss) / omega;
            uss + (sigma * t).exp() * (-uss * (omega * t).cos() + cc * (omega * t).sin())
        };
        sum += an * (k * (y - 0.5)).cos();
    }
    sum
}

/// Independent method-of-lines solution of `u_t = g + eta_s u_yy + tau_y`, `lam tau_t = eta_p u_y - tau` (staggered grid,
/// RK4); returns the centre-line velocity at the requested times (multiples of `dt`).
fn pde_centre(g: f64, eta_s: f64, eta_p: f64, lam: f64, times: &[f64]) -> Vec<f64> {
    let n = 100usize;
    let dy = 1.0 / n as f64;
    let dt = 5e-5;
    let mut u = vec![0.0; n + 1];
    let mut tau = vec![0.0; n]; // at half nodes
    let rhs = |u: &[f64], tau: &[f64]| -> (Vec<f64>, Vec<f64>) {
        let mut du = vec![0.0; n + 1];
        let mut dtau = vec![0.0; n];
        for j in 1..n {
            du[j] = g + eta_s * (u[j + 1] - 2.0 * u[j] + u[j - 1]) / (dy * dy) + (tau[j] - tau[j - 1]) / dy;
        }
        for j in 0..n {
            dtau[j] = (eta_p * (u[j + 1] - u[j]) / dy - tau[j]) / lam;
        }
        (du, dtau)
    };
    let mut out = Vec::new();
    let mut t = 0.0;
    for &target in times {
        while t < target - 0.5 * dt {
            let (k1u, k1t) = rhs(&u, &tau);
            let ax = |a: &[f64], k: &[f64], f: f64| -> Vec<f64> { a.iter().zip(k).map(|(x, y)| x + f * y).collect() };
            let (k2u, k2t) = rhs(&ax(&u, &k1u, 0.5 * dt), &ax(&tau, &k1t, 0.5 * dt));
            let (k3u, k3t) = rhs(&ax(&u, &k2u, 0.5 * dt), &ax(&tau, &k2t, 0.5 * dt));
            let (k4u, k4t) = rhs(&ax(&u, &k3u, dt), &ax(&tau, &k3t, dt));
            for j in 0..=n {
                u[j] += dt / 6.0 * (k1u[j] + 2.0 * k2u[j] + 2.0 * k3u[j] + k4u[j]);
            }
            for j in 0..n {
                tau[j] += dt / 6.0 * (k1t[j] + 2.0 * k2t[j] + 2.0 * k3t[j] + k4t[j]);
            }
            t += dt;
        }
        out.push(u[n / 2]);
    }
    out
}

const P1: (f64, f64, f64, f64) = (8.0, 0.2, 0.8, 0.2); // g, eta_s, eta_p, lambda

fn p1_times() -> Vec<f64> {
    (1..=30).map(|k| 0.05 * k as f64).collect()
}

fn p1_error(ny: usize, form: Formulation) -> f64 {
    let (g, es, ep, lam) = P1;
    let model = Model::OldroydB { lambda: lam, eta_p: ep };
    let mut s = make_solver(ny, es, g, model, form);
    let dt = 5e-4;
    let u_ss = g * 0.25 / (2.0 * (es + ep));
    let mut worst = 0.0f64;
    for k in 1..=30 {
        for _ in 0..100 {
            s.step(dt);
        }
        let t = 0.05 * k as f64;
        let r = modal_u(0.5, t, g, es, ep, lam);
        worst = worst.max((centre_u(&s, ny) - r).abs() / u_ss);
    }
    worst
}

#[test]
fn p1_derivation_check_modal_series_vs_pde() {
    let (g, es, ep, lam) = P1;
    let times = p1_times();
    let reference = pde_centre(g, es, ep, lam, &times);
    let u_ss = g * 0.25 / (2.0 * (es + ep));
    let mut worst = 0.0f64;
    let mut peak = 0.0f64;
    for (t, r) in times.iter().zip(&reference) {
        let m = modal_u(0.5, *t, g, es, ep, lam);
        worst = worst.max((m - r).abs() / u_ss);
        peak = peak.max(m / u_ss);
    }
    eprintln!("W27C P1 modal series vs 1-D PDE: max |diff|/u_ss {worst:.3e} (limit 2e-3); peak u_c/u_ss {peak:.4} (overshoot)");
    assert!(worst <= 2e-3);
    assert!(
        peak > 1.0,
        "no elastic overshoot in the reference: the set-up does not exercise elasticity"
    );
}

#[test]
fn p1_startup_oldroyd_b_vs_modal_series() {
    for form in [Formulation::Conformation, Formulation::LogConformation] {
        let e40 = p1_error(40, form);
        let e20 = p1_error(20, form);
        eprintln!(
            "W27C P1 {form:?}: error ny=40 {e40:.3e} (limit 2e-2), ny=20 {e20:.3e}, ratio {:.3} (limit 0.75)",
            e40 / e20
        );
        assert!(e40 <= 2e-2, "{form:?} {e40:e}");
        assert!(e40 / e20 <= 0.75, "{form:?} ratio {}", e40 / e20);
    }
}

// ---- steady profiles ---------------------------------------------------------------------------------------------------

fn run_steady(model: Model, form: Formulation, ny: usize) -> ViscoelasticSolver {
    let mut s = make_solver(ny, 0.1, 8.0, model, form);
    for _ in 0..6000 {
        s.step(2e-3);
    }
    s
}

#[test]
fn p2_steady_oldroyd_b_equals_newtonian_total_viscosity() {
    let ny = 32;
    let model = Model::OldroydB {
        lambda: 1.0,
        eta_p: 0.9,
    };
    let s = run_steady(model, Formulation::LogConformation, ny);
    let dy = 1.0 / ny as f64;
    let mut worst = 0.0f64;
    for j in 0..ny {
        let y = (j as f64 + 0.5) * dy;
        worst = worst.max((row_u(&s, j) - 4.0 * y * (1.0 - y)).abs());
    }
    // total wall shear: solvent part from the first cell, polymer part from the first-cell stress (zero-gradient extrapolation)
    let tau_w = s.tau()[0][0][1];
    let shear = 0.1 * row_u(&s, 0) / (0.5 * dy) + tau_w;
    let e_w = (shear - 4.0).abs() / 4.0;
    eprintln!(
        "W27C P2: max |u - u_N|/u_N,c {worst:.3e} (limit 1e-2); wall shear {shear:.4} vs 4, rel {e_w:.3e} (limit 1e-2)"
    );
    assert!(worst <= 1e-2);
    assert!(e_w <= 1e-2);
}

/// Reference profile of the fully developed shear-thinning channel flow: total shear stress `G y'`, `eta_s g + tau(g) = G y'`.
fn quad_profile(model: &Model, eta_s: f64, g_acc: f64) -> impl Fn(f64) -> f64 {
    let n = 4000usize;
    let h = 0.5;
    let rate_at = |sigma: f64| -> f64 {
        let (mut lo, mut hi) = (0.0f64, 1.0f64);
        let tot = |g: f64| eta_s * g + steady_shear(model, g).0 * g;
        while tot(hi) < sigma {
            hi *= 2.0;
        }
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if tot(mid) < sigma {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        0.5 * (lo + hi)
    };
    // u(y') = int_{y'}^{h} rate dy'
    let ys: Vec<f64> = (0..=n).map(|i| h * i as f64 / n as f64).collect();
    let rates: Vec<f64> = ys.iter().map(|&y| rate_at(g_acc * y)).collect();
    let mut u = vec![0.0; n + 1];
    for i in (0..n).rev() {
        u[i] = u[i + 1] + 0.5 * (rates[i] + rates[i + 1]) * (ys[i + 1] - ys[i]);
    }
    move |yp: f64| {
        let x = (yp / h * n as f64).clamp(0.0, n as f64 - 1e-9);
        let i = x.floor() as usize;
        let f = x - i as f64;
        u[i] * (1.0 - f) + u[i + 1] * f
    }
}

fn p3_run(registered_sign: bool) {
    let ny = 32;
    let models = [
        Model::Giesekus {
            lambda: 1.0,
            eta_p: 0.9,
            alpha: 0.2,
        },
        Model::Ptt {
            lambda: 1.0,
            eta_p: 0.9,
            epsilon: 0.25,
        },
        Model::FeneP {
            lambda: 1.0,
            eta_p: 0.9,
            l2: 20.0,
        },
    ];
    for m in models {
        let reference = quad_profile(&m, 0.1, 8.0);
        let s = run_steady(m, Formulation::LogConformation, ny);
        let dy = 1.0 / ny as f64;
        let uc_ref = reference(0.0);
        let mut worst = 0.0f64;
        for j in 0..ny {
            let yp = ((j as f64 + 0.5) * dy - 0.5).abs();
            worst = worst.max((row_u(&s, j) - reference(yp)).abs() / uc_ref);
        }
        let uc = centre_u(&s, ny);
        // shape: u at y' = h/2 over the centre value; the Newtonian parabola gives 0.75
        let quarter = 0.5 * (row_u(&s, ny / 4 - 1) + row_u(&s, ny / 4)) / uc;
        eprintln!(
            "W27C P3 {m:?}: max |u-u_ref|/u_ref,c {worst:.3e} (limit 2e-2); centre {uc:.4} (ref {uc_ref:.4}, Newtonian total-viscosity 1.0); u(h/2)/u_c {quarter:.4} (Newtonian 0.75)"
        );
        assert!(worst <= 2e-2, "{m:?} {worst:e}");
        if registered_sign {
            assert!(
                uc < 1.0,
                "{m:?}: registered claim, centre velocity smaller than the Newtonian one"
            );
        } else {
            assert!(
                uc > 1.0,
                "{m:?}: shear thinning must raise the centre velocity above the Newtonian value"
            );
            assert!(quarter > 0.75, "{m:?}: profile must be flatter than the parabola");
        }
    }
}

/// Registered P3 as written: the claim "centre velocity smaller than the Newtonian one" has the wrong sign (a thinning fluid at the
/// same pressure gradient flows faster); set-up defect, kept as a record (ADR 0051 addendum).
#[test]
#[ignore = "set-up defect: registered sign of the centre-velocity claim is wrong; see p3_steady_shear_thinning_profiles_corrected"]
fn p3_steady_shear_thinning_profiles() {
    p3_run(true);
}

#[test]
fn p3_steady_shear_thinning_profiles_corrected() {
    p3_run(false);
}

#[test]
fn p5_newtonian_limit() {
    let (g, es, _ep, lam) = P1;
    let ny = 40;
    let dt = 5e-4;
    // plain solver
    let geom = channel_geom(ny);
    let u = no_slip(&geom);
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut st = PimpleSettings::piso(es);
    st.n_correctors = 4;
    st.momentum_solver.tol_rel = 1e-12;
    st.pressure_solver.tol_rel = 1e-12;
    let mut plain = PimpleSolver::new(geom.clone(), u, p, st);
    plain.source_force = Some(BodyForce::from_cells(&geom, vec![[g, 0.0, 0.0]; geom.n_cells()]));
    let mut zero = make_solver(
        ny,
        es,
        g,
        Model::OldroydB {
            lambda: lam,
            eta_p: 0.0,
        },
        Formulation::LogConformation,
    );
    let mut tiny = make_solver(
        ny,
        es,
        g,
        Model::OldroydB {
            lambda: lam,
            eta_p: 1e-10,
        },
        Formulation::LogConformation,
    );
    for _ in 0..200 {
        plain.step(dt);
        zero.step(dt);
        tiny.step(dt);
    }
    let (mut d0, mut d1) = (0.0f64, 0.0f64);
    for c in 0..geom.n_cells() {
        for k in 0..3 {
            d0 = d0.max((plain.u.values[c][k] - zero.flow.u.values[c][k]).abs());
            d1 = d1.max((plain.u.values[c][k] - tiny.flow.u.values[c][k]).abs());
        }
    }
    eprintln!("W27C P5: eta_p = 0 max diff {d0:e} (limit exactly 0); eta_p = 1e-10 max diff {d1:e} (limit 1e-8)");
    assert!(d0 == 0.0);
    assert!(d1 <= 1e-8);
}
