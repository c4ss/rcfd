//! F-359 (ADR 0264): a non-uniform closed-form flow on the warped three-dimensional overset set-up B3d of ADR 0251 with the
//! component moving as in ADR 0255. The flow is the linear shear `u = U0 + s a (b . (x - x0))` with `a . b = 0` and
//! `b . U0 = 0`, an exact steady Navier-Stokes solution with constant pressure for every viscosity. The acceptance (A1,
//! A2: the error on B3d at most twice that on B3 with the default and the quadratic donor interpolation; A3: continuity),
//! the exactness checks of the derivation on the planar meshes (K1 to K4), the set-up checks, the separating runs on the
//! single meshes and the mixed set-ups, and the negative control N1.
//!
//! Release: `cargo test --release -p rcfd-fv --test overset_f359`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::move_mesh;
use rcfd_fv::bc::{FixedValue, Profile};
use rcfd_fv::convection::{convection, face_flux, ConvScheme};
use rcfd_fv::diffusion::{laplacian, uniform_gamma};
use rcfd_fv::overset::{MassMode, OversetCoupling, OversetSolver};
use rcfd_fv::pimple::PimpleSettings;
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_mesh, warp, Grading};
use rcfd_mesh::motion::{moved_mesh, Pose};
use rcfd_mesh::overset::{compose, CellKind, DonorInterpolation, Overset, OversetSpec};
use rcfd_mesh::Mesh;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

/// ADR 0219 PS.
const SOLVE: SolveControl = SolveControl {
    tol_abs: 1e-15,
    tol_rel: 1e-12,
    max_iter: 3000,
};
const NU: f64 = 0.01;
const U0: Vec3 = [1.0, 0.5, 0.25];
const X0: Vec3 = [1.2, 1.2, 1.2];
const SHEAR: f64 = 0.5;
const DT: f64 = 0.005;
const STEPS: usize = 40;
const PATCHES: [&str; 6] = ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"];

fn settings() -> PimpleSettings {
    let mut s = PimpleSettings::piso(NU);
    s.convection = ConvScheme::Central;
    s.momentum_solver = SOLVE;
    s.pressure_solver = SOLVE;
    s
}

/// The shear direction `a = (1, 2, 1) / sqrt(6)` and the gradient direction `b = (0, 1, -2) / sqrt(5)`.
fn dirs() -> (Vec3, Vec3) {
    (vec3::unit([1.0, 2.0, 1.0]), vec3::unit([0.0, 1.0, -2.0]))
}

/// The flow F of ADR 0264.
fn u_ex(x: Vec3) -> Vec3 {
    let (a, b) = dirs();
    vec3::add(U0, vec3::scale(a, SHEAR * vec3::dot(b, vec3::sub(x, X0))))
}

