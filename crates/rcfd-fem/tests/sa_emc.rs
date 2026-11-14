//! F-570 (ADR 0514): the energy-momentum conserving step (Simo and Tarnow) of `nl_transient`. Checks S0e to S3e (the free
//! spinning bar of ADR 0479 over ten turns), TP (the tolerance probe, a record), N1e, N2e (the trapezoidal step on the same
//! bar, negative controls), L1e (the linear limit against F-398), P1e, P2e (the pendulum), D1e, D2e (damping), R1e, R2e
//! (save and restore), T1e (threads), H1e (halving) and F1e (refusals) of the registration. Every aggregated value is
//! required finite before it enters a maximum.

use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::mass::consistent;
use rcfd_fem::material::{HyperelasticEnergy, MaterialAxes};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::nl_transient::{Integrator as NlIntegrator, NlTransientOptions, NlTransientState, StepKind};
use rcfd_fem::nonlinear::{solve_with, Contribution, DriverExtensions, NewtonOptions};
use rcfd_fem::transient::{Integrator, LoadHistory, LoadTable, Method, Rayleigh, TransientOptions};
use rcfd_fem::{BlockProperty, Constraint, FemError, Load, Material, SolidFormulation, SolidModel};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

/// Measured values against their limits; every value is printed; a non-finite value is a miss; the test fails at the end if
/// any missed.
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
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value >= limit;
        println!(
            "{name}: {value:.6e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} not >= {limit:e}"));
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

/// The running maximum of finite values; panics on a non-finite one (an `f64::max` would hide a `NaN`).
fn worst(acc: &mut f64, value: f64, what: &str) {
    assert!(value.is_finite(), "{what}: non-finite value {value}");
    *acc = acc.max(value);
}

fn key_of(e: &FemError) -> Option<&'static str> {
    match e {
        FemError::InvalidValue { key, .. } | FemError::FieldLength { key, .. } => Some(key),
        _ => None,
    }
}

fn refused_with<T>(r: rcfd_fem::Result<T>, key: &str, text: &str) -> (bool, String) {
    match r {
        Ok(_) => (false, "accepted".to_string()),
        Err(e) => {
            let s = e.to_string();
            (key_of(&e) == Some(key) && s.contains(text), s)
        }
    }
}

fn mv(m: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; m.n];
    m.matvec(x, &mut y);
    y
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

fn inf_norm(x: &[f64]) -> f64 {
    x.iter().fold(0.0f64, |m, v| {
        assert!(v.is_finite(), "non-finite value");
        m.max(v.abs())
    })
}

// ----------------------------------------------------------------------------------------------- the slender bar

const L_BAR: f64 = 1.0;
const B_BAR: f64 = 0.02;
const RHO: f64 = 7850.0;
const G: f64 = 9.81;

fn slender_mesh() -> rcfd_fem::mesh::SolidMesh {
    hex_box([0.0, -B_BAR / 2.0, -B_BAR / 2.0], [L_BAR, B_BAR, B_BAR], [10, 2, 2]).unwrap()
}

fn soft() -> Material {
    Material::isotropic_elastic(2.0e9, 0.3, RHO, 0.0).unwrap()
}

/// The Newton options of the bar (ADR 0514 arithmetic 2): residual `1e-7`, energy `1e-10`, 20 iterations, 5 halvings.
fn bar_newton() -> NewtonOptions {
    let mut o = NewtonOptions::new(1);
    o.residual_tolerance = 1e-7;
    o
}

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
        let mr = mv(&self.m, &r);
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

/// A contribution of zero force (for the refusal of F1e).
struct Zero;

impl Contribution for Zero {
    fn name(&self) -> &str {
        "zero"
    }
    fn add(&self, _: &[f64], _: f64, _: &mut [f64], _: &mut Vec<(usize, usize, f64)>) -> rcfd_fem::Result<()> {
        Ok(())
    }
}

