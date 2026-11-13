//! F-345 (ADR 0379): BDF2 for the library's moving-mesh step (`PimpleSolver::set_moving_mesh_time_scheme`) and the
//! three-level follow conditions of a moving wall (`ale::set_follow_conditions_bdf2`).
//!
//! E: the opt-in, the refusals and the history. G: a uniform flow on the deforming meshes of ADR 0033 G2. Q: the overset
//! step with every cell calculated against the library's BDF2 moving step on BF of ADR 0219, and the library at rest. F: the
//! three-level wall velocity and acceleration. A (ignored heavy records, run once): the time error of BF and TR.
//! The limits are those registered in ADR 0379 before the code ran.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::{move_mesh, set_follow_conditions, set_follow_conditions_bdf2, FollowHistory};
use rcfd_fv::bc::{FixedValue, Profile};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::overset::{MassMode, OversetCoupling, OversetSolver};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::sixdof::patch_force_moment;
use rcfd_fv::sliding::SlidingStep;
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{
    axis_coords, box_mesh, cavity_2d_rect, cylinder_ogrid, expansion_for_end_cell, rectilinear_2d, Grading,
};
use rcfd_mesh::motion::{moved_mesh, Pose};
use rcfd_mesh::overset::{compose, CellKind, Overset, OversetSpec};
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::sync::Arc;
use std::time::Instant;

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
    fn within(&mut self, id: &str, value: f64, lo: f64, hi: f64) {
        let ok = value >= lo && value <= hi;
        println!(
            "{id}: {value:.4e} (limit in [{lo:.3e}, {hi:.3e}]) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} outside [{lo:e}, {hi:e}]"));
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

fn bdf2() -> TimeScheme {
    TimeScheme::Bdf2
}

fn max_vec_diff(a: &[Vec3], b: &[Vec3]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| vec3::norm(vec3::sub(*x, *y)))
        .fold(0.0, f64::max)
}

fn max_diff(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max)
}

fn bits_equal(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

fn vec_bits_equal(a: &[Vec3], b: &[Vec3]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| (0..3).all(|k| x[k].to_bits() == y[k].to_bits()))
}

fn same_state(a: &PimpleSolver, b: &PimpleSolver) -> bool {
    vec_bits_equal(&a.u.values, &b.u.values)
        && bits_equal(&a.p.values, &b.p.values)
        && bits_equal(&a.phi, &b.phi)
        && a.time.to_bits() == b.time.to_bits()
}

// ---- the G2 set-up of ADR 0033 (ale_gcl.rs) -----------------------------------------------------------------------------

const TIGHT: SolveControl = SolveControl {
    tol_abs: 1e-16,
    tol_rel: 1e-13,
    max_iter: 5000,
};

fn g2_settings(nu: f64) -> PimpleSettings {
    let mut s = PimpleSettings::piso(nu);
    s.convection = ConvScheme::Central;
    s.momentum_solver = TIGHT;
    s.pressure_solver = TIGHT;
    s
}

fn mesh_a() -> Mesh {
    box_mesh(
        [5, 4, 3],
        [1.0, 0.8, 0.6],
        [Grading::Expansion(2.0), Grading::Symmetric(2.0), Grading::Uniform],
    )
    .unwrap()
}

type Motion = Box<dyn Fn(&Mesh, f64) -> Vec<Vec3>>;

/// The four motions of ADR 0033 G2.
fn g2_motions() -> Vec<(&'static str, Motion)> {
    let tau = std::f64::consts::TAU;
    let centre = [0.5, 0.4, 0.3];
    vec![
        (
            "translation",
            Box::new(|m: &Mesh, t: f64| {
                m.points()
                    .iter()
                    .map(|&p| vec3::add(p, vec3::scale([0.4, -0.3, 0.2], t)))
                    .collect()
            }),
        ),
        (
            "rotation",
            Box::new(move |m: &Mesh, t: f64| {
                let pose = Pose {
                    origin: centre,
                    axis: [0.0, 0.0, 1.0],
                    angle: t,
                    translation: [0.0; 3],
                };
                m.points().iter().map(|&p| pose.apply(p)).collect()
            }),
        ),
        (
            "oscillation",
            Box::new(move |m: &Mesh, t: f64| {
                let dx = 0.05 * (tau * t / 0.5).sin();
                m.points().iter().map(|&p| vec3::add(p, [dx, 0.0, 0.0])).collect()
            }),
        ),
        (
            "expansion",
            Box::new(move |m: &Mesh, t: f64| {
                let s = 1.0 + 0.2 * (tau * t / 0.5).sin();
                m.points()
                    .iter()
                    .map(|&p| vec3::add(centre, vec3::scale(vec3::sub(p, centre), s)))
                    .collect()
            }),
        ),
    ]
}

const U0: Vec3 = [1.0, 0.5, -0.25];

