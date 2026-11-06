//! F-285 (ADR 0198): diagnosis runs of the default steady kernel on F-208's warped mesh W. Ignored tests, run by name in
//! release, each once: `cargo test --release -p rcfd-fv --test warped_kernel_f285_diag -- --ignored --nocapture <name>`.
//!
//! D0 the kernel (V0) on W: growth factor, sign of the mode, where it lives. D1 the correction step alone: power iteration of
//! linear maps of a cell velocity field (`Pi_GG`, `Pi_GG1`, `Pi_R`, `Pi_LS`, `Pi_GG,beta`) on W, W2, K and G. D3 the fix
//! candidates of the registration in the kernel (the test-only switch `SimpleSolver::correction_test_only` and the existing
//! `reconstructed_correction`). The meshes and the manufactured solution M are those of `pressure_interp_f246.rs`, copied
//! unchanged.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{FixedValue, Profile};
use rcfd_fv::convection::{face_flux, ConvScheme};
use rcfd_fv::gradient::{green_gauss, least_squares};
use rcfd_fv::pressure::{boundary_values_pbc, pressure_bc, solve_pressure, PressureBc, PressureControls};
use rcfd_fv::simple::{BodyForce, CorrectionTestOnly, Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::{LinearOptions, SolveControl};
use rcfd_mesh::gen::{box_mesh, warp, Grading};
use rcfd_mesh::Mesh;
use std::sync::Arc;
use std::time::Instant;

const TP: f64 = 2.0 * std::f64::consts::PI;
/// Kinematic viscosity of the manufactured solution.
const NU: f64 = 0.1;
/// ADR 0193 stop rule: estimated remaining iteration error over the largest exact speed.
const ITER_ERROR: f64 = 1e-10;

#[derive(Clone, Copy, Debug, PartialEq)]
enum MeshKind {
    G,
    K,
    W,
    /// ADR 0193 amendment 1: W with amplitude 0.07.
    W2,
}

/// The meshes of F-208 (copied from `pressure_interp_f246.rs`).
fn make_mesh(kind: MeshKind, n: usize) -> Mesh {
    let un = [Grading::Uniform; 3];
    match kind {
        MeshKind::G => box_mesh([n; 3], [1.0; 3], un).unwrap(),
        MeshKind::K => warp(&box_mesh([n; 3], [1.0; 3], un).unwrap(), |p| {
            [p[0] + 0.7 * p[1] + 0.3 * p[2], p[1] + 0.3 * p[2], p[2]]
        })
        .unwrap(),
        MeshKind::W | MeshKind::W2 => {
            let a = if kind == MeshKind::W { 0.1 } else { 0.07 };
            warp(&box_mesh([n; 3], [1.0; 3], un).unwrap(), |p| {
                [
                    p[0] + a * (TP * p[1]).sin() * (TP * p[2]).sin(),
                    p[1] + a * (TP * p[0]).sin() * (TP * p[2]).sin(),
                    p[2] + a * (TP * p[0]).sin() * (TP * p[1]).sin(),
                ]
            })
            .unwrap()
        }
    }
}

fn geometry(kind: MeshKind, n: usize) -> Arc<FvGeometry> {
    Arc::new(FvGeometry::new(Arc::new(make_mesh(kind, n))).unwrap())
}

/// Logical indices `(i, j, k)` of every cell, from the cell centres of the unwarped box (the warp moves points only).
fn logical(n: usize) -> Vec<[usize; 3]> {
    let m = box_mesh([n; 3], [1.0; 3], [Grading::Uniform; 3]).unwrap();
    m.cell_centres()
        .iter()
        .map(|c| c.map(|x| ((x * n as f64).floor() as usize).min(n - 1)))
        .collect()
}

/// Number of logical indices of a cell on the boundary layer `{0, n - 1}`: 2 or 3 for an edge cell, at least 1 for a wall cell.
fn on_boundary(ijk: [usize; 3], n: usize) -> usize {
    ijk.iter().filter(|&&i| i == 0 || i == n - 1).count()
}

// ---- manufactured solution (ADR 0193 M, copied) -----------------------------------------------------------------------------------

fn exact_u(x: Vec3) -> Vec3 {
    [
        (1.1 * x[1] + 0.3).sin() * (0.9 * x[2] + 0.2).cos(),
        (1.2 * x[0] + 0.1).cos() * (0.8 * x[2] + 0.4).sin(),
        (0.7 * x[0] + 0.5).sin() * (1.3 * x[1] + 0.2).cos(),
    ]
}

fn exact_p(x: Vec3) -> f64 {
    (0.9 * x[0] + 0.3).cos() * (1.2 * x[1] + 0.1).sin() * (0.8 * x[2] + 0.5).cos()
}

fn grad_p(x: Vec3) -> Vec3 {
    let (s1, c1) = (0.9 * x[0] + 0.3).sin_cos();
    let (s2, c2) = (1.2 * x[1] + 0.1).sin_cos();
    let (s3, c3) = (0.8 * x[2] + 0.5).sin_cos();
    [-0.9 * s1 * s2 * c3, 1.2 * c1 * c2 * c3, -0.8 * c1 * s2 * s3]
}

fn source(x: Vec3) -> Vec3 {
    let u = exact_u(x);
    let (sa1, ca1) = (1.1 * x[1] + 0.3).sin_cos();
    let (sb1, cb1) = (0.9 * x[2] + 0.2).sin_cos();
    let (sa2, ca2) = (1.2 * x[0] + 0.1).sin_cos();
    let (sb2, cb2) = (0.8 * x[2] + 0.4).sin_cos();
    let (sa3, ca3) = (0.7 * x[0] + 0.5).sin_cos();
    let (sb3, cb3) = (1.3 * x[1] + 0.2).sin_cos();
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
    [0, 1, 2].map(|i| conv[i] + NU * k2[i] * u[i] + gp[i])
}

/// The kernel of ADR 0193 M on `kind` at `n` (V0) with `alpha_u`, `alpha_p`; `cand` the D3 candidate.
fn kernel(kind: MeshKind, n: usize, au: f64, ap: f64, cand: Candidate) -> (Arc<FvGeometry>, SimpleSolver, f64) {
    let geom = geometry(kind, n);
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
            if geom.empty[f] {
                0.0
            } else if f < n_int {
                let w = geom.weights[f];
                let a = vec3::add(vec3::scale(cell[own[f]], w), vec3::scale(cell[nei[f]], 1.0 - w));
                vec3::dot(a, sf[f])
            } else {
                vec3::dot(grad_p(cf[f]), sf[f])
            }
        })
        .collect();
    let mut s = SimpleSettings::new(NU, Coupling::Simple);
    s.alpha_u = au;
    s.alpha_p = ap;
    s.convection = ConvScheme::Central;
    s.non_orth_correctors = 1;
    s.momentum_solver.tol_abs = 1e-300;
    s.pressure_solver.tol_abs = 1e-300;
    let mut sol = SimpleSolver::new(geom.clone(), u, p, s);
    sol.body_force = Some(BodyForce { cell, face });
    match cand {
        Candidate::Default => {}
        Candidate::R => sol.reconstructed_correction = true,
        Candidate::Ls => sol.correction_test_only = Some(CorrectionTestOnly::LeastSquares),
        Candidate::Beta(b) => sol.correction_test_only = Some(CorrectionTestOnly::Beta(b)),
    }
    let umax = cc.iter().map(|&x| vec3::norm(exact_u(x))).fold(0.0, f64::max);
    (geom, sol, umax)
}

