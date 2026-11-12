//! F-335 (ADR 0238) records R2: the bits of three overset runs of ADR 0219 that the outer iterations of F-335 must not
//! change while a solver does not opt in, recorded on the library of main 65c36e60 before any library change of F-335
//! (Windows; printed elsewhere, ADR 0137). R2a: the uniform flow U, 80 steps (open domain, the component moves and turns).
//! R2b: the Taylor-Green set-up TG_1, 200 steps (closed domain). R2c: the cylinder TR, the first 20 steps (a moving wall).
//! Hash of each: the bits of every cell velocity, pressure and face flux, the time, the step count and every step's `delta`.

use rcfd_core::vec3::Vec3;
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::overset::{MassMode, OversetCoupling, OversetSolver};
use rcfd_fv::pimple::PimpleSettings;
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{axis_coords, cylinder_ogrid, expansion_for_end_cell, rectilinear_2d, Grading};
use rcfd_mesh::motion::{moved_mesh, Pose};
use rcfd_mesh::overset::{compose, Overset, OversetSpec};
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::sync::Arc;

const DEPTH: f64 = 0.1;
/// Solver controls of every set-up (ADR 0219 PS).
const SOLVE: SolveControl = SolveControl {
    tol_abs: 1e-15,
    tol_rel: 1e-12,
    max_iter: 3000,
};

/// ADR 0219 PS.
fn settings(nu: f64) -> PimpleSettings {
    let mut s = PimpleSettings::piso(nu);
    s.convection = ConvScheme::Central;
    s.momentum_solver = SOLVE;
    s.pressure_solver = SOLVE;
    s
}

fn push(bytes: &mut Vec<u8>, x: f64) {
    bytes.extend_from_slice(&x.to_bits().to_le_bytes());
}

/// The hash of a solver's state and of the `delta` of every step.
fn state_hash(s: &OversetSolver) -> String {
    let mut bytes = Vec::new();
    for v in &s.u.values {
        for x in v {
            push(&mut bytes, *x);
        }
    }
    for x in s.p.values.iter().chain(&s.phi) {
        push(&mut bytes, *x);
    }
    push(&mut bytes, s.time);
    bytes.extend_from_slice(&(s.history.len() as u64).to_le_bytes());
    for h in &s.history {
        push(&mut bytes, h.delta);
    }
    format!("{:016x}", rcfd_core::hash::fnv64(&bytes))
}

fn check(name: &str, got: &str, want: &str) {
    println!("{name}: {got}");
    if cfg!(windows) {
        assert_eq!(got, want, "{name}");
    }
}

fn uniform(x0: f64, len: f64, n: usize) -> Vec<f64> {
    (0..=n).map(|i| x0 + len * i as f64 / n as f64).collect()
}

fn geometry_at(ov: &Overset, pose: &Pose) -> Arc<FvGeometry> {
    let mesh = moved_mesh(&ov.composite.mesh, ov.composite.points_at(pose)).unwrap();
    Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap())
}

fn box_overset(bg: Mesh, half: f64, n: usize, hole: f64) -> Overset {
    let cs = uniform(-half, 2.0 * half, n);
    let comp = rectilinear_2d(&cs, &cs, DEPTH).unwrap();
    let composite = compose(&bg, &comp, "c_").unwrap();
    let spec = OversetSpec {
        outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax"].map(String::from).to_vec(),
        hole: Arc::new(move |x: Vec3| x[0].abs().max(x[1].abs()) < hole),
    };
    Overset::new(composite, spec).unwrap()
}

// ---- U of ADR 0219 ---------------------------------------------------------------------------------------------------

const U_FLOW: Vec3 = [1.0, 0.5, 0.0];

fn u_pose(t: f64) -> Pose {
    Pose {
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        angle: t,
        translation: [t, 0.5 * t, 0.0],
    }
}

/// U for 80 steps; `opt` is applied to the solver before the first step.
fn u_hash(opt: impl Fn(&mut OversetSolver)) -> String {
    const DT_U: f64 = 0.01;
    let bs = uniform(-2.5, 5.0, 50);
    let ov = box_overset(rectilinear_2d(&bs, &bs, DEPTH).unwrap(), 0.6, 16, 0.25);
    let pose = u_pose(0.0);
    let geom = geometry_at(&ov, &pose);
    let mut u = VectorField::uniform("U", &geom, U_FLOW);
    for name in ["xmin", "ymin", "ymax"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector(U_FLOW))).unwrap();
    }
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut s = OversetSolver::new(
        geom,
        u,
        p,
        settings(0.01),
        MassMode::Conservative,
        Some(OversetCoupling { overset: ov, pose }),
    )
    .unwrap();
    opt(&mut s);
    for k in 1..=80 {
        s.step(DT_U, u_pose(k as f64 * DT_U), &[]).unwrap();
    }
    state_hash(&s)
}

// ---- TG_1 of ADR 0219 ------------------------------------------------------------------------------------------------

const NU_TG: f64 = 0.1;

fn tg_u(x: Vec3, t: f64) -> Vec3 {
    let e = (-2.0 * NU_TG * t).exp();
    [-x[0].cos() * x[1].sin() * e, x[0].sin() * x[1].cos() * e, 0.0]
}

fn tg_p(x: Vec3, t: f64) -> f64 {
    -((2.0 * x[0]).cos() + (2.0 * x[1]).cos()) * (-4.0 * NU_TG * t).exp() / 4.0
}

fn tg_pose(t: f64) -> Pose {
    Pose {
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        angle: t,
        translation: [PI / 2.0 - 0.3 + 0.5 * t, PI / 2.0 - 0.2 + 0.3 * t, 0.0],
    }
}

