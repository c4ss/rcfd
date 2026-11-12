//! F-368 (ADR 0367): the opt-in skewness and non-orthogonal corrections of the overset step
//! (`OversetSolver::set_geometry_corrections`): the defaults fingerprint D1 (recorded on the tree before any library
//! change), the acceptance (A1, A1d, A2, A2d: the linear shear of ADR 0264 on B3d against B3; A3 continuity; A4 the uniform
//! flow of ADR 0255), the exactness of the corrected operators (K1 to K6), the warped background alone (K7, K7p), the
//! set-up checks, the separating runs and the control N1o.
//!
//! Release: `cargo test --release -p rcfd-fv --test overset_f368`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::move_mesh;
use rcfd_fv::bc::{FixedValue, Profile};
use rcfd_fv::convection::{convection, ConvScheme};
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::diffusion::{laplacian, uniform_gamma};
use rcfd_fv::overset::{FringeFlux, GeometryCorrections, MassMode, OversetCoupling, OversetSolver};
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

// ---- the set-ups of ADR 0251 (the same code as crates/rcfd-fv/tests/overset_f359.rs) ------------------------------------

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

/// The shear flow F with its boundary values (ADR 0264).
fn shear_fields(geom: &Arc<FvGeometry>) -> (VectorField, ScalarField) {
    let mut u = VectorField::from_fn("U", geom, u_ex);
    set_dirichlet(&mut u, geom, geom);
    let p = ScalarField::uniform("p", geom, 0.0);
    (u, p)
}

/// The uniform flow of ADR 0255 with its boundary values.
fn uniform_fields(geom: &Arc<FvGeometry>) -> (VectorField, ScalarField) {
    let mut u = VectorField::uniform("U", geom, U0);
    for name in ["xmin", "ymin", "ymax", "zmin", "zmax"] {
        u.set_bc(geom, name, Box::new(FixedValue::vector(U0))).unwrap();
    }
    let mut p = ScalarField::uniform("p", geom, 0.0);
    p.set_bc(geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    (u, p)
}

/// An overset solver at `P(0)` on `bg` and `comp` with the fields of `flow`.
fn overset_solver(
    bg: &Mesh,
    comp: &Mesh,
    mode: DonorInterpolation,
    s: PimpleSettings,
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
        s,
        MassMode::Conservative,
        Some(OversetCoupling { overset: ov, pose: p0 }),
    )
    .unwrap()
}

// ---- D1: the defaults fingerprint ----------------------------------------------------------------------------------------

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

/// The fingerprint of a solver's state.
fn state(h: &mut Fnv, s: &OversetSolver) {
    for k in 0..3 {
        h.f64s(s.u.values.iter().map(|v| v[k]));
    }
    h.f64s(s.p.values.iter().copied());
    h.f64s(s.phi.iter().copied());
}

/// 5 steps of an overset solver; a failing step folds its message and ends the run.
fn overset_fingerprint(mut s: OversetSolver) -> u64 {
    let mut h = Fnv::new();
    for k in 1..=5 {
        match s.step(DT, pose(k as f64 * DT), &[]) {
            Ok(_) => state(&mut h, &s),
            Err(e) => {
                h.bytes(format!("step {k}: {e}").as_bytes());
                break;
            }
        }
    }
    h.0
}