/// D3 candidates (ADR 0198).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Candidate {
    Default,
    R,
    Ls,
    Beta(f64),
}

/// Volume-weighted inner product of two cell vector fields.
fn dot_v(vol: &[f64], a: &[Vec3], b: &[Vec3]) -> f64 {
    (0..a.len()).map(|c| vol[c] * vec3::dot(a[c], b[c])).sum()
}

/// `(f_edge, f_wall)` of a cell vector field (fractions of `sum V |a|^2`) and the cell of the largest `|a|`.
fn where_it_lives(n: usize, vol: &[f64], a: &[Vec3]) -> (f64, f64, usize) {
    let ijk = logical(n);
    let (mut tot, mut edge, mut wall, mut big, mut arg) = (0.0, 0.0, 0.0, 0.0, 0);
    for c in 0..a.len() {
        let e = vol[c] * vec3::dot(a[c], a[c]);
        tot += e;
        let b = on_boundary(ijk[c], n);
        if b >= 2 {
            edge += e;
        }
        if b >= 1 {
            wall += e;
        }
        let m = vec3::norm(a[c]);
        if m > big {
            big = m;
            arg = c;
        }
    }
    (edge / tot, wall / tot, arg)
}

// ---- D0 -------------------------------------------------------------------------------------------------------------------------

