//! F-134 (ADR 0219): overset (chimera) coupling of a moving component mesh with a static background mesh, in the library.
//!
//! Q0: the overset step with every cell calculated equals the library's ALE step. U: a uniform flow through a moving,
//! rotating component (with the negative control N2). T: the Taylor-Green vortex across a moving component (closed form;
//! order, error against the background alone, the closed-domain mass coupling; negative controls N3, N4). A: the
//! acceptance, a cylinder moving through a background mesh (TR) against a body-fitted run (BF): mass (A1a, A1b) and drag
//! (A2), the set-up checks X1 to X4 and the negative control NM. S0: the smoke run that sized TR. E: refusals.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::{move_mesh, set_follow_conditions};
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::overset::{MassMode, OversetCoupling, OversetSolver};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::sixdof::patch_force_moment;
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{axis_coords, cylinder_ogrid, expansion_for_end_cell, rectilinear_2d, Grading};
use rcfd_mesh::motion::{moved_mesh, Pose};
use rcfd_mesh::overset::{compose, CellKind, Overset, OversetSpec, Stencil};
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::sync::Arc;
use std::time::Instant;

/// Kinematic viscosity of TR and BF: Re = U D / nu = 40 with U = 1, D = 1.
const NU: f64 = 0.025;
/// Time step of TR and BF.
const DT: f64 = 0.02;
/// Depth of every 2-D mesh.
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