struct Spin {
    model: SolidModel,
    u0: Vec<f64>,
    v0: Vec<f64>,
    steady_ok: bool,
}

/// The free spinning bar of ADR 0479 D5 and its steady start (S0e).
fn spin() -> Spin {
    let omega = 2.0 * PI;
    let centre = [L_BAR / 2.0, 0.0, 0.0];
    let velocity = [0.5, 0.25, 0.1];
    let mut model = SolidModel::new(slender_mesh());
    model.add_material("all", soft()).unwrap();
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
    let steady = solve_with(&model, &prescribed, &[], &NewtonOptions::new(1), ext);
    let steady_ok = steady.is_ok();
    let u0 = steady.unwrap().result.displacement_values().to_vec();
    let mut v0 = vec![0.0; n];
    for (i, p) in nodes.iter().enumerate() {
        let rx = p[0] + u0[dofs.equation(i, 0).unwrap()] - centre[0];
        let ry = p[1] + u0[dofs.equation(i, 1).unwrap()] - centre[1];
        let w = [-omega * ry + velocity[0], omega * rx + velocity[1], velocity[2]];
        for (comp, wc) in w.iter().enumerate() {
            v0[dofs.equation(i, comp).unwrap()] = *wc;
        }
    }
    Spin {
        model,
        u0,
        v0,
        steady_ok,
    }
}

fn spin_integrator(s: &Spin, options: &NlTransientOptions) -> NlIntegrator {
    let mut it = NlIntegrator::new(&s.model, &[], options).unwrap();
    it.start(0.0, &s.u0, &s.v0).unwrap();
    it
}

fn energy(it: &NlIntegrator) -> f64 {
    it.kinetic_energy().unwrap() + it.strain_energy().unwrap()
}

/// What a run of the spinning bar measured: the worst relative changes of `P`, `J`, `E`, the steps done, the error if any,
/// the largest final residual and the halvings.
struct SpinRun {
    dp: f64,
    dj: f64,
    de: f64,
    steps: u64,
    error: Option<String>,
    residual: f64,
    halvings: usize,
}

fn run_spin(s: &Spin, options: &NlTransientOptions, steps: u64) -> SpinRun {
    let mut it = spin_integrator(s, options);
    let (p0, j0) = it.momentum().unwrap();
    let e0 = energy(&it);
    let mut r = SpinRun {
        dp: 0.0,
        dj: 0.0,
        de: 0.0,
        steps: 0,
        error: None,
        residual: 0.0,
        halvings: 0,
    };
    for _ in 0..steps {
        match it.step() {
            Ok(rep) => {
                worst(&mut r.residual, rep.residual, "final residual");
                r.halvings += rep.halvings;
            }
            Err(e) => {
                r.error = Some(e.to_string());
                break;
            }
        }
        r.steps += 1;
        let (p, j) = it.momentum().unwrap();
        worst(&mut r.dp, norm3(sub3(p, p0)) / norm3(p0), "dP");
        worst(&mut r.dj, norm3(sub3(j, j0)) / norm3(j0), "dJ");
        worst(&mut r.de, (energy(&it) - e0).abs() / e0.abs(), "dE");
    }
    r
}

fn emc_options(newton: NewtonOptions) -> NlTransientOptions {
    NlTransientOptions::new(0.01).newton(newton).energy_momentum()
}

#[test]
fn s0e_s1e_s2e_s3e_spinning_bar_ten_turns() {
    let s = spin();
    let mut c = Checks::new();
    c.holds("S0e steady spin solve", s.steady_ok, "");
    let r = run_spin(&s, &emc_options(bar_newton()), 1000);
    println!(
        "spin, energy-momentum: {} steps, halvings {}, largest final residual {:e}, error {:?}",
        r.steps, r.halvings, r.residual, r.error
    );
    c.holds(
        "S0e 1000 steps converged",
        r.steps == 1000 && r.error.is_none(),
        &format!("{} steps, {:?}", r.steps, r.error),
    );
    c.at_most("S1e linear momentum max |P - P_0|/|P_0|", r.dp, 1e-10);
    c.at_most("S2e angular momentum max |J - J_0|/|J_0|", r.dj, 1e-6);
    c.at_most("S3e energy max |E - E_0|/|E_0|", r.de, 1e-6);
    c.finish();
}

