//! F-246 (ADR 0193 amendment 1): diagnosis runs of the default kernel (V0) behind the amendment, convergence only (no error of
//! a variant is computed). `diag_w` runs the manufactured solution of `pressure_interp_f246.rs` on the warped mesh W (amplitude
//! `F246_AMP`, default F-208's 0.1) at `F246_N` (default 32) with the set-up `F246_DIAG` (A the registered one, A200 the same
//! for 200 iterations, B to M the alternatives of the amendment) and viscosity `F246_NU` (default 0.1); `diag_w_quality` prints
//! the geometry of W at n = 16, 32, 64; `diag_box` runs the stratified box of B on K at n = 16 with V0 and the set-up `F246_DIAG`
//! (a to d). Run by name: `cargo test --release -p rcfd-fv --test pressure_interp_f246_diag -- --ignored --nocapture`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{FixedValue, Profile};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::simple::{BodyForce, Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_mesh, warp, Grading};
use rcfd_mesh::quality::check_mesh;
use std::sync::Arc;
use std::time::Instant;

const TP: f64 = 2.0 * std::f64::consts::PI;

fn env_f(name: &str, default: f64) -> f64 {
    std::env::var(name).ok().and_then(|x| x.parse().ok()).unwrap_or(default)
}

/// Warp amplitude of W.
fn amp() -> f64 {
    env_f("F246_AMP", 0.1)
}

/// Kinematic viscosity of the manufactured solution.
fn nu() -> f64 {
    env_f("F246_NU", 0.1)
}

/// The F-208 meshes K and W (W with the amplitude `amp`).
fn geometry_of(w: bool, n: usize) -> Arc<FvGeometry> {
    let b = box_mesh([n; 3], [1.0; 3], [Grading::Uniform; 3]).unwrap();
    let m = if w {
        warp(&b, |p| {
            [
                p[0] + amp() * (TP * p[1]).sin() * (TP * p[2]).sin(),
                p[1] + amp() * (TP * p[0]).sin() * (TP * p[2]).sin(),
                p[2] + amp() * (TP * p[0]).sin() * (TP * p[1]).sin(),
            ]
        })
    } else {
        warp(&b, |p| [p[0] + 0.7 * p[1] + 0.3 * p[2], p[1] + 0.3 * p[2], p[2]])
    }
    .unwrap();
    Arc::new(FvGeometry::new(Arc::new(m)).unwrap())
}

// ---- manufactured solution (ADR 0193 M) ------------------------------------------------------------------------------------------

/// Velocity; every component is independent of its own coordinate, so the divergence is zero exactly.
fn exact_u(x: Vec3) -> Vec3 {
    [
        (1.1 * x[1] + 0.3).sin() * (0.9 * x[2] + 0.2).cos(),
        (1.2 * x[0] + 0.1).cos() * (0.8 * x[2] + 0.4).sin(),
        (0.7 * x[0] + 0.5).sin() * (1.3 * x[1] + 0.2).cos(),
    ]
}

/// Kinematic pressure.
fn exact_p(x: Vec3) -> f64 {
    (0.9 * x[0] + 0.3).cos() * (1.2 * x[1] + 0.1).sin() * (0.8 * x[2] + 0.5).cos()
}

fn grad_p(x: Vec3) -> Vec3 {
    let (s1, c1) = (0.9 * x[0] + 0.3).sin_cos();
    let (s2, c2) = (1.2 * x[1] + 0.1).sin_cos();
    let (s3, c3) = (0.8 * x[2] + 0.5).sin_cos();
    [-0.9 * s1 * s2 * c3, 1.2 * c1 * c2 * c3, -0.8 * c1 * s2 * s3]
}

