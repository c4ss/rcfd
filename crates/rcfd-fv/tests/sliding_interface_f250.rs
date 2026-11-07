//! F-250 (ADR 0216): a sliding (rotating) non-conformal interface on a cylinder in the incompressible PISO path. The
//! rotor block turns rigidly against the stator block and the interface is rebuilt every step
//! (`rcfd_mesh::sliding`, `rcfd_fv::sliding`, `PimpleSolver::step_sliding`). Checks F (the interface solve), S (flux
//! conservation across the interface, acceptance A1), U (uniform flow on a rotating interface), R (rigid rotation against
//! conforming meshes, acceptance A2), T (Taylor-Couette torque, acceptance A3), D (the decaying azimuthal mode, a
//! closed-form transient check), E (refusals) and X0 (set-up). The limits are those registered in ADR 0216 before this code
//! ran; each test prints every measured number and then asserts the registered checks it carries.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::{move_mesh, set_follow_conditions};
use rcfd_fv::bc::{FixedValue, ZeroGradient};
use rcfd_fv::convection::{face_flux, ConvScheme};
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver, StepInfo};
use rcfd_fv::sliding::{interface_balance, rotating_wall_conditions, sliding_step, SlidingStep};
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{annulus_3d, cylinder_ogrid, Grading};
use rcfd_mesh::motion::{mat_vec, rotation_matrix, Pose};
use rcfd_mesh::non_conformal::interface_non_orthogonality;
use rcfd_mesh::sliding::{SlidingInterface, SlidingMesh};
use rcfd_mesh::Mesh;
use std::collections::HashMap;
use std::f64::consts::{PI, TAU};
use std::sync::Arc;

const L: f64 = 0.1;
const RI: f64 = 1.0;
const RM: f64 = 1.5;
const RO: f64 = 2.0;
const Z: Vec3 = [0.0, 0.0, 1.0];
const TIGHT: SolveControl = SolveControl {
    tol_abs: 1e-16,
    tol_rel: 1e-13,
    max_iter: 5000,
};

/// PS of ADR 0216.
fn settings(nu: f64) -> PimpleSettings {
    let mut s = PimpleSettings::piso(nu);
    s.convection = ConvScheme::Central;
    s.non_orth_correctors = 2;
    s.momentum_solver = TIGHT;
    s.pressure_solver = TIGHT;
    s
}

/// P (`n_stator = 120`) or P= (`n_stator = 144`).
fn pair(n_stator: usize) -> SlidingInterface {
    let a = cylinder_ogrid(144, 8, RI, RM, Grading::Uniform, L).unwrap();
    let b = cylinder_ogrid(n_stator, 8, RM, RO, Grading::Uniform, L).unwrap();
    SlidingInterface::cylindrical(a, "farfield", b, "cylinder", vec3::ZERO, Z, 1e-8).unwrap()
}

/// P3.
fn pair3() -> SlidingInterface {
    let a = annulus_3d(48, 4, 2, RI, RM, 0.4, 0.0, Grading::Uniform).unwrap();
    let b = annulus_3d(40, 4, 3, RM, RO, 0.4, 0.0, Grading::Uniform).unwrap();
    SlidingInterface::cylindrical(a, "outer", b, "inner", vec3::ZERO, Z, 1e-8).unwrap()
}

fn geom_of(m: &Mesh) -> Arc<FvGeometry> {
    Arc::new(FvGeometry::new(Arc::new(m.clone())).unwrap())
}

fn xhat(x: Vec3) -> Vec3 {
    vec3::unit([x[0], x[1], 0.0])
}

fn rigid(omega: f64, x: Vec3) -> Vec3 {
    vec3::scale(vec3::cross(Z, x), omega)
}

fn per_face(g: &FvGeometry, patch: &str, f: impl Fn(Vec3) -> Vec3) -> Box<FixedValue> {
    let cf = g.mesh.face_centres();
    Box::new(FixedValue(Profile::PerFace(
        g.mesh.patch(patch).unwrap().faces().map(|k| f(cf[k])).collect(),
    )))
}

/// GCL residual of a step's data, computed here from its volumes and mesh flux: `max_c abs(V1 - V0 - dt sum sigma m) /
/// mean(V1)`.
fn gcl_of(st: &SlidingStep, dt: f64) -> f64 {
    let m = &st.geom.mesh;
    let ni = m.n_internal_faces();
    let mut s = vec![0.0; m.n_cells()];
    for f in 0..m.n_faces() {
        s[m.owner()[f]] += st.mesh_flux[f] * dt;
        if f < ni {
            s[m.neighbour()[f]] -= st.mesh_flux[f] * dt;
        }
    }
    let v1 = m.cell_volumes();
    let mean = v1.iter().sum::<f64>() / v1.len() as f64;
    (0..v1.len()).fold(0.0f64, |a, c| a.max((v1[c] - st.vol_old[c] - s[c]).abs())) / mean
}

/// Continuity of the remapped old flux: `max_c abs(sum sigma phi_old) / max abs(S)`.
fn remap_of(st: &SlidingStep) -> f64 {
    let m = &st.geom.mesh;
    let ni = m.n_internal_faces();
    let mut d = vec![0.0; m.n_cells()];
    for f in 0..m.n_faces() {
        d[m.owner()[f]] += st.phi_old[f];
        if f < ni {
            d[m.neighbour()[f]] -= st.phi_old[f];
        }
    }
    let smax = m.face_area_vectors().iter().map(|s| vec3::norm(*s)).fold(0.0, f64::max);
    d.iter().fold(0.0f64, |a, x| a.max(x.abs())) / smax
}

fn pairs_of(m: &SlidingMesh) -> Vec<(usize, usize)> {
    let mut v: Vec<(usize, usize)> = m
        .report
        .interface_faces
        .iter()
        .map(|&f| (m.mesh.owner()[f], m.mesh.neighbour()[f]))
        .collect();
    v.sort_unstable();
    v
}