fn d0(kind: MeshKind, n: usize, label: &str, au: f64, ap: f64) {
    let start = Instant::now();
    let (geom, mut sol, _) = kernel(kind, n, au, ap, Candidate::Default);
    let vol = geom.mesh.cell_volumes().to_vec();
    let cc = geom.mesh.cell_centres();
    let mut d: Vec<f64> = Vec::new();
    let mut hist: Vec<(Vec<Vec3>, Vec<f64>)> = Vec::new();
    for k in 1..=400 {
        let i = sol.iterate();
        d.push(i.max_du);
        if k >= 398 {
            hist.push((sol.u.values.clone(), sol.p.values.clone()));
        }
        if !i.max_du.is_finite() {
            println!("D0 {label} {kind:?} n={n}: not finite at iteration {k}");
            return;
        }
    }
    let g = (d[399] / d[199]).powf(1.0 / 200.0);
    let du =
        |a: usize, b: usize| -> Vec<Vec3> { (0..vol.len()).map(|c| vec3::sub(hist[a].0[c], hist[b].0[c])).collect() };
    let (d400, d399) = (du(2, 1), du(1, 0));
    let c = dot_v(&vol, &d400, &d399) / (dot_v(&vol, &d400, &d400) * dot_v(&vol, &d399, &d399)).sqrt();
    let (f_edge, f_wall, arg) = where_it_lives(n, &vol, &d400);
    let ijk = logical(n)[arg];
    let z = (ijk[2] as f64 + 0.5) / n as f64;
    let dp_max = (0..vol.len())
        .map(|c| (hist[2].1[c] - hist[1].1[c]).abs())
        .fold(0.0, f64::max);
    let du_max = d400.iter().map(|v| vec3::norm(*v)).fold(0.0, f64::max);
    println!(
        "D0 {label} {kind:?} n={n}: g {g:.4}; c {c:.4}; f_edge {f_edge:.4}, f_wall {f_wall:.4}; largest |du_400| {du_max:.3e} at cell {arg} ijk {ijk:?} x {:?} sin(2 pi z_logical) {:.3}; max|dp|/max|du| {:.3e}; max_du at 1, 10, 100, 200, 300, 400: {:.2e} {:.2e} {:.2e} {:.2e} {:.2e} {:.2e}; {:.0} s",
        cc[arg],
        (TP * z).sin(),
        dp_max / du_max,
        d[0],
        d[9],
        d[99],
        d[199],
        d[299],
        d[399],
        start.elapsed().as_secs_f64()
    );
}

#[test]
#[ignore = "diagnosis (ADR 0198 D0)"]
fn d0_w32_a() {
    d0(MeshKind::W, 32, "A", 0.7, 0.3);
}

#[test]
#[ignore = "diagnosis (ADR 0198 D0)"]
fn d0_w32_l() {
    d0(MeshKind::W, 32, "L", 0.2, 0.1);
}

#[test]
#[ignore = "diagnosis (ADR 0198 D0)"]
fn d0_w16_a() {
    d0(MeshKind::W, 16, "A", 0.7, 0.3);
}

// ---- D1 -------------------------------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
enum Op {
    Gg,
    Gg1,
    R,
    Ls,
    GgBeta(f64),
}

struct SubMap {
    geom: Arc<FvGeometry>,
    pbc: PressureBc,
    gamma: Vec<f64>,
}

impl SubMap {
    fn new(geom: Arc<FvGeometry>) -> Self {
        let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
        for p in geom.mesh.patches() {
            u.set_bc(&geom, &p.name, Box::new(FixedValue::vector([0.0; 3])))
                .unwrap();
        }
        let p = ScalarField::uniform("p", &geom, 0.0);
        let pbc = pressure_bc(&geom, &u, &p);
        assert!(pbc.fixed_u.iter().all(|&x| x) && !pbc.has_dirichlet());
        let gamma = vec![1.0; geom.mesh.n_faces()];
        SubMap { geom, pbc, gamma }
    }