#[test]
fn tp_tolerance_probe_record() {
    let s = spin();
    for tol in [1e-1, 1e-2, 1e-3, 1e-4, 1e-5, 1e-6] {
        let mut o = NewtonOptions::new(1);
        o.residual_tolerance = tol;
        o.energy_tolerance = 1.0;
        let r = run_spin(&s, &emc_options(o), 1000);
        let all = r.error.is_none() && r.dp <= 1e-10 && r.dj <= 1e-6 && r.de <= 1e-6;
        println!(
            "TP residual tolerance {tol:e}: {} steps, dP {:.3e}, dJ {:.3e}, dE {:.3e}, largest final residual {:.3e}, \
             halvings {}, error {:?}; all three limits {}",
            r.steps,
            r.dp,
            r.dj,
            r.de,
            r.residual,
            r.halvings,
            r.error,
            if all { "met" } else { "not met" }
        );
    }
}

#[test]
fn n1e_n2e_trapezoidal_negative_control() {
    let s = spin();
    let r = run_spin(&s, &NlTransientOptions::new(0.01).newton(bar_newton()), 600);
    println!("spin, Newmark: {} steps, error {:?}", r.steps, r.error);
    let mut c = Checks::new();
    c.at_least("N1e Newmark energy max |E - E_0|/|E_0| over 600 steps", r.de, 1e-6);
    c.at_least(
        "N2e Newmark angular momentum max |J - J_0|/|J_0| over 600 steps",
        r.dj,
        1e-6,
    );
    c.finish();
}

#[test]
fn r1e_r2e_save_and_restore() {
    let s = spin();
    let o = emc_options(bar_newton());
    let mut c = Checks::new();
    let mut whole = spin_integrator(&s, &o);
    whole.run(60).unwrap();
    let mut first = spin_integrator(&s, &o);
    first.run(30).unwrap();
    let saved = first.state().unwrap();
    let bytes = saved.to_bytes();
    let mut second = NlIntegrator::new(&s.model, &[], &o).unwrap();
    second.restore(NlTransientState::from_bytes(&bytes).unwrap()).unwrap();
    second.run(30).unwrap();
    c.holds(
        "R1e split run has the bits of the unsplit run",
        second.state().unwrap().to_bytes() == whole.state().unwrap().to_bytes(),
        "state bytes",
    );
    c.holds(
        "R2e bytes round trip",
        NlTransientState::from_bytes(&bytes).unwrap().to_bytes() == bytes,
        "",
    );
    c.holds("R2e magic RCFDNLE1", &bytes[..8] == b"RCFDNLE1", "");
    c.holds(
        "R2e the restored state is of the energy-momentum step",
        NlTransientState::from_bytes(&bytes).unwrap().step_kind() == StepKind::EnergyMomentum,
        "",
    );
    let method = NlTransientOptions::new(0.01).newton(bar_newton());
    let mut m_it = NlIntegrator::new(&s.model, &[], &method).unwrap();
    let (ok, d) = refused_with(m_it.restore(saved), "state", "ADR 0514");
    c.holds("R2e an energy-momentum state refused by a Method integrator", ok, &d);
    let m_state = spin_integrator(&s, &method).state().unwrap();
    let mut e_it = NlIntegrator::new(&s.model, &[], &o).unwrap();
    let (ok, d) = refused_with(e_it.restore(m_state), "state", "ADR 0514");
    c.holds("R2e a Method state refused by an energy-momentum integrator", ok, &d);
    c.finish();
}