/// The eight fingerprints of D1 (ADR 0367), in the registered order.
fn d1_fingerprints() -> [u64; 8] {
    let (bg, comp) = (background(true), component(true));
    let lin = DonorInterpolation::Linear;
    let f1 = overset_fingerprint(overset_solver(&bg, &comp, lin, settings(), shear_fields));
    let f2 = overset_fingerprint(overset_solver(
        &bg,
        &comp,
        DonorInterpolation::Quadratic,
        settings(),
        shear_fields,
    ));
    let f3 = {
        let mut s = overset_solver(&bg, &comp, lin, settings(), shear_fields);
        s.set_outer_iterations(2, 0.8, 0.5).unwrap();
        overset_fingerprint(s)
    };
    let f4 = {
        let mut s = overset_solver(&bg, &comp, lin, settings(), shear_fields);
        s.set_time_scheme(TimeScheme::Bdf2).unwrap();
        overset_fingerprint(s)
    };
    let f5 = {
        let mut s = overset_solver(&bg, &comp, lin, settings(), shear_fields);
        s.set_fringe_flux(FringeFlux::PrescribedOwnGradient);
        overset_fingerprint(s)
    };
    let f6 = {
        let mut st = settings();
        st.rhie_chow_step_consistent = true;
        overset_fingerprint(overset_solver(&bg, &comp, lin, st, shear_fields))
    };
    let f7 = {
        let reference = component(true);
        let at = |t: f64| -> Vec<Vec3> { reference.points().iter().map(|&x| pose(t).apply(x)).collect() };
        let mesh = moved_mesh(&reference, at(0.0)).unwrap();
        let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
        let (u, p) = shear_fields(&geom);
        let mut s = OversetSolver::new(geom, u, p, settings(), MassMode::Conservative, None).unwrap();
        let mut h = Fnv::new();
        for k in 1..=5 {
            let ms = move_mesh(&s.geom, at(k as f64 * DT), DT).unwrap();
            let g = s.geom.clone();
            set_dirichlet(&mut s.u, &g, &ms.geom);
            match s.advance_mesh(DT, ms, Vec::new()) {
                Ok(_) => state(&mut h, &s),
                Err(e) => {
                    h.bytes(format!("step {k}: {e}").as_bytes());
                    break;
                }
            }
        }
        h.0
    };
    let f8 = overset_fingerprint(overset_solver(&bg, &comp, lin, settings(), uniform_fields));
    [f1, f2, f3, f4, f5, f6, f7, f8]
}

/// ADR 0367 D1 record: the fingerprints printed (run on the tree before any library change of F-368).
#[test]
fn d1_defaults_fingerprint_record() {
    let f = d1_fingerprints();
    println!(
        "D1 fingerprints: {}",
        f.iter().map(|x| format!("{x:#018x}")).collect::<Vec<_>>().join(" ")
    );
}