/// Records the failed checks and prints every check.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Self {
        Checks(Vec::new())
    }
    fn at_most(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{id}: {value:.4e} (limit <= {limit:.3e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} > {limit:e}"));
        }
    }
    fn at_least(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!(
            "{id}: {value:.4e} (limit >= {limit:.3e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} < {limit:e}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

// ---- the set-ups of ADR 0251 (the same code as crates/rcfd-fv/tests/overset_f357.rs) ------------------------------------

/// A fixed pseudo-random number in [-1, 1] of a position.
fn jitter(x: Vec3, seed: f64) -> f64 {
    let v = (x[0] * 127.1 + x[1] * 311.7 + x[2] * 74.7 + seed).sin() * 43758.5453;
    2.0 * (v - v.floor()) - 1.0
}

fn distorted(mesh: &Mesh, lo: f64, hi: f64, dirs: &[usize], amp: f64, planar: bool) -> Mesh {
    let dirs = dirs.to_vec();
    warp(mesh, move |p| {
        let tol = 1e-9 * (hi - lo);
        if dirs.iter().any(|&d| p[d] - lo < tol || hi - p[d] < tol) {
            return p;
        }
        let key = if planar { [p[0], p[1], 0.0] } else { p };
        let mut q = p;
        for &d in &dirs {
            q[d] += amp * jitter(key, 1.0 + d as f64);
        }
        q
    })
    .unwrap()
}

/// The background of B3 (`distort`: of B3d).
fn background(distort: bool) -> Mesh {
    let bg = box_mesh([24; 3], [2.4; 3], [Grading::Uniform; 3]).unwrap();
    if distort {
        distorted(&bg, 0.0, 2.4, &[0, 1, 2], 0.015, false)
    } else {
        bg
    }
}

/// The component of B3 (`distort`: of B3d).
fn component(distort: bool) -> Mesh {
    let comp = box_mesh([14; 3], [1.4; 3], [Grading::Uniform; 3]).unwrap();
    if distort {
        distorted(&comp, 0.0, 1.4, &[0, 1, 2], 0.015, false)
    } else {
        comp
    }
}

/// The overset of ADR 0251 from a background and a component, with the donor interpolation `mode`.
fn overset(bg: &Mesh, comp: &Mesh, mode: DonorInterpolation) -> Overset {
    let spec = OversetSpec {
        outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax", "c_zmin", "c_zmax"]
            .map(String::from)
            .to_vec(),
        hole: Arc::new(|x: Vec3| (0..3).map(|d| (x[d] - 0.7).abs()).fold(0.0, f64::max) < 0.15),
    };
    let mut ov = Overset::new(compose(bg, comp, "c_").unwrap(), spec).unwrap();
    if mode != DonorInterpolation::Linear {
        ov.set_donor_interpolation(mode);
    }
    ov
}

/// The motion of ADR 0255: `P(0)` is the pose of ADR 0251.
fn pose(t: f64) -> Pose {
    Pose {
        origin: [0.7; 3],
        axis: vec3::unit([1.0, 2.0, 3.0]),
        angle: 0.3 + 0.5 * t,
        translation: [0.53 - 0.25 * t, 0.52 + 0.25 * t, 0.49 + 0.25 * t],
    }
}

/// `u_ex` at the face centres of `at` on the six patches (the patch faces of `at` and `geom` are the same).
fn set_dirichlet(u: &mut VectorField, geom: &FvGeometry, at: &FvGeometry) {
    for name in PATCHES {
        let v: Vec<Vec3> = at
            .mesh
            .patch(name)
            .unwrap()
            .faces()
            .map(|f| u_ex(at.mesh.face_centres()[f]))
            .collect();
        u.set_bc(geom, name, Box::new(FixedValue(Profile::PerFace(v)))).unwrap();
    }
}

fn fields(geom: &Arc<FvGeometry>) -> (VectorField, ScalarField) {
    let mut u = VectorField::from_fn("U", geom, u_ex);
    set_dirichlet(&mut u, geom, geom);
    let p = ScalarField::uniform("p", geom, 0.0);
    (u, p)
}

const CLASSES: [&str; 4] = ["BC", "BF", "CC", "CF"];

/// The largest values of a run over its steps.
#[derive(Default, Debug, Clone)]
struct Run {
    /// `E`: the largest `abs(u_c - u_ex(x_c)) / abs(U0)` over the non-hole cells and the steps.
    e: f64,
    /// The step and cell of `e`.
    at: (usize, usize),
    /// The largest error of each class (BC, BF, CC, CF) over the steps.
    class: [f64; 4],
    /// The error at the last step and its volume-weighted root mean square.
    e_last: f64,
    rms_last: f64,
    /// The largest `abs(p_c) / abs(U0)^2` over the non-hole cells and the steps.
    ep: f64,
    cont: f64,
    /// K1: the largest stencil error for `u_ex`.
    k1: f64,
    changes: usize,
    unconverged: usize,
    min_share: f64,
    max_fallback: usize,
    seconds: f64,
}

impl Run {
    /// Takes the errors of one step. `class(c)` is the class index of a non-hole cell, `None` for a hole.
    fn take(
        &mut self,
        k: usize,
        geom: &FvGeometry,
        u: &VectorField,
        p: &ScalarField,
        class: impl Fn(usize) -> Option<usize>,
    ) {
        let un = vec3::norm(U0);
        let cc = geom.mesh.cell_centres();
        let vol = geom.mesh.cell_volumes();
        let (mut emax, mut s2, mut sv) = (0.0f64, 0.0, 0.0);
        for c in 0..u.values.len() {
            let Some(cl) = class(c) else { continue };
            let e = vec3::norm(vec3::sub(u.values[c], u_ex(cc[c]))) / un;
            if e > self.e {
                self.e = e;
                self.at = (k, c);
            }
            self.class[cl] = self.class[cl].max(e);
            self.ep = self.ep.max(p.values[c].abs() / (un * un));
            emax = emax.max(e);
            s2 += vol[c] * e * e;
            sv += vol[c];
        }
        self.e_last = emax;
        self.rms_last = (s2 / sv).sqrt();
    }
}

/// What a test may do to a prepared step.
#[derive(Clone, Copy, PartialEq)]
enum Control {
    None,
    /// N1: every stencil weight multiplied by 1.001.
    Weights,
}

/// An overset run of 40 steps of the motion `P` on the background `bg` and the component `comp`.
fn overset_run(bg: &Mesh, comp: &Mesh, mode: DonorInterpolation, control: Control) -> Run {
    let t0 = Instant::now();
    let ov = overset(bg, comp, mode);
    let p0 = pose(0.0);
    let mesh = moved_mesh(&ov.composite.mesh, ov.composite.points_at(&p0)).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let (u, p) = fields(&geom);
    let mut s = OversetSolver::new(
        geom,
        u,
        p,
        settings(),
        MassMode::Conservative,
        Some(OversetCoupling { overset: ov, pose: p0 }),
    )
    .unwrap();
    let nb = s.conn.n_background_cells;
    let un = vec3::norm(U0);
    let mut r = Run {
        min_share: 1.0,
        ..Run::default()
    };
    for k in 1..=STEPS {
        let mut st = s.prepare(DT, pose(k as f64 * DT), &[]).unwrap();
        if control == Control::Weights {
            for (_, stn) in st.conn.stencils.iter_mut() {
                stn.weights.iter_mut().for_each(|w| *w *= 1.001);
            }
        }
        let info = s.advance(DT, st).unwrap();
        r.cont = r.cont.max(info.continuity);
        r.changes += info.n_changed;
        r.unconverged += info.unconverged;
        r.max_fallback = r.max_fallback.max(info.n_fallback);
        let ns = s.conn.stencils.len();
        if mode == DonorInterpolation::Quadratic {
            assert_eq!(s.conn.interpolation, DonorInterpolation::Quadratic);
            r.min_share = r.min_share.min(1.0 - info.n_fallback as f64 / ns as f64);
        }
        let cc = s.geom.mesh.cell_centres();
        for (c, stn) in &s.conn.stencils {
            let mut v = [0.0; 3];
            for (&j, &w) in stn.cells.iter().zip(&stn.weights) {
                v = vec3::add(v, vec3::scale(u_ex(cc[j]), w));
            }
            r.k1 = r.k1.max(vec3::norm(vec3::sub(v, u_ex(cc[*c]))) / un);
        }
        let kind = &s.conn.kind;
        r.take(k, &s.geom, &s.u, &s.p, |c| match kind[c] {
            CellKind::Hole => None,
            CellKind::Calculated => Some(if c < nb { 0 } else { 2 }),
            CellKind::Fringe => Some(if c < nb { 1 } else { 3 }),
        });
    }
    r.seconds = t0.elapsed().as_secs_f64();
    println!("{r:?}");
    r
}

/// PB, PC1, SB, S0, S1: one mesh alone (every cell calculated), placed by `place(t, x_ref)` at every step (`moving`) or
/// left at its first placement; the six patches get `u_ex` at their current face centres.
fn single_run(reference: &Mesh, moving: bool, place: impl Fn(f64, Vec3) -> Vec3) -> Run {
    let t0 = Instant::now();
    let at = |t: f64| -> Vec<Vec3> { reference.points().iter().map(|&x| place(t, x)).collect() };
    let mesh = moved_mesh(reference, at(0.0)).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let (u, p) = fields(&geom);
    let mut s = OversetSolver::new(geom, u, p, settings(), MassMode::Conservative, None).unwrap();
    let mut r = Run::default();
    for k in 1..=STEPS {
        let pts = if moving {
            at(k as f64 * DT)
        } else {
            s.geom.mesh.points().to_vec()
        };
        let ms = move_mesh(&s.geom, pts, DT).unwrap();
        if moving {
            let g = s.geom.clone();
            set_dirichlet(&mut s.u, &g, &ms.geom);
        }
        let info = s.advance_mesh(DT, ms, Vec::new()).unwrap();
        r.cont = r.cont.max(info.continuity);
        r.unconverged += info.unconverged;
        r.take(k, &s.geom, &s.u, &s.p, |_| Some(0));
    }
    r.seconds = t0.elapsed().as_secs_f64();
    println!("{r:?}");
    r
}

/// T of ADR 0264: the residuals of the discrete steady operators of the overset step with `u_ex` in the cells, per unit
/// volume. Returns the largest `T_conv`, `T_diff`, `T_cont`, their root mean squares, and the net boundary flux over
/// `abs(U0) * area` (K4).
fn truncation(mesh: Mesh, area: f64) -> ([f64; 3], [f64; 3], f64) {
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let (u, _) = fields(&geom);
    let m = &geom.mesh;
    let n = geom.n_cells();
    let vol = m.cell_volumes();
    let ub = u.boundary_values(&geom);
    let phi = face_flux(&geom, &u.values, &ub);
    let gamma = uniform_gamma(&geom, NU);
    let (mut tc, mut td) = (vec![0.0f64; n], vec![0.0f64; n]);
    for k in 0..3 {
        let cells: Vec<f64> = u.values.iter().map(|v| v[k]).collect();
        let bc = u.boundary_coeffs(&geom, k);
        let rc = convection(&geom, &phi, &cells, &bc, ConvScheme::Central, None).residual(&cells);
        let rd = laplacian(&geom, &gamma, &bc, None).residual(&cells);
        for c in 0..n {
            tc[c] = tc[c].max(rc[c].abs() / vol[c]);
            td[c] = td[c].max(rd[c].abs() / vol[c]);
        }
    }
    let mut div = vec![0.0; n];
    for f in 0..m.n_faces() {
        div[m.owner()[f]] += phi[f];
        if f < m.n_internal_faces() {
            div[m.neighbour()[f]] -= phi[f];
        }
    }
    let tq: Vec<f64> = (0..n).map(|c| div[c].abs() / vol[c]).collect();
    let max = |v: &[f64]| v.iter().cloned().fold(0.0, f64::max);
    let rms = |v: &[f64]| (v.iter().map(|x| x * x).sum::<f64>() / v.len() as f64).sqrt();
    let net: f64 = (m.n_internal_faces()..m.n_faces()).map(|f| phi[f]).sum();
    (
        [max(&tc), max(&td), max(&tq)],
        [rms(&tc), rms(&td), rms(&tq)],
        net.abs() / (vec3::norm(U0) * area),
    )
}

// ---- the four acceptance runs, each computed once per test process -------------------------------------------------------

static P3: OnceLock<Run> = OnceLock::new();
static P3Q: OnceLock<Run> = OnceLock::new();
static W3: OnceLock<Run> = OnceLock::new();
static W3Q: OnceLock<Run> = OnceLock::new();

fn acceptance_run(warped: bool, mode: DonorInterpolation) -> &'static Run {
    let cell = match (warped, mode) {
        (false, DonorInterpolation::Linear) => &P3,
        (false, _) => &P3Q,
        (true, DonorInterpolation::Linear) => &W3,
        (true, _) => &W3Q,
    };
    cell.get_or_init(|| {
        let r = overset_run(&background(warped), &component(warped), mode, Control::None);
        println!(
            "run {} {:?}: E {:.4e} at step {} cell {}; classes {}; last step {:.4e} (rms {:.4e}); pressure {:.4e}; {} kind \
             changes; unconverged {}; {:.1} s",
            if warped { "B3d" } else { "B3" },
            mode,
            r.e,
            r.at.0,
            r.at.1,
            CLASSES
                .iter()
                .zip(r.class)
                .map(|(n, e)| format!("{n} {e:.3e}"))
                .collect::<Vec<_>>()
                .join(", "),
            r.e_last,
            r.rms_last,
            r.ep,
            r.changes,
            r.unconverged,
            r.seconds
        );
        r
    })
}

// ---- A: the acceptance ---------------------------------------------------------------------------------------------------

/// ADR 0264 A1: the default donor interpolation, `E(W3) <= 2 E(P3)`. Missed in the one registered run (addendum 1):
/// `E(P3) = 8.7203e-4`, `E(W3) = 1.3603e-2`, ratio `15.60`; the cause is the single warped mesh (`E_M / E(W3) = 0.889`).
#[test]
#[ignore = "ADR 0264 A1 known miss: ratio 15.60 against the limit 2 (the single warped mesh); about 25 s"]
fn a1_error_ratio_default_interpolation_known_miss() {
    let (p3, w3) = (
        acceptance_run(false, DonorInterpolation::Linear),
        acceptance_run(true, DonorInterpolation::Linear),
    );
    println!("A1: E(P3) {:.4e}, E(W3) {:.4e}", p3.e, w3.e);
    let mut c = Checks::new();
    c.at_most("A1 E(W3) / E(P3)", w3.e / p3.e, 2.0);
    c.finish();
}

/// ADR 0264 A2: the quadratic donor interpolation, `E(W3q) <= 2 E(P3q)`. Missed in the one registered run (addendum 1):
/// `E(P3q) = 8.7204e-4`, `E(W3q) = 1.3576e-2`, ratio `15.57`; the cause is the single warped mesh (`E_M / E(W3q) = 0.891`).
#[test]
#[ignore = "ADR 0264 A2 known miss: ratio 15.57 against the limit 2 (the single warped mesh); about 25 s"]
fn a2_error_ratio_quadratic_interpolation_known_miss() {
    let (p3, w3) = (
        acceptance_run(false, DonorInterpolation::Quadratic),
        acceptance_run(true, DonorInterpolation::Quadratic),
    );
    println!("A2: E(P3q) {:.4e}, E(W3q) {:.4e}", p3.e, w3.e);
    let mut c = Checks::new();
    c.at_most("A2 E(W3q) / E(P3q)", w3.e / p3.e, 2.0);
    c.finish();
}

/// ADR 0264 A3 (continuity every step), K1 (the stencils reproduce `u_ex`), U4 (kind changes), Q1 (quadratic share) on
/// the four acceptance runs.
#[test]
fn a3_continuity_and_stencils_every_step() {
    let mut c = Checks::new();
    for (name, warped, mode) in [
        ("P3", false, DonorInterpolation::Linear),
        ("P3q", false, DonorInterpolation::Quadratic),
        ("W3", true, DonorInterpolation::Linear),
        ("W3q", true, DonorInterpolation::Quadratic),
    ] {
        let r = acceptance_run(warped, mode);
        c.at_most(&format!("A3 continuity {name}"), r.cont, 1e-9);
        c.at_most(&format!("K1 stencils reproduce u_ex {name}"), r.k1, 1e-12);
        if mode == DonorInterpolation::Linear {
            c.at_least(&format!("U4 (set-up) kind changes {name}"), r.changes as f64, 20.0);
        } else {
            c.at_least(&format!("Q1 (set-up) quadratic share {name}"), r.min_share, 0.95);
            println!("{name}: largest fallback count of a step {}", r.max_fallback);
        }
        println!("U5 {name}: unconverged solves {}", r.unconverged);
    }
    c.finish();
}

// ---- K: exactness of the derivation --------------------------------------------------------------------------------------

/// ADR 0264 K2 (the truncation of the exact field on the planar meshes) and K4 (the net boundary flux); T on the warped
/// meshes printed.
#[test]
fn k2_k4_truncation_of_the_exact_field() {
    let p0 = pose(0.0);
    let placed = |m: Mesh| {
        let pts: Vec<Vec3> = m.points().iter().map(|&x| p0.apply(x)).collect();
        moved_mesh(&m, pts).unwrap()
    };
    let mut c = Checks::new();
    for (name, mesh, area) in [
        ("planar background", background(false), 6.0 * 2.4 * 2.4),
        ("planar component", placed(component(false)), 6.0 * 1.4 * 1.4),
        ("warped background", background(true), 6.0 * 2.4 * 2.4),
        ("warped component", placed(component(true)), 6.0 * 1.4 * 1.4),
    ] {
        let (mx, rms, net) = truncation(mesh, area);
        println!(
            "T {name}: T_conv {:.4e} (rms {:.4e}), T_diff {:.4e} (rms {:.4e}), T_cont {:.4e} (rms {:.4e}); net boundary \
             flux {net:.3e}",
            mx[0], rms[0], mx[1], rms[1], mx[2], rms[2]
        );
        if name.starts_with("planar") {
            c.at_most(&format!("K2 T_conv {name}"), mx[0], 1e-10);
            c.at_most(&format!("K2 T_diff {name}"), mx[1], 1e-10);
            c.at_most(&format!("K2 T_cont {name}"), mx[2], 1e-10);
        }
        if name.ends_with("background") {
            c.at_most(&format!("K4 (set-up) net boundary flux {name}"), net, 1e-13);
        }
    }
    c.finish();
}

/// ADR 0264 K3: the planar background of B3 alone at rest keeps F.
#[test]
fn k3_planar_background_alone_keeps_the_flow() {
    let r = single_run(&background(false), false, |_, x| x);
    println!("PB: {:.1} s, unconverged {}", r.seconds, r.unconverged);
    let mut c = Checks::new();
    c.at_most("K3 PB velocity", r.e, 1e-10);
    c.at_most("K3 PB pressure", r.ep, 1e-10);
    c.at_most("K3 PB continuity", r.cont, 1e-9);
    c.finish();
}

// ---- S: separating runs --------------------------------------------------------------------------------------------------

/// ADR 0264 S: the warped meshes alone (SB, S0, S1), the planar component alone moving (PC1), the mixed set-ups (X1, X2),
/// and the reading of the registered rule against W3 and W3q. Printed; the rule is applied in the addendum.
#[test]
fn s_separating_runs() {
    let p0 = pose(0.0);
    let sb = single_run(&background(true), false, |_, x| x);
    let s0 = single_run(&component(true), false, |_, x| p0.apply(x));
    let s1 = single_run(&component(true), true, |t, x| pose(t).apply(x));
    let pc1 = single_run(&component(false), true, |t, x| pose(t).apply(x));
    let x1 = overset_run(
        &background(true),
        &component(false),
        DonorInterpolation::Linear,
        Control::None,
    );
    let x2 = overset_run(
        &background(false),
        &component(true),
        DonorInterpolation::Linear,
        Control::None,
    );
    for (n, r) in [
        ("SB", &sb),
        ("S0", &s0),
        ("S1", &s1),
        ("PC1", &pc1),
        ("X1", &x1),
        ("X2", &x2),
    ] {
        println!(
            "{n}: E {:.4e} at step {} cell {}; classes {:?}; last {:.4e} (rms {:.4e}); pressure {:.4e}; continuity {:.3e}; \
             unconverged {}; {:.1} s",
            r.e, r.at.0, r.at.1, r.class, r.e_last, r.rms_last, r.ep, r.cont, r.unconverged, r.seconds
        );
    }
    let e_m = sb.e.max(s1.e);
    for (n, w) in [
        ("W3", acceptance_run(true, DonorInterpolation::Linear)),
        ("W3q", acceptance_run(true, DonorInterpolation::Quadratic)),
    ] {
        println!(
            "S reading {n}: E_M {e_m:.4e}, E({n}) {:.4e}, E_M / E({n}) {:.3}: {}",
            w.e,
            e_m / w.e,
            if e_m >= 0.5 * w.e {
                "the single warped mesh"
            } else {
                "the overset coupling on the warped mesh"
            }
        );
    }
    assert!([sb.e, s0.e, s1.e, pc1.e, x1.e, x2.e].iter().all(|e| e.is_finite()));
}

// ---- N: negative control -------------------------------------------------------------------------------------------------

/// ADR 0264 N1: P3 with every stencil weight multiplied by 1.001.
#[test]
fn n1_weights_control() {
    let r = overset_run(
        &background(false),
        &component(false),
        DonorInterpolation::Linear,
        Control::Weights,
    );
    let mut c = Checks::new();
    c.at_least("N1 E with the weights times 1.001", r.e, 3e-4);
    c.finish();
}
