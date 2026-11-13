//! F-487 (ADR 0387): a consistent cell pressure gradient in the overset geometry corrections: the flow V of ADR 0382 on B3
//! and B3d with the component moving as in ADR 0255, with `OversetSolver::set_consistent_geometry_corrections` (the
//! acceptance A1 to A3), ADR 0367's linear shear and uniform flow with it (A4), the warped background alone and the share of
//! the warped background in the remainder (A6), the gradient at operator level (K1 to K6), the set-up checks and refusals,
//! and the fingerprints D1c of the corrections with ADR 0382's L.
//!
//! Release: `cargo test --release -p rcfd-fv --test overset_f487`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::move_mesh;
use rcfd_fv::bc::{FaceCoeffs, FixedValue, Profile};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::overset::{GeometryCorrections, MassMode, OversetCoupling, OversetSolver};
use rcfd_fv::pimple::PimpleSettings;
use rcfd_fv::pressure::{boundary_values_pbc, pressure_bc, PressureBc};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_mesh, warp, Grading};
use rcfd_mesh::motion::{moved_mesh, Pose};
use rcfd_mesh::overset::{compose, CellKind, DonorInterpolation, Overset, OversetSpec};
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

/// ADR 0219 PS.
const SOLVE: SolveControl = SolveControl {
    tol_abs: 1e-15,
    tol_rel: 1e-12,
    max_iter: 3000,
};
const NU: f64 = 0.01;
const DT: f64 = 0.005;
const PATCHES: [&str; 6] = ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"];
/// The flow V of ADR 0382: box side, vortex amplitude, axial velocity.
const L: f64 = 2.4;
const VA: f64 = 1.0;
const WZ: f64 = 0.5;

fn k_wave() -> f64 {
    PI / L
}

/// `u_ex` of V at `(x, t)`.
fn u_ex(x: Vec3, t: f64) -> Vec3 {
    let k = k_wave();
    let (xi, eta) = (x[0] - L / 2.0, x[1] - L / 2.0);
    let f = (-2.0 * NU * k * k * t).exp();
    [
        -VA * (k * xi).cos() * (k * eta).sin() * f,
        VA * (k * xi).sin() * (k * eta).cos() * f,
        WZ,
    ]
}

/// `p_ex` of V at `(x, t)`.
fn p_ex(x: Vec3, t: f64) -> f64 {
    let k = k_wave();
    let (xi, eta) = (x[0] - L / 2.0, x[1] - L / 2.0);
    let f = (-2.0 * NU * k * k * t).exp();
    -(VA * VA / 4.0) * ((2.0 * k * xi).cos() + (2.0 * k * eta).cos()) * f * f
}

fn settings(p_level: f64) -> PimpleSettings {
    let mut s = PimpleSettings::piso(NU);
    s.convection = ConvScheme::Central;
    s.momentum_solver = SOLVE;
    s.pressure_solver = SOLVE;
    s.p_ref = (0, p_level);
    s
}

// ---- the set-ups of ADR 0251 (the same code as crates/rcfd-fv/tests/overset_f472.rs) ------------------------------------

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

/// `u_ex(., t)` at the face centres of the six background patches.
fn set_conditions(u: &mut VectorField, geom: &FvGeometry, t: f64) {
    for name in PATCHES {
        let v: Vec<Vec3> = geom
            .mesh
            .patch(name)
            .unwrap()
            .faces()
            .map(|f| u_ex(geom.mesh.face_centres()[f], t))
            .collect();
        u.set_bc(geom, name, Box::new(FixedValue(Profile::PerFace(v)))).unwrap();
    }
}

/// An overset solver at `P(0)` on `bg` and `comp` with V at `t = 0` (ADR 0382 set-ups).
fn vortex_solver(bg: &Mesh, comp: &Mesh, mode: DonorInterpolation) -> OversetSolver {
    let ov = overset(bg, comp, mode);
    let p0 = pose(0.0);
    let mesh = moved_mesh(&ov.composite.mesh, ov.composite.points_at(&p0)).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |x| u_ex(x, 0.0));
    set_conditions(&mut u, &geom, 0.0);
    let p = ScalarField::from_fn("p", &geom, |x| p_ex(x, 0.0));
    let level = p_ex(geom.mesh.cell_centres()[0], 0.0);
    OversetSolver::new(
        geom,
        u,
        p,
        settings(level),
        MassMode::Conservative,
        Some(OversetCoupling { overset: ov, pose: p0 }),
    )
    .unwrap()
}

/// One step of V: the boundary values at the new time, then the step to `P(k dt)`.
fn vortex_step(s: &mut OversetSolver, k: usize) -> Result<rcfd_fv::overset::OversetInfo, String> {
    let t = k as f64 * DT;
    let g = s.geom.clone();
    set_conditions(&mut s.u, &g, t);
    s.step(DT, pose(t), &[])
}

// ---- D1c: the fingerprints of the corrections with L -----------------------------------------------------------------------