/// Records the failed checks and prints every check.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Self {
        Checks(Vec::new())
    }
    /// `value <= limit`.
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
    /// `value >= limit`.
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
    fn holds(&mut self, id: &str, ok: bool, what: &str) {
        println!("{id}: {what} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{id}: {what}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

// ---- set-ups ---------------------------------------------------------------------------------------------------------

/// Displacement of the cylinder along x (ADR 0219 M10).
fn displacement(t: f64) -> f64 {
    if t <= 1.0 {
        -(0.5 * t - (PI * t).sin() / (2.0 * PI))
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

fn bf_mesh() -> Mesh {
    let (r_out, g) = radial(66);
    cylinder_ogrid(96, 66, 0.5, r_out, g, DEPTH).unwrap()
}

/// Nodes from `x0` over `len` in `n` cells, the cell at the far end (`toward_end`) or at `x0` of size `h`.
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

fn uniform(x0: f64, len: f64, n: usize) -> Vec<f64> {
    (0..=n).map(|i| x0 + len * i as f64 / n as f64).collect()
}

/// TR's background (20,350 cells) and component (the inner 30 layers of BF, 2880 cells) as an overset.
fn tr_overset() -> Overset {
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
    let (r_out, g) = radial(30);
    let comp = cylinder_ogrid(96, 30, 0.5, r_out, g, DEPTH).unwrap();
    let composite = compose(&bg, &comp, "c_").unwrap();
    let spec = OversetSpec {
        outer_patches: vec!["c_farfield".into()],
        hole: Arc::new(|x: Vec3| x[0] * x[0] + x[1] * x[1] < 1.0),
    };
    Overset::new(composite, spec).unwrap()
}

fn tr_pose(t: f64) -> Pose {
    Pose::translation([2.0 + displacement(t), 0.0, 0.0])
}

/// The composite at `pose` as a geometry.
fn geometry_at(ov: &Overset, pose: &Pose) -> Arc<FvGeometry> {
    let mesh = moved_mesh(&ov.composite.mesh, ov.composite.points_at(pose)).unwrap();
    Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap())
}

/// TR at `t = 0` with the mass mode `mass`.
fn tr_new(mass: MassMode) -> OversetSolver {
    let ov = tr_overset();
    let pose = tr_pose(0.0);
    let geom = geometry_at(&ov, &pose);
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for name in ["xmin", "ymin", "ymax", "c_cylinder"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    OversetSolver::new(
        geom,
        u,
        p,
        settings(NU),
        mass,
        Some(OversetCoupling { overset: ov, pose }),
    )
    .unwrap()
}

fn walls_tr() -> Vec<String> {
    vec!["c_cylinder".to_string()]
}

/// Net flux through a patch.
fn patch_flux(geom: &FvGeometry, phi: &[f64], patch: &str) -> f64 {
    geom.mesh.patch(patch).unwrap().faces().map(|f| phi[f]).sum()
}

fn force_x(geom: &FvGeometry, u: &VectorField, p: &ScalarField, patch: &str, centre: Vec3) -> f64 {
    patch_force_moment(geom, u, p, 1.0, NU, &[patch.to_string()], centre)
        .unwrap()
        .0[0]
}

/// BF at `t = 0`.
fn bf_new() -> (PimpleSolver, Mesh) {
    let mesh0 = bf_mesh();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh0.clone())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for name in ["cylinder", "farfield"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    (PimpleSolver::new(geom, u, p, settings(NU)), mesh0)
}

/// The points of BF at step `n`.
fn bf_points(mesh0: &Mesh, n: usize) -> Vec<Vec3> {
    let d = displacement(n as f64 * DT);
    mesh0.points().iter().map(|q| [q[0] + d, q[1], q[2]]).collect()
}

fn push(bytes: &mut Vec<u8>, x: f64) {
    bytes.extend_from_slice(&x.to_bits().to_le_bytes());
}

// ---- Q0 --------------------------------------------------------------------------------------------------------------

/// ADR 0219 Q0: BF for 20 steps with the overset step (every cell calculated) against the library's ALE step.
#[test]
fn q0_overset_step_with_every_cell_calculated_equals_the_library_ale_step() {
    let (mut a, mesh0) = bf_new();
    let mut b = OversetSolver::new(
        a.geom.clone(),
        a.u.clone(),
        a.p.clone(),
        settings(NU),
        MassMode::Conservative,
        None,
    )
    .unwrap();
    let walls = vec!["cylinder".to_string()];
    for n in 1..=20 {
        let pts = bf_points(&mesh0, n);
        let st = move_mesh(&a.geom, pts.clone(), DT).unwrap();
        let old = a.geom.mesh.clone();
        let dp = set_follow_conditions(&mut a.u, &st, &old, &walls, DT).unwrap();
        a.step_ale_with_walls(DT, st.geom, st.mesh_flux, dp).unwrap();
        let st = move_mesh(&b.geom, pts, DT).unwrap();
        let old = b.geom.mesh.clone();
        let dp = set_follow_conditions(&mut b.u, &st, &old, &walls, DT).unwrap();
        b.advance_mesh(DT, st, dp).unwrap();
    }
    let du =
        a.u.values
            .iter()
            .zip(&b.u.values)
            .map(|(x, y)| vec3::norm(vec3::sub(*x, *y)))
            .fold(0.0, f64::max);
    let dp =
        a.p.values
            .iter()
            .zip(&b.p.values)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f64::max);
    let phi_max = a.phi.iter().map(|x| x.abs()).fold(0.0, f64::max);
    let dphi = a.phi.iter().zip(&b.phi).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max) / phi_max;
    let bits = a.u.values == b.u.values && a.p.values == b.p.values && a.phi == b.phi && a.time == b.time;
    let mut bytes = Vec::new();
    for v in &b.u.values {
        for x in v {
            push(&mut bytes, *x);
        }
    }
    for x in b.p.values.iter().chain(&b.phi) {
        push(&mut bytes, *x);
    }
    push(&mut bytes, b.time);
    bytes.extend_from_slice(&(b.history.len() as u64).to_le_bytes());
    println!(
        "Q0: bit for bit {bits}; hash of the overset run {:016x} (R1 af2c9e6bd3e5596d)",
        rcfd_core::hash::fnv64(&bytes)
    );
    let mut c = Checks::new();
    c.at_most("Q0 u", du, 1e-12);
    c.at_most("Q0 p", dp, 1e-12);
    c.at_most("Q0 phi", dphi, 1e-12);
    c.finish();
}

// ---- U ---------------------------------------------------------------------------------------------------------------

const U_FLOW: Vec3 = [1.0, 0.5, 0.0];
const DT_U: f64 = 0.01;

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

fn u_pose(t: f64) -> Pose {
    Pose {
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        angle: t,
        translation: [t, 0.5 * t, 0.0],
    }
}

fn u_new() -> OversetSolver {
    let bs = uniform(-2.5, 5.0, 50);
    let bg = rectilinear_2d(&bs, &bs, DEPTH).unwrap();
    let ov = box_overset(bg, 0.6, 16, 0.25);
    let pose = u_pose(0.0);
    let geom = geometry_at(&ov, &pose);
    let mut u = VectorField::uniform("U", &geom, U_FLOW);
    for name in ["xmin", "ymin", "ymax"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector(U_FLOW))).unwrap();
    }
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    OversetSolver::new(
        geom,
        u,
        p,
        settings(0.01),
        MassMode::Conservative,
        Some(OversetCoupling { overset: ov, pose }),
    )
    .unwrap()
}