#[derive(Debug, Default, Clone, Copy)]
struct Stats {
    cont: f64,
    gcl: f64,
    remap: f64,
    non_orth: f64,
    steps: usize,
    changed: usize,
}

/// The loop SL of ADR 0216.
struct Sliding<'a> {
    sc: &'a SlidingInterface,
    cur: SlidingMesh,
    s: PimpleSolver,
    omega: f64,
    dt: f64,
    stats: Stats,
}

type BcHook<'b> = &'b mut dyn FnMut(&mut PimpleSolver, &SlidingStep) -> Vec<f64>;
type AlterHook<'b> = &'b mut dyn FnMut(&mut SlidingStep, &PimpleSolver);

impl<'a> Sliding<'a> {
    fn new(sc: &'a SlidingInterface, make: impl FnOnce(Arc<FvGeometry>) -> PimpleSolver, omega: f64, dt: f64) -> Self {
        let cur = sc.mesh_at(0.0).unwrap();
        let s = make(geom_of(&cur.mesh));
        Sliding {
            sc,
            cur,
            s,
            omega,
            dt,
            stats: Stats::default(),
        }
    }

    fn angle(&self) -> f64 {
        self.cur.angle
    }

    fn step(&mut self, bc: BcHook, alter: AlterHook) -> StepInfo {
        let theta = self.omega * (self.stats.steps + 1) as f64 * self.dt;
        let new = self.sc.mesh_at(theta).unwrap();
        let mut st = sliding_step(&self.s.geom, &self.cur, &new, &self.s.phi, &self.s.u.values, self.dt).unwrap();
        alter(&mut st, &self.s);
        let no = interface_non_orthogonality(&new.mesh, &st.interface_faces, 70.0).unwrap();
        let s = &mut self.stats;
        s.gcl = s.gcl.max(gcl_of(&st, self.dt));
        s.remap = s.remap.max(remap_of(&st));
        s.non_orth = s.non_orth.max(no.worst_deg);
        if pairs_of(&new) != pairs_of(&self.cur) {
            s.changed += 1;
        }
        let dpdn = bc(&mut self.s, &st);
        let info = self.s.step_sliding(self.dt, st, dpdn).unwrap();
        assert!(
            self.s.u.values.iter().flatten().all(|x| x.is_finite()),
            "non-finite velocity at step {}",
            self.stats.steps + 1
        );
        self.stats.cont = self.stats.cont.max(info.continuity);
        self.stats.steps += 1;
        self.cur = new;
        info
    }

    /// Net flux through the interface faces of the current mesh, and their area.
    fn interface_flux(&self) -> (f64, f64) {
        let m = &self.s.geom.mesh;
        let faces = &self.cur.report.interface_faces;
        (
            faces.iter().map(|&f| self.s.phi[f]).sum(),
            faces.iter().map(|&f| m.face_area(f)).sum(),
        )
    }
}

fn no_bc() -> impl FnMut(&mut PimpleSolver, &SlidingStep) -> Vec<f64> {
    |_: &mut PimpleSolver, _: &SlidingStep| Vec::new()
}

fn no_alter() -> impl FnMut(&mut SlidingStep, &PimpleSolver) {
    |_: &mut SlidingStep, _: &PimpleSolver| {}
}

/// The rotor wall `cylinder` turning at `omega_w` (M6), its gradient returned.
fn rotor_wall(omega_w: f64, dt: f64) -> impl FnMut(&mut PimpleSolver, &SlidingStep) -> Vec<f64> {
    move |s: &mut PimpleSolver, st: &SlidingStep| {
        let g = s.geom.clone();
        rotating_wall_conditions(&mut s.u, st, &g, &["cylinder".to_string()], omega_w, dt).unwrap()
    }
}

type Fails = Vec<(&'static str, String)>;

fn check(fails: &mut Fails, id: &'static str, ok: bool, what: String) {
    println!("{id}: {what} -> {}", if ok { "met" } else { "MISSED" });
    if !ok {
        fails.push((id, what));
    }
}

fn require(fails: &Fails, ids: &[&str]) {
    let bad: Vec<_> = fails.iter().filter(|x| ids.contains(&x.0)).collect();
    assert!(bad.is_empty(), "{bad:?}");
}

fn x0(fails: &mut Fails, name: &str, st: &Stats, angle: f64, pitch: f64) {
    check(
        fails,
        "X0",
        st.non_orth <= 70.0 && angle >= pitch,
        format!(
            "{name}: worst interface non-orthogonality {:.2} deg (70), rotor angle {angle:.4} rad against the stator pitch {pitch:.4}",
            st.non_orth
        ),
    );
}

// -------------------------------------------------------------------------------------------------------------------
// F. The interface solve
// -------------------------------------------------------------------------------------------------------------------

/// F1 and F1n of ADR 0216.
#[test]
fn f1_interface_balance_meets_pseudo_random_targets() {
    let sm = pair(120).mesh_at(0.0123).unwrap();
    let m = &sm.mesh;
    let faces = &sm.report.interface_faces;
    let mut seed: u64 = 0x2160_0001;
    let mut lcg = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    };
    let mut on = vec![false; m.n_cells()];
    for &f in faces {
        on[m.owner()[f]] = true;
        on[m.neighbour()[f]] = true;
    }
    let mut t = vec![0.0; m.n_cells()];
    for c in 0..m.n_cells() {
        if on[c] {
            t[c] = lcg();
        }
    }
    let sol = interface_balance(m, faces, &t).unwrap();
    let mut d = vec![0.0; m.n_cells()];
    for (k, &f) in faces.iter().enumerate() {
        d[m.owner()[f]] += sol.values[k];
        d[m.neighbour()[f]] -= sol.values[k];
    }
    let cells: Vec<usize> = (0..m.n_cells()).filter(|&c| on[c]).collect();
    let mean = cells.iter().map(|&c| t[c]).sum::<f64>() / cells.len() as f64;
    let tmax = cells.iter().fold(0.0f64, |a, &c| a.max(t[c].abs()));
    let err = cells.iter().fold(0.0f64, |a, &c| a.max((d[c] - (t[c] - mean)).abs())) / tmax;
    let errn = cells.iter().fold(0.0f64, |a, &c| a.max((d[c] - t[c]).abs())) / tmax;
    let mut fails = Fails::new();
    println!(
        "F: {} cells, {} faces, mean {mean:.3e} (reported {:.3e}), {} iterations, reported residual {:.2e}",
        cells.len(),
        faces.len(),
        sol.removed_mean,
        sol.iterations,
        sol.residual
    );
    check(
        &mut fails,
        "F1",
        err <= 1e-12,
        format!("cell sums against t - mean {err:.2e} max|t| (1e-12)"),
    );
    check(
        &mut fails,
        "F1n",
        errn >= 1e-3,
        format!("against t without the mean {errn:.2e} max|t| (>= 1e-3)"),
    );
    require(&fails, &["F1", "F1n"]);
}