/// ADR 0367 D1: the fingerprints on this tree equal those of the tree before any library change of F-368 (recorded by
/// `d1_defaults_fingerprint_record` on 68849b03, the base library 2ecb97f6).
#[test]
fn d1_defaults_fingerprint_equals_the_base_tree() {
    const BASE: [u64; 8] = [
        0x7ed9_37a9_e05b_b338,
        0xbf99_f3bc_07e5_fdff,
        0x3150_9ceb_0d02_8a2f,
        0xcd2e_f9eb_d8dc_9295,
        0xca08_f2d7_cb22_ffe7,
        0xabcd_93b8_59c1_b26e,
        0xa489_4813_b5a6_f2ed,
        0x8b2e_bdb2_82e0_7af2,
    ];
    let f = d1_fingerprints();
    for (i, (a, b)) in f.iter().zip(BASE).enumerate() {
        println!(
            "D1 F{}: {a:#018x} (base {b:#018x}) {}",
            i + 1,
            if *a == b { "equal" } else { "DIFFERENT" }
        );
    }
    assert_eq!(f, BASE, "D1: the defaults changed");
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

// ---- K: the corrected operators on the exact field -----------------------------------------------------------------------

/// T of ADR 0367 (ADR 0264 T with the corrections C1 to C3): the largest and the root mean square of `T_conv`, `T_diff`
/// (per unit volume) and `T_cont` (per second) over the cells.
fn truncation_corrected(mesh: Mesh) -> ([f64; 3], [f64; 3]) {
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let (u, _) = shear_fields(&geom);
    let m = &geom.mesh;
    let n = geom.n_cells();
    let vol = m.cell_volumes();
    let ub = u.boundary_values(&geom);
    let gc = GeometryCorrections::new(&geom);
    let phi = gc.face_flux(&geom, &u.values, &ub, None);
    let grads = GeometryCorrections::vector_gradients(&geom, &u.values, &ub, None);
    let gamma = uniform_gamma(&geom, NU);
    let (mut tc, mut td) = (vec![0.0f64; n], vec![0.0f64; n]);
    for k in 0..3 {
        let cells: Vec<f64> = u.values.iter().map(|v| v[k]).collect();
        let bc = u.boundary_coeffs(&geom, k);
        let mut mc = convection(&geom, &phi, &cells, &bc, ConvScheme::Central, None);
        let q = gc.convection_correction(&geom, &phi, &u.values, None, &grads[k], 1.0);
        GeometryCorrections::add_convection_correction(&geom, &q, &mut mc);
        let rc = mc.residual(&cells);
        let rd = laplacian(&geom, &gamma, &bc, Some(&grads[k])).residual(&cells);
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
    ([max(&tc), max(&td), max(&tq)], [rms(&tc), rms(&td), rms(&tq)])
}

/// K5: the corrected pressure face flux of a linear pressure against its exact flux; returns (corrected, without `N_f`).
fn pressure_flux_error(mesh: Mesh) -> (f64, f64) {
    let geom = FvGeometry::new(Arc::new(mesh)).unwrap();
    let m = &geom.mesh;
    let c = [0.3, -0.7, 0.4];
    let pf = |x: Vec3| 2.0 + vec3::dot(c, x);
    let p: Vec<f64> = m.cell_centres().iter().map(|&x| pf(x)).collect();
    let bv: Vec<f64> = (m.n_internal_faces()..m.n_faces())
        .map(|f| pf(m.face_centres()[f]))
        .collect();
    let grad = GeometryCorrections::gradients(&geom, &p, &bv, None);
    let gc = GeometryCorrections::new(&geom);
    let nf = gc.non_orth_flux(&geom, &vec![1.0; m.n_faces()], &grad);
    let (own, nei, sf) = (m.owner(), m.neighbour(), m.face_area_vectors());
    let (mut with, mut without) = (0.0f64, 0.0f64);
    for f in 0..geom.n_internal {
        let scale = vec3::norm(c) * vec3::norm(sf[f]);
        let plain = geom.lap_coeff[f] * (p[nei[f]] - p[own[f]]);
        let exact = vec3::dot(c, sf[f]);
        with = with.max((plain + nf[f] - exact).abs() / scale);
        without = without.max((plain - exact).abs() / scale);
    }
    (with, without)
}

/// K6: the largest Frobenius norm of the warp tensor over `abs(S_f)^{3/2}`.
fn largest_warp(mesh: Mesh) -> f64 {
    let geom = FvGeometry::new(Arc::new(mesh)).unwrap();
    let gc = GeometryCorrections::new(&geom);
    let sf = geom.mesh.face_area_vectors();
    (0..geom.n_internal)
        .map(|f| {
            let w = &gc.warp[f];
            let fro = w.iter().flatten().map(|x| x * x).sum::<f64>().sqrt();
            fro / vec3::norm(sf[f]).powf(1.5)
        })
        .fold(0.0, f64::max)
}

/// ADR 0367 K1 to K6: the corrected operators on the exact fields.
#[test]
fn k1_k6_corrected_operators_on_the_exact_field() {
    let p0 = pose(0.0);
    let placed = |m: Mesh| {
        let pts: Vec<Vec3> = m.points().iter().map(|&x| p0.apply(x)).collect();
        moved_mesh(&m, pts).unwrap()
    };
    let mut c = Checks::new();
    for (name, mesh) in [
        ("planar background", background(false)),
        ("planar component", placed(component(false))),
        ("warped background", background(true)),
        ("warped component", placed(component(true))),
    ] {
        let (mx, rms) = truncation_corrected(mesh.clone());
        println!(
            "T corrected {name}: T_conv {:.4e} (rms {:.4e}), T_diff {:.4e} (rms {:.4e}), T_cont {:.4e} (rms {:.4e})",
            mx[0], rms[0], mx[1], rms[1], mx[2], rms[2]
        );
        if name.starts_with("planar") {
            c.at_most(&format!("K4 T_conv {name}"), mx[0], 1e-10);
            c.at_most(&format!("K4 T_diff {name}"), mx[1], 1e-10);
            c.at_most(&format!("K4 T_cont {name}"), mx[2], 1e-10);
        } else {
            c.at_most(&format!("K1 T_cont {name}"), mx[2], 1e-10);
            c.at_most(&format!("K2 T_diff {name}"), mx[1], 1e-10);
            c.at_most(&format!("K3 T_conv {name}"), mx[0], 5e-3);
            let (with, without) = pressure_flux_error(mesh);
            println!("K5 {name}: without the non-orthogonal flux {without:.4e}");
            c.at_most(&format!("K5 pressure face flux {name}"), with, 1e-10);
        }
    }
    c.at_least("K6 warp seen on B3d's background", largest_warp(background(true)), 1e-3);
    c.at_most("K6 no warp on B3's background", largest_warp(background(false)), 1e-12);
    c.finish();
}

// ---- the runs of the shear flow ------------------------------------------------------------------------------------------

const CLASSES: [&str; 4] = ["BC", "BF", "CC", "CF"];

/// The largest values of a run over its steps (as ADR 0264).
#[derive(Default, Debug, Clone)]
struct Run {
    e: f64,
    at: (usize, usize),
    class: [f64; 4],
    e_last: f64,
    rms_last: f64,
    ep: f64,
    cont: f64,
    k1: f64,
    changes: usize,
    unconverged: usize,
    min_share: f64,
    max_fallback: usize,
    seconds: f64,
}

impl Run {
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

/// An overset run of 40 steps of the shear flow, the opt-in `on`, the weights times `1.001` with `weights`.
fn overset_run(bg: &Mesh, comp: &Mesh, mode: DonorInterpolation, on: bool, weights: bool) -> Run {
    let t0 = Instant::now();
    let mut s = overset_solver(bg, comp, mode, settings(), shear_fields);
    if on {
        s.set_geometry_corrections(true);
    }
    let nb = s.conn.n_background_cells;
    let un = vec3::norm(U0);
    let mut r = Run {
        min_share: 1.0,
        ..Run::default()
    };
    for k in 1..=STEPS {
        let mut st = s.prepare(DT, pose(k as f64 * DT), &[]).unwrap();
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
    r
}

/// SBo, S0o, S1o, PC1o: one mesh alone with the opt-in, placed by `place(t, x_ref)` at every step (`moving`) or left at
/// its first placement; the six patches get `u_ex` at their current face centres.
fn single_run(reference: &Mesh, moving: bool, place: impl Fn(f64, Vec3) -> Vec3) -> Run {
    let t0 = Instant::now();
    let at = |t: f64| -> Vec<Vec3> { reference.points().iter().map(|&x| place(t, x)).collect() };
    let mesh = moved_mesh(reference, at(0.0)).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let (u, p) = shear_fields(&geom);
    let mut s = OversetSolver::new(geom, u, p, settings(), MassMode::Conservative, None).unwrap();
    s.set_geometry_corrections(true);
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
    r
}

static P3O: OnceLock<Run> = OnceLock::new();
static P3QO: OnceLock<Run> = OnceLock::new();
static W3O: OnceLock<Run> = OnceLock::new();
static W3QO: OnceLock<Run> = OnceLock::new();
static P3: OnceLock<Run> = OnceLock::new();
static P3Q: OnceLock<Run> = OnceLock::new();

/// The six acceptance runs, each computed once per test process.
fn acceptance_run(warped: bool, mode: DonorInterpolation, on: bool) -> &'static Run {
    let quad = mode == DonorInterpolation::Quadratic;
    let (cell, name) = match (warped, quad, on) {
        (false, false, true) => (&P3O, "P3o"),
        (false, true, true) => (&P3QO, "P3qo"),
        (true, false, true) => (&W3O, "W3o"),
        (true, true, true) => (&W3QO, "W3qo"),
        (false, false, false) => (&P3, "P3"),
        (false, true, false) => (&P3Q, "P3q"),
        (true, _, false) => panic!("the default on B3d is ADR 0264's run, not repeated here"),
    };
    cell.get_or_init(|| {
        let r = overset_run(&background(warped), &component(warped), mode, on, false);
        r.print(name);
        r
    })
}

// ---- A: the acceptance ---------------------------------------------------------------------------------------------------

/// ADR 0367 A1, A1d (default interpolation) and A2, A2d (quadratic): the error on B3d with the opt-in at most twice that
/// on B3 with the opt-in and with the default solver.
#[test]
fn a1_a2_error_ratios() {
    let mut c = Checks::new();
    for (id, mode) in [
        ("A1", DonorInterpolation::Linear),
        ("A2", DonorInterpolation::Quadratic),
    ] {
        let w = acceptance_run(true, mode, true);
        let po = acceptance_run(false, mode, true);
        let pd = acceptance_run(false, mode, false);
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

/// ADR 0367 A3 (continuity every step), K1s (the stencils reproduce `u_ex`), U4 (kind changes), Q1 (quadratic share) on
/// the four opt-in runs.
#[test]
fn a3_continuity_and_set_up_of_the_opt_in_runs() {
    let mut c = Checks::new();
    for (name, warped, mode) in [
        ("P3o", false, DonorInterpolation::Linear),
        ("P3qo", false, DonorInterpolation::Quadratic),
        ("W3o", true, DonorInterpolation::Linear),
        ("W3qo", true, DonorInterpolation::Quadratic),
    ] {
        let r = acceptance_run(warped, mode, true);
        c.at_most(&format!("A3 continuity {name}"), r.cont, 1e-9);
        c.at_most(&format!("K1s stencils reproduce u_ex {name}"), r.k1, 1e-12);
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

/// ADR 0367 A4: the uniform flow of ADR 0255 on B3d with the component moving, with the opt-in, the default and the
/// quadratic interpolation: velocity and pressure within 1e-10, continuity within 1e-9, every step.
#[test]
fn a4_uniform_flow_with_the_opt_in() {
    let un = vec3::norm(U0);
    let mut c = Checks::new();
    for (name, mode) in [
        ("U3o", DonorInterpolation::Linear),
        ("U3qo", DonorInterpolation::Quadratic),
    ] {
        let t0 = Instant::now();
        let mut s = overset_solver(&background(true), &component(true), mode, settings(), uniform_fields);
        s.set_geometry_corrections(true);
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
        c.at_most(&format!("A4 velocity {name}"), eu, 1e-10);
        c.at_most(&format!("A4 pressure {name}"), ep, 1e-10);
        c.at_most(&format!("A4 continuity {name}"), cont, 1e-9);
    }
    c.finish();
}

// ---- K7, S: the single meshes --------------------------------------------------------------------------------------------

static SBO: OnceLock<Run> = OnceLock::new();

fn sbo() -> &'static Run {
    SBO.get_or_init(|| {
        let r = single_run(&background(true), false, |_, x| x);
        r.print("SBo");
        r
    })
}

/// ADR 0367 K7, K7p: the warped background alone at rest with the opt-in.
#[test]
fn k7_warped_background_alone_with_the_opt_in() {
    let r = sbo();
    let mut c = Checks::new();
    c.at_most("K7 SBo velocity", r.e, 5e-4);
    c.at_most("K7p SBo pressure", r.ep, 1e-3);
    println!("SBo continuity {:.3e}, unconverged {}", r.cont, r.unconverged);
    c.finish();
}

/// ADR 0367 S: the warped component alone at rest (S0o) and moving (S1o), the planar component alone moving (PC1o), and
/// the registered reading against W3o and W3qo. Printed; the rule is applied in the addendum.
#[test]
fn s_separating_runs_with_the_opt_in() {
    let p0 = pose(0.0);
    let s0 = single_run(&component(true), false, |_, x| p0.apply(x));
    let s1 = single_run(&component(true), true, |t, x| pose(t).apply(x));
    let pc1 = single_run(&component(false), true, |t, x| pose(t).apply(x));
    for (n, r) in [("S0o", &s0), ("S1o", &s1), ("PC1o", &pc1)] {
        r.print(n);
    }
    let e_m = sbo().e.max(s1.e);
    for (n, w) in [
        ("W3o", acceptance_run(true, DonorInterpolation::Linear, true)),
        ("W3qo", acceptance_run(true, DonorInterpolation::Quadratic, true)),
    ] {
        println!(
            "S reading {n}: E_Mo {e_m:.4e}, E({n}) {:.4e}, E_Mo / E({n}) {:.3}: {}",
            w.e,
            e_m / w.e,
            if e_m >= 0.5 * w.e {
                "the single warped mesh"
            } else {
                "the overset coupling on the warped mesh"
            }
        );
    }
    assert!([s0.e, s1.e, pc1.e].iter().all(|e| e.is_finite()));
}

// ---- N: negative control -------------------------------------------------------------------------------------------------

/// ADR 0367 N1o: P3o with every stencil weight multiplied by 1.001.
#[test]
fn n1o_weights_control() {
    let r = overset_run(
        &background(false),
        &component(false),
        DonorInterpolation::Linear,
        true,
        true,
    );
    r.print("N1o");
    let mut c = Checks::new();
    c.at_least("N1o E with the weights times 1.001", r.e, 3e-4);
    c.finish();
}

/// The refusals of the opt-in (ADR 0367): BDF2, a prescribed fringe flux, `rhie_chow_step_consistent`.
#[test]
fn refusals_of_the_opt_in() {
    let (bg, comp) = (background(false), component(false));
    let lin = DonorInterpolation::Linear;
    let mut a = overset_solver(&bg, &comp, lin, settings(), shear_fields);
    a.set_time_scheme(TimeScheme::Bdf2).unwrap();
    a.set_geometry_corrections(true);
    let mut b = overset_solver(&bg, &comp, lin, settings(), shear_fields);
    b.set_fringe_flux(FringeFlux::PrescribedOwnGradient);
    b.set_geometry_corrections(true);
    let mut st = settings();
    st.rhie_chow_step_consistent = true;
    let mut c = overset_solver(&bg, &comp, lin, st, shear_fields);
    c.set_geometry_corrections(true);
    for s in [&mut a, &mut b, &mut c] {
        let e = s.step(DT, pose(DT), &[]).expect_err("refused");
        assert!(e.contains("ADR 0367"), "{e}");
    }
}