/// FNV-1a over bytes.
struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
    fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 ^= x as u64;
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    fn f64s(&mut self, v: impl IntoIterator<Item = f64>) {
        for x in v {
            self.bytes(&x.to_bits().to_le_bytes());
        }
    }
}

/// 5 steps of V on B3d with the geometry corrections and ADR 0382's L on; a failing step folds its message and ends the run.
fn d1c_fingerprint(mode: DonorInterpolation) -> u64 {
    let mut s = vortex_solver(&background(true), &component(true), mode);
    s.set_geometry_corrections(true);
    s.set_least_squares_pressure_gradient(true);
    let mut h = Fnv::new();
    for k in 1..=5 {
        match vortex_step(&mut s, k) {
            Ok(_) => {
                for c in 0..3 {
                    h.f64s(s.u.values.iter().map(|v| v[c]));
                }
                h.f64s(s.p.values.iter().copied());
                h.f64s(s.phi.iter().copied());
            }
            Err(e) => {
                h.bytes(format!("step {k}: {e}").as_bytes());
                break;
            }
        }
    }
    h.0
}

fn d1c_fingerprints() -> [u64; 2] {
    [
        d1c_fingerprint(DonorInterpolation::Linear),
        d1c_fingerprint(DonorInterpolation::Quadratic),
    ]
}

/// ADR 0387 D1c record: the fingerprints printed (run on the tree before any library change of F-487).
#[test]
fn d1c_l_fingerprint_record() {
    let f = d1c_fingerprints();
    println!(
        "D1c fingerprints: {}",
        f.iter().map(|x| format!("{x:#018x}")).collect::<Vec<_>>().join(" ")
    );
}