// -------------------------------------------------------------------------------------------------------------------
// S. Flux conservation across the sliding interface (A1)
// -------------------------------------------------------------------------------------------------------------------

/// S1 to S6, N3 and X0 (SF) of ADR 0216.
#[test]
fn s_radial_source_flow_through_the_sliding_interface() {
    let (u_in, nu, omega, dt) = (1.0, 0.1, 1.0, 0.005);
    let sc = pair(120);
    let mut run = Sliding::new(
        &sc,
        |g| {
            let mut u = VectorField::from_fn("U", &g, |c| vec3::scale(xhat(c), u_in * RI / c[0].hypot(c[1])));
            u.set_bc(&g, "cylinder", per_face(&g, "cylinder", |x| vec3::scale(xhat(x), u_in)))
                .unwrap();
            u.set_bc(&g, "farfield", Box::new(ZeroGradient)).unwrap();
            let mut p = ScalarField::uniform("p", &g, 0.0);
            p.set_bc(&g, "farfield", Box::new(FixedValue::scalar(0.0))).unwrap();
            PimpleSolver::new(g, u, p, settings(nu))
        },
        omega,
        dt,
    );
    let mut bc = |s: &mut PimpleSolver, st: &SlidingStep| {
        s.u.set_bc(
            &st.geom,
            "cylinder",
            per_face(&st.geom, "cylinder", |x| vec3::scale(xhat(x), u_in)),
        )
        .unwrap();
        Vec::new()
    };
    let (mut s1, mut s2, mut n3): (f64, f64, f64) = (0.0, 0.0, 0.0);
    for _ in 0..100 {
        run.step(&mut bc, &mut no_alter());
        let m = &run.s.geom.mesh;
        let phi = &run.s.phi;
        let q_in = -m.patch("cylinder").unwrap().faces().map(|f| phi[f]).sum::<f64>();
        let q_out = m.patch("farfield").unwrap().faces().map(|f| phi[f]).sum::<f64>();
        let (q_i, _) = run.interface_flux();
        s1 = s1.max((q_i - q_in).abs() / q_in);
        s2 = s2.max((q_out - q_in).abs() / q_in);
        let ub = run.s.u.boundary_values(&run.s.geom);
        let interp = face_flux(&run.s.geom, &run.s.u.values, &ub);
        let q_interp: f64 = run.cur.report.interface_faces.iter().map(|&f| interp[f]).sum();
        n3 = n3.max((q_interp - q_in).abs() / q_in);
    }
    let st = run.stats;
    let cc = run.s.geom.mesh.cell_centres();
    let err = run
        .s
        .u
        .values
        .iter()
        .zip(cc)
        .map(|(v, c)| vec3::norm(vec3::sub(*v, vec3::scale(xhat(*c), u_in * RI / c[0].hypot(c[1])))))
        .fold(0.0, f64::max);
    println!("SF: printed, no limit: max |u - U_in r_i/r| = {:.3e} U_in", err / u_in);
    let mut fails = Fails::new();
    check(
        &mut fails,
        "S1",
        s1 <= 1e-10,
        format!("|Q_I - Q_in|/Q_in {s1:.2e} (1e-10)"),
    );
    check(
        &mut fails,
        "S2",
        s2 <= 1e-10,
        format!("|Q_out - Q_in|/Q_in {s2:.2e} (1e-10)"),
    );
    check(
        &mut fails,
        "S3",
        st.cont <= 1e-9,
        format!("continuity {:.2e} (1e-9)", st.cont),
    );
    check(
        &mut fails,
        "S4",
        st.gcl <= 1e-12,
        format!("GCL {:.2e} mean(V) (1e-12)", st.gcl),
    );
    check(
        &mut fails,
        "S5",
        st.remap <= 1e-9,
        format!("remapped old flux {:.2e} (1e-9)", st.remap),
    );
    check(
        &mut fails,
        "S6",
        st.changed == st.steps,
        format!("overlaps changed in {} of {} steps", st.changed, st.steps),
    );
    check(
        &mut fails,
        "N3",
        n3 >= 1e-6,
        format!("interpolated interface flux {n3:.2e} Q_in (>= 1e-6)"),
    );
    x0(&mut fails, "SF", &st, run.angle(), TAU / 120.0);
    require(&fails, &["S1", "S2", "S3", "S4", "S5", "S6", "N3", "X0"]);
}

// -------------------------------------------------------------------------------------------------------------------
// U. Uniform flow on a rotating interface
// -------------------------------------------------------------------------------------------------------------------

