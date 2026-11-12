//! F-368 (ADR 0367): the opt-in skewness and non-orthogonal corrections of the overset step
//! (`OversetSolver::set_geometry_corrections`). The defaults fingerprint D1 (recorded on the tree before any library change).
//!
//! Release: `cargo test --release -p rcfd-fv --test overset_f368`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::move_mesh;
use rcfd_fv::bc::{FixedValue, Profile};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::overset::{FringeFlux, MassMode, OversetCoupling, OversetSolver};
use rcfd_fv::pimple::PimpleSettings;
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_mesh, warp, Grading};
use rcfd_mesh::motion::{moved_mesh, Pose};
use rcfd_mesh::overset::{compose, DonorInterpolation, Overset, OversetSpec};
use rcfd_mesh::Mesh;
use std::sync::Arc;

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