/// ADR 0387 D1c: the fingerprints on this tree equal those of the tree before any library change of F-487 (recorded by
/// `d1c_l_fingerprint_record` on f49fdd1f, the base library of ea491736).
#[test]
fn d1c_l_fingerprint_equals_the_base_tree() {
    const BASE: [u64; 2] = [0x6820_a06f_1b6a_1496, 0x861a_b174_c9e7_dbe2];
    let f = d1c_fingerprints();
    for (i, (a, b)) in f.iter().zip(BASE).enumerate() {
        println!(
            "D1c {}: {a:#018x} (base {b:#018x}) {}",
            ["linear", "quadratic"][i],
            if *a == b { "equal" } else { "DIFFERENT" }
        );
    }
    assert_eq!(f, BASE, "D1c: the corrections with L changed");
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
    fn holds(&mut self, id: &str, ok: bool) {
        println!("{id}: {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(id.to_string());
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

// ---- K: the gradient at operator level -----------------------------------------------------------------------------------

/// The exact gradient of `p_ex(., 0)`.
fn grad_p_ex(x: Vec3) -> Vec3 {
    let k = k_wave();
    let (xi, eta) = (x[0] - L / 2.0, x[1] - L / 2.0);
    let a = VA * VA * k / 2.0;
    [a * (2.0 * k * xi).sin(), a * (2.0 * k * eta).sin(), 0.0]
}

/// A pressure condition on every boundary face of `geom`: a prescribed normal gradient `grad(x_f) . n` (the coefficients
/// `(1, dn G, 0, G)`), or with `value` the exact value `pf(x_f)` (the coefficients of a fixed value).
fn condition(geom: &FvGeometry, pf: &dyn Fn(Vec3) -> f64, grad: &dyn Fn(Vec3) -> Vec3, value: bool) -> PressureBc {
    let m = &geom.mesh;
    let nb = m.n_faces() - m.n_internal_faces();
    let coeffs = (0..nb)
        .map(|i| {
            let f = m.n_internal_faces() + i;
            let x = m.face_centres()[f];
            if value {
                FaceCoeffs::from_value(0.0, pf(x), geom.dn[i])
            } else {
                let gn = vec3::dot(grad(x), vec3::unit(m.face_area_vectors()[f]));
                FaceCoeffs {
                    a: 1.0,
                    b: geom.dn[i] * gn,
                    c: 0.0,
                    d: gn,
                }
            }
        })
        .collect();
    PressureBc {
        coeffs,
        fixed_u: vec![!value; nb],
        coupled: vec![None; nb],
    }
}

/// The cells of `geom` with a boundary face.
fn wall_cells(geom: &FvGeometry) -> Vec<bool> {
    let m = &geom.mesh;
    let mut at = vec![false; geom.n_cells()];
    for f in m.n_internal_faces()..m.n_faces() {
        at[m.owner()[f]] = true;
    }
    at
}

/// The gradient C and the plain least-squares gradient (with `boundary_values_pbc`) of `pf` under `pbc` on `geom`.
fn both_gradients(geom: &FvGeometry, pf: &dyn Fn(Vec3) -> f64, pbc: &PressureBc) -> (Vec<Vec3>, Vec<Vec3>) {
    let p: Vec<f64> = geom.mesh.cell_centres().iter().map(|&x| pf(x)).collect();
    let c = GeometryCorrections::consistent_pressure_gradients(geom, &p, pbc, None);
    let l = GeometryCorrections::gradients(geom, &p, &boundary_values_pbc(geom, pbc, &p), None);
    (c, l)
}

/// The largest `abs(g - grad(x_c)) / scale` over the cells where `pick` holds.
fn largest_error(
    geom: &FvGeometry,
    g: &[Vec3],
    grad: &dyn Fn(Vec3) -> Vec3,
    scale: f64,
    pick: &dyn Fn(usize) -> bool,
) -> f64 {
    let cc = geom.mesh.cell_centres();
    (0..geom.n_cells())
        .filter(|&c| pick(c))
        .map(|c| vec3::norm(vec3::sub(g[c], grad(cc[c]))) / scale)
        .fold(0.0, f64::max)
}

/// ADR 0387 K1 to K6: the gradient C at operator level.
#[test]
fn k1_k6_the_consistent_gradient() {
    let p0 = pose(0.0);
    let placed = |m: Mesh| {
        let pts: Vec<Vec3> = m.points().iter().map(|&x| p0.apply(x)).collect();
        moved_mesh(&m, pts).unwrap()
    };
    let c = [0.3, -0.7, 0.4];
    let lin = |x: Vec3| 2.0 + vec3::dot(c, x);
    let lin_grad = |_: Vec3| c;
    let cn = vec3::norm(c);
    let pv = |x: Vec3| p_ex(x, 0.0);
    let zero = |_: Vec3| [0.0; 3];
    let scale = VA * VA * k_wave() / 2.0;
    let mut ch = Checks::new();
    for (name, mesh) in [
        ("planar background", background(false)),
        ("planar component", placed(component(false))),
        ("warped background", background(true)),
        ("warped component", placed(component(true))),
    ] {
        let geom = FvGeometry::new(Arc::new(mesh)).unwrap();
        let all = |_: usize| true;
        // K1 to K3: a linear pressure under its prescribed normal gradients
        let (gc, gl) = both_gradients(&geom, &lin, &condition(&geom, &lin, &lin_grad, false));
        let (ec, el) = (
            largest_error(&geom, &gc, &lin_grad, cn, &all),
            largest_error(&geom, &gl, &lin_grad, cn, &all),
        );
        println!("K {name}: linear p with its normal gradients: C {ec:.4e}, plain least squares {el:.4e}");
        // the zero-gradient condition of the step (every patch a fixed velocity), for p_ex of V
        let zg = condition(&geom, &pv, &zero, false);
        let (vc, vl) = both_gradients(&geom, &pv, &zg);
        let wall = wall_cells(&geom);
        let (cw, lw) = (
            largest_error(&geom, &vc, &grad_p_ex, scale, &|k| wall[k]),
            largest_error(&geom, &vl, &grad_p_ex, scale, &|k| wall[k]),
        );
        let (ci, li) = (
            largest_error(&geom, &vc, &grad_p_ex, scale, &|k| !wall[k]),
            largest_error(&geom, &vl, &grad_p_ex, scale, &|k| !wall[k]),
        );
        let diff = (0..geom.n_cells())
            .map(|k| vec3::norm(vec3::sub(vc[k], vl[k])) / scale)
            .fold(0.0, f64::max);
        println!(
            "K {name}: p_ex of V, zero gradient: wall cells C {cw:.4e}, plain {lw:.4e}; interior cells C {ci:.4e}, \
             plain {li:.4e}; largest C - plain {diff:.4e}"
        );
        match name {
            "warped background" => {
                ch.at_most("K1 C of a linear pressure, warped background", ec, 1e-10);
                ch.at_least("K2 (control) plain least squares, warped background", el, 1e-3);
                ch.at_most(
                    "K5 C / plain for p_ex of V, wall cells of the warped background",
                    cw / lw,
                    0.7,
                );
                // K6: with exact values instead of the zero gradient, the interior cells bit for bit
                let ex = condition(&geom, &pv, &zero, true);
                let p: Vec<f64> = geom.mesh.cell_centres().iter().map(|&x| pv(x)).collect();
                let gx = GeometryCorrections::consistent_pressure_gradients(&geom, &p, &ex, None);
                let same = (0..geom.n_cells()).filter(|&k| !wall[k]).all(|k| gx[k] == vc[k]);
                ch.holds(
                    "K6 C under the zero gradient and the exact values, interior cells bit for bit",
                    same,
                );
            }
            "planar background" => {
                ch.at_most(
                    "K3 plain least squares of a linear pressure, planar background",
                    el,
                    1e-10,
                );
                ch.at_most("K4 C - plain for p_ex of V, planar background", diff, 1e-12);
                // the manual zero-gradient condition is the one pressure_bc gives V's fixed velocities
                let g = Arc::new(geom.clone());
                let mut u = VectorField::from_fn("U", &g, |x| u_ex(x, 0.0));
                set_conditions(&mut u, &g, 0.0);
                let pf = ScalarField::uniform("p", &g, 0.0);
                let step = pressure_bc(&g, &u, &pf);
                ch.holds(
                    "K4 (set-up) the zero-gradient condition is pressure_bc's",
                    step.coeffs == zg.coeffs && step.coupled.iter().all(|x| x.is_none()),
                );
            }
            "planar component" => ch.at_most("K4 C - plain for p_ex of V, planar component", diff, 1e-12),
            _ => {}
        }
    }
    ch.finish();
}

// ---- the runs of V -------------------------------------------------------------------------------------------------------

const STEPS: usize = 40;
const CLASSES: [&str; 6] = ["BC", "BCw", "BCi", "BF", "CC", "CF"];

/// The pressure gradient of a run: the default solver, the corrections with ADR 0382's L, or C.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Opt {
    Default,
    L,
    C,
}

fn apply(s: &mut OversetSolver, opt: Opt) {
    match opt {
        Opt::Default => {}
        Opt::L => {
            s.set_geometry_corrections(true);
            s.set_least_squares_pressure_gradient(true);
        }
        Opt::C => s.set_consistent_geometry_corrections(true),
    }
}

/// The largest values of a run over its steps.
#[derive(Default, Debug, Clone)]
struct Run {
    e: f64,
    at: (usize, usize),
    class: [f64; 6],
    e_last: f64,
    rms_last: f64,
    ep: f64,
    cont: f64,
    changes: usize,
    unconverged: usize,
    min_share: f64,
    seconds: f64,
}

impl Run {
    fn print(&self, name: &str) {
        println!(
            "run {name}: E {:.4e} at step {} cell {}; classes {}; last step {:.4e} (rms {:.4e}); pressure {:.4e}; \
             continuity {:.3e}; {} kind changes; unconverged {}; {:.1} s",
            self.e,
            self.at.0,
            self.at.1,
            CLASSES
                .iter()
                .zip(self.class)
                .map(|(n, e)| format!("{n} {e:.3e}"))
                .collect::<Vec<_>>()
                .join(", "),
            self.e_last,
            self.rms_last,
            self.ep,
            self.cont,
            self.changes,
            self.unconverged,
            self.seconds
        );
    }

    /// The errors of step `k` at time `t` over the cells `cells` (cell, class index: 0 BC, 3 BF, 4 CC, 5 CF), the wall
    /// cells `wall` splitting BC into BCw and BCi.
    fn take(&mut self, k: usize, t: f64, s: &OversetSolver, cells: &[(usize, usize)], wall: &[bool]) {
        let cc = s.geom.mesh.cell_centres();
        let vol = s.geom.mesh.cell_volumes();
        let (mut emax, mut s2, mut sv, mut dp) = (0.0f64, 0.0, 0.0, 0.0);
        for &(c, cl) in cells {
            let e = vec3::norm(vec3::sub(s.u.values[c], u_ex(cc[c], t))) / VA;
            if e > self.e {
                self.e = e;
                self.at = (k, c);
            }
            self.class[cl] = self.class[cl].max(e);
            if cl == 0 {
                let w = if wall[c] { 1 } else { 2 };
                self.class[w] = self.class[w].max(e);
            }
            emax = emax.max(e);
            s2 += vol[c] * e * e;
            sv += vol[c];
            dp += vol[c] * (s.p.values[c] - p_ex(cc[c], t));
        }
        let mean = dp / sv;
        for &(c, _) in cells {
            self.ep = self.ep.max((s.p.values[c] - p_ex(cc[c], t) - mean).abs() / (VA * VA));
        }
        self.e_last = emax;
        self.rms_last = (s2 / sv).sqrt();
    }
}

/// A run of 40 steps of V on B3 (`warped`: B3d) with the component moving.
fn vortex_run(warped: bool, mode: DonorInterpolation, opt: Opt) -> Run {
    let t0 = Instant::now();
    let mut s = vortex_solver(&background(warped), &component(warped), mode);
    apply(&mut s, opt);
    let nb = s.conn.n_background_cells;
    let wall = wall_cells(&s.geom);
    let mut r = Run {
        min_share: 1.0,
        ..Run::default()
    };
    for k in 1..=STEPS {
        let info = vortex_step(&mut s, k).unwrap();
        let t = k as f64 * DT;
        r.cont = r.cont.max(info.continuity);
        r.changes += info.n_changed;
        r.unconverged += info.unconverged;
        if mode == DonorInterpolation::Quadratic {
            assert_eq!(s.conn.interpolation, DonorInterpolation::Quadratic);
            let ns = s.conn.stencils.len();
            r.min_share = r.min_share.min(1.0 - info.n_fallback as f64 / ns as f64);
        }
        let cells: Vec<(usize, usize)> = (0..s.u.values.len())
            .filter_map(|c| match s.conn.kind[c] {
                CellKind::Hole => None,
                CellKind::Calculated => Some((c, if c < nb { 0 } else { 4 })),
                CellKind::Fringe => Some((c, if c < nb { 3 } else { 5 })),
            })
            .collect();
        r.take(k, t, &s, &cells, &wall);
    }
    r.seconds = t0.elapsed().as_secs_f64();
    r
}

/// A run of 40 steps of V on the background alone at rest (no component, no coupling).
fn background_run(warped: bool, opt: Opt) -> Run {
    let t0 = Instant::now();
    let geom = Arc::new(FvGeometry::new(Arc::new(background(warped))).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |x| u_ex(x, 0.0));
    set_conditions(&mut u, &geom, 0.0);
    let p = ScalarField::from_fn("p", &geom, |x| p_ex(x, 0.0));
    let level = p_ex(geom.mesh.cell_centres()[0], 0.0);
    let mut s = OversetSolver::new(geom, u, p, settings(level), MassMode::Conservative, None).unwrap();
    apply(&mut s, opt);
    let wall = wall_cells(&s.geom);
    let cells: Vec<(usize, usize)> = (0..s.geom.n_cells()).map(|c| (c, 0)).collect();
    let mut r = Run::default();
    for k in 1..=STEPS {
        let t = k as f64 * DT;
        let g = s.geom.clone();
        set_conditions(&mut s.u, &g, t);
        let ms = move_mesh(&g, g.mesh.points().to_vec(), DT).unwrap();
        let info = s.advance_mesh(DT, ms, Vec::new()).unwrap();
        r.cont = r.cont.max(info.continuity);
        r.unconverged += info.unconverged;
        r.take(k, t, &s, &cells, &wall);
    }
    r.seconds = t0.elapsed().as_secs_f64();
    r
}

static P3OC: OnceLock<Run> = OnceLock::new();
static P3QOC: OnceLock<Run> = OnceLock::new();
static W3OC: OnceLock<Run> = OnceLock::new();
static W3QOC: OnceLock<Run> = OnceLock::new();
static P3: OnceLock<Run> = OnceLock::new();
static P3Q: OnceLock<Run> = OnceLock::new();
static W3OL: OnceLock<Run> = OnceLock::new();
static W3QOL: OnceLock<Run> = OnceLock::new();
static B0: OnceLock<Run> = OnceLock::new();
static BDL: OnceLock<Run> = OnceLock::new();
static BDC: OnceLock<Run> = OnceLock::new();

/// The registered runs of V with the component, each computed once per test process.
fn run(warped: bool, quad: bool, opt: Opt) -> &'static Run {
    let (cell, name) = match (warped, quad, opt) {
        (false, false, Opt::C) => (&P3OC, "P3oC"),
        (false, true, Opt::C) => (&P3QOC, "P3qoC"),
        (true, false, Opt::C) => (&W3OC, "W3oC"),
        (true, true, Opt::C) => (&W3QOC, "W3qoC"),
        (false, false, Opt::Default) => (&P3, "P3"),
        (false, true, Opt::Default) => (&P3Q, "P3q"),
        (true, false, Opt::L) => (&W3OL, "W3oL"),
        (true, true, Opt::L) => (&W3QOL, "W3qoL"),
        _ => panic!("not a registered run of ADR 0387"),
    };
    let mode = if quad {
        DonorInterpolation::Quadratic
    } else {
        DonorInterpolation::Linear
    };
    cell.get_or_init(|| {
        let r = vortex_run(warped, mode, opt);
        r.print(name);
        r
    })
}

/// The registered runs of V on the background alone.
fn alone(warped: bool, opt: Opt) -> &'static Run {
    let (cell, name) = match (warped, opt) {
        (false, Opt::Default) => (&B0, "B0"),
        (true, Opt::L) => (&BDL, "BdL"),
        (true, Opt::C) => (&BDC, "BdC"),
        _ => panic!("not a registered run of ADR 0387"),
    };
    cell.get_or_init(|| {
        let r = background_run(warped, opt);
        r.print(name);
        r
    })
}

