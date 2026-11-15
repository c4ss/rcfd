//! F-690 (ADR 0619): the stagnation acceptance at the rounding bound of the Newton residual, opt-in by
//! `NewtonOptions::stagnation`. Checks ST1 (S3 of ADR 0479 amendment 2), ST2 (the Shell3 strip step at the zero crossing of
//! ADR 0604 amendment 1, NC1b), ST3 (the pendulum of ADR 0479, heavy record), ST4 (the spinning bar with the Newmark step of
//! ADR 0514 N1e: the opt-in does not change an attempt that converges by the relative test) and ST5 (off by default).
//! Helpers copied from `sa_nl_rounding_floor.rs`, `sa_nl_inertia_reference_after_f571.rs` and `sa_emc.rs` (not edited).

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::mass::consistent;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::nl_transient::{Integrator as NlIntegrator, NlTransientOptions};
use rcfd_fem::nonlinear::{solve_with, Contribution, DriverExtensions, NewtonOptions};
use rcfd_fem::transient::{LoadHistory, LoadTable};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

/// Measured values against their limits; every value is printed; a non-finite value is a miss; the test fails at the end
/// if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} not <= {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

fn inf_norm(x: &[f64]) -> f64 {
    x.iter().fold(0.0f64, |m, v| m.max(v.abs()))
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn norm3(a: [f64; 3]) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

fn sub3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn bits(x: &[f64]) -> Vec<u64> {
    x.iter().map(|v| v.to_bits()).collect()
}

/// The driver defaults with the stagnation acceptance on.
fn stagnation() -> NewtonOptions {
    NewtonOptions::new(1).stagnation(true)
}

/// ADR 0604's iterations of S3 with the driver defaults (ADR 0619 reading 2, measured on the base).
const S3_ITERATIONS_ADR_0604: usize = 118;
/// ST1b: 98 + 10 (ADR 0619 reading 3).
const S3_TOTAL_LIMIT: usize = 108;
/// The registered corrections of a step at the floor (c1).
const CORRECTIONS_LIMIT: usize = 10;
/// ST3: half of ADR 0604's 37367 iterations of the pendulum, rounded down.
const PENDULUM_TOTAL_LIMIT: usize = 18683;

const E_STEEL: f64 = 2.0e11;
const RHO: f64 = 7850.0;

fn t_lin() -> f64 {
    2.0 * PI * (RHO / (3.0 * E_STEEL)).sqrt()
}

// --------------------------------------------------------------------------------------------- S3 of ADR 0479 (ST1)

/// The free Hex8 of ADR 0479 amendment 2 (S3) with its start velocity.
fn s3_model() -> (SolidModel, Vec<f64>) {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [1, 1, 1]).unwrap();
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", Material::isotropic_elastic(E_STEEL, 0.0, RHO, 0.0).unwrap())
        .unwrap();
    let dofs = model.dof_map();
    let n = dofs.n_equations();
    let mut v0 = vec![0.0; n];
    for i in 0..model.mesh().nodes().len() {
        for (comp, w) in [0.3, 0.2, 0.1].iter().enumerate() {
            v0[dofs.equation(i, comp).unwrap()] = *w;
        }
    }
    for &i in model.mesh().node_set("xmax").unwrap() {
        v0[dofs.equation(i, 0).unwrap()] += 10.0;
    }
    (model, v0)
}

/// A run of S3: the error of the first failed step, the displacement history (step 0 included), the largest `|P_n -
/// P_0|/|P_0|`, the total halvings, the iterations per step and the final relative residual per step.
struct S3Run {
    error: Option<String>,
    history: Vec<Vec<f64>>,
    dp: f64,
    halvings: usize,
    iterations: Vec<usize>,
    residuals: Vec<f64>,
}