/// Largest `max_c |u_c - U|/|U|` and `max_c |p_c - p_0|/|U|^2` over the steps.
fn uniform_run(sc: &SlidingInterface, u0: Vec3, dt: f64, steps: usize, alter: AlterHook) -> (f64, f64, Stats, f64) {
    let names: Vec<String> = sc
        .mesh_at(0.0)
        .unwrap()
        .mesh
        .patches()
        .iter()
        .filter(|p| p.kind != rcfd_mesh::PatchKind::Empty)
        .map(|p| p.name.clone())
        .collect();
    let mut run = Sliding::new(
        sc,
        |g| {
            let mut u = VectorField::uniform("U", &g, u0);
            for n in &names {
                u.set_bc(&g, n, Box::new(FixedValue::vector(u0))).unwrap();
            }
            let p = ScalarField::uniform("p", &g, 0.0);
            PimpleSolver::new(g, u, p, settings(0.1))
        },
        1.0,
        dt,
    );
    let un = vec3::norm(u0);
    let (mut eu, mut ep): (f64, f64) = (0.0, 0.0);
    for _ in 0..steps {
        run.step(&mut no_bc(), alter);
        eu = eu.max(
            run.s
                .u
                .values
                .iter()
                .fold(0.0f64, |a, v| a.max(vec3::norm(vec3::sub(*v, u0))))
                / un,
        );
        let p0 = run.s.p.values[0];
        ep = ep.max(run.s.p.values.iter().fold(0.0f64, |a, p| a.max((p - p0).abs())) / (un * un));
    }
    (eu, ep, run.stats, run.angle())
}

/// U1, U2 and X0 (U2) of ADR 0216.
#[test]
fn u2_uniform_flow_stays_uniform_on_the_sliding_pair() {
    let (eu, ep, st, angle) = uniform_run(&pair(120), [1.0, 0.5, 0.0], 0.005, 100, &mut no_alter());
    let mut fails = Fails::new();
    check(
        &mut fails,
        "U1",
        eu <= 1e-10 && ep <= 1e-10,
        format!("U2: velocity {eu:.2e} |U|, pressure {ep:.2e} |U|^2 (1e-10 each)"),
    );
    check(
        &mut fails,
        "U2",
        st.gcl <= 1e-12,
        format!("U2: GCL {:.2e} mean(V) (1e-12)", st.gcl),
    );
    x0(&mut fails, "U2", &st, angle, TAU / 120.0);
    require(&fails, &["U1", "U2", "X0"]);
}

/// U3, U4 and X0 (U3) of ADR 0216.
#[test]
fn u3_uniform_flow_stays_uniform_on_the_3d_sliding_pair() {
    let (eu, ep, st, angle) = uniform_run(&pair3(), [1.0, 0.5, 0.3], 0.01, 40, &mut no_alter());
    let mut fails = Fails::new();
    check(
        &mut fails,
        "U3",
        eu <= 1e-10 && ep <= 1e-10,
        format!("U3: velocity {eu:.2e} |U|, pressure {ep:.2e} |U|^2 (1e-10 each)"),
    );
    check(
        &mut fails,
        "U4",
        st.gcl <= 1e-12,
        format!("U3: GCL {:.2e} mean(V) (1e-12)", st.gcl),
    );
    x0(&mut fails, "U3", &st, angle, TAU / 40.0);
    require(&fails, &["U3", "U4", "X0"]);
}

/// N1 and N2 of ADR 0216 (negative controls of U1).
#[test]
fn n1_n2_uniform_flow_negative_controls() {
    let mut zero_if = |st: &mut SlidingStep, _: &PimpleSolver| {
        for &f in &st.interface_faces {
            st.mesh_flux[f] = 0.0;
        }
    };
    let (e1, _, _, _) = uniform_run(&pair(120), [1.0, 0.5, 0.0], 0.005, 100, &mut zero_if);
    let mut interp_if = |st: &mut SlidingStep, s: &PimpleSolver| {
        let fl = face_flux(
            &st.geom,
            &s.u.values,
            &vec![vec3::ZERO; st.geom.mesh.n_boundary_faces()],
        );
        for &f in &st.interface_faces {
            st.phi_old[f] = fl[f];
        }
    };
    let (e2, _, _, _) = uniform_run(&pair(120), [1.0, 0.5, 0.0], 0.005, 100, &mut interp_if);
    let mut fails = Fails::new();
    check(
        &mut fails,
        "N1",
        e1 >= 1e-6,
        format!("interface mesh flux zero: velocity {e1:.2e} |U| (>= 1e-6)"),
    );
    check(
        &mut fails,
        "N2",
        e2 >= 1e-6,
        format!("old interface flux not projected: velocity {e2:.2e} |U| (>= 1e-6)"),
    );
    require(&fails, &["N1", "N2"]);
}

// -------------------------------------------------------------------------------------------------------------------
// R. Rigid rotation (A2)
// -------------------------------------------------------------------------------------------------------------------

fn rigid_error(s: &PimpleSolver, omega: f64) -> f64 {
    let cc = s.geom.mesh.cell_centres();
    s.u.values
        .iter()
        .zip(cc)
        .map(|(v, c)| vec3::norm(vec3::sub(*v, rigid(omega, *c))))
        .fold(0.0, f64::max)
        / (omega * RO)
}

fn rigid_solver(g: Arc<FvGeometry>, omega: f64, nu: f64) -> PimpleSolver {
    let mut u = VectorField::from_fn("U", &g, |c| rigid(omega, c));
    for name in ["cylinder", "farfield"] {
        u.set_bc(&g, name, per_face(&g, name, |x| rigid(omega, x))).unwrap();
    }
    let p = ScalarField::from_fn("p", &g, |c| 0.5 * omega * omega * (c[0] * c[0] + c[1] * c[1]));
    PimpleSolver::new(g, u, p, settings(nu))
}

/// Static conforming control: `steps` of `step(dt)`.
fn static_rigid(mesh: &Mesh, omega: f64, nu: f64, dt: f64, steps: usize) -> PimpleSolver {
    let mut s = rigid_solver(geom_of(mesh), omega, nu);
    for _ in 0..steps {
        s.step(dt);
    }
    s
}