/// The U run; `perturb` multiplies every stencil weight by 1.001 after `prepare` (N2). Returns the largest velocity and
/// pressure deviations, GCL residual, continuity and outflow error, and the number of kind changes.
fn u_run(perturb: bool) -> (f64, f64, f64, f64, f64, usize) {
    let mut s = u_new();
    let nb = s.conn.n_background_cells;
    let n = s.geom.n_cells();
    let mean_v: f64 = s.geom.mesh.cell_volumes()[nb..].iter().sum::<f64>() / (n - nb) as f64;
    let un = vec3::norm(U_FLOW);
    let (mut eu, mut ep, mut gcl, mut cont, mut q, mut changes) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64, 0);
    for k in 1..=80 {
        let mut st = s.prepare(DT_U, u_pose(k as f64 * DT_U), &[]).unwrap();
        if perturb {
            for (_, stn) in st.conn.stencils.iter_mut() {
                stn.weights.iter_mut().for_each(|w| *w *= 1.001);
            }
        }
        let (v0, v1) = (
            s.geom.mesh.cell_volumes().to_vec(),
            st.geom.mesh.cell_volumes().to_vec(),
        );
        let mesh = &st.geom.mesh;
        let mut sum = vec![0.0; n];
        for f in 0..mesh.n_faces() {
            sum[mesh.owner()[f]] += st.mesh_flux[f];
            if f < mesh.n_internal_faces() {
                sum[mesh.neighbour()[f]] -= st.mesh_flux[f];
            }
        }
        for c in nb..n {
            gcl = gcl.max((v1[c] - v0[c] - DT_U * sum[c]).abs() / mean_v);
        }
        let info = s.advance(DT_U, st).unwrap();
        cont = cont.max(info.continuity);
        changes += info.n_changed;
        for c in 0..n {
            if s.conn.kind[c] != CellKind::Hole {
                eu = eu.max(vec3::norm(vec3::sub(s.u.values[c], U_FLOW)) / un);
                ep = ep.max(s.p.values[c].abs() / (un * un));
            }
        }
        let qx = patch_flux(&s.geom, &s.phi, "xmax");
        q = q.max((qx - 0.5).abs() / 0.5);
    }
    (eu, ep, gcl, cont, q, changes)
}

/// ADR 0219 U1 to U4: a uniform flow through a component that moves and turns, the hole and fringe rebuilt every step.
#[test]
fn u_uniform_flow_through_a_moving_rotating_component() {
    let (eu, ep, gcl, cont, q, changes) = u_run(false);
    let mut c = Checks::new();
    c.at_most("U1 velocity", eu, 1e-10);
    c.at_most("U1 pressure", ep, 1e-10);
    c.at_most("U2 GCL", gcl, 1e-12);
    c.at_most("U3 continuity", cont, 1e-9);
    c.at_most("U3 outflow", q, 1e-10);
    c.at_least("U4 (set-up) kind changes", changes as f64, 20.0);
    c.finish();
}