fn s3_run(options: NlTransientOptions) -> S3Run {
    let (model, v0) = s3_model();
    let n = model.dof_map().n_equations();
    let mut it = NlIntegrator::new(&model, &[], &options).unwrap();
    it.start(0.0, &vec![0.0; n], &v0).unwrap();
    let (p0, _) = it.momentum().unwrap();
    let mut run = S3Run {
        error: None,
        history: vec![it.displacement().unwrap()],
        dp: 0.0,
        halvings: 0,
        iterations: Vec::new(),
        residuals: Vec::new(),
    };
    for k in 0..50 {
        match it.step() {
            Ok(r) => {
                run.halvings += r.halvings;
                run.iterations.push(r.iterations);
                run.residuals.push(r.residual);
            }
            Err(e) => {
                run.error = Some(format!("step {}: {e}", k + 1));
                break;
            }
        }
        run.history.push(it.displacement().unwrap());
        run.dp = run.dp.max(norm3(sub3(it.momentum().unwrap().0, p0)));
    }
    run.dp /= norm3(p0);
    run
}

#[test]
fn st1_s3_free_hex8_with_the_stagnation_acceptance() {
    let mut c = Checks::new();
    let dt = t_lin() / 200.0;
    let run = s3_run(NlTransientOptions::new(dt).newton(stagnation()));
    let base = s3_run(NlTransientOptions::new(dt));
    match &run.error {
        None => c.holds("ST1a S3 every step converges", true, "50 of 50 steps"),
        Some(e) => c.holds("ST1a S3 every step converges", false, e),
    }
    c.at_most("ST1a S3 max |P_n - P_0|/|P_0|", run.dp, 1e-11);
    c.at_most("ST1a S3 total halvings", run.halvings as f64, 0.0);
    let total: usize = run.iterations.iter().sum();
    let total_base: usize = base.iterations.iter().sum();
    println!("ST1 iterations per step: {:?}", run.iterations);
    println!("ST1 base (ADR 0604) iterations per step: {:?}", base.iterations);
    println!("ST1 final relative residuals: {:?}", run.residuals);
    c.holds(
        "ST1b (set-up) the base run measures ADR 0604's total",
        total_base == S3_ITERATIONS_ADR_0604,
        &format!("{total_base} against {S3_ITERATIONS_ADR_0604}"),
    );
    c.at_most("ST1b S3 total iterations", total as f64, S3_TOTAL_LIMIT as f64);
    println!(
        "ST1b S3 total iterations {total} against the literal half of ADR 0604's: {} (recorded)",
        S3_ITERATIONS_ADR_0604 / 2
    );
    // The steps ADR 0604 accepted at the bound: those of the base whose final residual is above the tolerance.
    let floor: Vec<usize> = (0..base.residuals.len())
        .filter(|&i| base.residuals[i] > 1e-10)
        .collect();
    println!(
        "ST1 steps accepted at the bound by ADR 0604 (1-based): {:?}",
        floor.iter().map(|i| i + 1).collect::<Vec<_>>()
    );
    let floor_iters: usize = floor
        .iter()
        .map(|&i| run.iterations.get(i).copied().unwrap_or(usize::MAX / 64))
        .sum();
    let floor_base: usize = floor.iter().map(|&i| base.iterations[i]).sum();
    println!("ST1c ADR 0604's iterations of those steps: {floor_base}");
    c.holds(
        "ST1c (set-up) the base has a step at the bound",
        !floor.is_empty(),
        &format!("{}", floor.len()),
    );
    c.at_most(
        "ST1c S3 iterations of the steps ADR 0604 accepted at the bound",
        floor_iters as f64,
        10.0,
    );
    let worst_floor = floor
        .iter()
        .map(|&i| run.iterations.get(i).copied().unwrap_or(usize::MAX / 64))
        .max()
        .unwrap_or(0);
    c.at_most(
        "ST1d S3 corrections of a step at the floor",
        worst_floor as f64,
        CORRECTIONS_LIMIT as f64,
    );
    let first = floor.first().copied().unwrap_or(base.residuals.len());
    let same = (0..=first).all(|i| i < run.history.len() && bits(&run.history[i]) == bits(&base.history[i]));
    c.holds(
        "ST1e S3 the steps before the first floor step have ADR 0604's bits",
        same,
        &format!("steps 0 to {first}"),
    );
    let mut newton = NewtonOptions::new(1);
    newton.residual_tolerance = 1e-6;
    let loose = s3_run(NlTransientOptions::new(dt).newton(newton));
    if run.history.len() == loose.history.len() && loose.error.is_none() {
        let scale = loose.history.iter().fold(0.0f64, |m, u| m.max(inf_norm(u)));
        let d = run.history.iter().zip(&loose.history).fold(0.0f64, |m, (x, y)| {
            m.max(x.iter().zip(y).fold(0.0f64, |mm, (p, q)| mm.max((p - q).abs())))
        });
        c.at_most(
            "ST1f S3 against tolerance 1e-6, max |du|_inf / max |u|_inf",
            d / scale,
            1e-9,
        );
    } else {
        c.holds("ST1f S3 histories of equal length", false, "a run stopped");
    }
    c.finish();
}