/// Rotating conforming control (ADR 0033 G6): the whole mesh turns with `omega`, walls follow it.
fn rotating_rigid(mesh: &Mesh, omega: f64, nu: f64, dt: f64, steps: usize) -> PimpleSolver {
    let mut s = rigid_solver(geom_of(mesh), omega, nu);
    let names = vec!["cylinder".to_string(), "farfield".to_string()];
    for n in 1..=steps {
        let pose = Pose {
            origin: vec3::ZERO,
            axis: Z,
            angle: omega * n as f64 * dt,
            translation: vec3::ZERO,
        };
        let pts: Vec<Vec3> = mesh.points().iter().map(|&q| pose.apply(q)).collect();
        let st = move_mesh(&s.geom, pts, dt).unwrap();
        let old = s.geom.mesh.clone();
        let walls = set_follow_conditions(&mut s.u, &st, &old, &names, dt).unwrap();
        s.step_ale_with_walls(dt, st.geom, st.mesh_flux, walls).unwrap();
    }
    s
}

/// Sliding rigid rotation (RR / RR=): returns the run.
fn sliding_rigid(sc: &SlidingInterface, omega: f64, nu: f64, dt: f64, steps: usize) -> (Sliding<'_>, f64) {
    let mut run = Sliding::new(sc, |g| rigid_solver(g, omega, nu), omega, dt);
    let mut wall = rotor_wall(omega, dt);
    let mut net: f64 = 0.0;
    for _ in 0..steps {
        run.step(&mut wall, &mut no_alter());
        let (q, a) = run.interface_flux();
        net = net.max(q.abs() / (omega * RO * a));
    }
    (run, net)
}

/// R1, R1b, R3 and X0 (RR) of ADR 0216.
#[test]
fn r1_rigid_rotation_across_the_sliding_interface_against_conforming_meshes() {
    let (omega, nu, dt, steps) = (1.0, 1.0, 0.0005, 200);
    let sc = pair(120);
    let (run, net) = sliding_rigid(&sc, omega, nu, dt, steps);
    let e = rigid_error(&run.s, omega);
    let mut ctl = Vec::new();
    let mut rot = Vec::new();
    for n in [144, 120] {
        let mesh = cylinder_ogrid(n, 16, RI, RO, Grading::Uniform, L).unwrap();
        let es = rigid_error(&static_rigid(&mesh, omega, nu, dt, steps), omega);
        let er = rigid_error(&rotating_rigid(&mesh, omega, nu, dt, steps), omega);
        println!("RR control C{n}: static {es:.3e} Omega r_o, rotating {er:.3e} Omega r_o");
        ctl.push(es);
        rot.push(er);
    }
    let st = run.stats;
    let mut fails = Fails::new();
    let b = 1.5 * ctl[0].max(ctl[1]);
    let br = 1.5 * rot[0].max(rot[1]);
    check(
        &mut fails,
        "R1",
        e <= b,
        format!("sliding {e:.3e} Omega r_o against 1.5 x static controls {b:.3e}"),
    );
    check(
        &mut fails,
        "R1b",
        e <= br,
        format!("sliding {e:.3e} Omega r_o against 1.5 x rotating controls {br:.3e}"),
    );
    check(
        &mut fails,
        "R3",
        net <= 1e-10 && st.gcl <= 1e-12 && st.cont <= 1e-9,
        format!(
            "RR: net interface flux {net:.2e} Omega r_o A_I (1e-10), GCL {:.2e} (1e-12), continuity {:.2e} (1e-9)",
            st.gcl, st.cont
        ),
    );
    x0(&mut fails, "RR", &st, run.angle(), TAU / 120.0);
    require(&fails, &["R1", "R1b", "R3", "X0"]);
}

/// R2, R0, R3 and X0 (RR=) of ADR 0216.
#[test]
fn r2_rigid_rotation_equal_counts_against_the_conforming_run() {
    let (omega, nu, steps) = (1.0, 1.0, 160);
    let dt = TAU / (144.0 * 80.0);
    let sc = pair(144);
    let (run, net) = sliding_rigid(&sc, omega, nu, dt, steps);
    let ctl = static_rigid(&sc.mesh_at(0.0).unwrap().mesh, omega, nu, dt, steps);
    let e_ctl = rigid_error(&ctl, omega);
    let theta = run.angle();
    let back = rotation_matrix(Z, -theta);
    let nr = sc.n_rotor_cells();
    let diff = (0..run.s.u.values.len())
        .map(|c| {
            let v = run.s.u.values[c];
            let v = if c < nr { mat_vec(&back, v) } else { v };
            vec3::norm(vec3::sub(v, ctl.u.values[c]))
        })
        .fold(0.0, f64::max)
        / (omega * RO);
    let n_if = run.cur.report.interface_faces.len();
    let st = run.stats;
    println!(
        "RR=: rotor angle {theta:.6} (two pitches {:.6}); sliding error {:.3e} Omega r_o, control {e_ctl:.3e}",
        2.0 * TAU / 144.0,
        rigid_error(&run.s, omega)
    );
    let mut fails = Fails::new();
    check(
        &mut fails,
        "R0",
        n_if == 144,
        format!("{n_if} interface faces at the end (144)"),
    );
    check(
        &mut fails,
        "R2",
        diff <= 1.5 * e_ctl,
        format!(
            "cell difference {diff:.3e} Omega r_o against 1.5 x control error {:.3e}",
            1.5 * e_ctl
        ),
    );
    check(
        &mut fails,
        "R3",
        net <= 1e-10 && st.gcl <= 1e-12 && st.cont <= 1e-9,
        format!(
            "RR=: net interface flux {net:.2e} Omega r_o A_I (1e-10), GCL {:.2e} (1e-12), continuity {:.2e} (1e-9)",
            st.gcl, st.cont
        ),
    );
    x0(&mut fails, "RR=", &st, theta, TAU / 144.0);
    require(&fails, &["R0", "R2", "R3", "X0"]);
}

// -------------------------------------------------------------------------------------------------------------------
// T. Taylor-Couette torque (A3)
// -------------------------------------------------------------------------------------------------------------------

/// Distance from the axis and angular velocity `(x x u) . z / r^2`.
fn omega_of(x: Vec3, u: Vec3) -> (f64, f64) {
    let r = x[0].hypot(x[1]);
    (r, (x[0] * u[1] - x[1] * u[0]) / (r * r))
}