/// ADR 0219 N2 (negative control): every stencil weight multiplied by 1.001.
#[test]
fn n2_uniform_flow_negative_control_perturbed_weights() {
    let (eu, ..) = u_run(true);
    let mut c = Checks::new();
    c.at_least("N2 velocity deviation", eu, 1e-6);
    c.finish();
}

// ---- T ---------------------------------------------------------------------------------------------------------------

const NU_TG: f64 = 0.1;
const DT_TG: f64 = 0.0025;
const STEPS_TG: usize = 200;

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

fn tg_background(level: usize) -> Mesh {
    let bs = uniform(0.0, PI, 40 * level);
    rectilinear_2d(&bs, &bs, DEPTH).unwrap()
}

/// Set the exact velocity at time `t` on the four background sides.
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

/// The error of a velocity field against the exact one at `t` over the cells where `use_cell` holds.
fn tg_error(geom: &FvGeometry, u: &VectorField, t: f64, use_cell: impl Fn(usize) -> bool) -> f64 {
    let e = (-2.0 * NU_TG * t).exp();
    (0..geom.n_cells())
        .filter(|&c| use_cell(c))
        .map(|c| vec3::norm(vec3::sub(u.values[c], tg_u(geom.mesh.cell_centres()[c], t))) / e)
        .fold(0.0, f64::max)
}

/// TGB_L: the background alone with the library step.
fn tg_reference(level: usize) -> f64 {
    let geom = Arc::new(FvGeometry::new(Arc::new(tg_background(level))).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |x| tg_u(x, 0.0));
    tg_conditions(&geom, &mut u, 0.0);
    let p = ScalarField::from_fn("p", &geom, |x| tg_p(x, 0.0));
    let mut s = PimpleSolver::new(geom.clone(), u, p, settings(NU_TG));
    for k in 1..=STEPS_TG {
        tg_conditions(&geom, &mut s.u, k as f64 * DT_TG);
        s.step(DT_TG);
    }
    tg_error(&geom, &s.u, STEPS_TG as f64 * DT_TG, |_| true)
}

#[derive(Clone, Copy, PartialEq)]
enum Variant {
    Registered,
    ZeroMeshFlux,
    Nearest,
}

/// TG_L with the variant (N3: the mesh flux set to zero; N4: every stencil its donor cell). Returns the error, the largest
/// continuity and the range of `delta`.
fn tg_overset(level: usize, variant: Variant) -> (f64, f64, (f64, f64)) {
    let ov = box_overset(tg_background(level), 0.6, 20 * level, 0.25);
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
    let (mut cont, mut dlo, mut dhi) = (0.0f64, f64::INFINITY, f64::NEG_INFINITY);
    for k in 1..=STEPS_TG {
        let t = k as f64 * DT_TG;
        let g = s.geom.clone();
        tg_conditions(&g, &mut s.u, t);
        let mut st = s.prepare(DT_TG, tg_pose(t), &[]).unwrap();
        match variant {
            Variant::Registered => {}
            Variant::ZeroMeshFlux => st.mesh_flux.iter_mut().for_each(|m| *m = 0.0),
            Variant::Nearest => {
                for (_, stn) in st.conn.stencils.iter_mut() {
                    *stn = Stencil {
                        cells: vec![stn.cells[0]],
                        weights: vec![1.0],
                    };
                }
            }
        }
        let info = s.advance(DT_TG, st).unwrap();
        cont = cont.max(info.continuity);
        dlo = dlo.min(info.delta);
        dhi = dhi.max(info.delta);
    }
    let kinds = s.conn.kind.clone();
    let e = tg_error(&s.geom, &s.u, STEPS_TG as f64 * DT_TG, |c| kinds[c] != CellKind::Hole);
    (e, cont, (dlo, dhi))
}