// ------------------------------------------------------------------- the Shell3 strip at its zero crossing (ST2, NC1b)

const N_S: usize = 16;
const L_S: f64 = 12.0;
const B_S: f64 = 1.0;
const T_S: f64 = 0.1;
const E_S: f64 = 1.2e6;
const RHO_S: f64 = 1.0;

/// A grid of `nx x ny` cells on `[0, a] x [0, b]` at z = 0, nodes in rows of constant y (`j (nx + 1) + i`).
fn grid(a: f64, b: f64, nx: usize, ny: usize) -> (Vec<Vec3>, Vec<[usize; 4]>) {
    let idx = |i: usize, j: usize| j * (nx + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            nodes.push([a * i as f64 / nx as f64, b * j as f64 / ny as f64, 0.0]);
        }
    }
    let mut el = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            el.push([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    (nodes, el)
}

/// The cantilever strip of ADR 0522 D6 cut into 32 Shell3 (ADR 0545 `strip3`), clamped at `x = 0`.
fn strip3() -> SolidModel {
    let (nodes, cells) = grid(L_S, B_S, N_S, 1);
    let n = nodes.len();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = cells.iter().flat_map(|&[a, b, c, d]| [a, b, c, a, c, d]).collect();
    mesh.add_block(ElementKind::Shell3, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(T_S).unwrap()).unwrap();
    mesh.add_element_set("all", (0..2 * cells.len()).collect()).unwrap();
    mesh.add_node_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("root", vec![0, N_S + 1]).unwrap();
    mesh.add_node_set("tip", vec![N_S, 2 * N_S + 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E_S, 0.0, RHO_S, 0.0).unwrap())
        .unwrap();
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("root", comp, 0.0)).unwrap();
    }
    m
}

/// The start of ADR 0545 NT1: the model, `f_1` and `v0 = omega a phi/phi_z,tip`.
fn strip_start() -> (SolidModel, f64, Vec<f64>) {
    let m = strip3();
    let mut mo = ModalOptions::new(1);
    mo.max_iterations = 2000;
    mo.tolerance = 1e-8;
    let modal = modal_analysis(&m, &mo).unwrap();
    let f1 = modal.frequencies[0];
    let qz = m.dof_map().equation(N_S, 2).expect("tip u_z");
    let phi = &modal.modes[0];
    let omega = 2.0 * PI * f1;
    let v0: Vec<f64> = phi.iter().map(|x| omega * 1e-3 * L_S * x / phi[qz]).collect();
    (m, f1, v0)
}