// ---- A: the acceptance ---------------------------------------------------------------------------------------------------

/// ADR 0387 A1, A1d (default interpolation) and A2, A2d (quadratic): the error of V on B3d with C at most twice that on B3
/// with C and with the default solver.
#[test]
fn a1_a2_error_ratios() {
    let mut c = Checks::new();
    for (id, quad) in [("A1", false), ("A2", true)] {
        let w = run(true, quad, Opt::C);
        let po = run(false, quad, Opt::C);
        let pd = run(false, quad, Opt::Default);
        println!(
            "{id}: E(B3d, C) {:.4e}, E(B3, C) {:.4e}, E(B3, default) {:.4e}; P-diff {:.3e}",
            w.e,
            po.e,
            pd.e,
            (po.e - pd.e).abs() / pd.e
        );
        c.at_most(&format!("{id} E(B3d C) / E(B3 C)"), w.e / po.e, 2.0);
        c.at_most(&format!("{id}d E(B3d C) / E(B3 default)"), w.e / pd.e, 2.0);
    }
    c.finish();
}

/// ADR 0387 A3 (continuity every step of the four runs with C), U4 (kind changes), Q1 (quadratic share), U5.
#[test]
fn a3_continuity_and_set_up_of_the_runs_with_c() {
    let mut c = Checks::new();
    for (name, warped, quad) in [
        ("P3oC", false, false),
        ("P3qoC", false, true),
        ("W3oC", true, false),
        ("W3qoC", true, true),
    ] {
        let r = run(warped, quad, Opt::C);
        c.at_most(&format!("A3 continuity {name}"), r.cont, 1e-9);
        if quad {
            c.at_least(&format!("Q1 (set-up) quadratic share {name}"), r.min_share, 0.95);
        } else {
            c.at_least(&format!("U4 (set-up) kind changes {name}"), r.changes as f64, 20.0);
        }
        println!("U5 {name}: unconverged solves {}", r.unconverged);
    }
    for (name, quad) in [("P3", false), ("P3q", true)] {
        println!(
            "U5 {name}: unconverged solves {}",
            run(false, quad, Opt::Default).unconverged
        );
    }
    c.finish();
}

