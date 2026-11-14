//! F-401 (ADR 0434): the elastic bar of F-400 (`DynamicSolid`, nu = 0, clamped) closing a channel of incompressible fluid
//! (`FlowWall`, the moving-mesh PISO step with the prescribed wall gradient of F-351), coupled by `coupled_step` with Aitken's
//! factor and with the least-squares update, at mass ratios `rho_f L_f/(rho_s L_s)` of 0.5 and 4, started from the static
//! release of a tip traction; against the root of `k L tan(k L) = rho_s L_s/(rho_f L_f)` after the time-step estimate of the
//! discrete model of the pairing (ADR 0434 D2); the energy balance on the interface over ten periods; the loose control; the
//! 3:2 wall with the bilinear projection of F-504; failures of either participant; the non-uniform part of the interface force.
//!
//! The limits, set-ups, model arithmetic and mutations were registered in ADR 0434 before this file existed. The root, the
//! 1D chain and the bar are those of `si_solid_dynamic*.rs` (ADR 0410, 0412, 0414; copied, those files are not edited); the
//! fluid bits are those of `cosim_f139.rs` (copied).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fem::builders::hex_box;
use rcfd_fem::transient::{LoadHistory, LoadTable, TransientOptions};
use rcfd_fem::{Constraint, Load, Material, SolidModel};
use rcfd_fv::bc::{FixedValue, Symmetry};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::cosim::{
    coupled_step, interface_rows, AtCap, CoupledStep, CouplingError, CouplingSettings, FlowWall, Fluid, InterfaceMap,
    MapSettings, MeshMotion, Participant, Structure,
};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::motion::moved_mesh;
use rcfd_mesh::Mesh;
use rcfd_post::solid_dynamic::DynamicSolid;
use rcfd_post::solid_fsi::ChannelMotion;
use std::cell::Cell;
use std::f64::consts::PI;
use std::rc::Rc;

const L_S: f64 = 4.0;
const W: f64 = 1.0;
const NX: usize = 20;
const NF: usize = 4;
const E: f64 = 2e11;
const RHO_S: f64 = 7850.0;
const L_F: f64 = 32.0;
const NXF: usize = 24;
const EXPANSION: f64 = 15.0;
const NU: f64 = 1e-4;
const U: f64 = 1e-3;
const STEPS: usize = 1001;
const TOL: f64 = 1e-6;
const WALL: &str = "xmin";

/// Measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Self {
        Checks(Vec::new())
    }
    fn at_most(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{id}: {value:e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} > {limit:e}"));
        }
    }
    fn below(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value < limit;
        println!(
            "{id}: {value:e} (limit < {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} >= {limit:e}"));
        }
    }
    fn at_least(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!(
            "{id}: {value:e} (limit >= {limit:e}) {}",
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
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

// ---- the closed form and the discrete model (ADR 0434 D1, D2, D8) ---------------------------------------------------------

/// The first root of `x tan x = rhs` on `(0, pi/2)`.
fn root(rhs: f64) -> f64 {
    let (mut lo, mut hi) = (0.0f64, PI / 2.0);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if mid * mid.tan() < rhs {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

#[derive(Debug, Clone, Copy)]
struct Params {
    omega: f64,
    dt: f64,
    m_a: f64,
    rho_f: f64,
    /// The release force `P = E A U/L_s`.
    p: f64,
}

fn params(ratio: f64) -> Params {
    let kl = root(1.0 / ratio);
    let omega = kl * (E / RHO_S).sqrt() / L_S;
    let dt = 2.0 * PI / omega / 100.0;
    let a = W * W;
    Params {
        omega,
        dt,
        m_a: ratio * RHO_S * a * L_S,
        rho_f: ratio * RHO_S * L_S / L_F,
        p: E * a * U / L_S,
    }
}

/// A symmetric tridiagonal matrix (diagonal, off-diagonal).
#[derive(Clone)]
struct Tri(Vec<f64>, Vec<f64>);

impl Tri {
    fn mul(&self, x: &[f64]) -> Vec<f64> {
        let n = x.len();
        (0..n)
            .map(|i| {
                let mut y = self.0[i] * x[i];
                if i > 0 {
                    y += self.1[i - 1] * x[i - 1];
                }
                if i + 1 < n {
                    y += self.1[i] * x[i + 1];
                }
                y
            })
            .collect()
    }
    /// Thomas algorithm.
    fn solve(&self, b: &[f64]) -> Vec<f64> {
        let n = b.len();
        let (mut c, mut d) = (vec![0.0; n], vec![0.0; n]);
        for i in 0..n {
            let sub = if i > 0 { self.1[i - 1] } else { 0.0 };
            let den = self.0[i] - if i > 0 { sub * c[i - 1] } else { 0.0 };
            c[i] = if i + 1 < n { self.1[i] / den } else { 0.0 };
            d[i] = (b[i] - if i > 0 { sub * d[i - 1] } else { 0.0 }) / den;
        }
        let mut x = vec![0.0; n];
        for i in (0..n).rev() {
            x[i] = d[i] - if i + 1 < n { c[i] * x[i + 1] } else { 0.0 };
        }
        x
    }
    /// `self + s other`.
    fn add(&self, s: f64, other: &Tri) -> Tri {
        Tri(
            self.0.iter().zip(&other.0).map(|(a, b)| a + s * b).collect(),
            self.1.iter().zip(&other.1).map(|(a, b)| a + s * b).collect(),
        )
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// The 1D chain of ADR 0410 D5 (nodes 1..=NX, node 0 clamped, area W^2): `(K, M)`.
fn chain() -> (Tri, Tri) {
    let h = L_S / NX as f64;
    let a = W * W;
    let (ke, me) = (E * a / h, RHO_S * a * h / 6.0);
    let mut kd = vec![2.0 * ke; NX];
    let mut md = vec![4.0 * me; NX];
    kd[NX - 1] = ke;
    md[NX - 1] = 2.0 * me;
    (Tri(kd, vec![-ke; NX - 1]), Tri(md, vec![me; NX - 1]))
}

/// The discrete model of ADR 0434 D2 from the initial state of D4: the tip displacement and the energy at every step
/// (index 0 the start). `lag = false`: the Newmark-consistent piston of ADR 0410 (the force `-m_a a_{n+1}`).
struct ModelRun {
    tip: Vec<f64>,
    energy: Vec<f64>,
}

fn model(p: &Params, steps: usize, lag: bool) -> ModelRun {
    let (k, m) = chain();
    let dt = p.dt;
    let x: Vec<f64> = (1..=NX).map(|i| L_S * i as f64 / NX as f64).collect();
    let mut u: Vec<f64> = x.iter().map(|x| U * x / L_S).collect();
    let mut v = vec![0.0; NX];
    let mut load = [0.0; NX];
    load[NX - 1] = p.p;
    let ku = k.mul(&u);
    let mut a = m.solve(&load.iter().zip(&ku).map(|(f, g)| f - g).collect::<Vec<_>>());
    let s = m.add(0.25 * dt * dt, &k);
    let mut sm = s.clone();
    sm.0[NX - 1] += if lag { 0.25 * p.m_a } else { p.m_a };
    let energy = |u: &[f64], v: &[f64]| 0.5 * dot(v, &m.mul(v)) + 0.5 * dot(u, &k.mul(u));
    let mut out = ModelRun {
        tip: vec![u[NX - 1]],
        energy: vec![energy(&u, &v)],
    };
    let mut u_prev_tip = u[NX - 1];
    for _ in 0..steps {
        let us: Vec<f64> = (0..NX).map(|i| u[i] + dt * v[i] + 0.25 * dt * dt * a[i]).collect();
        let mut rhs: Vec<f64> = k.mul(&us).iter().map(|x| -x).collect();
        if lag {
            rhs[NX - 1] -= p.m_a / (dt * dt) * (us[NX - 1] - 2.0 * u[NX - 1] + u_prev_tip);
        }
        let a1 = sm.solve(&rhs);
        let u1: Vec<f64> = (0..NX).map(|i| us[i] + 0.25 * dt * dt * a1[i]).collect();
        let v1: Vec<f64> = (0..NX).map(|i| v[i] + 0.5 * dt * (a[i] + a1[i])).collect();
        u_prev_tip = u[NX - 1];
        u = u1;
        v = v1;
        a = a1;
        out.tip.push(u[NX - 1]);
        out.energy.push(energy(&u, &v));
    }
    out
}

/// Zero crossings by linear interpolation; `Omega_m = 2 pi/T_m`, `T_m = 2 (t_last - t_first)/(N_c - 1)`.
fn measured_omega(dt: f64, x: &[f64]) -> (f64, usize) {
    let mut c = Vec::new();
    for i in 1..x.len() {
        if (x[i - 1] > 0.0) != (x[i] > 0.0) && x[i] != x[i - 1] {
            c.push(dt * ((i - 1) as f64 + x[i - 1] / (x[i - 1] - x[i])));
        }
    }
    let n = c.len();
    assert!(n >= 3, "too few crossings: {n}");
    let tm = 2.0 * (c[n - 1] - c[0]) / (n - 1) as f64;
    (2.0 * PI / tm, n)
}

// ---- set-ups (ADR 0434 D4) -------------------------------------------------------------------------------------------------

/// The bar: Hex8 20 x 4 x 4, nu = 0, clamped on xmin, the release traction `P/A` in `+x` on xmax.
fn bar(p: &Params) -> SolidModel {
    let mut model = SolidModel::new(hex_box([0.0; 3], [L_S, W, W], [NX, NF, NF]).unwrap());
    model
        .add_material("all", Material::isotropic_elastic(E, 0.0, RHO_S, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        model.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    model
        .add_load(Load::traction("xmax", [p.p / (W * W), 0.0, 0.0]))
        .unwrap();
    model
}

/// The solid at the static displaced state, at rest, its release table ending at `end_steps` steps.
fn solid(p: &Params, end_steps: usize) -> DynamicSolid {
    let model = bar(p);
    let table = LoadTable::new(vec![(0.0, 1.0), (p.dt, 0.0), (end_steps as f64 * p.dt, 0.0)]).unwrap();
    let loads = [LoadHistory::model_loads(&model, table).unwrap()];
    let mut s = DynamicSolid::new("solid", &model, "xmax", &loads, &TransientOptions::new(p.dt), 1).unwrap();
    let dofs = model.dof_map();
    let n = s.integrator().n_unknowns();
    let mut u0 = vec![0.0; n];
    for (node, x) in model.mesh().nodes().iter().enumerate() {
        if let Some(e) = dofs.equation(node, 0) {
            u0[e] = U * x[0] / L_S;
        }
    }
    s.start(0.0, &u0, &vec![0.0; n]).unwrap();
    s
}

/// The channel in the reference configuration: `m x m` cells across, 24 graded along `x`, the wall `xmin` at `x = L_s`.
fn channel(m: usize) -> Mesh {
    let mesh = box_mesh(
        [NXF, m, m],
        [L_F, W, W],
        [Grading::Expansion(EXPANSION), Grading::Uniform, Grading::Uniform],
    )
    .unwrap();
    let points = mesh.points().iter().map(|p| [p[0] + L_S, p[1], p[2]]).collect();
    moved_mesh(&mesh, points).unwrap()
}

fn settings(nu: f64) -> PimpleSettings {
    let mut s = PimpleSettings::piso(nu);
    s.convection = ConvScheme::Central;
    for c in [&mut s.momentum_solver, &mut s.pressure_solver] {
        c.tol_abs = 1e-16;
        c.tol_rel = 1e-13;
        c.max_iter = 5000;
    }
    s
}

/// Fluid at rest, `p = 0`, on `mesh` (points as given).
fn flow(mesh0: &Mesh, points: Vec<Vec3>, nu: f64) -> PimpleSolver {
    let mesh = moved_mesh(mesh0, points).unwrap();
    let geom = std::sync::Arc::new(FvGeometry::new(std::sync::Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    for name in ["ymin", "ymax", "zmin", "zmax"] {
        u.set_bc(&geom, name, Box::new(Symmetry)).unwrap();
    }
    let mut pr = ScalarField::uniform("p", &geom, 0.0);
    pr.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    PimpleSolver::new(geom, u, pr, settings(nu))
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Method {
    Aitken,
    LeastSquares,
    Loose,
}

#[derive(Debug, Clone, Copy)]
struct Case {
    ratio: f64,
    /// Fluid cells across the section: 4 matching, 6 the 3:2 wall.
    m: usize,
    method: Method,
    nu: f64,
    /// The release table ends here (steps).
    table_end: usize,
}

impl Case {
    fn new(ratio: f64, m: usize, method: Method) -> Self {
        Case {
            ratio,
            m,
            method,
            nu: NU,
            table_end: STEPS + 2,
        }
    }
    fn map_settings(&self) -> MapSettings {
        if self.m == NF {
            MapSettings::projection(1e-9)
        } else {
            MapSettings::bilinear_projection(1e-9)
        }
    }
}

struct Parts {
    p: Params,
    fluid: FlowWall,
    solid: DynamicSolid,
    map: InterfaceMap,
    coupling: CouplingSettings,
    /// Reference areas of the fluid's wall faces.
    areas: Vec<f64>,
    /// The interface nodal forces of the release load (their norm is the force scale).
    release: Vec<Vec3>,
    /// The mesh point ids of the wall points.
    wall_ids: Vec<usize>,
    mesh0: Mesh,
    /// The initial interface displacement mapped to the wall points.
    initial_points: Vec<Vec3>,
}

fn quad_area(points: &[Vec3], f: &[usize]) -> f64 {
    let d1 = vec3::sub(points[f[2]], points[f[0]]);
    let d2 = vec3::sub(points[f[3]], points[f[1]]);
    0.5 * vec3::norm(vec3::cross(d1, d2))
}

fn interface_displacement(s: &DynamicSolid) -> Vec<Vec3> {
    // the interface values of the full displacement: through the dof map of the bar
    let model = bar(&params(1.0));
    let dofs = model.dof_map();
    let u = s.displacement();
    s.interface()
        .nodes()
        .iter()
        .map(|&n| {
            let at = |c: usize| dofs.equation(n, c).map_or(0.0, |e| u[e]);
            [at(0), at(1), at(2)]
        })
        .collect()
}

/// The parts of a case; `wrap` may replace the mesh motion (the failing motion of FS2).
fn parts_with(case: &Case, wrap: &dyn Fn(MeshMotion) -> MeshMotion) -> Parts {
    let p = params(case.ratio);
    let solid = solid(&p, case.table_end);
    let mesh0 = channel(case.m);
    let motion = ChannelMotion::new(&mesh0, WALL, 0, L_F, 1e-9).unwrap();
    // a probe on the reference mesh gives the wall points and faces for the map
    let probe = FlowWall::new(
        flow(&mesh0, mesh0.points().to_vec(), case.nu),
        WALL,
        &mesh0,
        p.rho_f,
        case.nu,
        motion.clone().into_motion(),
    )
    .unwrap();
    let (map, _) = InterfaceMap::build(
        &solid.node_coordinates(),
        &solid.elements(),
        &probe.point_coordinates(),
        probe.face_points(),
        &case.map_settings(),
    )
    .unwrap();
    let q0 = interface_displacement(&solid);
    let initial_points = map.displacement_to_points(&q0).unwrap();
    let start = motion.positions(&initial_points).unwrap();
    let fluid = FlowWall::new(
        flow(&mesh0, start, case.nu),
        WALL,
        &mesh0,
        p.rho_f,
        case.nu,
        wrap(motion.into_motion()),
    )
    .unwrap();
    let ref_points = fluid.point_coordinates();
    let areas = fluid.face_points().iter().map(|f| quad_area(&ref_points, f)).collect();
    // the release load on the interface nodes: the uniform traction mapped by the consistent (matching) weights of the
    // solid's own face, i.e. P A_e/4 at the corners of every interface element
    let nodes = solid.node_coordinates();
    let mut release = vec![vec3::ZERO; nodes.len()];
    for el in solid.elements() {
        let a = quad_area(&nodes, &el);
        for &i in &el {
            release[i][0] += p.p / (W * W) * a / 4.0;
        }
    }
    let scale = norm3(&release);
    let mut coupling = match case.method {
        Method::Aitken => CouplingSettings::aitken(0.5),
        Method::LeastSquares => CouplingSettings::least_squares(0.5),
        Method::Loose => CouplingSettings::aitken(0.5),
    };
    coupling.tolerance = TOL;
    coupling.force_scale = scale;
    coupling.max_iterations = match (case.method, case.m == NF) {
        (Method::Loose, _) => 1,
        (Method::Aitken, false) => 20,
        _ => 10,
    };
    coupling.at_cap = if case.method == Method::Loose {
        AtCap::Keep
    } else {
        AtCap::Stop
    };
    let wall_ids = FlowWall::point_ids(&mesh0, WALL).unwrap();
    Parts {
        p,
        fluid,
        solid,
        map,
        coupling,
        areas,
        release,
        wall_ids,
        mesh0,
        initial_points,
    }
}

fn parts(case: &Case) -> Parts {
    parts_with(case, &|m| m)
}

fn norm3(v: &[Vec3]) -> f64 {
    v.iter().flatten().map(|x| x * x).sum::<f64>().sqrt()
}

fn total(v: &[Vec3]) -> Vec3 {
    v.iter().fold(vec3::ZERO, |a, b| vec3::add(a, *b))
}

/// `eta` of ADR 0434 D6 before the normalisation: `|F^nu|` and `|sum F|`.
fn non_uniform(forces: &[Vec3], areas: &[f64]) -> (f64, f64) {
    let s = total(forces);
    let a: f64 = areas.iter().sum();
    let nu: f64 = forces
        .iter()
        .zip(areas)
        .map(|(f, af)| {
            let d = vec3::sub(*f, vec3::scale(s, af / a));
            vec3::dot(d, d)
        })
        .sum();
    (nu.sqrt(), vec3::norm(s))
}

fn face_work(d0: &[Vec3], d1: &[Vec3], f0: &[Vec3], f1: &[Vec3]) -> f64 {
    (0..d0.len())
        .map(|i| vec3::dot(vec3::sub(d1[i], d0[i]), vec3::scale(vec3::add(f0[i], f1[i]), 0.5)))
        .sum()
}

// ---- runs ------------------------------------------------------------------------------------------------------------------

struct Run {
    p: Params,
    /// The signal (mean interface `x` displacement) at `t_0 .. t_n`.
    signal: Vec<f64>,
    /// Energy of the solid at `t_0 .. t_n`.
    energy: Vec<f64>,
    /// Cumulative fluid work on the faces from `t_0` to `t_k`.
    work: Vec<f64>,
    iterations: Vec<usize>,
    converged: Vec<bool>,
    /// `|F^nu|` and `|sum F|` at `t_1 .. t_n`.
    nonuniform: Vec<(f64, f64)>,
    /// Total axial held force of every step.
    held: Vec<f64>,
    /// Largest transverse and axial interface displacement.
    transverse: (f64, f64),
    last: Option<CoupledStep>,
    error: Option<CouplingError>,
}

impl Run {
    fn steps(&self) -> usize {
        self.iterations.len()
    }
    fn eta_max(&self, from: usize, to: usize) -> f64 {
        let scale = self.nonuniform.iter().fold(0.0f64, |m, x| m.max(x.1));
        self.nonuniform[from - 1..to].iter().fold(0.0f64, |m, x| m.max(x.0)) / scale
    }
    fn histogram(&self) -> Vec<usize> {
        let mut h = vec![0; self.iterations.iter().max().copied().unwrap_or(0) + 1];
        for &k in &self.iterations {
            h[k] += 1;
        }
        h
    }
    fn mean_iterations(&self) -> f64 {
        self.iterations.iter().sum::<usize>() as f64 / self.steps() as f64
    }
    fn omega(&self) -> (f64, usize) {
        measured_omega(self.p.dt, &self.signal)
    }
    /// `|E_to - E_from - W(from..to)|/E_from`.
    fn balance(&self, from: usize, to: usize) -> f64 {
        let de = self.energy[to] - self.energy[from];
        let w = self.work[to] - self.work[from];
        (de - w).abs() / self.energy[from]
    }
}

fn mean_x(q: &[Vec3]) -> f64 {
    q.iter().map(|v| v[0]).sum::<f64>() / q.len() as f64
}

fn solid_energy(s: &DynamicSolid) -> f64 {
    s.integrator().kinetic_energy().unwrap() + s.integrator().strain_energy().unwrap()
}

/// Runs `steps` coupled steps on the parts (stops at an error, or for the loose control at `10 U`).
fn run_parts(parts: &mut Parts, steps: usize) -> Run {
    let p = parts.p;
    let q0 = interface_displacement(&parts.solid);
    let mut run = Run {
        p,
        signal: vec![mean_x(&q0)],
        energy: vec![solid_energy(&parts.solid)],
        work: vec![0.0],
        iterations: Vec::new(),
        converged: Vec::new(),
        nonuniform: Vec::new(),
        held: Vec::new(),
        transverse: (0.0, 0.0),
        last: None,
        error: None,
    };
    let mut d_prev = parts
        .map
        .face_displacements(&parts.map.displacement_to_points(&q0).unwrap())
        .unwrap();
    let mut f_prev = parts.fluid.forces().unwrap();
    let loose = parts.coupling.max_iterations == 1;
    for _ in 0..steps {
        let step = match coupled_step(&mut parts.fluid, &mut parts.solid, &parts.map, p.dt, &parts.coupling) {
            Ok(s) => s,
            Err(e) => {
                run.error = Some(e);
                break;
            }
        };
        let q = &step.node_displacements;
        let d = parts
            .map
            .face_displacements(&parts.map.displacement_to_points(q).unwrap())
            .unwrap();
        let w = face_work(&d_prev, &d, &f_prev, &step.face_forces);
        run.work.push(run.work.last().unwrap() + w);
        run.signal.push(mean_x(q));
        run.energy.push(solid_energy(&parts.solid));
        run.iterations.push(step.iterations);
        run.converged.push(step.converged);
        run.nonuniform.push(non_uniform(&step.face_forces, &parts.areas));
        run.held.push(total(&step.node_forces)[0]);
        for v in q {
            run.transverse.0 = run.transverse.0.max(v[1].abs()).max(v[2].abs());
            run.transverse.1 = run.transverse.1.max(v[0].abs());
        }
        d_prev = d;
        f_prev.clone_from(&step.face_forces);
        run.last = Some(step);
        if loose && run.signal.last().unwrap().abs() >= 10.0 * U {
            break;
        }
    }
    run
}

fn run_case(case: &Case, steps: usize) -> Run {
    let mut parts = parts(case);
    run_parts(&mut parts, steps)
}

fn method_id(m: Method) -> &'static str {
    match m {
        Method::Aitken => "Aitken",
        Method::LeastSquares => "least squares",
        Method::Loose => "loose",
    }
}

/// The iteration record of a run: every step converged within the cap (I1, I2, I2b).
fn iteration_checks(checks: &mut Checks, id: &str, run: &Run, steps: usize, cap: usize) {
    println!(
        "{id}: {} steps, iterations largest {}, mean {:.4}, histogram {:?}",
        run.steps(),
        run.iterations.iter().max().copied().unwrap_or(0),
        run.mean_iterations(),
        run.histogram()
    );
    if let Some(e) = &run.error {
        println!("{id}: stopped: {e}");
    }
    checks.holds(
        id,
        run.steps() == steps && run.error.is_none() && run.converged.iter().all(|&c| c),
        &format!("all {steps} steps converged to {TOL:e}"),
    );
    checks.at_most(
        &format!("{id} largest iterations"),
        run.iterations.iter().max().copied().unwrap_or(usize::MAX) as f64,
        cap as f64,
    );
}

/// Resultant and work of the last step's face forces against the nodal forces (MC).
fn map_conservation(checks: &mut Checks, id: &str, parts_map: &InterfaceMap, step: &CoupledStep) {
    let nodal = parts_map.forces_to_nodes(&step.face_forces).unwrap();
    let (rf, rn) = (total(&step.face_forces), total(&nodal));
    let scale: f64 = step.face_forces.iter().map(|f| vec3::norm(*f)).sum();
    checks.at_most(&format!("{id} resultant"), vec3::norm(vec3::sub(rf, rn)) / scale, 1e-12);
    let q = &step.node_displacements;
    let d = parts_map
        .face_displacements(&parts_map.displacement_to_points(q).unwrap())
        .unwrap();
    let wf: f64 = step.face_forces.iter().zip(&d).map(|(f, x)| vec3::dot(*f, *x)).sum();
    let wn: f64 = nodal.iter().zip(q).map(|(f, x)| vec3::dot(*f, *x)).sum();
    let ws: f64 = step
        .face_forces
        .iter()
        .zip(&d)
        .map(|(f, x)| vec3::norm(*f) * vec3::norm(*x))
        .sum();
    checks.at_most(&format!("{id} work"), (wf - wn).abs() / ws, 1e-12);
}

// ---- the heavy registered runs (ten periods) --------------------------------------------------------------------------------

/// A matching run: I1, F1, EN, G1, G2, FP, ED (and NC1 at ratio 4, Aitken).
fn matching_record(checks: &mut Checks, ratio: f64, method: Method) -> Run {
    let case = Case::new(ratio, NF, method);
    let run = run_case(&case, STEPS);
    let tag = format!("ratio {ratio}, matching, {}", method_id(method));
    println!("== {tag}");
    iteration_checks(checks, &format!("I1 {tag}"), &run, STEPS, 10);
    let p = run.p;
    let m = model(&p, STEPS, true);
    let (om_m, nc) = run.omega();
    let (om_model, nc_model) = measured_omega(p.dt, &m.tip);
    let delta_dt = om_model / p.omega - 1.0;
    let delta_visc = 0.0;
    println!(
        "{tag}: omega {:.9} rad/s, Omega_m {om_m:.9} ({nc} crossings), Omega_model {om_model:.9} ({nc_model}), delta_dt {delta_dt:e}",
        p.omega
    );
    checks.at_most(
        &format!("F1 {tag} after the estimate"),
        (om_m / p.omega - 1.0 - delta_dt - delta_visc).abs(),
        2e-2,
    );
    checks.at_most(&format!("F1 {tag} raw"), (om_m / p.omega - 1.0).abs(), 2e-2);
    checks.at_most(&format!("FP {tag}"), (om_m / om_model - 1.0).abs(), 2.5e-4);
    let n = run.steps().min(STEPS);
    if n == STEPS {
        checks.at_most(&format!("EN {tag}"), run.balance(1, STEPS), 1e-4);
        let ratio_run = run.energy[STEPS] / run.energy[1];
        let ratio_model = m.energy[STEPS] / m.energy[1];
        println!("{tag}: E_1001/E_1 {ratio_run:e}, model {ratio_model:e}");
        checks.at_most(&format!("ED {tag}"), (ratio_run / ratio_model - 1.0).abs(), 2e-2);
        checks.at_most(&format!("G1 {tag}"), run.eta_max(1, STEPS), 1e-6);
        let growth = run.eta_max(902, 1001) / run.eta_max(101, 200);
        println!(
            "{tag}: eta second period {:e}, last period {:e}",
            run.eta_max(101, 200),
            run.eta_max(902, 1001)
        );
        checks.at_most(&format!("G2 {tag}"), growth, 10.0);
    } else {
        checks.holds(&format!("EN, ED, G1, G2 {tag}"), false, "the run did not finish");
    }
    if ratio == 4.0 && method == Method::Aitken {
        let nolag = model(&p, STEPS, false);
        let (om_nolag, _) = measured_omega(p.dt, &nolag.tip);
        checks.at_least(
            "NC1 ratio 4 against the model without the lag",
            (om_m / om_nolag - 1.0).abs(),
            5e-4,
        );
    }
    run
}

/// A 3:2 run against its matching run: I2/I2b, EN, G2, G3, MC, TR, M2.
fn nonmatching_record(checks: &mut Checks, ratio: f64, method: Method, matching: &Run) {
    let case = Case::new(ratio, 6, method);
    let mut parts = parts(&case);
    let run = run_parts(&mut parts, STEPS);
    let tag = format!("ratio {ratio}, 3:2, {}", method_id(method));
    println!("== {tag}");
    let (id, cap) = if method == Method::Aitken {
        ("I2", 20)
    } else {
        ("I2b", 10)
    };
    iteration_checks(checks, &format!("{id} {tag}"), &run, STEPS, cap);
    if run.steps() == STEPS {
        checks.at_most(&format!("EN {tag}"), run.balance(1, STEPS), 1e-4);
        println!(
            "{tag}: eta second period {:e}, last period {:e}",
            run.eta_max(101, 200),
            run.eta_max(902, 1001)
        );
        checks.at_most(
            &format!("G2 {tag}"),
            run.eta_max(902, 1001) / run.eta_max(101, 200),
            10.0,
        );
        checks.at_most(&format!("G3 {tag}"), run.eta_max(1, STEPS), 1e-2);
        map_conservation(checks, &format!("MC {tag}"), &parts.map, run.last.as_ref().unwrap());
        println!(
            "TR {tag}: max |q_yz|/max |q_x| = {:e} (record)",
            run.transverse.0 / run.transverse.1
        );
        let (om_n, nc) = run.omega();
        let (om_0, _) = matching.omega();
        println!("{tag}: Omega_3:2 {om_n:.9} ({nc} crossings), Omega_matching {om_0:.9}");
        checks.below(&format!("M2 {tag}"), (om_n / om_0 - 1.0).abs(), 2e-3);
    } else {
        checks.holds(&format!("EN, G2, G3, MC, M2 {tag}"), false, "the run did not finish");
    }
}

#[test]
#[ignore = "heavy record (ADR 0434): ten periods, ratio 0.5, matching and 3:2"]
fn r05_ten_periods_matching_and_nonmatching() {
    let mut checks = Checks::new();
    let a = matching_record(&mut checks, 0.5, Method::Aitken);
    let _ = matching_record(&mut checks, 0.5, Method::LeastSquares);
    nonmatching_record(&mut checks, 0.5, Method::Aitken, &a);
    checks.finish();
}

#[test]
#[ignore = "heavy record (ADR 0434): ten periods, ratio 4, matching and 3:2, Aitken"]
fn r4_aitken_ten_periods_matching_and_nonmatching() {
    let mut checks = Checks::new();
    let a = matching_record(&mut checks, 4.0, Method::Aitken);
    nonmatching_record(&mut checks, 4.0, Method::Aitken, &a);
    checks.finish();
}

#[test]
#[ignore = "heavy record (ADR 0434): ten periods, ratio 4, matching and 3:2, least squares"]
fn r4_least_squares_ten_periods_matching_and_nonmatching() {
    let mut checks = Checks::new();
    let l = matching_record(&mut checks, 4.0, Method::LeastSquares);
    nonmatching_record(&mut checks, 4.0, Method::LeastSquares, &l);
    checks.finish();
}

// ---- fast tests ---------------------------------------------------------------------------------------------------------

#[test]
#[cfg_attr(debug_assertions, ignore = "run in release")]
fn lc_the_loose_coupling_grows_at_ratio_4() {
    let run = run_case(&Case::new(4.0, NF, Method::Loose), 10);
    let mut checks = Checks::new();
    let stop = run.steps();
    println!("LC: stopped after step {stop}, signal {:?}", run.signal);
    if let Some(e) = &run.error {
        println!("LC: error {e}");
    }
    let reached = run.signal.last().unwrap().abs() >= 10.0 * U;
    checks.holds(
        "LC reaches 10 U within 10 steps",
        reached && stop <= 10,
        &format!("step {stop}"),
    );
    // geometric mean of |H_{n+1}/H_n| over steps 2..stop
    let growth = if stop > 2 {
        (run.held[stop - 1].abs() / run.held[1].abs()).powf(1.0 / (stop - 2) as f64)
    } else {
        0.0
    };
    println!("LC held forces {:?}", run.held);
    checks.at_least("LC growth per step", growth, 5.0);
    checks.finish();
}

fn qf1(method: Method, nu: f64) -> Run {
    let mut case = Case::new(4.0, NF, method);
    case.nu = nu;
    run_case(&case, 25)
}

#[test]
#[cfg_attr(debug_assertions, ignore = "run in release")]
fn qf1_vs_fast_matching_run_against_the_model() {
    let mut checks = Checks::new();
    let p = params(4.0);
    let m = model(&p, 25, true);
    let mut aitken_signal = Vec::new();
    for method in [Method::Aitken, Method::LeastSquares] {
        let run = qf1(method, NU);
        let tag = format!("QF1 {}", method_id(method));
        iteration_checks(&mut checks, &tag, &run, 25, 10);
        let n = run.signal.len().min(m.tip.len());
        let dev = (0..n).fold(0.0f64, |w, i| w.max((run.signal[i] - m.tip[i]).abs())) / U;
        checks.at_most(&format!("{tag} signal against the model"), dev, 1e-3);
        if run.steps() == 25 {
            checks.at_most(&format!("{tag} energy balance"), run.balance(1, 25), 1e-6);
            checks.at_most(&format!("{tag} eta"), run.eta_max(1, 25), 1e-6);
        }
        if method == Method::Aitken {
            aitken_signal = run.signal.clone();
        }
    }
    let viscous = qf1(Method::Aitken, 1e-2);
    let n = viscous.signal.len().min(aitken_signal.len());
    let dev = (0..n).fold(0.0f64, |w, i| w.max((viscous.signal[i] - aitken_signal[i]).abs())) / U;
    checks.holds("VS run complete", viscous.steps() == 25 && n == 26, "25 steps");
    checks.at_most("VS viscosity x 100", dev, 1e-6);
    checks.finish();
}

#[test]
#[cfg_attr(debug_assertions, ignore = "run in release")]
fn qf2_fast_nonmatching_run() {
    let mut checks = Checks::new();
    for method in [Method::Aitken, Method::LeastSquares] {
        let mut parts = parts(&Case::new(4.0, 6, method));
        let run = run_parts(&mut parts, 10);
        let tag = format!("QF2 {}", method_id(method));
        let cap = if method == Method::Aitken { 20 } else { 10 };
        iteration_checks(&mut checks, &tag, &run, 10, cap);
        if let Some(last) = &run.last {
            map_conservation(&mut checks, &format!("{tag} MC"), &parts.map, last);
        }
        if run.steps() == 10 {
            checks.at_most(&format!("{tag} eta"), run.eta_max(1, 10), 1e-2);
        }
    }
    checks.finish();
}

#[test]
#[cfg_attr(debug_assertions, ignore = "run in release")]
fn s0_m0_the_initial_state_and_the_matching_map() {
    let mut checks = Checks::new();
    for m in [NF, 6] {
        let parts = parts(&Case::new(4.0, m, Method::Aitken));
        let p = parts.p;
        let a0 = parts.solid.acceleration().iter().fold(0.0f64, |w, x| w.max(x.abs()));
        checks.at_most(
            &format!("S0 m = {m} max |a0|/(omega^2 U)"),
            a0 / (p.omega * p.omega * U),
            1e-9,
        );
        let f0 = parts.fluid.forces().unwrap();
        checks.holds(
            &format!("S0 m = {m} initial fluid force"),
            f0.iter().flatten().all(|&x| x == 0.0),
            "zero",
        );
        let mapped = parts
            .initial_points
            .iter()
            .fold(0.0f64, |w, d| w.max((d[0] - U).abs()).max(d[1].abs()).max(d[2].abs()));
        checks.at_most(&format!("S0 m = {m} mapped initial displacement"), mapped / U, 1e-15);
        let pts = parts.fluid.flow.geom.mesh.points();
        let placed = parts.wall_ids.iter().zip(&parts.initial_points).all(|(&q, d)| {
            let r = parts.mesh0.points()[q];
            pts[q][0].to_bits() == (r[0] + d[0]).to_bits() && pts[q][1] == r[1] && pts[q][2] == r[2]
        });
        checks.holds(
            &format!("S0 m = {m} fluid wall at the displaced face"),
            placed,
            "bit for bit",
        );
        println!("S0 m = {m}: release force norm {:e}", norm3(&parts.release));
        if m == NF {
            let s = &parts.solid;
            let (rows, _) = interface_rows(
                &s.node_coordinates(),
                &s.elements(),
                &parts.fluid.point_coordinates(),
                &MapSettings::projection(1e-9),
            )
            .unwrap();
            let nodes = s.node_coordinates();
            let points = parts.fluid.point_coordinates();
            let mut worst = 0.0f64;
            for (row, x) in rows.iter().zip(&points) {
                for &(i, w) in row {
                    let same = vec3::norm(vec3::sub(nodes[i], *x)) < 1e-12;
                    worst = worst.max((w - if same { 1.0 } else { 0.0 }).abs());
                }
                worst = worst.max((row.iter().map(|r| r.1).sum::<f64>() - 1.0).abs());
                let found = row.iter().any(|&(i, _)| vec3::norm(vec3::sub(nodes[i], *x)) < 1e-12);
                if !found {
                    worst = worst.max(1.0);
                }
            }
            checks.at_most("M0 matching rows against the identity", worst, 1e-14);
        } else {
            let s = &parts.solid;
            let (rows, _) = interface_rows(
                &s.node_coordinates(),
                &s.elements(),
                &parts.fluid.point_coordinates(),
                &MapSettings::bilinear_projection(1e-9),
            )
            .unwrap();
            let worst = rows
                .iter()
                .fold(0.0f64, |w, r| w.max((r.iter().map(|x| x.1).sum::<f64>() - 1.0).abs()));
            checks.at_most("MC 3:2 row sums", worst, 1e-12);
        }
    }
    checks.finish();
}

// ---- failures (FS1, FS2) ----------------------------------------------------------------------------------------------------

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

/// Every value of the fluid participant's state, as bits (as `cosim_f139.rs`).
fn fluid_bits(f: &FlowWall) -> Vec<u64> {
    let mut out = Vec::new();
    out.extend(f.flow.u.values.iter().flatten().map(|x| x.to_bits()));
    out.extend(bits(&f.flow.p.values));
    out.extend(bits(&f.flow.phi));
    out.push(f.flow.time.to_bits());
    out.extend(f.flow.geom.mesh.points().iter().flatten().map(|x| x.to_bits()));
    out.extend(
        f.flow
            .u
            .boundary_values(&f.flow.geom)
            .iter()
            .flatten()
            .map(|x| x.to_bits()),
    );
    out.extend(bits(&f.wall_dpdn));
    out.push(f.flow.history.len() as u64);
    out
}

fn solid_bits(s: &DynamicSolid) -> (u64, Vec<u8>, Vec<u64>) {
    (
        s.time().to_bits(),
        s.state().to_bytes(),
        s.forces().iter().flatten().map(|x| x.to_bits()).collect(),
    )
}

fn step_bits(s: &CoupledStep) -> Vec<u64> {
    let mut out = vec![s.iterations as u64, s.converged as u64];
    out.extend(bits(&s.residuals));
    out.extend(bits(&s.residual_norms));
    out.extend(bits(&s.factors));
    for v in [&s.node_forces, &s.node_displacements, &s.face_forces] {
        out.extend(v.iter().flatten().map(|x| x.to_bits()));
    }
    out
}

#[test]
#[cfg_attr(debug_assertions, ignore = "run in release")]
fn fs1_a_failing_solid_leaves_both_at_the_start_of_the_step() {
    let mut case = Case::new(4.0, NF, Method::Aitken);
    case.table_end = 3;
    let mut parts = parts(&case);
    let mut checks = Checks::new();
    let run = run_parts(&mut parts, 3);
    checks.holds(
        "FS1 steps 1 to 3",
        run.steps() == 3 && run.error.is_none() && run.converged.iter().all(|&c| c),
        "converged",
    );
    let (fb, sb) = (fluid_bits(&parts.fluid), solid_bits(&parts.solid));
    let p = parts.p;
    let e = coupled_step(&mut parts.fluid, &mut parts.solid, &parts.map, p.dt, &parts.coupling).unwrap_err();
    println!("FS1: {e}");
    checks.holds(
        "FS1 located",
        e.participant == "solid" && e.iteration == 1 && e.message.contains("load table"),
        &format!("participant '{}', iteration {}", e.participant, e.iteration),
    );
    checks.holds("FS1 fluid restored", fluid_bits(&parts.fluid) == fb, "bit for bit");
    checks.holds("FS1 solid restored", solid_bits(&parts.solid) == sb, "bit for bit");
    checks.finish();
}

#[test]
#[cfg_attr(debug_assertions, ignore = "run in release")]
fn fs2_a_failing_fluid_leaves_both_at_the_start_of_the_step() {
    let case = Case::new(4.0, NF, Method::Aitken);
    let calls = Rc::new(Cell::new(0usize));
    let fail_at: Rc<Cell<Option<usize>>> = Rc::new(Cell::new(None));
    let (c, f) = (calls.clone(), fail_at.clone());
    let wrap = move |mut inner: MeshMotion| -> MeshMotion {
        let (c, f) = (c.clone(), f.clone());
        Box::new(move |d: &[Vec3]| {
            c.set(c.get() + 1);
            if f.get() == Some(c.get()) {
                return Err("mesh motion: a failure injected by FS2".into());
            }
            inner(d)
        })
    };
    let mut parts = parts_with(&case, &wrap);
    let mut checks = Checks::new();
    let run = run_parts(&mut parts, 2);
    checks.holds(
        "FS2 steps 1 and 2",
        run.steps() == 2 && run.error.is_none(),
        "converged",
    );
    let (fb, sb) = (fluid_bits(&parts.fluid), solid_bits(&parts.solid));
    let p = parts.p;
    fail_at.set(Some(calls.get() + 2));
    let e = coupled_step(&mut parts.fluid, &mut parts.solid, &parts.map, p.dt, &parts.coupling).unwrap_err();
    println!("FS2: {e}");
    checks.holds(
        "FS2 located",
        e.participant == "fluid" && e.iteration == 2,
        &format!("participant '{}', iteration {}", e.participant, e.iteration),
    );
    checks.holds("FS2 fluid restored", fluid_bits(&parts.fluid) == fb, "bit for bit");
    checks.holds("FS2 solid restored", solid_bits(&parts.solid) == sb, "bit for bit");
    fail_at.set(None);
    let again = coupled_step(&mut parts.fluid, &mut parts.solid, &parts.map, p.dt, &parts.coupling).unwrap();
    let mut reference = parts_with(&case, &|m| m);
    let _ = run_parts(&mut reference, 2);
    let r = coupled_step(
        &mut reference.fluid,
        &mut reference.solid,
        &reference.map,
        p.dt,
        &reference.coupling,
    )
    .unwrap();
    checks.holds(
        "FS2 the repeated step equals the unfailed run",
        step_bits(&again) == step_bits(&r)
            && fluid_bits(&parts.fluid) == fluid_bits(&reference.fluid)
            && solid_bits(&parts.solid) == solid_bits(&reference.solid),
        "bit for bit",
    );
    checks.finish();
}

// ---- the helper (H1, H2, NC2) ----------------------------------------------------------------------------------------------

#[test]
fn h1_nc2_the_channel_motion() {
    let mut checks = Checks::new();
    let mesh = channel(NF);
    let motion = ChannelMotion::new(&mesh, WALL, 0, L_F, 1e-9).unwrap();
    let ids = FlowWall::point_ids(&mesh, WALL).unwrap();
    let pts = mesh.points();
    let d: Vec<Vec3> = ids
        .iter()
        .map(|&q| {
            let r = pts[q];
            [1e-3 * (1.0 + r[1] + 2.0 * r[2]), 0.3e-3, -0.2e-3]
        })
        .collect();
    let moved = motion.positions(&d).unwrap();
    let wall_ok = ids.iter().zip(&d).all(|(&q, dq)| {
        moved[q][0].to_bits() == (pts[q][0] + dq[0]).to_bits()
            && moved[q][1].to_bits() == pts[q][1].to_bits()
            && moved[q][2].to_bits() == pts[q][2].to_bits()
    });
    checks.holds("H1 wall points", wall_ok, "p + d_x e_x bit for bit");
    let outlet = (0..pts.len())
        .filter(|&i| pts[i][0] == L_S + L_F)
        .all(|i| moved[i].iter().zip(&pts[i]).all(|(a, b)| a.to_bits() == b.to_bits()));
    checks.holds("H1 outlet points", outlet, "unchanged bit for bit");
    // every point: linear weight along x of its own line (the mid point of the channel is not a mesh point on the graded
    // axis, so the weight is checked at every point against the formula)
    let mut worst = 0.0f64;
    for (i, p) in pts.iter().enumerate() {
        let k = ids
            .iter()
            .position(|&q| (pts[q][1] - p[1]).abs() < 1e-9 && (pts[q][2] - p[2]).abs() < 1e-9)
            .unwrap();
        let w = (1.0 - (p[0] - L_S).abs() / L_F).max(0.0);
        worst = worst.max((moved[i][0] - (p[0] + w * d[k][0])).abs());
    }
    checks.at_most("H1 linear weight (m)", worst, 1e-15);
    // the point at half the channel length: on a uniform channel (the graded axis has no point there)
    let uniform = {
        let m = box_mesh([4, 2, 2], [L_F, W, W], [Grading::Uniform; 3]).unwrap();
        let points = m.points().iter().map(|p| [p[0] + L_S, p[1], p[2]]).collect();
        moved_mesh(&m, points).unwrap()
    };
    let mu = ChannelMotion::new(&uniform, WALL, 0, L_F, 1e-9).unwrap();
    let pu = mu.positions(&vec![[2e-3, 0.0, 0.0]; mu.n_wall_points()]).unwrap();
    let mut half = 0usize;
    let mut half_worst = 0.0f64;
    for (a, b) in pu.iter().zip(uniform.points()) {
        if ((b[0] - L_S) - 0.5 * L_F).abs() < 1e-12 {
            half += 1;
            half_worst = half_worst.max((a[0] - b[0] - 1e-3).abs());
        }
    }
    checks.holds("H1 points at half the length", half > 0, &format!("{half} points"));
    checks.at_most("H1 half length moved by d_x/2 (m)", half_worst, 1e-15);
    let transverse_moved = moved
        .iter()
        .zip(pts)
        .any(|(a, b)| a[1].to_bits() != b[1].to_bits() || a[2].to_bits() != b[2].to_bits());
    checks.holds("H1 no transverse motion", !transverse_moved, "y and z unchanged");
    let only_transverse: Vec<Vec3> = d.iter().map(|v| [0.0, v[1], v[2]]).collect();
    let still = motion
        .positions(&only_transverse)
        .unwrap()
        .iter()
        .zip(pts)
        .all(|(a, b)| a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits()));
    checks.holds("NC2 transverse displacement only", still, "no point moves");
    checks.finish();
}

#[test]
fn h2_the_channel_motion_refuses() {
    let mut checks = Checks::new();
    let mesh = channel(NF);
    let cases: [(&str, Result<ChannelMotion, String>, &str); 6] = [
        (
            "unknown patch",
            ChannelMotion::new(&mesh, "nowhere", 0, L_F, 1e-9),
            "no patch",
        ),
        ("axis 3", ChannelMotion::new(&mesh, WALL, 3, L_F, 1e-9), "axis"),
        ("length 0", ChannelMotion::new(&mesh, WALL, 0, 0.0, 1e-9), "length"),
        (
            "length NaN",
            ChannelMotion::new(&mesh, WALL, 0, f64::NAN, 1e-9),
            "length",
        ),
        ("tolerance 0", ChannelMotion::new(&mesh, WALL, 0, L_F, 0.0), "tolerance"),
        ("lines", ChannelMotion::new(&mesh, "ymin", 0, L_F, 1e-9), "on its line"),
    ];
    for (what, r, needle) in cases {
        let msg = r.err().unwrap_or_default();
        println!("H2 {what}: {msg}");
        checks.holds(&format!("H2 {what}"), msg.contains(needle), "refused, named");
    }
    let motion = ChannelMotion::new(&mesh, WALL, 0, L_F, 1e-9).unwrap();
    let n = motion.n_wall_points();
    let count = motion.positions(&vec![vec3::ZERO; n - 1]).err().unwrap_or_default();
    checks.holds("H2 count", count.contains("displacements for"), &count);
    let mut bad = vec![vec3::ZERO; n];
    bad[3][0] = f64::NAN;
    let nan = motion.positions(&bad).err().unwrap_or_default();
    checks.holds("H2 non-finite", nan.contains("non-finite"), &nan);
    checks.finish();
}