/// G2 with the BDF2 opt-in: the steps `dts`, optionally one step at rest first, the mesh flux scaled by `scale`. Returns the
/// largest `abs(u - u0) / abs(u0)` and `abs(p) / abs(u0)^2` over the steps, and the set-up numbers of G2s.
fn g2_run(motion: &Motion, dts: &[f64], rest_first: bool, scale: f64) -> (f64, f64, f64, f64) {
    let mesh = mesh_a();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh.clone())).unwrap());
    let mut u = VectorField::uniform("U", &geom, U0);
    for p in mesh.patches() {
        u.set_bc(&geom, &p.name, Box::new(FixedValue::vector(U0))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = PimpleSolver::new(geom, u, p, g2_settings(0.01));
    s.set_moving_mesh_time_scheme(bdf2()).unwrap();
    if rest_first {
        s.step(dts[0]);
    }
    let un = vec3::norm(U0);
    let (mut du, mut dp, mut dv, mut dm) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let mut t = 0.0;
    let mut last_flux: Option<Vec<f64>> = None;
    for &dt in dts {
        t += dt;
        let st = move_mesh(&s.geom, motion(&mesh, t), dt).unwrap();
        let (v0, v1) = (s.geom.mesh.cell_volumes(), st.geom.mesh.cell_volumes());
        for c in 0..v0.len() {
            dv = dv.max((v1[c] - v0[c]).abs() / v0[c]);
        }
        if let Some(lf) = &last_flux {
            let mx = st.mesh_flux.iter().chain(lf.iter()).fold(0.0f64, |a, b| a.max(b.abs()));
            if mx > 0.0 {
                dm = dm.max(max_diff(&st.mesh_flux, lf) / mx);
            }
        }
        last_flux = Some(st.mesh_flux.clone());
        let flux: Vec<f64> = st.mesh_flux.iter().map(|m| m * scale).collect();
        s.step_ale_with_walls(dt, st.geom, flux, Vec::new()).unwrap();
        du = du.max(
            s.u.values
                .iter()
                .map(|v| vec3::norm(vec3::sub(*v, U0)))
                .fold(0.0, f64::max)
                / un,
        );
        dp = dp.max(s.p.values.iter().fold(0.0f64, |a, b| a.max(b.abs())) / (un * un));
    }
    (du, dp, dv, dm)
}

#[test]
fn g2_uniform_flow_stays_uniform_with_bdf2_on_the_meshes_of_adr_0033() {
    let mut c = Checks::new();
    let uniform = vec![0.05; 10];
    let alternating: Vec<f64> = (0..10).map(|i| if i % 2 == 0 { 0.05 } else { 0.025 }).collect();
    for (name, motion) in g2_motions() {
        let (du, dp, dv, dm) = g2_run(&motion, &uniform, false, 1.0);
        c.at_most(&format!("G2b {name} u"), du, 1e-10);
        c.at_most(&format!("G2b {name} p"), dp, 1e-10);
        if name == "expansion" {
            c.at_least("G2s volume change", dv, 1e-2);
            c.at_least("G2s mesh-flux change", dm, 1e-2);
        }
        let (du, dp, _, _) = g2_run(&motion, &alternating, false, 1.0);
        c.at_most(&format!("G2v {name} u"), du, 1e-10);
        c.at_most(&format!("G2v {name} p"), dp, 1e-10);
        if name == "expansion" {
            let (du, dp, _, _) = g2_run(&motion, &uniform, true, 1.0);
            c.at_most("G2r u", du, 1e-10);
            c.at_most("G2r p", dp, 1e-10);
        }
    }
    c.finish();
}

#[test]
fn g2n_negative_control_scaled_mesh_flux() {
    let (_, motion) = g2_motions().into_iter().find(|(n, _)| *n == "expansion").unwrap();
    let (du, _, _, _) = g2_run(&motion, &[0.05; 10], false, 1.01);
    let mut c = Checks::new();
    c.at_least("G2n u", du, 1e-6);
    c.finish();
}

// ---- the lid cavity (E, QR) ---------------------------------------------------------------------------------------------

fn cavity(n: usize) -> (PimpleSolver, Mesh) {
    let mesh = cavity_2d_rect(n, n, 1.0, 1.0).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh.clone())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "walls", Box::new(FixedValue(Profile::Uniform([0.0; 3]))))
        .unwrap();
    u.set_bc(&geom, "lid", Box::new(FixedValue(Profile::Uniform([1.0, 0.0, 0.0]))))
        .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    (PimpleSolver::new(geom, u, p, g2_settings(0.05)), mesh)
}

const V_CAV: Vec3 = [0.6, 0.2, 0.0];
const DT_CAV: f64 = 0.02;

/// One moving step of the cavity to the points of `mesh0` translated by `V_CAV t`.
fn cav_move(s: &mut PimpleSolver, mesh0: &Mesh, t: f64) {
    let pts: Vec<Vec3> = mesh0
        .points()
        .iter()
        .map(|&p| vec3::add(p, vec3::scale(V_CAV, t)))
        .collect();
    let st = move_mesh(&s.geom, pts, DT_CAV).unwrap();
    s.step_ale_with_walls(DT_CAV, st.geom, st.mesh_flux, Vec::new())
        .unwrap();
}

