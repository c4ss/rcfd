//! F-134 (ADR 0219) record R1: the bits of the library run that F-134 must not change and that its equivalence check Q0
//! compares with, recorded on main bfa9a3ee before any code of F-134 (Windows; printed elsewhere, ADR 0137). R1 is the
//! body-fitted run BF of ADR 0219 for its first 20 steps: a circular cylinder of diameter 1 on an O-grid that translates
//! rigidly with the cylinder (ADR 0033's ALE step, the wall following the mesh, the far field at rest), Re = 40.

use rcfd_core::vec3::Vec3;
use rcfd_fv::ale::{move_mesh, set_follow_conditions};
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{cylinder_ogrid, Grading};
use rcfd_mesh::Mesh;
use std::sync::Arc;

/// Kinematic viscosity: Re = U D / nu = 40 with U = 1, D = 1.
const NU: f64 = 0.025;
/// Time step of every run of ADR 0219's set-ups TR and BF.
const DT: f64 = 0.02;
/// Depth of the one-cell-deep 2-D meshes.
const DEPTH: f64 = 0.1;
/// Solver controls of the set-ups (ADR 0219 PS).
const SOLVE: SolveControl = SolveControl {
    tol_abs: 1e-15,
    tol_rel: 1e-12,
    max_iter: 3000,
};

fn push(bytes: &mut Vec<u8>, x: f64) {
    bytes.extend_from_slice(&x.to_bits().to_le_bytes());
}

fn check(name: &str, bytes: &[u8], want: &str) {
    let got = format!("{:016x}", rcfd_core::hash::fnv64(bytes));
    println!("{name}: {got}");
    if cfg!(windows) {
        assert_eq!(got, want, "{name}");
    }
}

/// ADR 0219 PS: PISO, central convection, the solver controls `SOLVE`, everything else at the library defaults.
fn settings() -> PimpleSettings {
    let mut s = PimpleSettings::piso(NU);
    s.convection = ConvScheme::Central;
    s.momentum_solver = SOLVE;
    s.pressure_solver = SOLVE;
    s
}

/// Displacement of the cylinder along x at time `t` (ADR 0219 M10): a smooth start to the speed 1 in the -x direction over one
/// time unit, `v(t) = -(1 - cos(pi t))/2`, then constant speed.
fn displacement(t: f64) -> f64 {
    if t <= 1.0 {
        -(0.5 * t - (std::f64::consts::PI * t).sin() / (2.0 * std::f64::consts::PI))
    } else {
        -(0.5 + (t - 1.0))
    }
}

/// Outer radius and grading of an O-grid of `n` radial cells from `r = 0.5` whose cells grow by 1.08 from 0.01 at the wall.
fn radial(n: usize) -> (f64, Grading) {
    let q: f64 = 1.08;
    let len = 0.01 * (q.powi(n as i32) - 1.0) / (q - 1.0);
    (0.5 + len, Grading::Expansion(q.powi(n as i32 - 1)))
}

/// The body-fitted mesh BF: 96 cells around, 66 radial cells to `r = 20.46`.
fn bf_mesh() -> Mesh {
    let (r_out, g) = radial(66);
    cylinder_ogrid(96, 66, 0.5, r_out, g, DEPTH).unwrap()
}

/// R1: the first 20 steps of BF. Hash: the bits of every cell velocity, pressure and face flux, the time and the step count.
#[test]
fn r1_body_fitted_translating_cylinder_record() {
    let mesh0 = bf_mesh();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh0.clone())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for name in ["cylinder", "farfield"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = PimpleSolver::new(geom, u, p, settings());
    let walls = vec!["cylinder".to_string()];
    let mut cont: f64 = 0.0;
    for n in 1..=20 {
        let d = displacement(n as f64 * DT);
        let pts: Vec<Vec3> = mesh0.points().iter().map(|q| [q[0] + d, q[1], q[2]]).collect();
        let st = move_mesh(&s.geom, pts, DT).unwrap();
        let old = s.geom.mesh.clone();
        let dpdn = set_follow_conditions(&mut s.u, &st, &old, &walls, DT).unwrap();
        let info = s.step_ale_with_walls(DT, st.geom, st.mesh_flux, dpdn).unwrap();
        cont = cont.max(info.continuity);
    }
    println!("R1: {} cells, largest continuity {cont:.3e}", s.geom.n_cells());
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
    check("R1", &bytes, "af2c9e6bd3e5596d");
}