#[test]
fn t1e_threads_bit_identical() {
    let s = spin();
    let mut a = spin_integrator(&s, &emc_options(bar_newton()).threads(1));
    let mut b = spin_integrator(&s, &emc_options(bar_newton()).threads(4));
    a.run(50).unwrap();
    b.run(50).unwrap();
    let mut c = Checks::new();
    c.holds(
        "T1e 1 and 4 threads",
        a.state().unwrap().to_bytes() == b.state().unwrap().to_bytes(),
        "state bytes",
    );
    c.finish();
}

#[test]
fn h1e_halvings_and_the_error() {
    let s = spin();
    let mut it = spin_integrator(&s, &emc_options(bar_newton().max_iterations(1)));
    let before = it.state().unwrap().to_bytes();
    let mut c = Checks::new();
    match it.step() {
        Ok(r) => c.holds("H1e the step is refused", false, &format!("accepted: {r:?}")),
        Err(e) => {
            let text = e.to_string();
            let ok = matches!(&e, FemError::Solver { solver, .. } if *solver == "nl_transient")
                && text.contains("after 5 halvings");
            c.holds("H1e the error names the halvings", ok, &text);
        }
    }
    c.holds(
        "H1e the state is unchanged",
        it.state().unwrap().to_bytes() == before,
        "state bytes",
    );
    c.finish();
}