#[test]
fn e1_e4_refusals() {
    let mut c = Checks::new();
    // E1
    let (mut s, mesh0) = cavity(8);
    s.settings.time_scheme = bdf2();
    let (u0, p0, phi0, t0) = (s.u.values.clone(), s.p.values.clone(), s.phi.clone(), s.time);
    let pts: Vec<Vec3> = mesh0
        .points()
        .iter()
        .map(|&p| vec3::add(p, vec3::scale(V_CAV, DT_CAV)))
        .collect();
    let st = move_mesh(&s.geom, pts, DT_CAV).unwrap();
    let r = s.step_ale_with_walls(DT_CAV, st.geom.clone(), st.mesh_flux.clone(), Vec::new());
    let unchanged = vec_bits_equal(&s.u.values, &u0)
        && bits_equal(&s.p.values, &p0)
        && bits_equal(&s.phi, &phi0)
        && s.time.to_bits() == t0.to_bits();
    c.holds(
        "E1",
        matches!(&r, Err(m) if m == "a moving mesh supports the implicit Euler time scheme only") && unchanged,
        &format!("BDF2 without the opt-in refused ({r:?}), state unchanged {unchanged}"),
    );
    // E2
    for scheme in [TimeScheme::Bdf2Bounded, TimeScheme::CrankNicolson] {
        let (mut s, _) = cavity(8);
        let r = s.set_moving_mesh_time_scheme(scheme);
        let kept = s.settings.time_scheme == TimeScheme::ImplicitEuler;
        s.settings.time_scheme = bdf2();
        let still = s
            .step_ale_with_walls(DT_CAV, st.geom.clone(), st.mesh_flux.clone(), Vec::new())
            .is_err();
        c.holds(
            "E2",
            r.is_err() && kept && still,
            &format!(
                "{scheme:?} refused {}, settings unchanged {kept}, no opt-in set {still}",
                r.is_err()
            ),
        );
    }
    // E3
    let (mut s, _) = cavity(8);
    s.set_moving_mesh_time_scheme(bdf2()).unwrap();
    let g = s.geom.clone();
    let r1 = s.replace_mesh(g.clone(), s.u.values.clone(), s.p.values.clone(), s.phi.clone());
    let step = SlidingStep {
        geom: g.clone(),
        old_angle: 0.0,
        angle: 0.0,
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        mesh_flux: vec![0.0; g.mesh.n_faces()],
        vol_old: g.mesh.cell_volumes().to_vec(),
        phi_old: s.phi.clone(),
        old_defect: vec![0.0; g.n_internal],
        interface_faces: Vec::new(),
        gcl_residual: 0.0,
        flux_residual: 0.0,
    };
    let r2 = s.step_sliding(DT_CAV, step, Vec::new());
    c.holds(
        "E3",
        r1.is_err() && r2.is_err(),
        &format!(
            "replace_mesh refused {}, step_sliding refused {}",
            r1.is_err(),
            r2.is_err()
        ),
    );
    // E4
    let (mut u, ms0, ms1, old0, old1) = {
        let mesh0 = ogrid();
        let geom = Arc::new(FvGeometry::new(Arc::new(mesh0.clone())).unwrap());
        let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
        for name in ["cylinder", "farfield"] {
            u.set_bc(&geom, name, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
        }
        let ms0 = move_mesh(&geom, shifted(&mesh0, [0.01, 0.0, 0.0]), 0.01).unwrap();
        let ms1 = move_mesh(&ms0.geom, shifted(&mesh0, [0.03, 0.0, 0.0]), 0.01).unwrap();
        (u, ms0.clone(), ms1, geom.mesh.clone(), ms0.geom.mesh.clone())
    };
    let mut h = FollowHistory::new();
    set_follow_conditions_bdf2(&mut u, &ms0, &old0, &["cylinder".to_string()], 0.01, &mut h).unwrap();
    let before = u.boundary_values(&ms1.geom);
    let r = set_follow_conditions_bdf2(
        &mut u,
        &ms1,
        &old1,
        &["cylinder".to_string(), "farfield".to_string()],
        0.01,
        &mut h,
    );
    let after = u.boundary_values(&ms1.geom);
    c.holds(
        "E4",
        r.is_err() && vec_bits_equal(&before, &after) && !h.is_empty(),
        &format!("another patch list refused {}, conditions unchanged", r.is_err()),
    );
    c.finish();
}

#[test]
fn e5_e7_the_moving_history() {
    let mut c = Checks::new();
    // E5: a step at rest drops the history
    let run5 = |toggle: bool| {
        let (mut s, mesh0) = cavity(8);
        s.set_moving_mesh_time_scheme(bdf2()).unwrap();
        cav_move(&mut s, &mesh0, DT_CAV);
        cav_move(&mut s, &mesh0, 2.0 * DT_CAV);
        s.step(DT_CAV);
        if toggle {
            s.set_moving_mesh_time_scheme(TimeScheme::ImplicitEuler).unwrap();
            s.set_moving_mesh_time_scheme(bdf2()).unwrap();
        }
        cav_move(&mut s, &mesh0, 3.0 * DT_CAV);
        s
    };
    let (a, b) = (run5(false), run5(true));
    c.holds("E5", same_state(&a, &b), "equal bit for bit");
    // E6: opt-in and back is the default
    let (mut a, mesh0) = cavity(8);
    a.set_moving_mesh_time_scheme(bdf2()).unwrap();
    a.set_moving_mesh_time_scheme(TimeScheme::ImplicitEuler).unwrap();
    let (mut b, _) = cavity(8);
    for n in 1..=5 {
        cav_move(&mut a, &mesh0, n as f64 * DT_CAV);
        cav_move(&mut b, &mesh0, n as f64 * DT_CAV);
    }
    c.holds("E6", same_state(&a, &b), "equal bit for bit");
    // E7: the first moving step of a history is the default implicit Euler moving step
    let (mut a, mesh0) = cavity(8);
    a.settings.time_scheme = bdf2();
    a.set_moving_mesh_time_scheme(bdf2()).unwrap();
    a.step(DT_CAV);
    cav_move(&mut a, &mesh0, DT_CAV);
    let (mut b, _) = cavity(8);
    b.settings.time_scheme = bdf2();
    b.step(DT_CAV);
    b.settings.time_scheme = TimeScheme::ImplicitEuler;
    cav_move(&mut b, &mesh0, DT_CAV);
    c.holds("E7", same_state(&a, &b), "equal bit for bit");
    c.finish();
}

#[test]
fn qr_library_bdf2_moving_step_at_rest_equals_the_bdf2_step() {
    let (mut a, _) = cavity(16);
    a.settings.time_scheme = bdf2();
    let (mut b, _) = cavity(16);
    b.set_moving_mesh_time_scheme(bdf2()).unwrap();
    for _ in 0..20 {
        a.step(DT_CAV);
        let pts = b.geom.mesh.points().to_vec();
        let st = move_mesh(&b.geom, pts, DT_CAV).unwrap();
        b.step_ale_with_walls(DT_CAV, st.geom, st.mesh_flux, Vec::new())
            .unwrap();
    }
    let phi_max = a.phi.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    println!("QR: bit for bit {}", same_state(&a, &b));
    let mut c = Checks::new();
    c.at_most("QR u", max_vec_diff(&a.u.values, &b.u.values), 1e-12);
    c.at_most("QR p", max_diff(&a.p.values, &b.p.values), 1e-12);
    c.at_most("QR phi", max_diff(&a.phi, &b.phi) / phi_max, 1e-12);
    c.finish();
}

// ---- BF and TR of ADR 0219 --------------------------------------------------------------------------------------------

const NU: f64 = 0.025;
const DT: f64 = 0.02;
const DEPTH: f64 = 0.1;
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

fn displacement(t: f64) -> f64 {
    if t <= 1.0 {
        -(0.5 * t - (PI * t).sin() / (2.0 * PI))
    } else {
        -(0.5 + (t - 1.0))
    }
}

fn radial(n: usize) -> (f64, Grading) {
    let q: f64 = 1.08;
    let len = 0.01 * (q.powi(n as i32) - 1.0) / (q - 1.0);
    (0.5 + len, Grading::Expansion(q.powi(n as i32 - 1)))
}

fn bf_mesh() -> Mesh {
    let (r_out, g) = radial(66);
    cylinder_ogrid(96, 66, 0.5, r_out, g, DEPTH).unwrap()
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

fn uniform(x0: f64, len: f64, n: usize) -> Vec<f64> {
    (0..=n).map(|i| x0 + len * i as f64 / n as f64).collect()
}

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

fn geometry_at(ov: &Overset, pose: &Pose) -> Arc<FvGeometry> {
    let mesh = moved_mesh(&ov.composite.mesh, ov.composite.points_at(pose)).unwrap();
    Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap())
}

/// TR at `t = 0` with the settings `set`.
fn tr_new(set: PimpleSettings) -> OversetSolver {
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
        set,
        MassMode::Conservative,
        Some(OversetCoupling { overset: ov, pose }),
    )
    .unwrap()
}