/// ADR 0387 C0 (C acts) and A6: the share of the warped background in the remainder, from the background alone.
#[test]
fn c0_a6_share_of_the_warped_background() {
    let mut c = Checks::new();
    let b0 = alone(false, Opt::Default);
    for (opt, x) in [(Opt::L, "L"), (Opt::C, "C")] {
        let bd = alone(true, opt);
        for (quad, sfx) in [(false, ""), (true, "q")] {
            let w = run(true, quad, opt);
            let pd = run(false, quad, Opt::Default);
            let rem = w.e - pd.e;
            let share = if rem > 0.0 {
                format!("{:.4}", (bd.e - b0.e) / rem)
            } else {
                "not defined (no remainder)".to_string()
            };
            println!(
                "A6 {x}, {}: remainder R = E(W3{sfx}o{x}) {:.4e} - E(P3{sfx}) {:.4e} = {rem:.4e}; share of the warped \
                 background rho = (E(Bd{x}) {:.4e} - E(B0) {:.4e}) / R = {share}; BCw, BCi of W3{sfx}o{x} {:.3e}, {:.3e}, \
                 of Bd{x} {:.3e}, {:.3e}, of B0 {:.3e}, {:.3e}; ADR 0382's form E - 2 E(P3{sfx}) = {:.4e}",
                if quad { "quadratic" } else { "default interpolation" },
                w.e,
                pd.e,
                bd.e,
                b0.e,
                w.class[1],
                w.class[2],
                bd.class[1],
                bd.class[2],
                b0.class[1],
                b0.class[2],
                w.e - 2.0 * pd.e
            );
        }
        for (name, r) in [(format!("Bd{x}"), bd), ("B0".to_string(), b0)] {
            println!(
                "U5 {name}: unconverged solves {}, continuity {:.3e}",
                r.unconverged, r.cont
            );
        }
    }
    let (wc, wl) = (run(true, true, Opt::C), run(true, true, Opt::L));
    c.at_least("C0 (set-up) C acts on W3qo", (wc.e - wl.e).abs() / wl.e, 1e-6);
    for (name, quad) in [("W3oL", false), ("W3qoL", true)] {
        println!("U5 {name}: unconverged solves {}", run(true, quad, Opt::L).unconverged);
    }
    c.finish();
}