/// ADR 0219 T1 to T3: the Taylor-Green vortex across a moving, turning component at two resolutions.
#[test]
fn t_taylor_green_vortex_across_a_moving_component() {
    let t0 = Instant::now();
    let (eb1, eb2) = (tg_reference(1), tg_reference(2));
    let (e1, c1, d1) = tg_overset(1, Variant::Registered);
    let (e2, c2, d2) = tg_overset(2, Variant::Registered);
    println!(
        "T: background alone e(TGB_1) {eb1:.4e}, e(TGB_2) {eb2:.4e}, order {:.3}; overset e(TG_1) {e1:.4e}, e(TG_2) {e2:.4e}; delta TG_1 [{:.3e}, {:.3e}], TG_2 [{:.3e}, {:.3e}]; {:.1} s",
        (eb1 / eb2).log2(),
        d1.0,
        d1.1,
        d2.0,
        d2.1,
        t0.elapsed().as_secs_f64()
    );
    let mut c = Checks::new();
    c.at_least("T1 observed order", (e1 / e2).log2(), 1.7);
    c.at_most("T2 TG_1 / TGB_1", e1 / eb1, 2.0);
    c.at_most("T2 TG_2 / TGB_2", e2 / eb2, 2.0);
    c.at_most("T3 continuity", c1.max(c2), 1e-9);
    c.finish();
}

/// ADR 0219 N3 and N4 (negative controls) on TG_1.
#[test]
fn n3_n4_taylor_green_negative_controls() {
    let eb1 = tg_reference(1);
    let (e3, ..) = tg_overset(1, Variant::ZeroMeshFlux);
    let (e4, ..) = tg_overset(1, Variant::Nearest);
    println!("N3/N4: e(TGB_1) {eb1:.4e}");
    let mut c = Checks::new();
    c.at_least("N3 zero mesh flux, e / e(TGB_1)", e3 / eb1, 2.0);
    c.at_least("N4 nearest donor, e / e(TGB_1)", e4 / eb1, 2.0);
    c.finish();
}

// ---- A ---------------------------------------------------------------------------------------------------------------

/// ADR 0219 S0 (smoke, run before the registered checks to size the test): TR for 5 steps.
#[test]
fn s0_smoke_tr_five_steps() {
    let t0 = Instant::now();
    let mut s = tr_new(MassMode::Conservative);
    println!(
        "S0: set-up {:.2} s, {} cells",
        t0.elapsed().as_secs_f64(),
        s.geom.n_cells()
    );
    for n in 1..=5 {
        let t1 = Instant::now();
        let info = s.step(DT, tr_pose(n as f64 * DT), &walls_tr()).unwrap();
        println!(
            "S0: step {n}: {:.2} s, p iterations {}, unconverged {}, continuity {:.2e}, delta {:.3e}, holes {}, fringe {} + {}",
            t1.elapsed().as_secs_f64(),
            info.p_iters,
            info.unconverged,
            info.continuity,
            info.delta,
            info.n_hole,
            info.n_fringe_background,
            info.n_fringe_component
        );
    }
    assert!(s.u.values.iter().flatten().chain(&s.p.values).all(|x| x.is_finite()));
}

