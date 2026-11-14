//! F-421 (ADR 0446): aerodynamic damping and added mass of a mode by prescribed motion of the wall (the energy method).
//! The harmonic fit, the work per cycle and the periodicity refusal on synthetic histories; the piston on F-401's open
//! column (added mass, damping ratio against the time-step and viscous estimate); the piston driving a plane channel against
//! the oscillating-channel impedance `A i w rho L_f/(1 - tanh(kappa)/kappa)` after the extrapolation in the time step, and its
//! low-frequency limit `12 mu L_f/h^2`; the damping ratio of the bar on the open column from this method and from the decay
//! of F-401's coupled run.
//!
//! The limits, set-ups and mutations were registered in ADR 0446 before this file existed. The bar, the column and the
//! coupled run are those of `si_fsi_piston.rs` (ADR 0434; copied, that file is not edited).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fem::builders::hex_box;
use rcfd_fem::transient::{LoadHistory, LoadTable, TransientOptions};
use rcfd_fem::{Constraint, Load, Material, SolidModel};
use rcfd_fv::bc::{FixedValue, Symmetry};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::cosim::{coupled_step, CouplingSettings, FlowWall, Fluid, InterfaceMap, MapSettings, Structure};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::motion::moved_mesh;
use rcfd_mesh::Mesh;
use rcfd_post::solid_aerodamp::{
    drive_mode, periodic_response, trapezoid_work, Harmonic, ModeHistory, ModeMotion, PeriodicResponse,
};
use rcfd_post::solid_dynamic::DynamicSolid;
use rcfd_post::solid_fsi::ChannelMotion;
use std::f64::consts::PI;

const TOL_PERIODIC: f64 = 1e-6;

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

// ---- synthetic histories (H0, H1, H2, P1, DR) ----------------------------------------------------------------------------

const SQ_M: f64 = 0.7;
const SQ_P: f64 = 1.3;
const SQ_Q: f64 = -0.45;
const S_Q0: f64 = 2.5e-3;
const S_W: f64 = 7.0;
const S_N: usize = 64;

fn synthetic(t: f64) -> f64 {
    SQ_M + SQ_P * (S_W * t).sin() + SQ_Q * (S_W * t).cos() + 0.3 * (2.0 * S_W * t).sin() - 0.2 * (3.0 * S_W * t).cos()
}

fn s_dt() -> f64 {
    2.0 * PI / (S_W * S_N as f64)
}

/// The discrete Fourier sums of cycle `c` (0-based) of samples at `t_n = n dt`, `n = 1..`: `(Q_p, Q_q)`.
fn dft(values: &[f64], c: usize) -> (f64, f64) {
    let dt = s_dt();
    let (mut p, mut q) = (0.0, 0.0);
    for n in c * S_N..(c + 1) * S_N {
        let t = (n + 1) as f64 * dt;
        p += values[n] * (S_W * t).sin();
        q += values[n] * (S_W * t).cos();
    }
    (2.0 * p / S_N as f64, 2.0 * q / S_N as f64)
}

