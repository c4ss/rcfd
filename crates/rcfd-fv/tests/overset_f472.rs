//! F-472 (ADR 0382): the geometry corrections of the overset step (ADR 0367) with a varying pressure: the Taylor-Green
//! vortex with a uniform axial velocity (the flow V) on B3 and B3d with the component moving as in ADR 0255; the acceptance
//! (A1, A1d, A2, A2d, A3), the share of the uncorrected Green-Gauss pressure gradient (A5) by the separating opt-in
//! `OversetSolver::set_least_squares_pressure_gradient` (L0 to L4, R1), the two gradients at operator level (K1 to K4), the
//! set-up checks, the control N1 and the fingerprints D1b of the opt-in without L.
//!
//! Release: `cargo test --release -p rcfd-fv --test overset_f472`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{FixedValue, Profile};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::gradient::green_gauss;
use rcfd_fv::overset::{GeometryCorrections, MassMode, OversetCoupling, OversetSolver};
use rcfd_fv::pimple::PimpleSettings;
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

// ---- the set-ups of ADR 0251 (the same code as crates/rcfd-fv/tests/overset_f368.rs) ------------------------------------

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

// ---- D1b: the fingerprints of the opt-in without L -----------------------------------------------------------------------

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

/// 5 steps of V on B3d with the geometry corrections on; a failing step folds its message and ends the run.
fn d1b_fingerprint(mode: DonorInterpolation) -> u64 {
    let mut s = vortex_solver(&background(true), &component(true), mode);
    s.set_geometry_corrections(true);
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

fn d1b_fingerprints() -> [u64; 2] {
    [
        d1b_fingerprint(DonorInterpolation::Linear),
        d1b_fingerprint(DonorInterpolation::Quadratic),
    ]
}

/// ADR 0382 D1b record: the fingerprints printed (run on the tree before any library change of F-472).
#[test]
fn d1b_opt_in_fingerprint_record() {
    let f = d1b_fingerprints();
    println!(
        "D1b fingerprints: {}",
        f.iter().map(|x| format!("{x:#018x}")).collect::<Vec<_>>().join(" ")
    );
}

/// ADR 0382 D1b: the fingerprints on this tree equal those of the tree before any library change of F-472 (recorded by
/// `d1b_opt_in_fingerprint_record` on ca36d108, the base library 4bcfd339).
#[test]
fn d1b_opt_in_fingerprint_equals_the_base_tree() {
    const BASE: [u64; 2] = [0x6e61_17e1_b918_51a9, 0x69a7_19b8_6040_e649];
    let f = d1b_fingerprints();
    for (i, (a, b)) in f.iter().zip(BASE).enumerate() {
        println!(
            "D1b {}: {a:#018x} (base {b:#018x}) {}",
            ["linear", "quadratic"][i],
            if *a == b { "equal" } else { "DIFFERENT" }
        );
    }
    assert_eq!(f, BASE, "D1b: the opt-in without L changed");
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

// ---- K: the two gradients at operator level ------------------------------------------------------------------------------

/// The exact gradient of `p_ex(., 0)`.
fn grad_p_ex(x: Vec3) -> Vec3 {
    let k = k_wave();
    let (xi, eta) = (x[0] - L / 2.0, x[1] - L / 2.0);
    let a = VA * VA * k / 2.0;
    [a * (2.0 * k * xi).sin(), a * (2.0 * k * eta).sin(), 0.0]
}

/// The Green-Gauss and the least-squares gradient of `pf` (exact boundary values) on `mesh`: the largest error of each over
/// the cells (`interior`: the cells without a boundary face only) against `grad`, and its rms, both over `scale`.
fn gradient_errors(
    mesh: &Mesh,
    pf: impl Fn(Vec3) -> f64,
    grad: impl Fn(Vec3) -> Vec3,
    scale: f64,
    interior: bool,
) -> ([f64; 2], [f64; 2]) {
    let geom = FvGeometry::new(Arc::new(mesh.clone())).unwrap();
    let m = &geom.mesh;
    let p: Vec<f64> = m.cell_centres().iter().map(|&x| pf(x)).collect();
    let bv: Vec<f64> = (m.n_internal_faces()..m.n_faces())
        .map(|f| pf(m.face_centres()[f]))
        .collect();
    let mut at_boundary = vec![false; geom.n_cells()];
    for f in m.n_internal_faces()..m.n_faces() {
        at_boundary[m.owner()[f]] = true;
    }
    let gg = green_gauss(&geom, &p, &bv, 0);
    let ls = GeometryCorrections::gradients(&geom, &p, &bv, None);
    let (mut mx, mut s2, mut n) = ([0.0f64; 2], [0.0f64; 2], 0usize);
    for c in 0..geom.n_cells() {
        if interior && at_boundary[c] {
            continue;
        }
        let ex = grad(m.cell_centres()[c]);
        for (i, g) in [&gg, &ls].iter().enumerate() {
            let e = vec3::norm(vec3::sub(g[c], ex)) / scale;
            mx[i] = mx[i].max(e);
            s2[i] += e * e;
        }
        n += 1;
    }
    (mx, s2.map(|x| (x / n as f64).sqrt()))
}

/// ADR 0382 K1 to K4: the least-squares and the Green-Gauss gradient of a linear pressure and of `p_ex` of V.
#[test]
fn k1_k4_the_two_pressure_gradients() {
    let p0 = pose(0.0);
    let placed = |m: Mesh| {
        let pts: Vec<Vec3> = m.points().iter().map(|&x| p0.apply(x)).collect();
        moved_mesh(&m, pts).unwrap()
    };
    let c = [0.3, -0.7, 0.4];
    let lin = |x: Vec3| 2.0 + vec3::dot(c, x);
    let cn = vec3::norm(c);
    let scale = VA * VA * k_wave() / 2.0;
    let mut ch = Checks::new();
    for (name, mesh, warped) in [
        ("planar background", background(false), false),
        ("planar component", placed(component(false)), false),
        ("warped background", background(true), true),
        ("warped component", placed(component(true)), true),
    ] {
        let (lmx, lrms) = gradient_errors(&mesh, lin, |_| c, cn, false);
        let (vmx, vrms) = gradient_errors(&mesh, |x| p_ex(x, 0.0), grad_p_ex, scale, true);
        println!(
            "K {name}: linear p: GG {:.4e} (rms {:.4e}), LS {:.4e} (rms {:.4e}); p_ex of V (interior cells): GG {:.4e} \
             (rms {:.4e}), LS {:.4e} (rms {:.4e})",
            lmx[0], lrms[0], lmx[1], lrms[1], vmx[0], vrms[0], vmx[1], vrms[1]
        );
        if warped {
            ch.at_most(&format!("K1 LS of a linear pressure, {name}"), lmx[1], 1e-10);
        } else {
            ch.at_most(&format!("K3 GG of a linear pressure, {name}"), lmx[0], 1e-10);
        }
        if name == "warped background" {
            ch.at_least(&format!("K2 GG of a linear pressure, {name}"), lmx[0], 1e-3);
            ch.at_most(&format!("K4 LS / GG for p_ex of V, {name}"), vmx[1] / vmx[0], 0.5);
        }
    }
    ch.finish();
}

// ---- the runs of V -------------------------------------------------------------------------------------------------------

const STEPS: usize = 40;
const CLASSES: [&str; 4] = ["BC", "BF", "CC", "CF"];

/// The largest values of a run over its steps.
#[derive(Default, Debug, Clone)]
struct Run {
    e: f64,
    at: (usize, usize),
    class: [f64; 4],
    e_last: f64,
    rms_last: f64,
    ep: f64,
    cont: f64,
    changes: usize,
    unconverged: usize,
    min_share: f64,
    max_fallback: usize,
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
}

/// A run of 40 steps of V: the geometry corrections `on`, L `ls`, the weights times `1.001` with `weights`.
fn vortex_run(warped: bool, mode: DonorInterpolation, on: bool, ls: bool, weights: bool) -> Run {
    let t0 = Instant::now();
    let mut s = vortex_solver(&background(warped), &component(warped), mode);
    if on {
        s.set_geometry_corrections(true);
    }
    if ls {
        s.set_least_squares_pressure_gradient(true);
    }
    let nb = s.conn.n_background_cells;
    let mut r = Run {
        min_share: 1.0,
        ..Run::default()
    };
    for k in 1..=STEPS {
        let t = k as f64 * DT;
        let g = s.geom.clone();
        set_conditions(&mut s.u, &g, t);
        let mut st = s.prepare(DT, pose(t), &[]).unwrap();
        if weights {
            for (_, stn) in st.conn.stencils.iter_mut() {
                stn.weights.iter_mut().for_each(|w| *w *= 1.001);
            }
        }
        let info = s.advance(DT, st).unwrap();
        r.cont = r.cont.max(info.continuity);
        r.changes += info.n_changed;
        r.unconverged += info.unconverged;
        r.max_fallback = r.max_fallback.max(info.n_fallback);
        if mode == DonorInterpolation::Quadratic {
            assert_eq!(s.conn.interpolation, DonorInterpolation::Quadratic);
            let ns = s.conn.stencils.len();
            r.min_share = r.min_share.min(1.0 - info.n_fallback as f64 / ns as f64);
        }
        let cc = s.geom.mesh.cell_centres();
        let vol = s.geom.mesh.cell_volumes();
        let (mut emax, mut s2, mut sv, mut dp) = (0.0f64, 0.0, 0.0, 0.0);
        let cells: Vec<(usize, usize)> = (0..s.u.values.len())
            .filter_map(|c| match s.conn.kind[c] {
                CellKind::Hole => None,
                CellKind::Calculated => Some((c, if c < nb { 0 } else { 2 })),
                CellKind::Fringe => Some((c, if c < nb { 1 } else { 3 })),
            })
            .collect();
        for &(c, cl) in &cells {
            let e = vec3::norm(vec3::sub(s.u.values[c], u_ex(cc[c], t))) / VA;
            if e > r.e {
                r.e = e;
                r.at = (k, c);
            }
            r.class[cl] = r.class[cl].max(e);
            emax = emax.max(e);
            s2 += vol[c] * e * e;
            sv += vol[c];
            dp += vol[c] * (s.p.values[c] - p_ex(cc[c], t));
        }
        let mean = dp / sv;
        for &(c, _) in &cells {
            r.ep = r.ep.max((s.p.values[c] - p_ex(cc[c], t) - mean).abs() / (VA * VA));
        }
        r.e_last = emax;
        r.rms_last = (s2 / sv).sqrt();
    }
    r.seconds = t0.elapsed().as_secs_f64();
    r
}

static P3O: OnceLock<Run> = OnceLock::new();
static P3QO: OnceLock<Run> = OnceLock::new();
static W3O: OnceLock<Run> = OnceLock::new();
static W3QO: OnceLock<Run> = OnceLock::new();
static P3: OnceLock<Run> = OnceLock::new();
static P3Q: OnceLock<Run> = OnceLock::new();
static P3OL: OnceLock<Run> = OnceLock::new();
static P3QOL: OnceLock<Run> = OnceLock::new();
static W3OL: OnceLock<Run> = OnceLock::new();
static W3QOL: OnceLock<Run> = OnceLock::new();

/// The registered runs of V, each computed once per test process.
fn run(warped: bool, quad: bool, on: bool, ls: bool) -> &'static Run {
    let (cell, name) = match (warped, quad, on, ls) {
        (false, false, true, false) => (&P3O, "P3o"),
        (false, true, true, false) => (&P3QO, "P3qo"),
        (true, false, true, false) => (&W3O, "W3o"),
        (true, true, true, false) => (&W3QO, "W3qo"),
        (false, false, false, false) => (&P3, "P3"),
        (false, true, false, false) => (&P3Q, "P3q"),
        (false, false, true, true) => (&P3OL, "P3oL"),
        (false, true, true, true) => (&P3QOL, "P3qoL"),
        (true, false, true, true) => (&W3OL, "W3oL"),
        (true, true, true, true) => (&W3QOL, "W3qoL"),
        _ => panic!("not a registered run of ADR 0382"),
    };
    let mode = if quad {
        DonorInterpolation::Quadratic
    } else {
        DonorInterpolation::Linear
    };
    cell.get_or_init(|| {
        let r = vortex_run(warped, mode, on, ls, false);
        r.print(name);
        r
    })
}

// ---- A: the acceptance ---------------------------------------------------------------------------------------------------

/// ADR 0382 A1, A1d (default interpolation) and A2, A2d (quadratic): the error of V on B3d with the geometry corrections at
/// most twice that on B3 with them and with the default solver.
#[test]
fn a1_a2_error_ratios() {
    let mut c = Checks::new();
    for (id, quad) in [("A1", false), ("A2", true)] {
        let w = run(true, quad, true, false);
        let po = run(false, quad, true, false);
        let pd = run(false, quad, false, false);
        println!(
            "{id}: E(B3d, opt-in) {:.4e}, E(B3, opt-in) {:.4e}, E(B3, default) {:.4e}; P-diff {:.3e}",
            w.e,
            po.e,
            pd.e,
            (po.e - pd.e).abs() / pd.e
        );
        c.at_most(&format!("{id} E(B3d o) / E(B3 o)"), w.e / po.e, 2.0);
        c.at_most(&format!("{id}d E(B3d o) / E(B3 default)"), w.e / pd.e, 2.0);
    }
    c.finish();
}

/// ADR 0382 A3 (continuity every step of the four opt-in runs), U4 (kind changes), Q1 (quadratic share), U5.
#[test]
fn a3_continuity_and_set_up_of_the_opt_in_runs() {
    let mut c = Checks::new();
    for (name, warped, quad) in [
        ("P3o", false, false),
        ("P3qo", false, true),
        ("W3o", true, false),
        ("W3qo", true, true),
    ] {
        let r = run(warped, quad, true, false);
        c.at_most(&format!("A3 continuity {name}"), r.cont, 1e-9);
        if quad {
            c.at_least(&format!("Q1 (set-up) quadratic share {name}"), r.min_share, 0.95);
            println!("{name}: largest fallback count of a step {}", r.max_fallback);
        } else {
            c.at_least(&format!("U4 (set-up) kind changes {name}"), r.changes as f64, 20.0);
        }
        println!("U5 {name}: unconverged solves {}", r.unconverged);
    }
    for (name, quad) in [("P3", false), ("P3q", true)] {
        println!(
            "U5 {name}: unconverged solves {}",
            run(false, quad, false, false).unconverged
        );
    }
    c.finish();
}

// ---- A5, L: the share of the uncorrected Green-Gauss pressure gradient ---------------------------------------------------

/// ADR 0382 A5 (the share, named), L0 (L acts), L1, L2 (the separating ratios), L3 (continuity), Q1 on W3qoL, U5.
#[test]
fn a5_l_share_of_the_green_gauss_pressure_gradient() {
    let mut c = Checks::new();
    for (id, quad, sfx) in [("L1", false, ""), ("L2", true, "q")] {
        let w = run(true, quad, true, false);
        let wl = run(true, quad, true, true);
        let po = run(false, quad, true, false);
        let pl = run(false, quad, true, true);
        let sigma = (w.e - wl.e) / w.e;
        let chi = if w.e > po.e {
            format!("{:.4}", (w.e - wl.e) / (w.e - po.e))
        } else {
            "not defined (E(W3o) <= E(P3o))".to_string()
        };
        println!(
            "A5 the share of the uncorrected Green-Gauss pressure gradient ({}): sigma{sfx} = (E(W3{sfx}o) {:.4e} - \
             E(W3{sfx}oL) {:.4e}) / E(W3{sfx}o) = {sigma:.4}; excess share chi{sfx} = {chi}; E(P3{sfx}o) {:.4e}, \
             E(P3{sfx}oL) {:.4e}",
            if quad { "quadratic" } else { "default interpolation" },
            w.e,
            wl.e,
            po.e,
            pl.e
        );
        if !quad {
            c.at_least("L0 (set-up) L acts on W3o", (wl.e - w.e).abs() / w.e, 1e-6);
        } else {
            c.at_least("Q1 (set-up) quadratic share W3qoL", wl.min_share, 0.95);
        }
        c.at_most(&format!("{id} E(W3{sfx}oL) / E(P3{sfx}oL)"), wl.e / pl.e, 2.0);
        for (name, r) in [(format!("P3{sfx}oL"), pl), (format!("W3{sfx}oL"), wl)] {
            c.at_most(&format!("L3 continuity {name}"), r.cont, 1e-9);
            println!("U5 {name}: unconverged solves {}", r.unconverged);
        }
    }
    c.finish();
}

/// ADR 0382 L4: the uniform flow of ADR 0255 on B3d with the component moving, with the corrections and L.
#[test]
fn l4_uniform_flow_with_l() {
    let u0: Vec3 = [1.0, 0.5, 0.25];
    let un = vec3::norm(u0);
    let t0 = Instant::now();
    let ov = overset(&background(true), &component(true), DonorInterpolation::Linear);
    let p0 = pose(0.0);
    let mesh = moved_mesh(&ov.composite.mesh, ov.composite.points_at(&p0)).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, u0);
    for name in ["xmin", "ymin", "ymax", "zmin", "zmax"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector(u0))).unwrap();
    }
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let coupling = Some(OversetCoupling { overset: ov, pose: p0 });
    let mut s = OversetSolver::new(geom, u, p, settings(0.0), MassMode::Conservative, coupling).unwrap();
    s.set_geometry_corrections(true);
    s.set_least_squares_pressure_gradient(true);
    let (mut eu, mut ep, mut cont, mut unc) = (0.0f64, 0.0f64, 0.0f64, 0);
    for k in 1..=STEPS {
        let info = s.step(DT, pose(k as f64 * DT), &[]).unwrap();
        cont = cont.max(info.continuity);
        unc += info.unconverged;
        for cl in 0..s.u.values.len() {
            if s.conn.kind[cl] != CellKind::Hole {
                eu = eu.max(vec3::norm(vec3::sub(s.u.values[cl], u0)) / un);
                ep = ep.max(s.p.values[cl].abs() / (un * un));
            }
        }
    }
    println!("U3oL: unconverged {unc}, {:.1} s", t0.elapsed().as_secs_f64());
    let mut c = Checks::new();
    c.at_most("L4 velocity U3oL", eu, 1e-10);
    c.at_most("L4 pressure U3oL", ep, 1e-10);
    c.at_most("L4 continuity U3oL", cont, 1e-9);
    c.finish();
}

/// ADR 0382 R1: a step with L on and the geometry corrections off is refused.
#[test]
fn r1_least_squares_gradient_needs_the_corrections() {
    let mut s = vortex_solver(&background(false), &component(false), DonorInterpolation::Linear);
    s.set_least_squares_pressure_gradient(true);
    assert!(s.least_squares_pressure_gradient() && !s.geometry_corrections());
    let e = vortex_step(&mut s, 1).expect_err("refused");
    println!("R1: {e}");
    assert!(e.contains("ADR 0382"), "{e}");
}

// ---- N: negative control -------------------------------------------------------------------------------------------------

/// ADR 0382 N1: P3o with every stencil weight multiplied by 1.001.
#[test]
fn n1_weights_control() {
    let r = vortex_run(false, DonorInterpolation::Linear, true, false, true);
    r.print("N1");
    let po = run(false, false, true, false);
    let mut c = Checks::new();
    c.at_least("N1 E(N1) - E(P3o)", r.e - po.e, 3e-4);
    c.finish();
}