/// BF at `t = 0` with the settings `set`.
fn bf_new(set: PimpleSettings) -> (PimpleSolver, Mesh) {
    let mesh0 = bf_mesh();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh0.clone())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for name in ["cylinder", "farfield"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    (PimpleSolver::new(geom, u, p, set), mesh0)
}

fn bf_points_at(mesh0: &Mesh, t: f64) -> Vec<Vec3> {
    let d = displacement(t);
    mesh0.points().iter().map(|q| [q[0] + d, q[1], q[2]]).collect()
}

fn walls(name: &str) -> Vec<String> {
    vec![name.to_string()]
}

/// The library and the overset solver on BF for 20 steps; `n_outer` outer iterations, `rc` the step-consistent face
/// coefficient; the overset solver with BDF2 when `overset_bdf2` (three-level walls) and implicit Euler otherwise
/// (two-level walls).
fn qb_pair(n_outer: usize, rc: bool, overset_bdf2: bool) -> (PimpleSolver, OversetSolver) {
    let mut set = settings(NU);
    set.rhie_chow_step_consistent = rc;
    let mut lib_set = set.clone();
    lib_set.n_outer = n_outer;
    let (mut a, mesh0) = bf_new(lib_set);
    a.set_moving_mesh_time_scheme(bdf2()).unwrap();
    let mut b = OversetSolver::new(
        a.geom.clone(),
        a.u.clone(),
        a.p.clone(),
        set,
        MassMode::Conservative,
        None,
    )
    .unwrap();
    if n_outer != 1 {
        b.set_outer_iterations(n_outer, 1.0, 1.0).unwrap();
    }
    if overset_bdf2 {
        b.set_time_scheme(bdf2()).unwrap();
    }
    let w = walls("cylinder");
    let (mut ha, mut hb) = (FollowHistory::new(), FollowHistory::new());
    for n in 1..=20 {
        let pts = bf_points_at(&mesh0, n as f64 * DT);
        let st = move_mesh(&a.geom, pts.clone(), DT).unwrap();
        let old = a.geom.mesh.clone();
        let dp = set_follow_conditions_bdf2(&mut a.u, &st, &old, &w, DT, &mut ha).unwrap();
        a.step_ale_with_walls(DT, st.geom, st.mesh_flux, dp).unwrap();
        let st = move_mesh(&b.geom, pts, DT).unwrap();
        let old = b.geom.mesh.clone();
        let dp = if overset_bdf2 {
            set_follow_conditions_bdf2(&mut b.u, &st, &old, &w, DT, &mut hb).unwrap()
        } else {
            set_follow_conditions(&mut b.u, &st, &old, &w, DT).unwrap()
        };
        b.advance_mesh(DT, st, dp).unwrap();
    }
    (a, b)
}