// ---- R: the refusals -----------------------------------------------------------------------------------------------------

/// ADR 0387 R1, R2: a step with C and the corrections off, and with C and L both on, is refused.
#[test]
fn r1_r2_refusals_of_c() {
    let mut a = vortex_solver(&background(false), &component(false), DonorInterpolation::Linear);
    a.set_consistent_geometry_corrections(true);
    assert!(a.consistent_pressure_gradient() && a.geometry_corrections());
    a.set_geometry_corrections(false);
    let e1 = vortex_step(&mut a, 1).expect_err("R1 refused");
    println!("R1: {e1}");
    let mut b = vortex_solver(&background(false), &component(false), DonorInterpolation::Linear);
    b.set_consistent_geometry_corrections(true);
    b.set_least_squares_pressure_gradient(true);
    let e2 = vortex_step(&mut b, 1).expect_err("R2 refused");
    println!("R2: {e2}");
    assert!(e1.contains("ADR 0387") && e2.contains("ADR 0387"), "{e1} / {e2}");
}

// ---- A4: ADR 0367's linear shear and uniform flow with C -----------------------------------------------------------------

/// The flows of ADR 0367 (the same code as crates/rcfd-fv/tests/overset_f368.rs), run with C.
mod shear {
    use super::{apply, background, component, overset, pose, Checks, Opt};
    use rcfd_core::vec3::{self, Vec3};
    use rcfd_fv::ale::move_mesh;
    use rcfd_fv::bc::{FixedValue, Profile};
    use rcfd_fv::convection::ConvScheme;
    use rcfd_fv::overset::{MassMode, OversetCoupling, OversetSolver};
    use rcfd_fv::pimple::PimpleSettings;
    use rcfd_fv::{FvGeometry, ScalarField, VectorField};
    use rcfd_linalg::SolveControl;
    use rcfd_mesh::motion::moved_mesh;
    use rcfd_mesh::overset::{CellKind, DonorInterpolation};
    use rcfd_mesh::Mesh;
    use std::sync::{Arc, OnceLock};
    use std::time::Instant;

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