    fn apply(&self, v: &[Vec3], op: Op) -> Vec<Vec3> {
        let g = &self.geom;
        let mesh = &g.mesh;
        let nb = mesh.n_faces() - g.n_internal;
        let phi = face_flux(g, v, &vec![[0.0; 3]; nb]);
        let ctl = PressureControls {
            p_ref: (0, 0.0),
            non_orth_correctors: usize::from(op == Op::Gg1),
            solver: SolveControl {
                tol_abs: 1e-300,
                tol_rel: 1e-12,
                max_iter: 20000,
            },
            amg: true,
            linear: LinearOptions::default(),
        };
        let mut q = vec![0.0; v.len()];
        solve_pressure(g, &self.gamma, &self.pbc, &phi, &mut q, &ctl);
        let bq = boundary_values_pbc(g, &self.pbc, &q);
        let corr: Vec<Vec3> = match op {
            Op::Gg | Op::Gg1 => green_gauss(g, &q, &bq, 0),
            Op::GgBeta(b) => green_gauss(g, &q, &bq, 0)
                .into_iter()
                .map(|x| vec3::scale(x, b))
                .collect(),
            Op::Ls => least_squares(g, &q, &bq),
            Op::R => {
                // -R(-Lambda q): the reconstruction of the face correction, with the sign of a gradient
                let (own, nei, cf, cc, vol) = (
                    mesh.owner(),
                    mesh.neighbour(),
                    mesh.face_centres(),
                    mesh.cell_centres(),
                    mesh.cell_volumes(),
                );
                let mut r = vec![[0.0; 3]; v.len()];
                for f in 0..g.n_internal {
                    let dphi = -self.gamma[f] * g.lap_coeff[f] * (q[nei[f]] - q[own[f]]);
                    let (o, nn) = (own[f], nei[f]);
                    r[o] = vec3::add(r[o], vec3::scale(vec3::sub(cf[f], cc[o]), dphi / vol[o]));
                    r[nn] = vec3::sub(r[nn], vec3::scale(vec3::sub(cf[f], cc[nn]), dphi / vol[nn]));
                }
                r.into_iter().map(|x| vec3::scale(x, -1.0)).collect()
            }
        };
        v.iter().zip(&corr).map(|(a, b)| vec3::sub(*a, *b)).collect()
    }
}

/// Deterministic pseudo-random numbers in [-0.5, 0.5) (64-bit linear congruential generator, seed 0x5eed_f285).
fn lcg_field(n: usize) -> Vec<Vec3> {
    let mut x: u64 = 0x5eed_f285;
    let mut next = || {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (x >> 11) as f64 / (1u64 << 53) as f64 - 0.5
    };
    (0..n).map(|_| [next(), next(), next()]).collect()
}

fn d1(kind: MeshKind, n: usize, op: Op, steps: usize) {
    let start = Instant::now();
    let geom = geometry(kind, n);
    let vol = geom.mesh.cell_volumes().to_vec();
    let map = SubMap::new(geom.clone());
    let norm = |a: &[Vec3]| dot_v(&vol, a, a).sqrt();
    let mut v = lcg_field(vol.len());
    let s = norm(&v);
    v.iter_mut().for_each(|x| *x = vec3::scale(*x, 1.0 / s));
    let mut ratios = Vec::new();
    let mut c = 0.0;
    for step in 1..=steps {
        let w = map.apply(&v, op);
        let nw = norm(&w);
        ratios.push(nw);
        if step == steps {
            c = dot_v(&vol, &w, &v) / nw;
        }
        v = w.into_iter().map(|x| vec3::scale(x, 1.0 / nw)).collect();
    }
    let last = &ratios[steps - 100..];
    let rho = (last.iter().map(|r| r.ln()).sum::<f64>() / last.len() as f64).exp();
    let (f_edge, f_wall, arg) = where_it_lives(n, &vol, &v);
    println!(
        "D1 {op:?} {kind:?} n={n}: rho {rho:.4} (steps {}..{steps}); c {c:.4}; f_edge {f_edge:.4}, f_wall {f_wall:.4}; largest cell {arg} ijk {:?}; ratios at 1, 10, 50, 100, {steps}: {:.3} {:.3} {:.3} {:.3} {:.3}; {:.0} s",
        steps - 99,
        logical(n)[arg],
        ratios[0],
        ratios[9],
        ratios[49],
        ratios[99],
        ratios[steps - 1],
        start.elapsed().as_secs_f64()
    );
}

/// D1 on one mesh: every map of the registration.
fn d1_all(kind: MeshKind, n: usize) {
    for op in [
        Op::Gg,
        Op::Gg1,
        Op::R,
        Op::Ls,
        Op::GgBeta(0.75),
        Op::GgBeta(0.5),
        Op::GgBeta(0.25),
    ] {
        d1(kind, n, op, 300);
    }
}

#[test]
#[ignore = "diagnosis (ADR 0198 D1)"]
fn d1_w32() {
    d1_all(MeshKind::W, 32);
}