fn qb_check(c: &mut Checks, id: &str, a: &PimpleSolver, b: &OversetSolver) {
    let du = max_vec_diff(&a.u.values, &b.u.values);
    let dp = max_diff(&a.p.values, &b.p.values);
    let phi_max = a.phi.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    let dphi = max_diff(&a.phi, &b.phi) / phi_max;
    let bits =
        vec_bits_equal(&a.u.values, &b.u.values) && bits_equal(&a.p.values, &b.p.values) && bits_equal(&a.phi, &b.phi);
    println!("{id}: bit for bit {bits}");
    c.at_most(&format!("{id} u"), du, 1e-12);
    c.at_most(&format!("{id} p"), dp, 1e-12);
    c.at_most(&format!("{id} phi"), dphi, 1e-12);
}

#[test]
fn qb_overset_bdf2_with_every_cell_calculated_equals_the_library_bdf2_moving_step_on_bf() {
    let mut c = Checks::new();
    let (a1, b1) = qb_pair(1, false, true);
    qb_check(&mut c, "QB1", &a1, &b1);
    let (a, b) = qb_pair(3, false, true);
    qb_check(&mut c, "QB3", &a, &b);
    let (a, b) = qb_pair(3, true, true);
    qb_check(&mut c, "QB3c", &a, &b);
    let (_, e) = qb_pair(1, false, false);
    c.at_least("QBn u", max_vec_diff(&a1.u.values, &e.u.values), 1e-8);
    c.finish();
}

// ---- F: the three-level follow conditions ------------------------------------------------------------------------------

fn ogrid() -> Mesh {
    cylinder_ogrid(24, 6, 0.5, 1.5, Grading::Uniform, 0.1).unwrap()
}

fn shifted(m: &Mesh, d: Vec3) -> Vec<Vec3> {
    m.points().iter().map(|&p| vec3::add(p, d)).collect()
}

/// One step of a follow run: the time, the wall values, `dp/dn`, the unit normals and area vectors of the wall faces, and
/// the three-level mesh flux of the wall faces (`NaN` on the first step).
struct FollowStep {
    t: f64,
    ub: Vec<Vec3>,
    dpdn: Vec<f64>,
    normal: Vec<Vec3>,
    s: Vec<Vec3>,
    m3: Vec<f64>,
}