/// Source acceleration `f = (u . grad) u - nu lap u + grad p`.
fn source(x: Vec3) -> Vec3 {
    let u = exact_u(x);
    let (sa1, ca1) = (1.1 * x[1] + 0.3).sin_cos();
    let (sb1, cb1) = (0.9 * x[2] + 0.2).sin_cos();
    let (sa2, ca2) = (1.2 * x[0] + 0.1).sin_cos();
    let (sb2, cb2) = (0.8 * x[2] + 0.4).sin_cos();
    let (sa3, ca3) = (0.7 * x[0] + 0.5).sin_cos();
    let (sb3, cb3) = (1.3 * x[1] + 0.2).sin_cos();
    // derivatives that are not zero
    let (u_y, u_z) = (1.1 * ca1 * cb1, -0.9 * sa1 * sb1);
    let (v_x, v_z) = (-1.2 * sa2 * sb2, 0.8 * ca2 * cb2);
    let (w_x, w_y) = (0.7 * ca3 * cb3, -1.3 * sa3 * sb3);
    let conv = [
        u[1] * u_y + u[2] * u_z,
        u[0] * v_x + u[2] * v_z,
        u[0] * w_x + u[1] * w_y,
    ];
    let k2 = [1.1 * 1.1 + 0.9 * 0.9, 1.2 * 1.2 + 0.8 * 0.8, 0.7 * 0.7 + 1.3 * 1.3];
    let gp = grad_p(x);
    [0, 1, 2].map(|i| conv[i] + nu() * k2[i] * u[i] + gp[i])
}

/// Diagnosis of the W set-up (ADR 0193 set-up-defect rule): V0 on W, convergence only (no error is computed).
#[allow(clippy::too_many_arguments)]
fn run(
    n: usize,
    label: &str,
    coupling: Coupling,
    au: f64,
    ap: f64,
    corr: usize,
    recon: bool,
    conv: ConvScheme,
    cap: usize,
) {
    run_x(n, label, coupling, au, ap, corr, recon, conv, cap, 0)
}
#[allow(clippy::too_many_arguments)]
fn run_x(
    n: usize,
    label: &str,
    coupling: Coupling,
    au: f64,
    ap: f64,
    corr: usize,
    recon: bool,
    conv: ConvScheme,
    cap: usize,
    extra: u32,
) {
    let start = Instant::now();
    let geom = geometry_of(true, n);
    let mesh = &geom.mesh;
    let (cc, cf, sf) = (mesh.cell_centres(), mesh.face_centres(), mesh.face_area_vectors());
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let n_int = geom.n_internal;
    let bfaces: Vec<usize> = (n_int..mesh.n_faces()).filter(|&f| !geom.empty[f]).collect();
    let e_net: f64 = bfaces.iter().map(|&f| vec3::dot(exact_u(cf[f]), sf[f])).sum();
    let a_b: f64 = bfaces.iter().map(|&f| vec3::norm(sf[f])).sum();
    let ub = |f: usize| vec3::sub(exact_u(cf[f]), vec3::scale(vec3::unit(sf[f]), e_net / a_b));
    let mut u = VectorField::from_fn("U", &geom, exact_u);
    for p in mesh.patches() {
        let vals: Vec<Vec3> = p.faces().map(ub).collect();
        u.set_bc(&geom, &p.name, Box::new(FixedValue(Profile::PerFace(vals))))
            .unwrap();
    }
    let p = ScalarField::from_fn("p", &geom, exact_p);
    let cell: Vec<Vec3> = cc.iter().map(|&x| source(x)).collect();
    let face: Vec<f64> = (0..mesh.n_faces())
        .map(|f| {
            if f < n_int {
                let w = geom.weights[f];
                vec3::dot(
                    vec3::add(vec3::scale(cell[own[f]], w), vec3::scale(cell[nei[f]], 1.0 - w)),
                    sf[f],
                )
            } else {
                vec3::dot(grad_p(cf[f]), sf[f])
            }
        })
        .collect();
    let mut s = SimpleSettings::new(nu(), coupling);
    s.alpha_u = au;
    s.alpha_p = ap;
    s.convection = conv;
    s.non_orth_correctors = corr;
    s.simplec_non_orth_compensation = coupling == Coupling::Simplec;
    s.momentum_solver.tol_abs = 1e-300;
    s.pressure_solver.tol_abs = 1e-300;
    let mut sol = SimpleSolver::new(geom.clone(), u, p, s);
    sol.body_force = Some(BodyForce { cell, face });
    sol.reconstructed_correction = recon;
    if extra == 1 {
        sol.pseudo = Some(rcfd_fv::simple::PseudoTransient {
            mode: rcfd_fv::simple::PseudoMode::Local { cfl: 1.0 },
            factor: 1.0,
        });
    }
    if extra == 2 {
        sol.settings.h_from_solution = false;
    }
    if extra == 3 {
        sol.pressure_interpolation = rcfd_fv::pressure_interp::PressureInterpolation::SkewCorrected;
    }
    let umax = cc.iter().map(|&x| vec3::norm(exact_u(x))).fold(0.0, f64::max);
    let mut d: Vec<f64> = Vec::new();
    let mut line = String::new();
    let mut outcome = "cap";
    for k in 1..=cap {
        let i = sol.iterate();
        d.push(i.max_du);
        if k % 100 == 0 || (k <= 60 && k % 3 == 0) {
            line.push_str(&format!(" {k}:{:.1e}/{:.1e}/{:.1e}", i.max_du, i.res_u, i.res_p));
        }
        if !i.max_du.is_finite() {
            outcome = "diverged";
            break;
        }
        if k >= 100 {
            let rho = (d[k - 1] / d[k - 51]).powf(1.0 / 50.0);
            if rho < 1.0 && d[k - 1] * rho / (1.0 - rho) <= 1e-10 * umax {
                outcome = "converged";
                break;
            }
        }
    }
    let (mut worst, mut wc) = (0.0f64, 0usize);
    for c in 0..cc.len() {
        let e = vec3::norm(vec3::sub(sol.u.values[c], exact_u(cc[c])));
        if e > worst {
            worst = e;
            wc = c;
        }
    }
    println!("W n={n} {label}: worst cell {wc} at {:?} (|du| {worst:.2e})", cc[wc]);
    println!(
        "W n={n} {label}: {outcome} after {} iterations, {:.0} s;{line}",
        d.len(),
        start.elapsed().as_secs_f64()
    );
}