/// M7 on a wall: (second-order torque, first-order torque).
fn wall_torque(s: &PimpleSolver, patch: &str, omega_w: f64, next: isize, nu: f64) -> (f64, f64) {
    let m = &s.geom.mesh;
    let (cc, cf) = (m.cell_centres(), m.face_centres());
    let (mut t2, mut t1) = (0.0, 0.0);
    for f in m.patch(patch).unwrap().faces() {
        let c1 = m.owner()[f];
        let c2 = (c1 as isize + next) as usize;
        let r0 = cf[f][0].hypot(cf[f][1]);
        let (r1, w1) = omega_of(cc[c1], s.u.values[c1]);
        let (r2, w2) = omega_of(cc[c2], s.u.values[c2]);
        let w0 = omega_w;
        let d = w0 * (2.0 * r0 - r1 - r2) / ((r0 - r1) * (r0 - r2))
            + w1 * (r0 - r2) / ((r1 - r0) * (r1 - r2))
            + w2 * (r0 - r1) / ((r2 - r0) * (r2 - r1));
        let a = m.face_area(f);
        t2 += nu * r0 * r0 * a * d;
        t1 += nu * r0 * r0 * a * (w1 - w0) / (r1 - r0);
    }
    (t2.abs(), t1.abs())
}

/// M7 through the interface faces.
fn interface_torque(s: &PimpleSolver, faces: &[usize], nu: f64) -> f64 {
    let m = &s.geom.mesh;
    let (cc, cf) = (m.cell_centres(), m.face_centres());
    faces
        .iter()
        .map(|&f| {
            let (o, n) = (m.owner()[f], m.neighbour()[f]);
            let (rp, wp) = omega_of(cc[o], s.u.values[o]);
            let (rn, wn) = omega_of(cc[n], s.u.values[n]);
            let rf = cf[f][0].hypot(cf[f][1]);
            nu * rf * rf * m.face_area(f) * (wn - wp) / (rn - rp)
        })
        .sum::<f64>()
        .abs()
}

fn tc_exact(r: f64) -> f64 {
    let d = RO * RO - RI * RI;
    -RI * RI / d * r + RI * RI * RO * RO / d / r
}

fn tc_velocity_error(s: &PimpleSolver) -> f64 {
    let cc = s.geom.mesh.cell_centres();
    s.u.values
        .iter()
        .zip(cc)
        .map(|(v, c)| {
            let r = c[0].hypot(c[1]);
            vec3::norm(vec3::sub(*v, vec3::scale(vec3::cross(Z, xhat(*c)), tc_exact(r))))
        })
        .fold(0.0, f64::max)
}

fn tc_solver(g: Arc<FvGeometry>, nu: f64) -> PimpleSolver {
    let mut u = VectorField::uniform("U", &g, vec3::ZERO);
    u.set_bc(&g, "cylinder", per_face(&g, "cylinder", |x| rigid(1.0, x)))
        .unwrap();
    u.set_bc(&g, "farfield", Box::new(FixedValue::vector(vec3::ZERO)))
        .unwrap();
    let p = ScalarField::uniform("p", &g, 0.0);
    PimpleSolver::new(g, u, p, settings(nu))
}

/// T1 to T5 and X0 (TC) of ADR 0216.
#[test]
fn t_taylor_couette_torque_across_the_sliding_interface() {
    let (nu, dt, steps) = (1.0, 0.005, 240);
    let b = RI * RI * RO * RO / (RO * RO - RI * RI);
    let t_ex = 4.0 * PI * nu * L * b;
    let sc = pair(120);
    let mut run = Sliding::new(&sc, |g| tc_solver(g, nu), 1.0, dt);
    let mut wall = rotor_wall(1.0, dt);
    let mut net: f64 = 0.0;
    let mut hist = Vec::new();
    for _ in 0..steps {
        run.step(&mut wall, &mut no_alter());
        let (q, a) = run.interface_flux();
        net = net.max(q.abs() / a);
        hist.push(wall_torque(&run.s, "cylinder", 1.0, 144, nu).0);
    }
    let (t_in, t_in1) = wall_torque(&run.s, "cylinder", 1.0, 144, nu);
    let (t_out, t_out1) = wall_torque(&run.s, "farfield", 0.0, -120, nu);
    let t_i = interface_torque(&run.s, &run.cur.report.interface_faces, nu);
    let change = (hist[steps - 1] - hist[steps - 21]).abs() / hist[steps - 1];
    println!(
        "TC: T_ex {t_ex:.6}; velocity error {:.3e} V_i; first-order wall torques {:.6} ({:+.3e}), {:.6} ({:+.3e})",
        tc_velocity_error(&run.s),
        t_in1,
        t_in1 / t_ex - 1.0,
        t_out1,
        t_out1 / t_ex - 1.0
    );
    for n in [144usize, 120] {
        let mut s = tc_solver(
            geom_of(&cylinder_ogrid(n, 16, RI, RO, Grading::Uniform, L).unwrap()),
            nu,
        );
        for _ in 0..steps {
            s.step(dt);
        }
        let ti = wall_torque(&s, "cylinder", 1.0, n as isize, nu).0;
        let to = wall_torque(&s, "farfield", 0.0, -(n as isize), nu).0;
        println!(
            "TC control C{n} (static): T_in {:+.3e}, T_out {:+.3e} relative; velocity error {:.3e} V_i",
            ti / t_ex - 1.0,
            to / t_ex - 1.0,
            tc_velocity_error(&s)
        );
    }
    let st = run.stats;
    let mut fails = Fails::new();
    let rel = |t: f64| (t / t_ex - 1.0).abs();
    check(
        &mut fails,
        "T1",
        rel(t_in) <= 2e-2,
        format!("rotor wall {t_in:.6}, {:+.3e} (2e-2)", t_in / t_ex - 1.0),
    );
    check(
        &mut fails,
        "T2",
        rel(t_out) <= 2e-2,
        format!("stator wall {t_out:.6}, {:+.3e} (2e-2)", t_out / t_ex - 1.0),
    );
    check(
        &mut fails,
        "T3",
        rel(t_i) <= 2e-2,
        format!("interface {t_i:.6}, {:+.3e} (2e-2)", t_i / t_ex - 1.0),
    );
    check(
        &mut fails,
        "T4",
        change <= 1e-4,
        format!("rotor wall torque change over 20 steps {change:.2e} (1e-4)"),
    );
    check(
        &mut fails,
        "T5",
        net <= 1e-10 && st.gcl <= 1e-12 && st.cont <= 1e-9,
        format!(
            "net interface flux {net:.2e} V_i A_I (1e-10), GCL {:.2e} (1e-12), continuity {:.2e} (1e-9)",
            st.gcl, st.cont
        ),
    );
    x0(&mut fails, "TC", &st, run.angle(), TAU / 120.0);
    require(&fails, &["T1", "T2", "T3", "T4", "T5", "X0"]);
}