fn follow_run(points: &dyn Fn(&Mesh, f64) -> Vec<Vec3>, dts: &[f64]) -> Vec<FollowStep> {
    let mesh0 = ogrid();
    let mut geom = Arc::new(FvGeometry::new(Arc::new(mesh0.clone())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for name in ["cylinder", "farfield"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let w = walls("cylinder");
    let mut h = FollowHistory::new();
    let mut out = Vec::new();
    let mut t = 0.0;
    let mut last: Option<(Vec<f64>, f64)> = None;
    for &dt in dts {
        t += dt;
        let st = move_mesh(&geom, points(&mesh0, t), dt).unwrap();
        let old = geom.mesh.clone();
        let dp = set_follow_conditions_bdf2(&mut u, &st, &old, &w, dt, &mut h).unwrap();
        let ni = st.geom.n_internal;
        let faces: Vec<usize> = st.geom.mesh.patch("cylinder").unwrap().faces().collect();
        let bv = u.boundary_values(&st.geom);
        let sf = st.geom.mesh.face_area_vectors();
        let m3: Vec<f64> = match &last {
            None => vec![f64::NAN; faces.len()],
            Some((m0, dt0)) => {
                let r = dt / dt0;
                let (c0, c2) = ((1.0 + 2.0 * r) / (1.0 + r), r * r / (1.0 + r));
                faces.iter().map(|&f| c0 * st.mesh_flux[f] - c2 / r * m0[f]).collect()
            }
        };
        out.push(FollowStep {
            t,
            ub: faces.iter().map(|&f| bv[f - ni]).collect(),
            dpdn: faces.iter().map(|&f| dp[f - ni]).collect(),
            normal: faces
                .iter()
                .map(|&f| vec3::scale(sf[f], 1.0 / vec3::norm(sf[f])))
                .collect(),
            s: faces.iter().map(|&f| sf[f]).collect(),
            m3,
        });
        last = Some((st.mesh_flux.clone(), dt));
        geom = st.geom;
    }
    out
}

#[test]
fn f1_f4_three_level_wall_velocity_and_acceleration() {
    let mut c = Checks::new();
    // F1: constant acceleration, exact from the fourth step on
    let x1 = |m: &Mesh, t: f64| shifted(m, [0.3 * t + 0.8 * t * t, -0.2 * t + 0.5 * t * t, 0.0]);
    let v1 = |t: f64| [0.3 + 1.6 * t, -0.2 + 1.0 * t, 0.0];
    let a1: Vec3 = [1.6, 1.0, 0.0];
    let alternating: Vec<f64> = (0..8).map(|i| if i % 2 == 0 { 0.01 } else { 0.005 }).collect();
    for (label, dts) in [("uniform", vec![0.01; 8]), ("alternating", alternating)] {
        let run = follow_run(&x1, &dts);
        let (mut ev, mut ea) = (0.0f64, 0.0f64);
        for st in &run[3..] {
            let v = v1(st.t);
            for k in 0..st.ub.len() {
                ev = ev.max(vec3::norm(vec3::sub(st.ub[k], v)) / vec3::norm(v));
                ea = ea.max((st.dpdn[k] + vec3::dot(st.normal[k], a1)).abs() / vec3::norm(a1));
            }
        }
        c.at_most(&format!("F1 {label} velocity"), ev, 1e-11);
        c.at_most(&format!("F1 {label} acceleration"), ea, 1e-8);
    }
    // F2: x = t^3, the velocity error -2 h^2, the acceleration exact
    let x2 = |m: &Mesh, t: f64| shifted(m, [t * t * t, 0.0, 0.0]);
    let t_end = 0.4;
    let mut errs = Vec::new();
    for (h, n) in [(0.02, 20usize), (0.01, 40)] {
        let run = follow_run(&x2, &vec![h; n]);
        let st = run.last().unwrap();
        println!("F2 h {h}: t {:.6}", st.t);
        let v: Vec3 = [3.0 * t_end * t_end, 0.0, 0.0];
        let a: Vec3 = [6.0 * t_end, 0.0, 0.0];
        let err = st.ub.iter().map(|u| vec3::norm(vec3::sub(*u, v))).fold(0.0, f64::max);
        let ea = (0..st.ub.len())
            .map(|k| (st.dpdn[k] + vec3::dot(st.normal[k], a)).abs())
            .fold(0.0, f64::max)
            / vec3::norm(a);
        c.at_most(&format!("F2e h {h}"), (err / (2.0 * h * h) - 1.0).abs(), 1e-6);
        c.at_most(&format!("F2a h {h}"), ea, 1e-8);
        errs.push(err);
    }
    c.at_least("F2v ratio", errs[0] / errs[1], 3.9);
    // F3: rotation and translation, no relative flux through the wall
    let x3 = |m: &Mesh, t: f64| {
        let pose = Pose {
            origin: [0.0; 3],
            axis: [0.0, 0.0, 1.0],
            angle: 0.5 * t + 2.0 * t * t,
            translation: [0.4 * t + t * t, 0.3 * t, 0.0],
        };
        m.points().iter().map(|&p| pose.apply(p)).collect()
    };
    let run = follow_run(&x3, &[0.01; 8]);
    let mut worst = 0.0f64;
    for st in &run[1..] {
        let mx = st.m3.iter().fold(0.0f64, |a, b| a.max(b.abs()));
        for k in 0..st.ub.len() {
            worst = worst.max((vec3::dot(st.ub[k], st.s[k]) - st.m3[k]).abs() / mx);
        }
    }
    c.at_most("F3 relative wall flux", worst, 1e-12);
    // F4: the first call is set_follow_conditions
    let mesh0 = ogrid();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh0.clone())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for name in ["cylinder", "farfield"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let st = move_mesh(&geom, x3(&mesh0, 0.01), 0.01).unwrap();
    let mut u2 = u.clone();
    let w = walls("cylinder");
    let d1 = set_follow_conditions_bdf2(&mut u, &st, &geom.mesh, &w, 0.01, &mut FollowHistory::new()).unwrap();
    let d2 = set_follow_conditions(&mut u2, &st, &geom.mesh, &w, 0.01).unwrap();
    c.holds(
        "F4",
        bits_equal(&d1, &d2) && vec_bits_equal(&u.boundary_values(&st.geom), &u2.boundary_values(&st.geom)),
        "the first call equals set_follow_conditions bit for bit",
    );
    c.finish();
}

/// F5 / F5d: TR, 4 steps by `step`, the walls installed by `prepare` against the test's own call.
fn f5_run(bdf: bool) -> bool {
    let mut s = tr_new(settings(NU));
    if bdf {
        s.set_time_scheme(bdf2()).unwrap();
    }
    let ov = tr_overset();
    let w = walls("c_cylinder");
    let mut h = FollowHistory::new();
    let mut all = true;
    for n in 1..=4 {
        let pose = tr_pose(n as f64 * DT);
        let mut u_copy = s.u.clone();
        let ms = move_mesh(&s.geom, ov.composite.points_at(&pose), DT).unwrap();
        let old = s.geom.mesh.clone();
        let dp = if bdf {
            set_follow_conditions_bdf2(&mut u_copy, &ms, &old, &w, DT, &mut h).unwrap()
        } else {
            set_follow_conditions(&mut u_copy, &ms, &old, &w, DT).unwrap()
        };
        let st = s.prepare(DT, pose, &w).unwrap();
        let ok = bits_equal(&st.wall_dpdn, &dp)
            && vec_bits_equal(&s.u.boundary_values(&st.geom), &u_copy.boundary_values(&ms.geom));
        println!("F5 bdf2 {bdf} step {n}: equal {ok}");
        all &= ok;
        s.advance(DT, st).unwrap();
    }
    all
}

#[test]
fn f5_overset_prepare_installs_the_three_level_walls_with_bdf2() {
    let mut c = Checks::new();
    c.holds(
        "F5",
        f5_run(true),
        "prepare with BDF2: the three-level walls, bit for bit",
    );
    c.holds(
        "F5d",
        f5_run(false),
        "prepare without BDF2: the two-level walls, bit for bit",
    );
    c.finish();
}

// ---- A: the time error of BF and TR (heavy records) ---------------------------------------------------------------------

const T_A: f64 = 1.0;
const DTS_A: [f64; 3] = [0.02, 0.01, 0.005];

fn order_settings(rc: bool) -> PimpleSettings {
    let mut s = settings(NU);
    s.rhie_chow_step_consistent = rc;
    s
}

/// BF to `T_A` with step `dt`: the velocity at `T_A`, the largest continuity, `F_x` of the cylinder at `T_A`.
fn bf_order_run(bdf: bool, three_level: bool, rc: bool, dt: f64) -> (Vec<Vec3>, f64, f64) {
    let mut set = order_settings(rc);
    set.n_outer = 3;
    let (mut s, mesh0) = bf_new(set);
    if bdf {
        s.set_moving_mesh_time_scheme(bdf2()).unwrap();
    }
    let w = walls("cylinder");
    let mut h = FollowHistory::new();
    let n = (T_A / dt).round() as usize;
    let mut cont = 0.0f64;
    for k in 1..=n {
        let st = move_mesh(&s.geom, bf_points_at(&mesh0, k as f64 * dt), dt).unwrap();
        let old = s.geom.mesh.clone();
        let dp = if three_level {
            set_follow_conditions_bdf2(&mut s.u, &st, &old, &w, dt, &mut h).unwrap()
        } else {
            set_follow_conditions(&mut s.u, &st, &old, &w, dt).unwrap()
        };
        let info = s.step_ale_with_walls(dt, st.geom, st.mesh_flux, dp).unwrap();
        cont = cont.max(info.continuity);
    }
    let centre = [displacement(T_A), 0.0, 0.0];
    let fx = patch_force_moment(&s.geom, &s.u, &s.p, 1.0, NU, &w, centre).unwrap().0[0];
    (s.u.values.clone(), cont, fx)
}

/// The ratio `d(0.02) / d(0.01)` of three runs and the two differences.
fn ratio(runs: &[Vec<Vec3>], use_cell: &dyn Fn(usize) -> bool) -> (f64, f64, f64) {
    let d = |a: &[Vec3], b: &[Vec3]| {
        (0..a.len())
            .filter(|&c| use_cell(c))
            .map(|c| vec3::norm(vec3::sub(a[c], b[c])))
            .fold(0.0, f64::max)
    };
    let (d0, d1) = (d(&runs[0], &runs[2]), d(&runs[1], &runs[2]));
    (d0 / d1, d0, d1)
}

#[test]
#[ignore = "heavy record (about 4 minutes): ADR 0379 A-BF, A-BFn, A-c of BF"]
fn a_bf_time_error_ratio_with_bdf2_and_three_level_walls() {
    let mut c = Checks::new();
    let variants = [
        ("A-BF BDF2, three-level walls, rc", true, true, true),
        ("A-BFn implicit Euler, two-level walls, rc", false, false, true),
        ("(printed) BDF2, two-level walls, rc", true, false, true),
        ("(printed) BDF2, three-level walls, no rc", true, true, false),
    ];
    for (i, (label, bdf, tl, rc)) in variants.iter().enumerate() {
        let t0 = Instant::now();
        let mut runs = Vec::new();
        let mut cont = 0.0f64;
        for dt in DTS_A {
            let (u, ct, fx) = bf_order_run(*bdf, *tl, *rc, dt);
            println!("{label}: dt {dt}: F_x(T) {fx:.10e}, continuity {ct:.3e}");
            cont = cont.max(ct);
            runs.push(u);
        }
        let (r, d0, d1) = ratio(&runs, &|_| true);
        println!(
            "{label}: d(0.02) {d0:.4e}, d(0.01) {d1:.4e}, R {r:.4}, {:.1} s",
            t0.elapsed().as_secs_f64()
        );
        match i {
            0 => {
                c.at_least("A-BF R", r, 4.0);
                c.at_most("A-c BF continuity", cont, 1e-9);
            }
            1 => c.within("A-BFn R", r, 2.5, 3.5),
            2 => c.at_most("A-c BF continuity (two-level walls)", cont, 1e-9),
            _ => c.at_most("A-c BF continuity (no rc)", cont, 1e-9),
        }
    }
    c.finish();
}

#[test]
#[ignore = "heavy record (about 6 to 15 minutes): ADR 0379 A-TR, A-c of TR"]
fn a_tr_time_error_ratio_with_bdf2_and_three_level_walls() {
    let mut c = Checks::new();
    let w = walls("c_cylinder");
    let mut runs = Vec::new();
    let mut kinds: Vec<CellKind> = Vec::new();
    let mut nb = 0;
    let mut centres: Vec<Vec3> = Vec::new();
    let mut cont = 0.0f64;
    for dt in DTS_A {
        let t0 = Instant::now();
        let mut s = tr_new(order_settings(true));
        s.set_outer_iterations(3, 1.0, 1.0).unwrap();
        s.set_time_scheme(bdf2()).unwrap();
        let n = (T_A / dt).round() as usize;
        let mut fallback = 0usize;
        for k in 1..=n {
            let info = s.step(dt, tr_pose(k as f64 * dt), &w).unwrap();
            cont = cont.max(info.continuity);
            fallback += s.euler_fallback_cells();
        }
        let centre = [2.0 + displacement(T_A), 0.0, 0.0];
        let fx = patch_force_moment(&s.geom, &s.u, &s.p, 1.0, NU, &w, centre).unwrap().0[0];
        println!(
            "TR dt {dt}: F_x(T) {fx:.10e}, Euler-row cells {fallback}, {:.1} s",
            t0.elapsed().as_secs_f64()
        );
        kinds = s.conn.kind.clone();
        nb = s.conn.n_background_cells;
        centres = s.geom.mesh.cell_centres().to_vec();
        runs.push(s.u.values.clone());
    }
    let non_hole = |c: usize| kinds[c] != CellKind::Hole;
    let (r, d0, d1) = ratio(&runs, &non_hole);
    println!("A-TR: d(0.02) {d0:.4e}, d(0.01) {d1:.4e}");
    let (rc_, c0, c1) = ratio(&runs, &|c| non_hole(c) && c >= nb);
    println!("A-TR component cells: d(0.02) {c0:.4e}, d(0.01) {c1:.4e}, R {rc_:.4}");
    let (rb, b0, b1) = ratio(&runs, &|c| non_hole(c) && c < nb);
    println!("A-TR background cells: d(0.02) {b0:.4e}, d(0.01) {b1:.4e}, R {rb:.4}");
    let centre = [2.0 + displacement(T_A), 0.0, 0.0];
    let worst = (0..runs[0].len())
        .filter(|&c| non_hole(c))
        .max_by(|&a, &b| {
            let da = vec3::norm(vec3::sub(runs[0][a], runs[2][a]));
            let db = vec3::norm(vec3::sub(runs[0][b], runs[2][b]));
            da.total_cmp(&db)
        })
        .unwrap();
    println!(
        "A-TR largest d(0.02) in cell {worst} ({}), distance from the cylinder centre {:.4}",
        if worst >= nb { "component" } else { "background" },
        vec3::norm(vec3::sub(centres[worst], centre))
    );
    c.at_least("A-TR R", r, 4.0);
    c.at_most("A-c TR continuity", cont, 1e-9);
    c.finish();
}