/// ADR 0219 A1a, A1b, A2 and X1 to X4: a cylinder moving through a background mesh against the body-fitted run.
#[test]
fn a_cylinder_moving_through_a_background_mesh_conserves_mass_and_matches_the_body_fitted_drag() {
    const STEPS: usize = 400;
    let t0 = Instant::now();
    let mut s = tr_new(MassMode::Conservative);
    let nb = s.conn.n_background_cells;
    let (mut a1a, mut a1b, mut x1, mut x2, mut x3, mut x4, mut iters) =
        (0.0f64, 0.0f64, true, true, 0usize, 0usize, 0usize);
    let (mut dlo, mut dhi) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut f_tr = vec![0.0; STEPS + 1];
    for n in 1..=STEPS {
        let pose = tr_pose(n as f64 * DT);
        let info = s.step(DT, pose, &walls_tr()).unwrap();
        a1a = a1a.max(info.continuity);
        a1b = a1b.max(patch_flux(&s.geom, &s.phi, "xmax").abs() / 0.1);
        let centre = pose.translation;
        let cc = s.geom.mesh.cell_centres();
        for c in 0..nb {
            if ((cc[c][0] - centre[0]).powi(2) + (cc[c][1] - centre[1]).powi(2)).sqrt() < 0.5 {
                x1 &= s.conn.kind[c] == CellKind::Hole;
            }
        }
        x2 &= info.n_fringe_component == 96 && info.n_hole > 0 && info.n_fringe_background > 0;
        x3 += info.n_changed;
        x4 += info.unconverged;
        iters += info.p_iters;
        dlo = dlo.min(info.delta);
        dhi = dhi.max(info.delta);
        f_tr[n] = force_x(&s.geom, &s.u, &s.p, "c_cylinder", centre);
    }
    let t_tr = t0.elapsed().as_secs_f64();
    let t1 = Instant::now();
    let (mut b, mesh0) = bf_new();
    let walls = vec!["cylinder".to_string()];
    let mut f_bf = vec![0.0; STEPS + 1];
    let (mut cont_bf, mut bf_cap) = (0.0f64, 0usize);
    for n in 1..=STEPS {
        let st = move_mesh(&b.geom, bf_points(&mesh0, n), DT).unwrap();
        let old = b.geom.mesh.clone();
        let dp = set_follow_conditions(&mut b.u, &st, &old, &walls, DT).unwrap();
        let info = b.step_ale_with_walls(DT, st.geom, st.mesh_flux, dp).unwrap();
        cont_bf = cont_bf.max(info.continuity);
        bf_cap = bf_cap.max(info.p_iters);
        f_bf[n] = force_x(&b.geom, &b.u, &b.p, "cylinder", [displacement(n as f64 * DT), 0.0, 0.0]);
    }
    let t_bf = t1.elapsed().as_secs_f64();
    let cd = |f: f64| f / (0.5 * DEPTH);
    let (mut a2, mut a2_at, mut mean_rel, mut mean_tr, mut mean_bf) = (0.0f64, 0usize, 0.0, 0.0, 0.0);
    for n in 100..=STEPS {
        let r = (f_tr[n] - f_bf[n]).abs() / f_bf[n].abs();
        if r > a2 {
            a2 = r;
            a2_at = n;
        }
        mean_rel += (f_tr[n] - f_bf[n]) / f_bf[n] / 301.0;
        mean_tr += cd(f_tr[n]) / 301.0;
        mean_bf += cd(f_bf[n]) / 301.0;
    }
    for n in [100, 200, 300, 400] {
        println!(
            "A: t = {:.0}: C_D overset {:.5}, body-fitted {:.5}, difference {:+.3e}",
            n as f64 * DT,
            cd(f_tr[n]),
            cd(f_bf[n]),
            (f_tr[n] - f_bf[n]) / f_bf[n]
        );
    }
    println!(
        "A: mean C_D over [2, 8]: overset {mean_tr:.5}, body-fitted {mean_bf:.5}, mean relative difference {mean_rel:+.3e}; largest at step {a2_at}"
    );
    println!(
        "A: delta / (U^2/2) in [{:.3e}, {:.3e}]; BF continuity {cont_bf:.2e}, BF largest pressure iterations of a step {bf_cap}; overset pressure iterations per step {:.1}; wall time overset {t_tr:.1} s, body-fitted {t_bf:.1} s",
        dlo / 0.5,
        dhi / 0.5,
        iters as f64 / STEPS as f64
    );
    let mut c = Checks::new();
    c.at_most("A1a continuity", a1a, 1e-9);
    c.at_most("A1b outflow / (U D L)", a1b, 1e-9);
    c.at_most("A2 drag", a2, 5e-2);
    c.holds("X1", x1, "every background cell within 0.5 of the centre is a hole");
    c.holds(
        "X2",
        x2,
        "96 component fringe cells, holes and background fringe cells in every step",
    );
    c.at_least("X3 kind changes", x3 as f64, 500.0);
    c.holds("X4 overset", x4 == 0, &format!("{x4} unconverged linear solves"));
    c.holds(
        "X4 body-fitted",
        bf_cap < 3000,
        &format!("largest pressure iterations of a step {bf_cap}"),
    );
    c.finish();
}