    /// The flow F of ADR 0264.
    fn u_ex(x: Vec3) -> Vec3 {
        let (a, b) = (vec3::unit([1.0, 2.0, 1.0]), vec3::unit([0.0, 1.0, -2.0]));
        vec3::add(U0, vec3::scale(a, SHEAR * vec3::dot(b, vec3::sub(x, X0))))
    }

    fn set_dirichlet(u: &mut VectorField, geom: &FvGeometry) {
        for name in PATCHES {
            let v: Vec<Vec3> = geom
                .mesh
                .patch(name)
                .unwrap()
                .faces()
                .map(|f| u_ex(geom.mesh.face_centres()[f]))
                .collect();
            u.set_bc(geom, name, Box::new(FixedValue(Profile::PerFace(v)))).unwrap();
        }
    }

    fn shear_fields(geom: &Arc<FvGeometry>) -> (VectorField, ScalarField) {
        let mut u = VectorField::from_fn("U", geom, u_ex);
        set_dirichlet(&mut u, geom);
        (u, ScalarField::uniform("p", geom, 0.0))
    }

    fn uniform_fields(geom: &Arc<FvGeometry>) -> (VectorField, ScalarField) {
        let mut u = VectorField::uniform("U", geom, U0);
        for name in ["xmin", "ymin", "ymax", "zmin", "zmax"] {
            u.set_bc(geom, name, Box::new(FixedValue::vector(U0))).unwrap();
        }
        let mut p = ScalarField::uniform("p", geom, 0.0);
        p.set_bc(geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
        (u, p)
    }

    fn overset_solver(
        bg: &Mesh,
        comp: &Mesh,
        mode: DonorInterpolation,
        flow: fn(&Arc<FvGeometry>) -> (VectorField, ScalarField),
    ) -> OversetSolver {
        let ov = overset(bg, comp, mode);
        let p0 = pose(0.0);
        let mesh = moved_mesh(&ov.composite.mesh, ov.composite.points_at(&p0)).unwrap();
        let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
        let (u, p) = flow(&geom);
        OversetSolver::new(
            geom,
            u,
            p,
            settings(),
            MassMode::Conservative,
            Some(OversetCoupling { overset: ov, pose: p0 }),
        )
        .unwrap()
    }

    #[derive(Default, Debug, Clone)]
    struct Run {
        e: f64,
        ep: f64,
        cont: f64,
        changes: usize,
        unconverged: usize,
        min_share: f64,
        seconds: f64,
    }

    impl Run {
        fn take(&mut self, u: &VectorField, p: &ScalarField, geom: &FvGeometry, keep: impl Fn(usize) -> bool) {
            let un = vec3::norm(U0);
            let cc = geom.mesh.cell_centres();
            for c in 0..u.values.len() {
                if keep(c) {
                    self.e = self.e.max(vec3::norm(vec3::sub(u.values[c], u_ex(cc[c]))) / un);
                    self.ep = self.ep.max(p.values[c].abs() / (un * un));
                }
            }
        }
        fn print(&self, name: &str) {
            println!(
                "run {name}: E {:.4e}; pressure {:.4e}; continuity {:.3e}; {} kind changes; quadratic share {:.3}; \
                 unconverged {}; {:.1} s",
                self.e, self.ep, self.cont, self.changes, self.min_share, self.unconverged, self.seconds
            );
        }
    }

    fn overset_run(warped: bool, mode: DonorInterpolation, opt: Opt) -> Run {
        let t0 = Instant::now();
        let mut s = overset_solver(&background(warped), &component(warped), mode, shear_fields);
        apply(&mut s, opt);
        let mut r = Run {
            min_share: 1.0,
            ..Run::default()
        };
        for k in 1..=STEPS {
            let info = s.step(DT, pose(k as f64 * DT), &[]).unwrap();
            r.cont = r.cont.max(info.continuity);
            r.changes += info.n_changed;
            r.unconverged += info.unconverged;
            if mode == DonorInterpolation::Quadratic {
                let ns = s.conn.stencils.len();
                r.min_share = r.min_share.min(1.0 - info.n_fallback as f64 / ns as f64);
            }
            let kind = s.conn.kind.clone();
            r.take(&s.u, &s.p, &s.geom, |c| kind[c] != CellKind::Hole);
        }
        r.seconds = t0.elapsed().as_secs_f64();
        r
    }

    static SP3OC: OnceLock<Run> = OnceLock::new();
    static SP3QOC: OnceLock<Run> = OnceLock::new();
    static SW3OC: OnceLock<Run> = OnceLock::new();
    static SW3QOC: OnceLock<Run> = OnceLock::new();
    static SP3: OnceLock<Run> = OnceLock::new();
    static SP3Q: OnceLock<Run> = OnceLock::new();

    fn run(warped: bool, quad: bool, opt: Opt) -> &'static Run {
        let (cell, name) = match (warped, quad, opt) {
            (false, false, Opt::C) => (&SP3OC, "SP3oC"),
            (false, true, Opt::C) => (&SP3QOC, "SP3qoC"),
            (true, false, Opt::C) => (&SW3OC, "SW3oC"),
            (true, true, Opt::C) => (&SW3QOC, "SW3qoC"),
            (false, false, Opt::Default) => (&SP3, "SP3"),
            (false, true, Opt::Default) => (&SP3Q, "SP3q"),
            _ => panic!("not a registered run of ADR 0387"),
        };
        let mode = if quad {
            DonorInterpolation::Quadratic
        } else {
            DonorInterpolation::Linear
        };
        cell.get_or_init(|| {
            let r = overset_run(warped, mode, opt);
            r.print(name);
            r
        })
    }