#[test]
#[ignore = "diagnosis (ADR 0198 D1)"]
fn d1_w16() {
    d1_all(MeshKind::W, 16);
}

#[test]
#[ignore = "diagnosis (ADR 0198 D1)"]
fn d1_w2_32() {
    d1_all(MeshKind::W2, 32);
}

#[test]
#[ignore = "diagnosis (ADR 0198 D1)"]
fn d1_k32() {
    d1_all(MeshKind::K, 32);
}

#[test]
#[ignore = "diagnosis (ADR 0198 D1)"]
fn d1_g32() {
    d1_all(MeshKind::G, 32);
}

#[test]
#[ignore = "diagnosis (ADR 0198 D1)"]
fn d1_w64_gg() {
    d1(MeshKind::W, 64, Op::Gg, 200);
}

// ---- D3 -------------------------------------------------------------------------------------------------------------------------

struct Run {
    converged: bool,
    iterations: usize,
    u: Vec<Vec3>,
    p: Vec<f64>,
    vol: Vec<f64>,
}

/// Set-up A with the candidate `cand` until ADR 0193's stop rule or `cap` iterations.
fn run_a(kind: MeshKind, n: usize, cand: Candidate, cap: usize) -> Run {
    let (geom, mut sol, umax) = kernel(kind, n, 0.7, 0.3, cand);
    let mut d: Vec<f64> = Vec::new();
    let mut converged = false;
    for k in 1..=cap {
        let i = sol.iterate();
        d.push(i.max_du);
        if !i.max_du.is_finite() {
            break;
        }
        if k >= 100 {
            let rho = (d[k - 1] / d[k - 51]).powf(1.0 / 50.0);
            if rho < 1.0 && d[k - 1] * rho / (1.0 - rho) <= ITER_ERROR * umax {
                converged = true;
                break;
            }
        }
    }
    Run {
        converged,
        iterations: d.len(),
        u: sol.u.values.clone(),
        p: sol.p.values.clone(),
        vol: geom.mesh.cell_volumes().to_vec(),
    }
}

/// Largest cell differences of `u` and of `p - mean(p)` between two runs.
fn differences(a: &Run, b: &Run) -> (f64, f64) {
    let tot: f64 = a.vol.iter().sum();
    let mean = |p: &[f64]| p.iter().zip(&a.vol).map(|(x, v)| x * v).sum::<f64>() / tot;
    let (ma, mb) = (mean(&a.p), mean(&b.p));
    let du =
        a.u.iter()
            .zip(&b.u)
            .map(|(x, y)| vec3::norm(vec3::sub(*x, *y)))
            .fold(0.0, f64::max);
    let dp =
        a.p.iter()
            .zip(&b.p)
            .map(|(x, y)| ((x - ma) - (y - mb)).abs())
            .fold(0.0, f64::max);
    (du, dp)
}

fn d3(kind: MeshKind, n: usize) {
    let start = Instant::now();
    let control = kind != MeshKind::W || n == 16;
    let base = control.then(|| run_a(kind, n, Candidate::Default, 5000));
    if let Some(b) = &base {
        println!(
            "D3 {kind:?} n={n} V0 without a switch: converged {}, {} iterations",
            b.converged, b.iterations
        );
    }
    for cand in [
        Candidate::R,
        Candidate::Ls,
        Candidate::Beta(0.75),
        Candidate::Beta(0.5),
        Candidate::Beta(0.25),
    ] {
        let r = run_a(kind, n, cand, 5000);
        let diff = base.as_ref().map(|b| differences(&r, b));
        println!(
            "D3 {kind:?} n={n} {cand:?}: converged {}, {} iterations; largest difference against V0 (u, p - mean) {diff:?}; {:.0} s",
            r.converged,
            r.iterations,
            start.elapsed().as_secs_f64()
        );
    }
}

#[test]
#[ignore = "diagnosis (ADR 0198 D3)"]
fn d3_w32() {
    d3(MeshKind::W, 32);
}

#[test]
#[ignore = "diagnosis (ADR 0198 D3)"]
fn d3_w16() {
    d3(MeshKind::W, 16);
}

#[test]
#[ignore = "diagnosis (ADR 0198 D3)"]
fn d3_k32() {
    d3(MeshKind::K, 32);
}

#[test]
#[ignore = "diagnosis (ADR 0198 D3)"]
fn d3_w2_32() {
    d3(MeshKind::W2, 32);
}