#[test]
#[ignore = "diagnosis"]
fn diag_w() {
    let c = ConvScheme::Central;
    let which = std::env::var("F246_DIAG").unwrap_or_default();
    let n: usize = std::env::var("F246_N").ok().and_then(|x| x.parse().ok()).unwrap_or(32);
    match which.as_str() {
        "A" => run(n, "A registered", Coupling::Simple, 0.7, 0.3, 1, false, c, 3000),
        "A200" => run(
            n,
            "A registered, 200 iterations",
            Coupling::Simple,
            0.7,
            0.3,
            1,
            false,
            c,
            200,
        ),
        "B" => run(n, "B correctors 2", Coupling::Simple, 0.7, 0.3, 2, false, c, 3000),
        "C" => run(n, "C face_flux", Coupling::Simple, 0.7, 0.3, 1, true, c, 3000),
        "D" => run(n, "D alpha 0.5/0.2", Coupling::Simple, 0.5, 0.2, 1, false, c, 4000),
        "E" => run(
            n,
            "E linear_upwind",
            Coupling::Simple,
            0.7,
            0.3,
            1,
            false,
            ConvScheme::LinearUpwind,
            3000,
        ),
        "F" => run(
            n,
            "F simplec 0.9/0.7 comp face_flux",
            Coupling::Simplec,
            0.9,
            0.7,
            1,
            true,
            c,
            3000,
        ),
        "G" => run(n, "G correctors 0", Coupling::Simple, 0.7, 0.3, 0, false, c, 3000),
        "H" => run(
            n,
            "H upwind correctors 0",
            Coupling::Simple,
            0.7,
            0.3,
            0,
            false,
            ConvScheme::Upwind,
            3000,
        ),
        "J" => run(
            n,
            "J upwind correctors 0 short",
            Coupling::Simple,
            0.7,
            0.3,
            0,
            false,
            ConvScheme::Upwind,
            30,
        ),
        "K" => run_x(
            n,
            "K pseudo local cfl 1",
            Coupling::Simple,
            0.7,
            0.3,
            1,
            false,
            c,
            60,
            1,
        ),
        "L" => run(n, "L alpha 0.2/0.1", Coupling::Simple, 0.2, 0.1, 1, false, c, 60),
        "M" => run_x(
            n,
            "M previous-iterate H",
            Coupling::Simple,
            0.7,
            0.3,
            1,
            false,
            c,
            60,
            2,
        ),
        "I" => run(
            n,
            "I upwind correctors 1",
            Coupling::Simple,
            0.7,
            0.3,
            1,
            false,
            ConvScheme::Upwind,
            3000,
        ),
        other => panic!("set F246_DIAG to a set-up of the amendment (got {other:?})"),
    }
}