#[test]
fn f1e_refusals() {
    let mut c = Checks::new();
    let o = emc_options(NewtonOptions::new(1));
    let with = |m: Material| {
        let mut model = SolidModel::new(slender_mesh());
        model.add_material("all", m).unwrap();
        model
    };
    let base = Material::isotropic_elastic(2.0e11, 0.3, RHO, 0.0).unwrap();
    let cases: [(&str, SolidModel); 3] = [
        ("J2", with(Material::j2(base, 2.0e8, 2.0e9, 1.0e9).unwrap())),
        (
            "NeoHookean",
            with(Material::hyperelastic(HyperelasticEnergy::neo_hookean(1e6, 1e9).unwrap(), 1100.0).unwrap()),
        ),
        (
            "orthotropic",
            with(
                Material::orthotropic(
                    [1.4e11, 1.0e10, 1.0e10],
                    [0.3, 0.3, 0.45],
                    [5.0e9, 5.0e9, 3.5e9],
                    1600.0,
                    [0.0; 3],
                    MaterialAxes::cylindrical([0.0; 3], [0.0, 0.0, 1.0]).unwrap(),
                )
                .unwrap(),
            ),
        ),
    ];
    for (name, model) in cases {
        let (ok, d) = refused_with(NlIntegrator::new(&model, &[], &o), "nl_transient", "ADR 0514");
        c.holds(&format!("F1e {name} refused"), ok, &d);
    }
    let mut mesh = slender_mesh();
    mesh.set_block_property(0, BlockProperty::solid(SolidFormulation::BBar))
        .unwrap();
    let mut model = SolidModel::new(mesh);
    model.add_material("all", soft()).unwrap();
    let (ok, d) = refused_with(NlIntegrator::new(&model, &[], &o), "nl_transient", "ADR 0514");
    c.holds("F1e BBar Hex8 refused", ok, &d);
    let s = spin();
    let mut it = spin_integrator(&s, &emc_options(bar_newton()));
    let before = it.state().unwrap().to_bytes();
    let (ok, d) = refused_with(it.step_with(&[&Zero]), "nl_transient", "ADR 0514");
    c.holds("F1e a contribution refused", ok, &d);
    c.holds(
        "F1e the state is unchanged by the refusal",
        it.state().unwrap().to_bytes() == before,
        "state bytes",
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------- the pendulum: P, D

fn pendulum_model() -> SolidModel {
    let mut mesh = slender_mesh();
    let nodes = mesh.nodes().to_vec();
    let pin: Vec<usize> = (0..nodes.len())
        .filter(|&i| near(nodes[i][0], 0.0) && near(nodes[i][2], 0.0))
        .collect();
    let mid: Vec<usize> = pin.iter().copied().filter(|&i| near(nodes[i][1], 0.0)).collect();
    assert_eq!((pin.len(), mid.len()), (3, 1), "pin line");
    mesh.add_node_set("pin", pin).unwrap();
    mesh.add_node_set("pin_mid", mid).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", soft()).unwrap();
    m.add_constraint(Constraint::displacement("pin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("pin", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("pin_mid", 1, 0.0)).unwrap();
    m.add_load(Load::gravity([0.0, 0.0, -G])).unwrap();
    m
}

fn pendulum_integrator(model: &SolidModel, damping: Option<Rayleigh>) -> NlIntegrator {
    let loads = [LoadHistory::model_loads(model, LoadTable::constant(1.0, 0.0, 10.0).unwrap()).unwrap()];
    let mut o = emc_options(bar_newton()).method(Method::average_acceleration());
    o.time_step = 0.005;
    if let Some(d) = damping {
        o = o.damping(d);
    }
    let mut it = NlIntegrator::new(model, &loads, &o).unwrap();
    let zero = vec![0.0; model.dof_map().n_equations()];
    it.start(0.0, &zero, &zero).unwrap();
    it
}

#[test]
fn p1e_p2e_pendulum() {
    let model = pendulum_model();
    let dofs = model.dof_map();
    let nodes = model.mesh().nodes();
    let tip: Vec<usize> = (0..nodes.len())
        .filter(|&i| near(nodes[i][0], L_BAR) && near(nodes[i][1], 0.0) && near(nodes[i][2], 0.0))
        .collect();
    assert_eq!(tip.len(), 1, "tip node");
    let (qx, qz) = (dofs.equation(tip[0], 0).unwrap(), dofs.equation(tip[0], 2).unwrap());
    let gravity = LoadHistory::model_loads(&model, LoadTable::constant(1.0, 0.0, 10.0).unwrap())
        .unwrap()
        .pattern;
    let mut it = pendulum_integrator(&model, None);
    let dt = 0.005;
    let swing = RHO * L_BAR * B_BAR * B_BAR * G * L_BAR / 2.0;
    let total = |it: &NlIntegrator| {
        let u = it.displacement().unwrap();
        energy(it) - dot(&gravity, &u)
    };
    let angle = |it: &NlIntegrator| {
        let u = it.displacement().unwrap();
        (L_BAR + u[qx]).atan2(-u[qz])
    };
    let e0 = total(&it);
    let mut psi_prev = angle(&it);
    let mut crossings = Vec::new();
    let (mut e_dev, mut res) = (0.0f64, 0.0f64);
    for k in 1..=400 {
        let r = it.step().unwrap();
        worst(&mut res, r.residual, "final residual");
        let psi = angle(&it);
        assert!(psi.is_finite());
        if (psi_prev > 0.0) != (psi > 0.0) {
            crossings.push((k - 1) as f64 * dt + dt * psi_prev / (psi_prev - psi));
        }
        psi_prev = psi;
        worst(&mut e_dev, (total(&it) - e0).abs() / swing, "energy");
    }
    let t_closed = 2.0 * PI * (2.0 * L_BAR / (3.0 * G)).sqrt() * 1.180340599016096;
    let n = crossings.len();
    let mut c = Checks::new();
    c.holds("P1e two crossings at least", n >= 2, &format!("{n} crossings"));
    let t_fe = if n >= 2 {
        2.0 * (crossings[n - 1] - crossings[0]) / (n - 1) as f64
    } else {
        f64::NAN
    };
    println!("pendulum, energy-momentum: {n} crossings, T_FE = {t_fe:.9e} s, largest final residual {res:e}");
    c.at_most("P1e period |T_FE - T|/T", (t_fe - t_closed).abs() / t_closed, 1e-2);
    c.at_most("P2e energy max |E - E_0|/(m g L/2)", e_dev, 1e-6);
    c.finish();
}

#[test]
fn d1e_d2e_damped_pendulum() {
    let model = pendulum_model();
    let alpha = 0.5;
    let gravity = LoadHistory::model_loads(&model, LoadTable::constant(1.0, 0.0, 10.0).unwrap())
        .unwrap()
        .pattern;
    let mut it = pendulum_integrator(
        &model,
        Some(Rayleigh {
            mass: alpha,
            stiffness: 0.0,
        }),
    );
    let h = 0.005;
    let swing = RHO * L_BAR * B_BAR * B_BAR * G * L_BAR / 2.0;
    let m = it.mass().clone();
    let total = |it: &NlIntegrator| {
        let u = it.displacement().unwrap();
        energy(it) - dot(&gravity, &u)
    };
    let e0 = total(&it);
    let mut dissipated = 0.0;
    let mut dev = 0.0f64;
    for _ in 0..400 {
        let v0 = it.velocity().unwrap();
        it.step().unwrap();
        let v1 = it.velocity().unwrap();
        let vm: Vec<f64> = v0.iter().zip(&v1).map(|(a, b)| 0.5 * (a + b)).collect();
        dissipated += h * alpha * dot(&vm, &mv(&m, &vm));
        worst(&mut dev, (total(&it) + dissipated - e0).abs() / swing, "balance");
    }
    let mut c = Checks::new();
    c.at_most("D1e max |E + D - E_0|/(m g L/2)", dev, 1e-6);
    c.at_least("D2e D_400/(m g L/2)", dissipated / swing, 1e-2);
    c.finish();
}

// ------------------------------------------------------------------------------------------------ bar B: L1e

#[test]
fn l1e_linear_limit_against_f398() {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [20, 1, 1]).unwrap();
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", Material::isotropic_elastic(2.0e11, 0.0, 7850.0, 0.0).unwrap())
        .unwrap();
    model.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    model.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    model.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    let modal = modal_analysis(&model, &ModalOptions::new(2)).unwrap();
    let phi = modal.modes[0].clone();
    let mass = consistent(&model).unwrap();
    let k = rcfd_fem::assemble::stiffness(&model).unwrap();
    let omega = (dot(&phi, &mv(&k, &phi)) / dot(&phi, &mv(&mass, &phi))).sqrt();
    let period = 2.0 * PI / omega;
    let dt = period / 40.0;
    let pattern: Vec<f64> = mv(&mass, &phi).iter().map(|x| 1e-6 * x).collect();
    let loads = [LoadHistory::new(
        pattern,
        LoadTable::new(vec![(0.0, 1.0), (10.0 * period, 1.0)]).unwrap(),
    )];
    let damping = Rayleigh::from_ratio(0.02, omega, omega).unwrap();
    let zero = vec![0.0; phi.len()];
    let mut c = Checks::new();
    for (name, damped) in [("undamped", false), ("damped", true)] {
        let mut lo = TransientOptions::new(dt).method(Method::average_acceleration());
        let mut no = NlTransientOptions::new(dt).energy_momentum();
        if damped {
            lo = lo.damping(damping);
            no = no.damping(damping);
        }
        let mut lin = Integrator::new(&model, &loads, &lo).unwrap();
        lin.start(0.0, &zero, &zero).unwrap();
        let mut nl = NlIntegrator::new(&model, &loads, &no).unwrap();
        nl.start(0.0, &zero, &zero).unwrap();
        let (mut num, mut den) = (0.0f64, 0.0f64);
        for _ in 0..80 {
            lin.step().unwrap();
            nl.step().unwrap();
            let ul = lin.displacement().unwrap();
            let un = nl.displacement().unwrap();
            let d: Vec<f64> = ul.iter().zip(&un).map(|(a, b)| a - b).collect();
            worst(&mut num, inf_norm(&d), "difference");
            worst(&mut den, inf_norm(&ul), "linear");
        }
        c.at_most(&format!("L1e {name} max |du|/max |u_lin|"), num / den, 1e-9);
    }
    c.finish();
}
