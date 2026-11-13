//! F-487 (ADR 0387): a consistent cell pressure gradient in the overset geometry corrections: the flow V of ADR 0382 on B3
//! and B3d with the component moving as in ADR 0255, with `OversetSolver::set_consistent_geometry_corrections` (the
//! acceptance A1 to A3), ADR 0367's linear shear and uniform flow with it (A4), the warped background alone and the share of
//! the warped background in the remainder (A6), the gradient at operator level (K1 to K6), the set-up checks and refusals,
//! and the fingerprints D1c of the corrections with ADR 0382's L.
//!
//! Release: `cargo test --release -p rcfd-fv --test overset_f487`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{FixedValue, Profile};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::overset::{MassMode, OversetCoupling, OversetSolver};
use rcfd_fv::pimple::PimpleSettings;
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_mesh, warp, Grading};
use rcfd_mesh::motion::{moved_mesh, Pose};
use rcfd_mesh::overset::{compose, DonorInterpolation, Overset, OversetSpec};
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::sync::Arc;

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
