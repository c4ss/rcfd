//! F-607 (ADR 0523): the discrete gradient of Gonzalez in the energy-momentum step of `nl_transient`. Checks A0 to A3 (the
//! neo-Hookean spinning bar over ten turns), I1, I1r, I2 (the discrete gradient of an isotropic elastic model against the F-570
//! step), K0 to K2 (the large-strain neo-Hookean cube), N1h (the average on the cube, negative control), R1h (save and
//! restore), T1h (threads) and F1h (refusals) of the registration, with amendment 1 (the neo-Hookean material with
//! `AlgorithmicStress::DiscreteGradient`). Every aggregated value is required finite before it enters a maximum.

use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::mass::consistent;
use rcfd_fem::material::HyperelasticEnergy;
use rcfd_fem::nl_transient::{AlgorithmicStress, Integrator as NlIntegrator, NlTransientOptions, NlTransientState};
use rcfd_fem::nonlinear::{solve_with, Contribution, DriverExtensions, NewtonOptions};
use rcfd_fem::{BlockProperty, FemError, Material, SolidFormulation, SolidModel};
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
const E_BAR: f64 = 2.0e9;
const NU_BAR: f64 = 0.3;

fn slender_mesh() -> rcfd_fem::mesh::SolidMesh {
    hex_box([0.0, -B_BAR / 2.0, -B_BAR / 2.0], [L_BAR, B_BAR, B_BAR], [10, 2, 2]).unwrap()
}

fn soft() -> Material {
    Material::isotropic_elastic(E_BAR, NU_BAR, RHO, 0.0).unwrap()
}

/// The neo-Hookean material of the bar's small-strain moduli (c1): `mu = E/(2 (1 + nu))`, `kappa = E/(3 (1 - 2 nu))`.
fn bar_neo_hookean() -> Material {
    let mu = E_BAR / (2.0 * (1.0 + NU_BAR));
    let kappa = E_BAR / (3.0 * (1.0 - 2.0 * NU_BAR));
    Material::hyperelastic(HyperelasticEnergy::neo_hookean(mu, kappa).unwrap(), RHO).unwrap()
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

/// A model with its start.
struct Start {
    model: SolidModel,
    u0: Vec<f64>,
    v0: Vec<f64>,
    steady_ok: bool,
}

/// The free spinning bar of ADR 0479 D5 with its steady start from the St. Venant-Kirchhoff bar (ADR 0523 D7), the model
/// with `material`.
fn spin(material: Material) -> Start {
    let omega = 2.0 * PI;
    let centre = [L_BAR / 2.0, 0.0, 0.0];
    let velocity = [0.5, 0.25, 0.1];
    let mut svk = SolidModel::new(slender_mesh());
    svk.add_material("all", soft()).unwrap();
    let dofs = svk.dof_map();
    let nodes = svk.mesh().nodes().to_vec();
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
        m: consistent(&svk).unwrap(),
        offset,
        omega,
    };
    let ext = DriverExtensions {
        contributions: vec![&spin],
        step_control: None,
    };
    let steady = solve_with(&svk, &prescribed, &[], &NewtonOptions::new(1), ext);
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
    let mut model = SolidModel::new(slender_mesh());
    model.add_material("all", material).unwrap();
    Start {
        model,
        u0,
        v0,
        steady_ok,
    }
}

fn integrator(s: &Start, options: &NlTransientOptions) -> NlIntegrator {
    let mut it = NlIntegrator::new(&s.model, &[], options).unwrap();
    it.start(0.0, &s.u0, &s.v0).unwrap();
    it
}

fn energy(it: &NlIntegrator) -> f64 {
    it.kinetic_energy().unwrap() + it.strain_energy().unwrap()
}

/// What a run measured: the worst relative changes of `P`, `J`, `E`, the steps done, the error if any, the largest final
/// residual and the halvings.
struct Run {
    dp: f64,
    dj: f64,
    de: f64,
    steps: u64,
    error: Option<String>,
    residual: f64,
    halvings: usize,
}