#[test]
fn st2_strip_step_at_the_zero_crossing_with_the_stagnation_acceptance() {
    let mut c = Checks::new();
    let (m, f1, v0) = strip_start();
    let n = v0.len();
    let hf = 1.0 / (40.0 * f1) / 1024.0;
    let mut it = NlIntegrator::new(&m, &[], &NlTransientOptions::new(hf).newton(stagnation())).unwrap();
    it.start(0.0, &vec![0.0; n], &v0).unwrap();
    match it.step() {
        Ok(rep) => {
            println!(
                "ST2 step: {} halvings, {} iterations, residual {:e}",
                rep.halvings, rep.iterations, rep.residual
            );
            c.holds(
                "ST2 accepted with 0 halvings",
                rep.halvings == 0,
                &format!("{} halvings", rep.halvings),
            );
            c.at_most(
                "ST2 corrections of the step at the floor",
                rep.iterations as f64,
                CORRECTIONS_LIMIT as f64,
            );
            c.at_most("ST2 final relative residual", rep.residual, 1e-7);
        }
        Err(e) => c.holds("ST2 accepted with 0 halvings", false, &format!("refused: {e}")),
    }
    c.finish();
}

// ------------------------------------------------------------------------------- the pendulum of ADR 0479 (ST3)

const L_BAR: f64 = 1.0;
const B_BAR: f64 = 0.02;
const G: f64 = 9.81;