    /// ADR 0387 A4, A4c: ADR 0367's A1, A1d, A2, A2d and A3 for the linear shear with C.
    #[test]
    fn a4_linear_shear_with_c() {
        let mut c = Checks::new();
        for (id, quad, sfx) in [("A4 default interpolation", false, ""), ("A4 quadratic", true, "q")] {
            let w = run(true, quad, Opt::C);
            let po = run(false, quad, Opt::C);
            let pd = run(false, quad, Opt::Default);
            c.at_most(&format!("{id}: E(SW3{sfx}oC) / E(SP3{sfx}oC)"), w.e / po.e, 2.0);
            c.at_most(&format!("{id}: E(SW3{sfx}oC) / E(SP3{sfx})"), w.e / pd.e, 2.0);
            for (name, r) in [(format!("SP3{sfx}oC"), po), (format!("SW3{sfx}oC"), w)] {
                c.at_most(&format!("A4c continuity {name}"), r.cont, 1e-9);
                println!(
                    "U5 {name}: unconverged solves {}; kind changes {}",
                    r.unconverged, r.changes
                );
            }
        }
        c.finish();
    }

    /// ADR 0387 A4u: ADR 0367's A4 (the uniform flow of ADR 0255 on B3d with the component moving) with C.
    #[test]
    fn a4u_uniform_flow_with_c() {
        let un = vec3::norm(U0);
        let mut c = Checks::new();
        for (name, mode) in [
            ("SU3oC", DonorInterpolation::Linear),
            ("SU3qoC", DonorInterpolation::Quadratic),
        ] {
            let t0 = Instant::now();
            let mut s = overset_solver(&background(true), &component(true), mode, uniform_fields);
            apply(&mut s, Opt::C);
            let (mut eu, mut ep, mut cont, mut unc) = (0.0f64, 0.0f64, 0.0f64, 0);
            for k in 1..=STEPS {
                let info = s.step(DT, pose(k as f64 * DT), &[]).unwrap();
                cont = cont.max(info.continuity);
                unc += info.unconverged;
                for cl in 0..s.u.values.len() {
                    if s.conn.kind[cl] != CellKind::Hole {
                        eu = eu.max(vec3::norm(vec3::sub(s.u.values[cl], U0)) / un);
                        ep = ep.max(s.p.values[cl].abs() / (un * un));
                    }
                }
            }
            println!("{name}: unconverged {unc}, {:.1} s", t0.elapsed().as_secs_f64());
            c.at_most(&format!("A4u velocity {name}"), eu, 1e-10);
            c.at_most(&format!("A4u pressure {name}"), ep, 1e-10);
            c.at_most(&format!("A4u continuity {name}"), cont, 1e-9);
        }
        c.finish();
    }

    /// ADR 0387 A4s: ADR 0367's K7, K7p (the shear on the warped background alone at rest) with C.
    #[test]
    fn a4s_warped_background_alone_with_c() {
        let t0 = Instant::now();
        let geom = Arc::new(FvGeometry::new(Arc::new(background(true))).unwrap());
        let (u, p) = shear_fields(&geom);
        let mut s = OversetSolver::new(geom, u, p, settings(), MassMode::Conservative, None).unwrap();
        apply(&mut s, Opt::C);
        let mut r = Run::default();
        for _ in 1..=STEPS {
            let g = s.geom.clone();
            let ms = move_mesh(&g, g.mesh.points().to_vec(), DT).unwrap();
            let info = s.advance_mesh(DT, ms, Vec::new()).unwrap();
            r.cont = r.cont.max(info.continuity);
            r.unconverged += info.unconverged;
            r.take(&s.u, &s.p, &s.geom, |_| true);
        }
        r.seconds = t0.elapsed().as_secs_f64();
        r.print("SSBoC");
        let mut c = Checks::new();
        c.at_most("A4s SSBoC velocity", r.e, 5e-4);
        c.at_most("A4s SSBoC pressure", r.ep, 1e-3);
        c.finish();
    }
}