// -------------------------------------------------------------------------------------------------------------------
// D. The decaying azimuthal mode (closed-form transient check)
// -------------------------------------------------------------------------------------------------------------------

fn simpson(f: impl Fn(f64) -> f64, a: f64, b: f64, n: usize) -> f64 {
    let h = (b - a) / n as f64;
    let mut s = f(a) + f(b);
    for i in 1..n {
        s += if i % 2 == 1 { 4.0 } else { 2.0 } * f(a + i as f64 * h);
    }
    s * h / 3.0
}

const NS: usize = 20000;

fn bessel_j(n: i32, x: f64) -> f64 {
    let nf = n as f64;
    simpson(|s| (nf * s - x * s.sin()).cos(), 0.0, PI, NS) / PI
}

fn bessel_y(n: i32, x: f64) -> f64 {
    let nf = n as f64;
    let a = simpson(|s| (x * s.sin() - nf * s).sin(), 0.0, PI, NS) / PI;
    let sgn = if n % 2 == 0 { 1.0 } else { -1.0 };
    let top = (50.0 / x).asinh();
    let b = simpson(
        |s| ((nf * s).exp() + sgn * (-nf * s).exp()) * (-x * s.sinh()).exp(),
        0.0,
        top,
        NS,
    ) / PI;
    a - b
}

/// The mode `phi(r)` of M8 with its eigenvalue `k`, cached by radius.
struct Mode {
    k: f64,
    j1a: f64,
    y1a: f64,
    cache: HashMap<i64, f64>,
}