fn pendulum_model() -> SolidModel {
    let mut mesh = hex_box([0.0, -B_BAR / 2.0, -B_BAR / 2.0], [L_BAR, B_BAR, B_BAR], [10, 2, 2]).unwrap();
    let nodes = mesh.nodes().to_vec();
    let pin: Vec<usize> = (0..nodes.len())
        .filter(|&i| near(nodes[i][0], 0.0) && near(nodes[i][2], 0.0))
        .collect();
    let mid: Vec<usize> = pin.iter().copied().filter(|&i| near(nodes[i][1], 0.0)).collect();
    assert_eq!((pin.len(), mid.len()), (3, 1), "pin line");
    mesh.add_node_set("pin", pin).unwrap();
    mesh.add_node_set("pin_mid", mid).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.0e9, 0.3, RHO, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("pin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("pin", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("pin_mid", 1, 0.0)).unwrap();
    m.add_load(Load::gravity([0.0, 0.0, -G])).unwrap();
    m
}

#[test]
#[ignore = "heavy record of ADR 0619 ST3 (1940 pendulum steps, most at the rounding floor); run once with --ignored; the fast tests on the same code path are st1 and st2"]
fn st3_pendulum_with_the_stagnation_acceptance() {
    let model = pendulum_model();
    let dofs = model.dof_map();
    let nodes = model.mesh().nodes().to_vec();
    let tip: Vec<usize> = (0..nodes.len())
        .filter(|&i| near(nodes[i][0], L_BAR) && near(nodes[i][1], 0.0) && near(nodes[i][2], 0.0))
        .collect();
    assert_eq!(tip.len(), 1, "tip node");
    let (qx, qz) = (dofs.equation(tip[0], 0).unwrap(), dofs.equation(tip[0], 2).unwrap());
    let loads = [LoadHistory::model_loads(&model, LoadTable::constant(1.0, 0.0, 10.0).unwrap()).unwrap()];
    let gravity = loads[0].pattern.clone();
    let dt = 0.005;
    let mut it = NlIntegrator::new(&model, &loads, &NlTransientOptions::new(dt).newton(stagnation())).unwrap();
    let zero = vec![0.0; dofs.n_equations()];
    it.start(0.0, &zero, &zero).unwrap();
    let steps = 1940;
    let mass = RHO * L_BAR * B_BAR * B_BAR;
    let swing = mass * G * L_BAR / 2.0;
    let energy = |it: &NlIntegrator| {
        let u = it.displacement().unwrap();
        it.kinetic_energy().unwrap() + it.strain_energy().unwrap() - dot(&gravity, &u)
    };
    let angle = |it: &NlIntegrator| {
        let u = it.displacement().unwrap();
        (L_BAR + u[qx]).atan2(-u[qz])
    };
    let e0 = energy(&it);
    let mut psi_prev = angle(&it);
    let mut crossings = Vec::new();
    let mut e_dev = 0.0f64;
    let (mut halvings, mut iterations, mut worst) = (0usize, 0usize, 0usize);
    let mut largest = 0.0f64;
    let mut c = Checks::new();
    let mut failed = None;
    for k in 1..=steps {
        match it.step() {
            Ok(r) => {
                halvings += r.halvings;
                iterations += r.iterations;
                worst = worst.max(r.iterations);
                largest = largest.max(r.residual);
            }
            Err(e) => {
                failed = Some(format!("step {k}: {e}"));
                break;
            }
        }
        let psi = angle(&it);
        if (psi_prev > 0.0) != (psi > 0.0) {
            crossings.push((k - 1) as f64 * dt + dt * psi_prev / (psi_prev - psi));
        }
        psi_prev = psi;
        e_dev = e_dev.max((energy(&it) - e0).abs());
    }
    match &failed {
        None => c.holds("ST3 every step converges", true, "1940 of 1940 steps"),
        Some(e) => c.holds("ST3 every step converges", false, e),
    }
    println!("ST3 largest final relative residual of a step: {largest:e}");
    c.at_most("ST3 total iterations", iterations as f64, PENDULUM_TOTAL_LIMIT as f64);
    c.at_most("ST3 corrections of a step", worst as f64, CORRECTIONS_LIMIT as f64);
    let t_closed = 2.0 * PI * (2.0 * L_BAR / (3.0 * G)).sqrt() * 1.180340599016096;
    let n = crossings.len();
    c.holds("ST3 crossings of psi = 0 (at least 9)", n >= 9, &format!("{n}"));
    if n >= 2 {
        let t_fe = 2.0 * (crossings[n - 1] - crossings[0]) / (n - 1) as f64;
        c.at_most("ST3 period |T_FE - T|/T", (t_fe - t_closed).abs() / t_closed, 1e-2);
    } else {
        c.holds("ST3 period", false, "fewer than 2 crossings");
    }
    c.at_most("ST3 energy max |E - E_0|/(m g L/2)", e_dev / swing, 1e-3);
    c.at_most("ST3 total halvings", halvings as f64, 19.0);
    c.finish();
}

// ------------------------------------------------------------------ the spinning bar of ADR 0514 N1e (ST4)

/// The centrifugal force of the rotating frame, `-omega^2 M P (X + u - c)`, with its tangent (ADR 0479 D5).
struct Centrifugal {
    m: CsrMatrix,
    offset: Vec<Option<f64>>,
    omega: f64,
}

impl Contribution for Centrifugal {
    fn name(&self) -> &str {
        "centrifugal"
    }
    fn add(
        &self,
        u: &[f64],
        _load_factor: f64,
        force: &mut [f64],
        tangent: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::Result<()> {
        let w2 = self.omega * self.omega;
        let r: Vec<f64> = (0..u.len()).map(|j| self.offset[j].map_or(0.0, |x| x + u[j])).collect();
        let mut mr = vec![0.0; self.m.n];
        self.m.matvec(&r, &mut mr);
        for (f, x) in force.iter_mut().zip(&mr) {
            *f -= w2 * x;
        }
        for i in 0..self.m.n {
            for p in self.m.row_ptr[i]..self.m.row_ptr[i + 1] {
                let j = self.m.col[p];
                if self.offset[j].is_some() {
                    tangent.push((i, j, -w2 * self.m.val[p]));
                }
            }
        }
        Ok(())
    }
    fn in_residual_reference(&self) -> bool {
        true
    }
}

/// The free spinning bar of ADR 0479 D5 and its steady start (ADR 0514 S0e): the model, `u0`, `v0`.
fn spin() -> (SolidModel, Vec<f64>, Vec<f64>) {
    let omega = 2.0 * PI;
    let centre = [L_BAR / 2.0, 0.0, 0.0];
    let velocity = [0.5, 0.25, 0.1];
    let mesh = hex_box([0.0, -B_BAR / 2.0, -B_BAR / 2.0], [L_BAR, B_BAR, B_BAR], [10, 2, 2]).unwrap();
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", Material::isotropic_elastic(2.0e9, 0.3, RHO, 0.0).unwrap())
        .unwrap();
    let dofs = model.dof_map();
    let nodes = model.mesh().nodes().to_vec();
    let n = dofs.n_equations();
    let mut offset = vec![None; n];
    let mut prescribed = Vec::new();
    for (i, p) in nodes.iter().enumerate() {
        for comp in 0..2 {
            offset[dofs.equation(i, comp).unwrap()] = Some(p[comp] - centre[comp]);
        }
        if near(p[0], centre[0]) {
            prescribed.push(Prescribed::new(i, 0, 0.0));
        }
        if near(p[1], 0.0) {
            prescribed.push(Prescribed::new(i, 1, 0.0));
        }
        if near(p[2], 0.0) {
            prescribed.push(Prescribed::new(i, 2, 0.0));
        }
    }
    let spin = Centrifugal {
        m: consistent(&model).unwrap(),
        offset,
        omega,
    };
    let ext = DriverExtensions {
        contributions: vec![&spin],
        step_control: None,
    };
    let steady = solve_with(&model, &prescribed, &[], &NewtonOptions::new(1), ext).unwrap();
    let u0 = steady.result.displacement_values().to_vec();
    let mut v0 = vec![0.0; n];
    for (i, p) in nodes.iter().enumerate() {
        let rx = p[0] + u0[dofs.equation(i, 0).unwrap()] - centre[0];
        let ry = p[1] + u0[dofs.equation(i, 1).unwrap()] - centre[1];
        let w = [-omega * ry + velocity[0], omega * rx + velocity[1], velocity[2]];
        for (comp, wc) in w.iter().enumerate() {
            v0[dofs.equation(i, comp).unwrap()] = *wc;
        }
    }
    (model, u0, v0)
}

/// 600 Newmark steps of the spinning bar with the Newton options of ADR 0514 arithmetic 2 (residual `1e-7`): the steps done
/// and the final state bytes.
fn spin_run(model: &SolidModel, u0: &[f64], v0: &[f64], newton: NewtonOptions) -> (u64, Vec<u8>) {
    let mut it = NlIntegrator::new(model, &[], &NlTransientOptions::new(0.01).newton(newton)).unwrap();
    it.start(0.0, u0, v0).unwrap();
    let mut steps = 0;
    for _ in 0..600 {
        if it.step().is_err() {
            break;
        }
        steps += 1;
    }
    (steps, it.state().unwrap().to_bytes())
}

#[test]
fn st4_spinning_bar_newmark_bits_with_and_without_the_opt_in() {
    let mut c = Checks::new();
    let (model, u0, v0) = spin();
    let mut off = NewtonOptions::new(1);
    off.residual_tolerance = 1e-7;
    let on = off.clone().stagnation(true);
    let (n_off, b_off) = spin_run(&model, &u0, &v0, off);
    let (n_on, b_on) = spin_run(&model, &u0, &v0, on);
    c.holds(
        "ST4 steps done with and without the opt-in",
        n_off == 600 && n_on == 600,
        &format!("{n_off}, {n_on}"),
    );
    c.holds(
        "ST4 the final state bytes with the opt-in equal those without",
        b_on == b_off,
        &format!("{} bytes", b_on.len()),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------ the default (ST5)

#[test]
fn st5_the_stagnation_acceptance_is_off_by_default() {
    let mut c = Checks::new();
    c.holds(
        "ST5 NewtonOptions::new(1).stagnation",
        !NewtonOptions::new(1).stagnation,
        "false",
    );
    c.holds(
        "ST5 NlTransientOptions::new(dt).newton.stagnation",
        !NlTransientOptions::new(0.01).newton.stagnation,
        "false",
    );
    c.holds(
        "ST5 the builder switches it on",
        NewtonOptions::new(1).stagnation(true).stagnation,
        "true",
    );
    c.finish();
}