#[test]
fn h0_h1_h2_synthetic_harmonics_and_work() {
    let mut c = Checks::new();
    let dt = s_dt();
    let amp = SQ_P.hypot(SQ_Q);
    let values: Vec<f64> = (1..=3 * S_N).map(|n| synthetic(n as f64 * dt)).collect();
    for cyc in 0..3 {
        let times: Vec<f64> = (cyc * S_N + 1..=(cyc + 1) * S_N).map(|n| n as f64 * dt).collect();
        let v = &values[cyc * S_N..(cyc + 1) * S_N];
        let h = Harmonic::fit(&times, v, S_W).unwrap();
        let e = (h.in_phase - SQ_P).abs().max((h.quadrature - SQ_Q).abs()) / amp;
        c.at_most(&format!("H1 cycle {}", cyc + 1), e, 1e-10);
        let w = trapezoid_work(&times, v, S_Q0, S_W, dt);
        c.at_most(
            &format!("H2 cycle {}", cyc + 1),
            (w - PI * S_Q0 * h.quadrature).abs() / (PI * S_Q0 * amp),
            1e-10,
        );
    }
    let r = periodic_response(&values, S_N, S_Q0, S_W, TOL_PERIODIC).unwrap();
    let e = (r.harmonic.in_phase - SQ_P)
        .abs()
        .max((r.harmonic.quadrature - SQ_Q).abs())
        / amp;
    c.at_most("H1 response", e, 1e-10);
    c.at_most(
        "H2 response work_per_cycle",
        (r.work_per_cycle() - PI * S_Q0 * SQ_Q).abs() / (PI * S_Q0 * amp),
        1e-10,
    );
    println!("H1 response cycle change {:e}", r.cycle_change);
    // H0: 64 samples over 0.7 period, mean and first harmonic only
    let dt7 = 0.7 * 2.0 * PI / S_W / S_N as f64;
    let times: Vec<f64> = (1..=S_N).map(|n| 0.3 + n as f64 * dt7).collect();
    let v: Vec<f64> = times
        .iter()
        .map(|t| SQ_M + SQ_P * (S_W * t).sin() + SQ_Q * (S_W * t).cos())
        .collect();
    let h = Harmonic::fit(&times, &v, S_W).unwrap();
    let scale = SQ_M.abs().max(amp);
    let e = (h.mean - SQ_M)
        .abs()
        .max((h.in_phase - SQ_P).abs())
        .max((h.quadrature - SQ_Q).abs())
        / scale;
    c.at_most("H0 partial period", e, 1e-10);
    c.finish();
}

#[test]
fn p1_non_periodic_history_refused_with_the_change() {
    let mut c = Checks::new();
    let dt = s_dt();
    let period = 2.0 * PI / S_W;
    let values: Vec<f64> = (1..=3 * S_N)
        .map(|n| {
            let t = n as f64 * dt;
            synthetic(t) + 0.1 * (-t / period).exp() * (S_W * t).cos()
        })
        .collect();
    let (p2, q2) = dft(&values, 1);
    let (p3, q3) = dft(&values, 2);
    let expected = (p3 - p2).hypot(q3 - q2) / p3.hypot(q3);
    println!("P1 change computed by the test {expected:e}");
    match periodic_response(&values, S_N, S_Q0, S_W, TOL_PERIODIC) {
        Ok(r) => c.holds(
            "P1 refused",
            false,
            &format!("accepted with change {:e}", r.cycle_change),
        ),
        Err(m) => {
            println!("P1 message: {m}");
            c.holds("P1 refused", m.contains("not periodic"), "refused as not periodic");
            let number = m
                .split(" is ")
                .nth(1)
                .and_then(|s| s.split_whitespace().next())
                .and_then(|s| s.parse::<f64>().ok());
            match number {
                Some(x) => c.at_most("P1 change in the message", (x - expected).abs() / expected, 1e-12),
                None => c.holds("P1 change in the message", false, "no number after ' is '"),
            }
        }
    }
    match periodic_response(&values, S_N, S_Q0, S_W, 1.0) {
        Ok(r) => c.at_most(
            "P1/NC3 accepted at tolerance 1, change",
            (r.cycle_change - expected).abs() / expected,
            1e-12,
        ),
        Err(m) => c.holds("P1/NC3 accepted at tolerance 1", false, &m),
    }
    let periodic: Vec<f64> = (1..=3 * S_N).map(|n| synthetic(n as f64 * dt)).collect();
    match periodic_response(&periodic, S_N, S_Q0, S_W, TOL_PERIODIC) {
        Ok(r) => c.at_most("P1 periodic history accepted, change", r.cycle_change, 1e-14),
        Err(m) => c.holds("P1 periodic history accepted", false, &m),
    }
    c.finish();
}

// ---- F-401's bar and open column (ADR 0434 D4; ADR 0446 D5, D7) -------------------------------------------------------------

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
const WALL: &str = "xmin";

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
    kl: f64,
    omega: f64,
    dt: f64,
    rho_f: f64,
    p: f64,
}