impl Mode {
    fn new() -> Self {
        let f = |k: f64| bessel_j(1, k * RI) * bessel_y(1, k * RO) - bessel_j(1, k * RO) * bessel_y(1, k * RI);
        let (mut lo, mut hi) = (2.5, 4.0);
        let flo = f(lo);
        assert!(flo * f(hi) < 0.0, "no root bracketed");
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if flo * f(mid) <= 0.0 {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        let k = 0.5 * (lo + hi);
        Mode {
            k,
            j1a: bessel_j(1, k * RI),
            y1a: bessel_y(1, k * RI),
            cache: HashMap::new(),
        }
    }

    fn phi(&mut self, r: f64) -> f64 {
        let key = (r * 1e12).round() as i64;
        let (k, j1a, y1a) = (self.k, self.j1a, self.y1a);
        *self
            .cache
            .entry(key)
            .or_insert_with(|| bessel_j(1, k * r) * y1a - j1a * bessel_y(1, k * r))
    }
}

fn mode_solver(g: Arc<FvGeometry>, mode: &mut Mode, scale: f64, nu: f64) -> PimpleSolver {
    let cc = g.mesh.cell_centres().to_vec();
    let mut u = VectorField::uniform("U", &g, vec3::ZERO);
    for (c, x) in cc.iter().enumerate() {
        let r = x[0].hypot(x[1]);
        u.values[c] = vec3::scale(vec3::cross(Z, xhat(*x)), mode.phi(r) / scale);
    }
    for name in ["cylinder", "farfield"] {
        u.set_bc(&g, name, Box::new(FixedValue::vector(vec3::ZERO))).unwrap();
    }
    let p = ScalarField::uniform("p", &g, 0.0);
    PimpleSolver::new(g, u, p, settings(nu))
}

fn amplitude(s: &PimpleSolver, mode: &mut Mode) -> f64 {
    let m = &s.geom.mesh;
    let (cc, v) = (m.cell_centres(), m.cell_volumes());
    let (mut num, mut den) = (0.0, 0.0);
    for c in 0..m.n_cells() {
        let r = cc[c][0].hypot(cc[c][1]);
        let ph = mode.phi(r);
        num += v[c] * vec3::dot(s.u.values[c], vec3::cross(Z, xhat(cc[c]))) * ph;
        den += v[c] * ph * ph;
    }
    num / den
}

/// B0, D1 to D3 and X0 (DM) of ADR 0216.
#[test]
fn d_decaying_mode_with_a_turning_rotor_zone() {
    let mut fails = Fails::new();
    let mut worst_w: f64 = 0.0;
    for n in [0, 1] {
        for x in [1.0, 2.0, 3.2, 6.4] {
            let w = bessel_j(n + 1, x) * bessel_y(n, x) - bessel_j(n, x) * bessel_y(n + 1, x);
            let ex = 2.0 / (PI * x);
            worst_w = worst_w.max((w - ex).abs() / ex);
        }
    }
    check(
        &mut fails,
        "B0",
        worst_w <= 1e-11,
        format!("Wronskian {worst_w:.2e} (1e-11)"),
    );
    let (nu, dt, steps) = (0.1, 0.005, 200);
    let mut mode = Mode::new();
    let lambda = nu * mode.k * mode.k;
    let scale = (0..=2000)
        .map(|i| RI + (RO - RI) * i as f64 / 2000.0)
        .map(|r| bessel_j(1, mode.k * r) * mode.y1a - mode.j1a * bessel_y(1, mode.k * r))
        .fold(0.0f64, |a, p| a.max(p.abs()));
    println!(
        "DM: k = {:.12}, lambda = nu k^2 = {lambda:.9}, max |phi| = {scale:.6e}",
        mode.k
    );
    let sc = pair(120);
    let mut run = Sliding::new(&sc, |g| mode_solver(g, &mut mode, scale, nu), 1.0, dt);
    let mut wall = rotor_wall(0.0, dt);
    let mut a1 = 0.0;
    for n in 1..=steps {
        run.step(&mut wall, &mut no_alter());
        if n == 40 {
            a1 = amplitude(&run.s, &mut mode);
        }
    }
    let a2 = amplitude(&run.s, &mut mode);
    let lam = (a1 / a2).ln() / 0.8;
    let mut ctl = Vec::new();
    for n in [144usize, 120] {
        let g = geom_of(&cylinder_ogrid(n, 16, RI, RO, Grading::Uniform, L).unwrap());
        let mut s = mode_solver(g, &mut mode, scale, nu);
        let mut b1 = 0.0;
        for k in 1..=steps {
            s.step(dt);
            if k == 40 {
                b1 = amplitude(&s, &mut mode);
            }
        }
        let lc = (b1 / amplitude(&s, &mut mode)).ln() / 0.8;
        println!(
            "DM control C{n} (static): rate {lc:.9}, relative {:+.3e}",
            lc / lambda - 1.0
        );
        ctl.push((lc - lambda).abs());
    }
    let st = run.stats;
    check(
        &mut fails,
        "D1",
        (lam / lambda - 1.0).abs() <= 2e-2,
        format!("rate {lam:.9}, relative {:+.3e} (2e-2)", lam / lambda - 1.0),
    );
    let bound = 1.5 * ctl[0].max(ctl[1]);
    check(
        &mut fails,
        "D2",
        (lam - lambda).abs() <= bound,
        format!(
            "|rate - lambda| {:.3e} against 1.5 x the larger control {bound:.3e}",
            (lam - lambda).abs()
        ),
    );
    check(
        &mut fails,
        "D3",
        st.gcl <= 1e-12 && st.cont <= 1e-9,
        format!("GCL {:.2e} (1e-12), continuity {:.2e} (1e-9)", st.gcl, st.cont),
    );
    x0(&mut fails, "DM", &st, run.angle(), TAU / 120.0);
    require(&fails, &["B0", "D1", "D2", "D3", "X0"]);
}

// -------------------------------------------------------------------------------------------------------------------
// E. Refusals
// -------------------------------------------------------------------------------------------------------------------

fn state_bits(s: &PimpleSolver) -> Vec<u64> {
    let mut v: Vec<u64> = s.u.values.iter().flatten().map(|x| x.to_bits()).collect();
    v.extend(s.p.values.iter().chain(&s.phi).map(|x| x.to_bits()));
    v.push(s.time.to_bits());
    v
}

/// E1 and E2 of ADR 0216.
#[test]
fn e_refusals_leave_the_solver_unchanged() {
    let dt = 0.005;
    let sc = pair(120);
    let m0 = sc.mesh_at(0.0).unwrap();
    let m1 = sc.mesh_at(dt).unwrap();
    let make = |scheme: TimeScheme| {
        let g = geom_of(&m0.mesh);
        let mut s = tc_solver(g, 1.0);
        s.settings.time_scheme = scheme;
        s
    };
    let mut fails = Fails::new();
    // E1: BDF2
    let mut s = make(TimeScheme::Bdf2);
    let st = sliding_step(&s.geom, &m0, &m1, &s.phi, &s.u.values, dt).unwrap();
    let before = state_bits(&s);
    let r1 = s.step_sliding(dt, st, Vec::new());
    let e1a = r1.is_err() && state_bits(&s) == before;
    println!("E1 BDF2: {r1:?}");
    // E1: a step of P3
    let sc3 = pair3();
    let (a3, b3) = (sc3.mesh_at(0.0).unwrap(), sc3.mesh_at(0.01).unwrap());
    let g3 = geom_of(&a3.mesh);
    let phi3 = vec![0.0; a3.mesh.n_faces()];
    let st3 = sliding_step(&g3, &a3, &b3, &phi3, &vec![vec3::ZERO; a3.mesh.n_cells()], 0.01).unwrap();
    let mut s = make(TimeScheme::ImplicitEuler);
    let before = state_bits(&s);
    let r2 = s.step_sliding(dt, st3, Vec::new());
    let e1b = r2.is_err() && state_bits(&s) == before;
    println!("E1 another pair: {r2:?}");
    // E1: a face viscosity
    let mut s = make(TimeScheme::ImplicitEuler);
    s.nu_face = Some(vec![1.0; m0.mesh.n_faces()]);
    let st = sliding_step(&s.geom, &m0, &m1, &s.phi, &s.u.values, dt).unwrap();
    let before = state_bits(&s);
    let r3 = s.step_sliding(dt, st, Vec::new());
    let e1c = r3.is_err() && state_bits(&s) == before;
    println!("E1 nu_face: {r3:?}");
    check(
        &mut fails,
        "E1",
        e1a && e1b && e1c,
        format!("refused, unchanged: {e1a}, {e1b}, {e1c}"),
    );
    // E2
    let s = make(TimeScheme::ImplicitEuler);
    let short = &s.phi[..s.phi.len() - 1];
    let r4 = sliding_step(&s.geom, &m0, &m1, short, &s.u.values, dt);
    let r5 = sliding_step(&s.geom, &m0, &b3, &s.phi, &s.u.values, dt);
    println!("E2: {:?}; {:?}", r4.as_ref().err(), r5.as_ref().err());
    check(
        &mut fails,
        "E2",
        r4.is_err() && r5.is_err(),
        "flux one short, mesh of another pair".to_string(),
    );
    require(&fails, &["E1", "E2"]);
}