#[test]
#[ignore = "diagnosis"]
fn diag_w_quality() {
    for n in [16usize, 32, 64] {
        let geom = geometry_of(true, n);
        let mesh = &geom.mesh;
        let (own, nei, cc, cf, sf) = (
            mesh.owner(),
            mesh.neighbour(),
            mesh.cell_centres(),
            mesh.face_centres(),
            mesh.face_area_vectors(),
        );
        let (mut wrong, mut wmin, mut wmax, mut lapmax) = (0, 1.0f64, 0.0f64, 0.0f64);
        for f in 0..geom.n_internal {
            let a = vec3::dot(sf[f], vec3::sub(cf[f], cc[own[f]]));
            let b = vec3::dot(sf[f], vec3::sub(cc[nei[f]], cf[f]));
            if a <= 0.0 || b <= 0.0 {
                wrong += 1;
            }
            wmin = wmin.min(geom.weights[f]);
            wmax = wmax.max(geom.weights[f]);
            lapmax = lapmax.max(geom.lap_coeff[f] * vec3::norm(geom.delta[f]) / vec3::norm(sf[f]));
        }
        let q = check_mesh(mesh);
        println!("W n={n}: centres on the wrong side of a face {wrong}; weights [{wmin:.3}, {wmax:.3}]; max lap |d|/|S| {lapmax:.2}; nonorth {:.2}; skew {:.3}; min vol {:.3e}; vol ratio {:.2}", q.max_non_orthogonality, q.max_skewness, q.min_volume, q.max_volume_ratio);
    }
}

/// B-a diagnosis: the stratified box on K at n = 16 with V0, largest speed and change after `iters` (no comparison with V2).
#[test]
#[ignore = "diagnosis"]
fn diag_box() {
    use rcfd_fv::energy::boussinesq_force;
    let which = std::env::var("F246_DIAG").unwrap_or_default();
    let (coupling, ap, corr, recon) = match which.as_str() {
        "a" => (Coupling::Simplec, 1.0, 0, false),
        "b" => (Coupling::Simplec, 1.0, 0, true),
        "c" => (Coupling::Simplec, 0.7, 0, false),
        "d" => (Coupling::Simple, 0.3, 0, false),
        other => panic!("set F246_DIAG to a, b, c or d (got {other:?})"),
    };
    let geom = geometry_of(false, 16);
    let mesh = &geom.mesh;
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    let mut t = ScalarField::from_fn("T", &geom, |x| x[2]);
    for p in mesh.patches() {
        u.set_bc(&geom, &p.name, Box::new(FixedValue::vector([0.0; 3])))
            .unwrap();
        let tb: Vec<Vec3> = p.faces().map(|f| [mesh.face_centres()[f][2], 0.0, 0.0]).collect();
        t.set_bc(&geom, &p.name, Box::new(FixedValue(Profile::PerFace(tb))))
            .unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = SimpleSettings::new(0.1, coupling);
    s.alpha_p = ap;
    s.non_orth_correctors = corr;
    let mut sol = SimpleSolver::new(geom.clone(), u, p, s);
    sol.body_force = Some(boussinesq_force(&geom, &t, 1.0, [0.0, 0.0, -10.0], 0.5));
    sol.reconstructed_correction = recon;
    let mut line = String::new();
    for k in 1..=400 {
        let i = sol.iterate();
        if k % 40 == 0 || k <= 10 {
            line.push_str(&format!(" {k}:{:.1e}", i.max_du));
        }
        if !i.max_du.is_finite() {
            line.push_str(&format!(" NaN at {k}"));
            break;
        }
    }
    println!("box {which}:{line}");
}