fn params(ratio: f64) -> Params {
    let kl = root(1.0 / ratio);
    let omega = kl * (E / RHO_S).sqrt() / L_S;
    Params {
        kl,
        omega,
        dt: 2.0 * PI / omega / 100.0,
        rho_f: ratio * RHO_S * L_S / L_F,
        p: E * W * W * U / L_S,
    }
}

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

/// The solid at the static displaced state, at rest (ADR 0434 D4).
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

fn column_mesh() -> Mesh {
    let mesh = box_mesh(
        [NXF, NF, NF],
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

/// Fluid at rest, `p = 0` at `xmax`, on `mesh0` moved to `points`; `walls` get zero velocity, `slips` the symmetry condition.
fn flow(mesh0: &Mesh, points: Vec<Vec3>, nu: f64, walls: &[&str], slips: &[&str]) -> PimpleSolver {
    let mesh = moved_mesh(mesh0, points).unwrap();
    let geom = std::sync::Arc::new(FvGeometry::new(std::sync::Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    for name in slips {
        u.set_bc(&geom, name, Box::new(Symmetry)).unwrap();
    }
    for name in walls {
        u.set_bc(&geom, name, Box::new(FixedValue::vector(vec3::ZERO))).unwrap();
    }
    let mut pr = ScalarField::uniform("p", &geom, 0.0);
    pr.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    PimpleSolver::new(geom, u, pr, settings(nu))
}

const SLIPS4: [&str; 4] = ["ymin", "ymax", "zmin", "zmax"];

/// The open column at the wall's reference position and the matching map of the bar's interface.
fn column_at_rest(p: &Params) -> (FlowWall, InterfaceMap, DynamicSolid) {
    let solid = solid(p, STEPS + 2);
    let mesh0 = column_mesh();
    let motion = ChannelMotion::new(&mesh0, WALL, 0, L_F, 1e-9).unwrap();
    let fluid = FlowWall::new(
        flow(&mesh0, mesh0.points().to_vec(), NU, &[], &SLIPS4),
        WALL,
        &mesh0,
        p.rho_f,
        NU,
        motion.into_motion(),
    )
    .unwrap();
    let (map, _) = InterfaceMap::build(
        &solid.node_coordinates(),
        &solid.elements(),
        &fluid.point_coordinates(),
        fluid.face_points(),
        &MapSettings::projection(1e-9),
    )
    .unwrap();
    (fluid, map, solid)
}

/// The piston mode on the column at F-401's frequency and step (ADR 0446 D5).
fn column_run(ratio: f64) -> (Params, ModeHistory) {
    let p = params(ratio);
    let (mut fluid, map, _) = column_at_rest(&p);
    let motion = ModeMotion {
        shape: vec![[1.0, 0.0, 0.0]; map.n_nodes()],
        amplitude: U,
        omega: p.omega,
        steps_per_cycle: 100,
        cycles: 3,
    };
    let h = drive_mode(&mut fluid, &map, &motion).unwrap();
    (p, h)
}

fn g_factor(wdt: f64) -> f64 {
    let s = 2.0 * (wdt / 2.0).sin() / wdt;
    s * s
}

#[test]
fn c1_c4_open_column() {
    let mut c = Checks::new();
    let (p, h) = column_run(4.0);
    let a = W * W;
    let m_col = p.rho_f * a * L_F;
    let r = match h.response(TOL_PERIODIC) {
        Ok(r) => r,
        Err(m) => panic!("C1: the column's history was refused: {m}"),
    };
    let wdt = p.omega * p.dt;
    println!(
        "column: w {:.6} rad/s, dt {:e} s, Q_p {:e}, Q_q {:e}, mean {:e}",
        p.omega, p.dt, r.harmonic.in_phase, r.harmonic.quadrature, r.harmonic.mean
    );
    let ma = r.added_mass();
    c.at_most("C1 |m_a/(rho_f A L_f) - 1|", (ma / m_col - 1.0).abs(), 2e-2);
    let zeta = r.damping_ratio(ma);
    let zeta_dt = wdt.tan() / 2.0;
    println!("C2 zeta_col {zeta:e}, tan(w dt)/2 {zeta_dt:e}");
    c.at_most("C2 zeta_col - (tan(w dt)/2 + 1e-4)", zeta - (zeta_dt + 1e-4), 0.0);
    c.at_most("C3 |zeta_col - tan(w dt)/2|", (zeta - zeta_dt).abs(), 1e-4);
    c.at_most(
        "C3 |m_a/(rho_f A L_f g cos(w dt)) - 1|",
        (ma / (m_col * g_factor(wdt) * wdt.cos()) - 1.0).abs(),
        1e-4,
    );
    c.at_most("C4 cycle change", r.cycle_change, 1e-6);
    let n = h.motion.steps_per_cycle;
    let fw = h.face_work[3 * n] - h.face_work[2 * n];
    println!(
        "C4 record: face work over the last cycle {fw:e}, pi q0 Q_q {:e}, ratio - 1 {:e}",
        r.work_per_cycle(),
        fw / r.work_per_cycle() - 1.0
    );
    c.finish();
}

// ---- the bar on the column: the method and the decay of F-401's coupled run (B1, B2, BZ, BC) -------------------------------

/// `phi^T M phi` with the consistent mass of the bar, `phi = sin(k x)/sin(k L_s)` on the `x` unknowns.
fn modal_mass(p: &Params) -> f64 {
    let model = bar(p);
    let m = rcfd_fem::mass::consistent(&model).unwrap();
    let dofs = model.dof_map();
    let k = p.kl / L_S;
    let mut phi = vec![0.0; m.n];
    for (node, x) in model.mesh().nodes().iter().enumerate() {
        if let Some(e) = dofs.equation(node, 0) {
            phi[e] = (k * x[0]).sin() / p.kl.sin();
        }
    }
    let mut y = vec![0.0; m.n];
    m.matvec(&phi, &mut y);
    phi.iter().zip(&y).map(|(a, b)| a * b).sum()
}

fn interface_x_mean(s: &DynamicSolid, model: &SolidModel) -> f64 {
    let dofs = model.dof_map();
    let u = s.displacement();
    let nodes = s.interface().nodes();
    nodes
        .iter()
        .map(|&n| dofs.equation(n, 0).map_or(0.0, |e| u[e]))
        .sum::<f64>()
        / nodes.len() as f64
}

fn measured_omega(dt: f64, x: &[f64]) -> f64 {
    let mut c = Vec::new();
    for i in 1..x.len() {
        if (x[i - 1] > 0.0) != (x[i] > 0.0) && x[i] != x[i - 1] {
            c.push(dt * ((i - 1) as f64 + x[i - 1] / (x[i - 1] - x[i])));
        }
    }
    let n = c.len();
    assert!(n >= 3, "too few crossings: {n}");
    2.0 * PI / (2.0 * (c[n - 1] - c[0]) / (n - 1) as f64)
}

/// F-401's coupled run (matching wall, Aitken): `(zeta_decay, Omega_m, all converged, largest iterations)`.
fn coupled_decay(ratio: f64) -> (f64, f64, bool, usize) {
    let p = params(ratio);
    let mut solid = solid(&p, STEPS + 2);
    let model = bar(&p);
    let mesh0 = column_mesh();
    let motion = ChannelMotion::new(&mesh0, WALL, 0, L_F, 1e-9).unwrap();
    let probe = FlowWall::new(
        flow(&mesh0, mesh0.points().to_vec(), NU, &[], &SLIPS4),
        WALL,
        &mesh0,
        p.rho_f,
        NU,
        motion.clone().into_motion(),
    )
    .unwrap();
    let (map, _) = InterfaceMap::build(
        &solid.node_coordinates(),
        &solid.elements(),
        &probe.point_coordinates(),
        probe.face_points(),
        &MapSettings::projection(1e-9),
    )
    .unwrap();
    let dofs = model.dof_map();
    let u = solid.displacement();
    let q0: Vec<Vec3> = solid
        .interface()
        .nodes()
        .iter()
        .map(|&n| {
            let at = |c: usize| dofs.equation(n, c).map_or(0.0, |e| u[e]);
            [at(0), at(1), at(2)]
        })
        .collect();
    let start = motion.positions(&map.displacement_to_points(&q0).unwrap()).unwrap();
    let mut fluid = FlowWall::new(
        flow(&mesh0, start, NU, &[], &SLIPS4),
        WALL,
        &mesh0,
        p.rho_f,
        NU,
        motion.into_motion(),
    )
    .unwrap();
    let nodes = solid.node_coordinates();
    let mut release = vec![vec3::ZERO; nodes.len()];
    for el in solid.elements() {
        let d1 = vec3::sub(nodes[el[2]], nodes[el[0]]);
        let d2 = vec3::sub(nodes[el[3]], nodes[el[1]]);
        let a = 0.5 * vec3::norm(vec3::cross(d1, d2));
        for &i in &el {
            release[i][0] += p.p / (W * W) * a / 4.0;
        }
    }
    let mut coupling = CouplingSettings::aitken(0.5);
    coupling.tolerance = 1e-6;
    coupling.force_scale = release.iter().flatten().map(|x| x * x).sum::<f64>().sqrt();
    coupling.max_iterations = 10;
    let energy = |s: &DynamicSolid| s.integrator().kinetic_energy().unwrap() + s.integrator().strain_energy().unwrap();
    let mut signal = vec![interface_x_mean(&solid, &model)];
    let mut e = vec![energy(&solid)];
    let (mut ok, mut largest) = (true, 0);
    for _ in 0..STEPS {
        match coupled_step(&mut fluid, &mut solid, &map, p.dt, &coupling) {
            Ok(step) => {
                ok &= step.converged;
                largest = largest.max(step.iterations);
            }
            Err(err) => panic!("BC: the coupled run failed: {err}"),
        }
        signal.push(interface_x_mean(&solid, &model));
        e.push(energy(&solid));
    }
    let om = measured_omega(p.dt, &signal);
    let zeta = -(e[STEPS] / e[1]).ln() / (2.0 * om * (STEPS - 1) as f64 * p.dt);
    println!(
        "ratio {ratio}: E_1001/E_1 {:.6}, Omega_m/omega - 1 {:e}",
        e[STEPS] / e[1],
        om / p.omega - 1.0
    );
    (zeta, om, ok, largest)
}

#[test]
fn b1_b2_bar_damping_by_the_method_and_by_the_decay() {
    let mut c = Checks::new();
    for ratio in [0.5, 4.0] {
        let (p, h) = column_run(ratio);
        let r = match h.response(TOL_PERIODIC) {
            Ok(r) => r,
            Err(m) => panic!("B1: the column's history was refused at ratio {ratio}: {m}"),
        };
        let ms = modal_mass(&p);
        let ma = r.added_mass();
        let zeta_m = r.damping_ratio(ms + ma);
        println!(
            "B1 ratio {ratio}: m_s {ms:.4} kg, m_a {ma:.4} kg, c {:e} N s/m, zeta_method {zeta_m:e}",
            r.damping()
        );
        let (zeta_d, om, ok, largest) = coupled_decay(ratio);
        println!("B2 ratio {ratio}: zeta_decay {zeta_d:e}");
        c.holds(
            &format!("BC ratio {ratio} converged"),
            ok && largest <= 10,
            &format!("every step converged, largest {largest}"),
        );
        c.at_most(
            &format!("BC ratio {ratio} |Omega_m/omega - 1|"),
            (om / p.omega - 1.0).abs(),
            2e-2,
        );
        c.at_most(
            &format!("BZ ratio {ratio} |zeta_method/zeta_decay - 1|"),
            (zeta_m / zeta_d - 1.0).abs(),
            5e-2,
        );
    }
    c.finish();
}

// ---- the plane channel (W5, W1, LF, QF, O, PR, NC1, NC2) -------------------------------------------------------------------

const H: f64 = 0.02;
const RHO: f64 = 1000.0;
const OMEGA: f64 = 2.0 * PI;

#[derive(Debug, Clone, Copy)]
struct ChannelCase {
    nx: usize,
    ny: usize,
    lf: f64,
    expansion: f64,
    wo: f64,
    cycles: usize,
}

impl ChannelCase {
    fn main(wo: f64, cycles: usize) -> Self {
        ChannelCase {
            nx: 120,
            ny: 40,
            lf: 100.0 * H,
            expansion: 21.0,
            wo,
            cycles,
        }
    }
    fn nu(&self) -> f64 {
        OMEGA * (H / 2.0).powi(2) / (self.wo * self.wo)
    }
}

fn channel_run(case: &ChannelCase, n: usize) -> ModeHistory {
    let mesh0 = box_mesh(
        [case.nx, case.ny, 1],
        [case.lf, H, H],
        [Grading::Expansion(case.expansion), Grading::Uniform, Grading::Uniform],
    )
    .unwrap();
    let nu = case.nu();
    let motion = ChannelMotion::new(&mesh0, WALL, 0, case.lf, 1e-9).unwrap();
    let mut fluid = FlowWall::new(
        flow(
            &mesh0,
            mesh0.points().to_vec(),
            nu,
            &["ymin", "ymax"],
            &["zmin", "zmax"],
        ),
        WALL,
        &mesh0,
        RHO,
        nu,
        motion.into_motion(),
    )
    .unwrap();
    let np = fluid.point_coordinates().len();
    let rows = (0..np).map(|i| vec![(i, 1.0)]).collect();
    let map = InterfaceMap::new(rows, np, fluid.face_points()).unwrap();
    let motion = ModeMotion {
        shape: vec![[1.0, 0.0, 0.0]; np],
        amplitude: 1e-3 * H,
        omega: OMEGA,
        steps_per_cycle: n,
        cycles: case.cycles,
    };
    drive_mode(&mut fluid, &map, &motion).unwrap()
}

#[derive(Debug, Clone, Copy)]
struct Cx(f64, f64);

impl Cx {
    fn div(self, o: Cx) -> Cx {
        let d = o.0 * o.0 + o.1 * o.1;
        Cx((self.0 * o.0 + self.1 * o.1) / d, (self.1 * o.0 - self.0 * o.1) / d)
    }
    fn exp(self) -> Cx {
        let r = self.0.exp();
        Cx(r * self.1.cos(), r * self.1.sin())
    }
}

/// `A i w rho L_f/(1 - tanh(kappa)/kappa)`, `kappa = (h/2) sqrt(i w/nu)` (ADR 0446 D3).
fn impedance_formula(case: &ChannelCase) -> Cx {
    let r = (H / 2.0) * (OMEGA / case.nu()).sqrt();
    let kappa = Cx(r * (PI / 4.0).cos(), r * (PI / 4.0).sin());
    let e = Cx(-2.0 * kappa.0, -2.0 * kappa.1).exp();
    let tanh = Cx(1.0 - e.0, -e.1).div(Cx(1.0 + e.0, e.1));
    let t = tanh.div(kappa);
    let a = H * H;
    Cx(0.0, a * OMEGA * RHO * case.lf).div(Cx(1.0 - t.0, -t.1))
}

fn response_of(c: &mut Checks, id: &str, h: &ModeHistory) -> Option<PeriodicResponse> {
    match h.response(TOL_PERIODIC) {
        Ok(r) => {
            c.at_most(&format!("PR {id} cycle change"), r.cycle_change, TOL_PERIODIC);
            Some(r)
        }
        Err(m) => {
            c.holds(&format!("PR {id}"), false, &m);
            None
        }
    }
}

/// The three runs of a channel case: `Z(100), Z(200), Z(400)` (`None` if refused), and the `N = 200` history.
fn channel_levels(c: &mut Checks, tag: &str, case: &ChannelCase) -> (Vec<Option<Cx>>, ModeHistory) {
    let mut z = Vec::new();
    let mut h200 = None;
    for n in [100, 200, 400] {
        let t0 = std::time::Instant::now();
        let h = channel_run(case, n);
        let r = response_of(c, &format!("{tag} N {n}"), &h);
        let zz = r.map(|r| {
            let (re, im) = r.impedance();
            Cx(re, im)
        });
        println!("{tag} N {n}: Z {zz:?} ({:.1} s)", t0.elapsed().as_secs_f64());
        z.push(zz);
        if n == 200 {
            h200 = Some(h);
        }
    }
    (z, h200.unwrap())
}

fn order(a: f64, b: f64, c: f64) -> f64 {
    ((a - b).abs() / (b - c).abs()).log2()
}

/// W5 and W1 (and their companions): the extrapolated impedance against the formula.
fn womersley_record(wo: f64, cycles: usize, id: &str) {
    let mut c = Checks::new();
    let case = ChannelCase::main(wo, cycles);
    let zf = impedance_formula(&case);
    println!("{id}: Wo {wo}, nu {:e}, formula Z {zf:?}", case.nu());
    let (z, h200) = channel_levels(&mut c, id, &case);
    if let [Some(z1), Some(z2), Some(z4)] = z[..] {
        let ext = Cx(2.0 * z4.0 - z2.0, 2.0 * z4.1 - z2.1);
        println!("{id}: Z_ext {ext:?}");
        for (n, zz) in [(100, z1), (200, z2), (400, z4)] {
            println!(
                "{id} N {n}: Re/Re_f - 1 {:e}, Im/Im_f - 1 {:e}",
                zz.0 / zf.0 - 1.0,
                zz.1 / zf.1 - 1.0
            );
        }
        c.at_most(&format!("{id} real"), (ext.0 / zf.0 - 1.0).abs(), 3e-2);
        c.at_most(&format!("{id} imaginary"), (ext.1 / zf.1 - 1.0).abs(), 3e-2);
        for (part, p) in [
            ("real", order(z1.0, z2.0, z4.0)),
            ("imaginary", order(z1.1, z2.1, z4.1)),
        ] {
            c.at_least(&format!("O {id} order {part}"), p, 0.8);
            c.at_most(&format!("O {id} order {part}"), p, 1.2);
        }
        if wo == 5.0 {
            c.at_least("NC1 |Re Z_200/Re Z - 1|", (z2.0 / zf.0 - 1.0).abs(), 5e-2);
        }
    } else {
        c.holds(id, false, "a run was refused");
    }
    if wo == 5.0 {
        let n = h200.motion.steps_per_cycle;
        let refused = periodic_response(
            &h200.generalised[..2 * n],
            n,
            h200.motion.amplitude,
            h200.motion.omega,
            TOL_PERIODIC,
        );
        match refused {
            Ok(r) => c.holds("NC2", false, &format!("accepted with change {:e}", r.cycle_change)),
            Err(m) => c.holds("NC2", m.contains("not periodic"), &m),
        }
    }
    c.finish();
}

#[test]
#[ignore = "heavy record (ADR 0446 W5): run once with --ignored"]
fn w5_womersley_5_record() {
    womersley_record(5.0, 6, "W5");
}

#[test]
#[ignore = "heavy record (ADR 0446 W1): run once with --ignored"]
fn w1_womersley_1_record() {
    womersley_record(1.0, 3, "W1");
}

#[test]
#[ignore = "heavy record (ADR 0446 LF): run once with --ignored"]
fn lf_low_frequency_limit_record() {
    let mut c = Checks::new();
    let case = ChannelCase::main(0.25, 3);
    let zf = impedance_formula(&case);
    let limit = 12.0 * RHO * case.nu() * case.lf * H * H / (H * H);
    println!(
        "LF: Wo 0.25, nu {:e}, formula Z {zf:?}, 12 mu L_f A/h^2 {limit:e} (formula/limit - 1 {:e})",
        case.nu(),
        zf.0 / limit - 1.0
    );
    let (z, _) = channel_levels(&mut c, "LF", &case);
    if let [Some(z1), Some(z2), Some(z4)] = z[..] {
        let ext = 2.0 * z4.0 - z2.0;
        println!("LF: Re Z_ext {ext:e}, Im Z_ext {:e}", 2.0 * z4.1 - z2.1);
        c.at_most("LF real", (ext / limit - 1.0).abs(), 3e-2);
        let p = order(z1.0, z2.0, z4.0);
        c.at_least("O LF order real", p, 0.8);
        c.at_most("O LF order real", p, 1.2);
    } else {
        c.holds("LF", false, "a run was refused");
    }
    c.finish();
}

#[test]
fn qf_fast_channel() {
    let mut c = Checks::new();
    let case = ChannelCase {
        nx: 30,
        ny: 8,
        lf: 20.0 * H,
        expansion: 5.5,
        wo: 1.0,
        cycles: 3,
    };
    let zf = impedance_formula(&case);
    let mut z = Vec::new();
    for n in [50, 100] {
        let h = channel_run(&case, n);
        if let Some(r) = response_of(&mut c, &format!("QF N {n}"), &h) {
            let (re, im) = r.impedance();
            z.push(Cx(re, im));
        }
    }
    if let [z1, z2] = z[..] {
        let ext = Cx(2.0 * z2.0 - z1.0, 2.0 * z2.1 - z1.1);
        println!("QF: formula {zf:?}, Z_50 {z1:?}, Z_100 {z2:?}, Z_ext {ext:?}");
        c.at_most("QF real", (ext.0 / zf.0 - 1.0).abs(), 0.2);
        c.at_most("QF imaginary", (ext.1 / zf.1 - 1.0).abs(), 0.2);
    }
    c.finish();
}

#[test]
fn dr_refusals() {
    let mut c = Checks::new();
    let p = params(4.0);
    let (mut fluid, map, _) = column_at_rest(&p);
    let good = ModeMotion {
        shape: vec![[1.0, 0.0, 0.0]; map.n_nodes()],
        amplitude: U,
        omega: p.omega,
        steps_per_cycle: 100,
        cycles: 3,
    };
    let mut cases: Vec<(&str, ModeMotion, &str)> = Vec::new();
    let mut m = good.clone();
    m.shape.pop();
    cases.push(("shape length", m, "interface nodes"));
    let mut m = good.clone();
    m.shape[3][1] = f64::NAN;
    cases.push(("non-finite shape", m, "non-finite mode shape"));
    let mut m = good.clone();
    m.steps_per_cycle = 4;
    cases.push(("N = 4", m, "steps per cycle"));
    let mut m = good.clone();
    m.cycles = 1;
    cases.push(("C = 1", m, "2 cycles"));
    let mut m = good.clone();
    m.amplitude = 0.0;
    cases.push(("amplitude 0", m, "amplitude"));
    let mut m = good.clone();
    m.omega = f64::NAN;
    cases.push(("frequency NaN", m, "frequency"));
    for (name, motion, needle) in cases {
        match drive_mode(&mut fluid, &map, &motion) {
            Ok(_) => c.holds(&format!("DR driver {name}"), false, "accepted"),
            Err(e) => c.holds(&format!("DR driver {name}"), e.contains(needle), &e),
        }
    }
    let fits: [(&str, Vec<f64>, Vec<f64>, &str); 3] = [
        ("length mismatch", vec![0.0, 1.0, 2.0], vec![1.0, 2.0], "times for"),
        ("two samples", vec![0.0, 1.0], vec![1.0, 2.0], "at least 3"),
        (
            "non-finite",
            vec![0.0, 1.0, 2.0],
            vec![1.0, f64::INFINITY, 2.0],
            "non-finite",
        ),
    ];
    for (name, t, v, needle) in fits {
        match Harmonic::fit(&t, &v, 1.0) {
            Ok(_) => c.holds(&format!("DR fit {name}"), false, "accepted"),
            Err(e) => c.holds(&format!("DR fit {name}"), e.contains(needle), &e),
        }
    }
    let one: Vec<f64> = (1..=S_N).map(|n| synthetic(n as f64 * s_dt())).collect();
    match periodic_response(&one, S_N, S_Q0, S_W, TOL_PERIODIC) {
        Ok(_) => c.holds("DR response C = 1", false, "accepted"),
        Err(e) => c.holds("DR response C = 1", e.contains("2 whole cycles"), &e),
    }
    let two: Vec<f64> = (1..=2 * S_N).map(|n| synthetic(n as f64 * s_dt())).collect();
    match periodic_response(&two, S_N, S_Q0, S_W, 0.0) {
        Ok(_) => c.holds("DR response tolerance 0", false, "accepted"),
        Err(e) => c.holds("DR response tolerance 0", e.contains("tolerance"), &e),
    }
    c.finish();
}