fn tg_conditions(geom: &FvGeometry, u: &mut VectorField, t: f64) {
    for name in ["xmin", "xmax", "ymin", "ymax"] {
        let v: Vec<Vec3> = geom
            .mesh
            .patch(name)
            .unwrap()
            .faces()
            .map(|f| tg_u(geom.mesh.face_centres()[f], t))
            .collect();
        u.set_bc(geom, name, Box::new(FixedValue(Profile::PerFace(v)))).unwrap();
    }
}

/// TG_1 for 200 steps (`dt = 0.0025`); `opt` is applied to the solver before the first step.
fn tg_hash(opt: impl Fn(&mut OversetSolver)) -> String {
    let dt = 0.5 / 200.0;
    let bs = uniform(0.0, PI, 40);
    let ov = box_overset(rectilinear_2d(&bs, &bs, DEPTH).unwrap(), 0.6, 20, 0.25);
    let pose = tg_pose(0.0);
    let geom = geometry_at(&ov, &pose);
    let mut u = VectorField::from_fn("U", &geom, |x| tg_u(x, 0.0));
    tg_conditions(&geom, &mut u, 0.0);
    let p = ScalarField::from_fn("p", &geom, |x| tg_p(x, 0.0));
    let mut s = OversetSolver::new(
        geom,
        u,
        p,
        settings(NU_TG),
        MassMode::Conservative,
        Some(OversetCoupling { overset: ov, pose }),
    )
    .unwrap();
    opt(&mut s);
    for k in 1..=200 {
        let t = k as f64 * dt;
        let g = s.geom.clone();
        tg_conditions(&g, &mut s.u, t);
        s.step(dt, tg_pose(t), &[]).unwrap();
    }
    state_hash(&s)
}

// ---- TR of ADR 0219 --------------------------------------------------------------------------------------------------

fn displacement(t: f64) -> f64 {
    if t <= 1.0 {
        -(0.5 * t - (PI * t).sin() / (2.0 * PI))
    } else {
        -(0.5 + (t - 1.0))
    }
}

fn graded(x0: f64, len: f64, n: usize, h: f64, toward_end: bool) -> Vec<f64> {
    let r = expansion_for_end_cell(n, len, h).unwrap();
    let r = if toward_end { r } else { 1.0 / r };
    axis_coords(n, len, Grading::Expansion(r))
        .unwrap()
        .into_iter()
        .map(|x| x0 + x)
        .collect()
}

fn join(parts: &[Vec<f64>]) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for p in parts {
        let skip = usize::from(!out.is_empty());
        out.extend_from_slice(&p[skip..]);
    }
    out
}

/// TR for its first 20 steps (`dt = 0.02`); `opt` is applied to the solver before the first step.
fn tr_hash(opt: impl Fn(&mut OversetSolver)) -> String {
    const DT: f64 = 0.02;
    let xs = join(&[
        graded(-25.0, 17.0, 30, 0.1, true),
        uniform(-8.0, 12.5, 125),
        graded(4.5, 17.5, 30, 0.1, false),
    ]);
    let ys = join(&[
        graded(-20.0, 17.5, 30, 0.1, true),
        uniform(-2.5, 5.0, 50),
        graded(2.5, 17.5, 30, 0.1, false),
    ]);
    let bg = rectilinear_2d(&xs, &ys, DEPTH).unwrap();
    let q: f64 = 1.08;
    let r_out = 0.5 + 0.01 * (q.powi(30) - 1.0) / (q - 1.0);
    let comp = cylinder_ogrid(96, 30, 0.5, r_out, Grading::Expansion(q.powi(29)), DEPTH).unwrap();
    let composite = compose(&bg, &comp, "c_").unwrap();
    let spec = OversetSpec {
        outer_patches: vec!["c_farfield".into()],
        hole: Arc::new(|x: Vec3| x[0] * x[0] + x[1] * x[1] < 1.0),
    };
    let ov = Overset::new(composite, spec).unwrap();
    let pose_at = |t: f64| Pose::translation([2.0 + displacement(t), 0.0, 0.0]);
    let pose = pose_at(0.0);
    let geom = geometry_at(&ov, &pose);
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for name in ["xmin", "ymin", "ymax", "c_cylinder"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut s = OversetSolver::new(
        geom,
        u,
        p,
        settings(0.025),
        MassMode::Conservative,
        Some(OversetCoupling { overset: ov, pose }),
    )
    .unwrap();
    opt(&mut s);
    let walls = vec!["c_cylinder".to_string()];
    for n in 1..=20 {
        s.step(DT, pose_at(n as f64 * DT), &walls).unwrap();
    }
    state_hash(&s)
}

// ---- the records -----------------------------------------------------------------------------------------------------

const R2A: &str = "7aa14d1e2e2da052";
const R2B: &str = "75ac2daf13879872";
const R2C: &str = "fddb420dc287502a";

/// R2a: U, 80 steps, a solver that does not opt in.
#[test]
fn r2a_uniform_flow_record() {
    check("R2a", &u_hash(|_| {}), R2A);
}

/// R2b: TG_1, 200 steps, a solver that does not opt in.
#[test]
fn r2b_taylor_green_level_1_record() {
    check("R2b", &tg_hash(|_| {}), R2B);
}

/// R2c: TR, the first 20 steps, a solver that does not opt in.
#[test]
fn r2c_moving_cylinder_record() {
    check("R2c", &tr_hash(|_| {}), R2C);
}