fn run(s: &Start, options: &NlTransientOptions, steps: u64) -> Run {
    let mut it = integrator(s, options);
    let (p0, j0) = it.momentum().unwrap();
    let e0 = energy(&it);
    let mut r = Run {
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

fn bar_options(stress: AlgorithmicStress) -> NlTransientOptions {
    NlTransientOptions::new(0.01)
        .newton(bar_newton())
        .energy_momentum()
        .algorithmic_stress(stress)
}

#[test]
fn a0_a1_a2_a3_neo_hookean_spinning_bar_ten_turns() {
    let s = spin(bar_neo_hookean());
    let mut c = Checks::new();
    c.holds("A0 steady spin solve (St. Venant-Kirchhoff)", s.steady_ok, "");
    let built = NlIntegrator::new(&s.model, &[], &bar_options(AlgorithmicStress::DiscreteGradient));
    c.holds(
        "A0 neo-Hookean integrator built",
        built.is_ok(),
        &format!("{:?}", built.as_ref().err()),
    );
    let r = run(&s, &bar_options(AlgorithmicStress::DiscreteGradient), 1000);
    println!(
        "NH bar, discrete gradient: {} steps, halvings {}, largest final residual {:e}, error {:?}",
        r.steps, r.halvings, r.residual, r.error
    );
    c.holds(
        "A0 1000 steps converged",
        r.steps == 1000 && r.error.is_none(),
        &format!("{} steps, {:?}", r.steps, r.error),
    );
    c.at_most("A1 linear momentum max |P - P_0|/|P_0|", r.dp, 1e-10);
    c.at_most("A2 angular momentum max |J - J_0|/|J_0|", r.dj, 1e-6);
    c.at_most("A3 energy max |E - E_0|/|E_0|", r.de, 1e-6);
    c.finish();
}

/// The relative step differences of two integrators after one step from the same state: `|du_a - du_b|_inf / |du_b|_inf`
/// and `|v_a - v_b|_inf / |v_b|_inf` (`b` the reference, `u_n` the common start).
fn step_difference(u_n: &[f64], a: &NlIntegrator, b: &NlIntegrator) -> (f64, f64) {
    let (ua, ub) = (a.displacement().unwrap(), b.displacement().unwrap());
    let (va, vb) = (a.velocity().unwrap(), b.velocity().unwrap());
    let dua: Vec<f64> = ua.iter().zip(u_n).map(|(x, y)| x - y).collect();
    let dub: Vec<f64> = ub.iter().zip(u_n).map(|(x, y)| x - y).collect();
    let du: Vec<f64> = dua.iter().zip(&dub).map(|(x, y)| x - y).collect();
    let dv: Vec<f64> = va.iter().zip(&vb).map(|(x, y)| x - y).collect();
    (inf_norm(&du) / inf_norm(&dub), inf_norm(&dv) / inf_norm(&vb))
}

/// At every step of a reference run, one step of the other integrator from the reference state (I1, I2): the largest of
/// the two differences of `step_difference` over the steps, the steps done and the error if any.
fn per_step(reference: &mut NlIntegrator, other: &mut NlIntegrator, steps: u64) -> (f64, u64, Option<String>) {
    let mut d = 0.0;
    for n in 0..steps {
        let state = reference.state().unwrap();
        let u_n = reference.displacement().unwrap();
        other.restore(state).unwrap();
        if let Err(e) = reference.step() {
            return (d, n, Some(format!("reference: {e}")));
        }
        if let Err(e) = other.step() {
            return (d, n, Some(format!("discrete gradient: {e}")));
        }
        let (du, dv) = step_difference(&u_n, other, reference);
        worst(&mut d, du.max(dv), "step difference");
    }
    (d, steps, None)
}

#[test]
fn i1_isotropic_bar_discrete_gradient_equals_the_f570_step() {
    let s = spin(soft());
    let mut c = Checks::new();
    let mut reference = integrator(&s, &bar_options(AlgorithmicStress::Auto));
    let mut other = integrator(&s, &bar_options(AlgorithmicStress::DiscreteGradient));
    let (d, steps, error) = per_step(&mut reference, &mut other, 1000);
    c.holds(
        "I1 1000 steps of both",
        steps == 1000 && error.is_none(),
        &format!("{steps} steps, {error:?}"),
    );
    c.at_most("I1 max over steps of the step difference (du, v)", d, 1e-12);
    c.finish();
}

#[test]
fn i1r_isotropic_bar_whole_runs_record() {
    let s = spin(soft());
    let mut a = integrator(&s, &bar_options(AlgorithmicStress::Auto));
    let mut b = integrator(&s, &bar_options(AlgorithmicStress::DiscreteGradient));
    let mut d = 0.0;
    for _ in 0..1000 {
        a.step().unwrap();
        b.step().unwrap();
        let (ua, ub) = (a.displacement().unwrap(), b.displacement().unwrap());
        let diff: Vec<f64> = ua.iter().zip(&ub).map(|(x, y)| x - y).collect();
        worst(&mut d, inf_norm(&diff) / inf_norm(&ua), "I1r");
    }
    println!("I1r (record) whole runs, max |u_DG - u_570|_inf / |u_570|_inf: {d:e}");
}

#[test]
fn r1h_neo_hookean_save_and_restore() {
    let s = spin(bar_neo_hookean());
    let o = bar_options(AlgorithmicStress::DiscreteGradient);
    let mut c = Checks::new();
    let mut whole = integrator(&s, &o);
    whole.run(60).unwrap();
    let mut first = integrator(&s, &o);
    first.run(30).unwrap();
    let bytes = first.state().unwrap().to_bytes();
    let mut second = NlIntegrator::new(&s.model, &[], &o).unwrap();
    second.restore(NlTransientState::from_bytes(&bytes).unwrap()).unwrap();
    second.run(30).unwrap();
    c.holds(
        "R1h 60 steps equal 30 + bytes + restore + 30",
        whole.state().unwrap().to_bytes() == second.state().unwrap().to_bytes(),
        "state bytes",
    );
    c.finish();
}

// ----------------------------------------------------------------------------------------------- the cube

const MU_CUBE: f64 = 1e5;
const KAPPA_CUBE: f64 = 1e6;
const RHO_CUBE: f64 = 1000.0;

fn cube_neo_hookean() -> Material {
    Material::hyperelastic(HyperelasticEnergy::neo_hookean(MU_CUBE, KAPPA_CUBE).unwrap(), RHO_CUBE).unwrap()
}

/// The isotropic elastic material of the cube's small-strain moduli (ADR 0523 D7).
fn cube_elastic() -> Material {
    let e = 9.0 * KAPPA_CUBE * MU_CUBE / (3.0 * KAPPA_CUBE + MU_CUBE);
    let nu = (3.0 * KAPPA_CUBE - 2.0 * MU_CUBE) / (2.0 * (3.0 * KAPPA_CUBE + MU_CUBE));
    Material::isotropic_elastic(e, nu, RHO_CUBE, 0.0).unwrap()
}

/// The free cube of ADR 0523 arithmetic 3: `u_0 = 0`, `v_0 = omega e_z x r + V + alpha r + gamma r_y e_x`.
fn cube(material: Material) -> Start {
    let mut model = SolidModel::new(hex_box([0.0; 3], [1.0; 3], [2, 2, 2]).unwrap());
    model.add_material("all", material).unwrap();
    let dofs = model.dof_map();
    let n = dofs.n_equations();
    let (omega, alpha, gamma) = (3.0, 5.0, 4.0);
    let velocity = [0.3, -0.2, 0.1];
    let mut v0 = vec![0.0; n];
    for (i, p) in model.mesh().nodes().iter().enumerate() {
        let r = [p[0] - 0.5, p[1] - 0.5, p[2] - 0.5];
        let w = [
            -omega * r[1] + velocity[0] + alpha * r[0] + gamma * r[1],
            omega * r[0] + velocity[1] + alpha * r[1],
            velocity[2] + alpha * r[2],
        ];
        for (comp, wc) in w.iter().enumerate() {
            v0[dofs.equation(i, comp).unwrap()] = *wc;
        }
    }
    Start {
        model,
        u0: vec![0.0; n],
        v0,
        steady_ok: true,
    }
}

fn cube_options(stress: AlgorithmicStress) -> NlTransientOptions {
    NlTransientOptions::new(0.004)
        .newton(NewtonOptions::new(1))
        .energy_momentum()
        .algorithmic_stress(stress)
}

#[test]
fn k0_k1_k2_neo_hookean_cube() {
    let s = cube(cube_neo_hookean());
    let mut c = Checks::new();
    let r = run(&s, &cube_options(AlgorithmicStress::DiscreteGradient), 500);
    println!(
        "NH cube, discrete gradient: {} steps, halvings {}, largest final residual {:e}, error {:?}",
        r.steps, r.halvings, r.residual, r.error
    );
    c.holds(
        "K0 500 steps converged",
        s.steady_ok && r.steps == 500 && r.error.is_none(),
        &format!("{} steps, {:?}", r.steps, r.error),
    );
    c.at_most("K1 energy max |E - E_0|/|E_0|", r.de, 1e-7);
    c.at_most("K2 linear momentum max |P - P_0|/|P_0|", r.dp, 1e-10);
    c.at_most("K2 angular momentum max |J - J_0|/|J_0|", r.dj, 1e-8);
    c.finish();
}

#[test]
fn n1h_average_on_the_neo_hookean_cube() {
    let s = cube(cube_neo_hookean());
    let mut c = Checks::new();
    let r = run(&s, &cube_options(AlgorithmicStress::Average), 500);
    println!(
        "NH cube, average: {} steps, halvings {}, largest final residual {:e}, error {:?}",
        r.steps, r.halvings, r.residual, r.error
    );
    c.holds("N1h at least 15 steps", r.steps >= 15, &format!("{} steps", r.steps));
    c.at_least("N1h energy max |E - E_0|/|E_0| (average)", r.de, 1e-4);
    c.finish();
}

#[test]
fn i2_isotropic_cube_discrete_gradient_equals_the_f570_step() {
    let s = cube(cube_elastic());
    let mut c = Checks::new();
    let mut reference = integrator(&s, &cube_options(AlgorithmicStress::Auto));
    let mut other = integrator(&s, &cube_options(AlgorithmicStress::DiscreteGradient));
    let (d, steps, error) = per_step(&mut reference, &mut other, 500);
    c.holds(
        "I2 500 steps of both",
        steps == 500 && error.is_none(),
        &format!("{steps} steps, {error:?}"),
    );
    c.at_most("I2 max over steps of the step difference (du, v)", d, 1e-12);
    c.finish();
}

#[test]
fn t1h_threads() {
    let s = cube(cube_neo_hookean());
    let mut c = Checks::new();
    let mut one = integrator(&s, &cube_options(AlgorithmicStress::DiscreteGradient).threads(1));
    let mut four = integrator(&s, &cube_options(AlgorithmicStress::DiscreteGradient).threads(4));
    one.run(50).unwrap();
    four.run(50).unwrap();
    c.holds(
        "T1h 1 and 4 threads",
        one.state().unwrap().to_bytes() == four.state().unwrap().to_bytes(),
        "state bytes",
    );
    c.finish();
}

#[test]
#[ignore = "record of ADR 0523: ADR 0555 (F-620) opened the nonlinear transient Method step to the hyperelastic material, retiring the row NH with the Method step refused; continued in sa_hyper_paths.rs f1h_refusals_after_f620 (ADR 0291 pattern, maintainer standing decision)"]
fn f1h_refusals() {
    let mut c = Checks::new();
    let dg = bar_options(AlgorithmicStress::DiscreteGradient);
    let with = |m: Material| {
        let mut model = SolidModel::new(slender_mesh());
        model.add_material("all", m).unwrap();
        model
    };
    let mr = with(Material::hyperelastic(HyperelasticEnergy::mooney_rivlin(3e5, 1e5, 1e9).unwrap(), 1100.0).unwrap());
    let (ok, d) = refused_with(NlIntegrator::new(&mr, &[], &dg), "nl_transient", "ADR 0523");
    c.holds("F1h Mooney-Rivlin refused with ADR 0523", ok, &d);
    let nh = with(bar_neo_hookean());
    let r = NlIntegrator::new(&nh, &[], &NlTransientOptions::new(0.01));
    let ok = matches!(r, Err(FemError::NotYetSupported { .. }));
    c.holds(
        "F1h NH with the Method step refused (NotYetSupported)",
        ok,
        &format!("{:?}", r.err()),
    );
    let r = NlIntegrator::new(&nh, &[], &bar_options(AlgorithmicStress::Auto));
    c.holds(
        "F1h NH with the energy-momentum step and Auto refused (amendment 1)",
        r.is_err(),
        &format!("{:?}", r.err()),
    );
    let r = NlIntegrator::new(&nh, &[], &dg);
    c.holds(
        "F1h NH with the energy-momentum step and DiscreteGradient accepted (amendment 1)",
        r.is_ok(),
        &format!("{:?}", r.err()),
    );
    let mut mesh = slender_mesh();
    mesh.set_block_property(0, BlockProperty::solid(SolidFormulation::BBar))
        .unwrap();
    let mut model = SolidModel::new(mesh);
    model.add_material("all", bar_neo_hookean()).unwrap();
    let (ok, d) = refused_with(NlIntegrator::new(&model, &[], &dg), "nl_transient", "ADR 0514");
    c.holds("F1h NH in a BBar Hex8 block refused with ADR 0514", ok, &d);
    c.finish();
}