/// ADR 0219 NM (negative control): TR with the interpolated (non-conservative) overlap for 100 steps.
#[test]
fn nm_interpolated_overlap_leaks_mass() {
    let mut s = tr_new(MassMode::Interpolated);
    let mut q = 0.0f64;
    for n in 1..=100 {
        s.step(DT, tr_pose(n as f64 * DT), &walls_tr()).unwrap();
        q = q.max(patch_flux(&s.geom, &s.phi, "xmax").abs() / 0.1);
    }
    let mut c = Checks::new();
    c.at_least("NM outflow / (U D L)", q, 1e-6);
    c.finish();
}

// ---- E ---------------------------------------------------------------------------------------------------------------

/// ADR 0219 E1: refused settings.
#[test]
fn e1_refused_settings() {
    let base = u_new();
    let mut cases: Vec<(&str, PimpleSettings)> = Vec::new();
    let mut s = settings(0.01);
    s.time_scheme = TimeScheme::Bdf2;
    cases.push(("BDF2", s));
    let mut s = settings(0.01);
    s.n_outer = 2;
    cases.push(("n_outer 2", s));
    let mut s = settings(0.01);
    s.non_orth_correctors = 1;
    cases.push(("non-orthogonal corrector", s));
    let mut s = settings(0.01);
    s.convection = ConvScheme::LinearUpwind;
    cases.push(("linear upwind", s));
    let mut s = settings(0.01);
    s.consistent_correctors = true;
    cases.push(("consistent correctors", s));
    let mut c = Checks::new();
    for (what, st) in cases {
        let r = OversetSolver::new(
            base.geom.clone(),
            base.u.clone(),
            base.p.clone(),
            st,
            MassMode::Conservative,
            base.coupling.clone(),
        );
        let msg = r.as_ref().err().cloned().unwrap_or_default();
        c.holds("E1", r.is_err(), &format!("{what} refused: {msg}"));
    }
    c.finish();
}

/// ADR 0219 E2: a step that uncovers a hole cell as a calculated cell is refused and leaves the state unchanged.
#[test]
fn e2_uncovered_hole_cell_is_refused() {
    let mut s = tr_new(MassMode::Conservative);
    let (u0, p0, phi0, t0) = (s.u.values.clone(), s.p.values.clone(), s.phi.clone(), s.time);
    let st = s
        .prepare(DT, Pose::translation([2.0 - 1.5, 0.0, 0.0]), &walls_tr())
        .unwrap();
    let r = s.advance(DT, st);
    let msg = r.as_ref().err().cloned().unwrap_or_default();
    println!("E2: {msg}");
    let mut c = Checks::new();
    c.holds(
        "E2",
        r.is_err() && msg.contains("uncovered"),
        "refused, naming 'uncovered'",
    );
    c.holds(
        "E2 state",
        s.u.values == u0 && s.p.values == p0 && s.phi == phi0 && s.time == t0,
        "u, p, phi, time unchanged",
    );
    c.finish();
}

/// ADR 0219 E3: a step prepared on another composite is refused.
#[test]
fn e3_step_of_another_composite_is_refused() {
    let mut other = u_new();
    let st = other.prepare(DT_U, u_pose(DT_U), &[]).unwrap();
    let mut s = tr_new(MassMode::Conservative);
    let r = s.advance(DT, st);
    println!("E3: {:?}", r.as_ref().err());
    let mut c = Checks::new();
    c.holds("E3", r.is_err(), "refused");
    c.finish();
}
