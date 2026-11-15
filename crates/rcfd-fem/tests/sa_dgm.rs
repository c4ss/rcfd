//! F-611 (ADR 0544): the energy-momentum step with F-425's hyperelastic energies (`AlgorithmicStress::
//! MaterialDiscreteGradient`). Checks A0 to A3 (the neo-Hookean spinning bar over ten turns), K0n to K2n and K0r to K2r (the
//! large-strain neo-Hookean and Mooney-Rivlin cubes), E2 (the two energies at a state), I3 (the isotropic material), the
//! records X1r and X2r, R1m (save and restore), T1m (threads) and F1m (refusals) of the registration. The set-ups are those of
//! `sa_dg.rs` (ADR 0523 D7), copied. Every aggregated value is required finite before it enters a maximum.

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

// ----------------------------------------------------------------------------------------------- the cube

const MU_CUBE: f64 = 1e5;
const KAPPA_CUBE: f64 = 1e6;
const RHO_CUBE: f64 = 1000.0;

fn cube_neo_hookean() -> Material {
    Material::hyperelastic(HyperelasticEnergy::neo_hookean(MU_CUBE, KAPPA_CUBE).unwrap(), RHO_CUBE).unwrap()
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

const MDG: AlgorithmicStress = AlgorithmicStress::MaterialDiscreteGradient;

#[test]
fn a0_a1_a2_a3_neo_hookean_spinning_bar_with_the_f425_energy() {
    let s = spin(bar_neo_hookean());
    let mut c = Checks::new();
    c.holds("A0 steady spin solve (St. Venant-Kirchhoff)", s.steady_ok, "");
    let built = NlIntegrator::new(&s.model, &[], &bar_options(MDG));
    c.holds(
        "A0 neo-Hookean integrator built (MaterialDiscreteGradient)",
        built.is_ok(),
        &format!("{:?}", built.as_ref().err()),
    );
    let r = run(&s, &bar_options(MDG), 1000);
    println!(
        "NH bar, F-425 energy: {} steps, halvings {}, largest final residual {:e}, error {:?}",
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

/// `max_n |u_a - u_b|_inf / |u_a|_inf` of two integrators run side by side from the same start (X1r, X2r).
fn side_by_side(s: &Start, a: &NlTransientOptions, b: &NlTransientOptions, steps: u64) -> f64 {
    let mut ia = integrator(s, a);
    let mut ib = integrator(s, b);
    let mut d = 0.0;
    for _ in 0..steps {
        ia.step().unwrap();
        ib.step().unwrap();
        let (ua, ub) = (ia.displacement().unwrap(), ib.displacement().unwrap());
        let diff: Vec<f64> = ua.iter().zip(&ub).map(|(x, y)| x - y).collect();
        worst(&mut d, inf_norm(&diff) / inf_norm(&ua), "side by side");
    }
    d
}

#[test]
fn x1r_neo_hookean_bar_step_local_against_f425_record() {
    let s = spin(bar_neo_hookean());
    let d = side_by_side(
        &s,
        &bar_options(AlgorithmicStress::DiscreteGradient),
        &bar_options(MDG),
        1000,
    );
    println!("X1r (record) NH bar, max |u_MDG - u_DG|_inf / |u_DG|_inf: {d:e}");
}

#[test]
fn i3_isotropic_bar_material_discrete_gradient_is_the_discrete_gradient() {
    let s = spin(soft());
    let mut c = Checks::new();
    let mut a = integrator(&s, &bar_options(AlgorithmicStress::DiscreteGradient));
    let mut b = integrator(&s, &bar_options(MDG));
    a.run(50).unwrap();
    b.run(50).unwrap();
    c.holds(
        "I3 50 steps, MaterialDiscreteGradient and DiscreteGradient",
        a.state().unwrap().to_bytes() == b.state().unwrap().to_bytes(),
        "state bytes",
    );
    c.finish();
}

/// The Mooney-Rivlin energy of the cube's small-strain moduli (ADR 0544 D5): `c10 = 3.5e4`, `c01 = 1.5e4`, `kappa = 1e6`.
fn cube_mooney_rivlin() -> Material {
    Material::hyperelastic(
        HyperelasticEnergy::mooney_rivlin(3.5e4, 1.5e4, KAPPA_CUBE).unwrap(),
        RHO_CUBE,
    )
    .unwrap()
}

fn cube_checks(label: &str, tag: &str, material: Material) {
    let s = cube(material);
    let mut c = Checks::new();
    let r = run(&s, &cube_options(MDG), 500);
    println!(
        "{label} cube, F-425 energy: {} steps, halvings {}, largest final residual {:e}, error {:?}",
        r.steps, r.halvings, r.residual, r.error
    );
    c.holds(
        &format!("K0{tag} 500 steps converged"),
        r.steps == 500 && r.error.is_none(),
        &format!("{} steps, {:?}", r.steps, r.error),
    );
    c.at_most(&format!("K1{tag} energy max |E - E_0|/|E_0|"), r.de, 1e-7);
    c.at_most(&format!("K2{tag} linear momentum max |P - P_0|/|P_0|"), r.dp, 1e-10);
    c.at_most(&format!("K2{tag} angular momentum max |J - J_0|/|J_0|"), r.dj, 1e-8);
    c.finish();
}

#[test]
fn k0n_k1n_k2n_neo_hookean_cube_with_the_f425_energy() {
    cube_checks("NH", "n", cube_neo_hookean());
}

#[test]
fn k0r_k1r_k2r_mooney_rivlin_cube() {
    cube_checks("MR", "r", cube_mooney_rivlin());
}

#[test]
fn x2r_neo_hookean_cube_step_local_against_f425_record() {
    let s = cube(cube_neo_hookean());
    let d = side_by_side(
        &s,
        &cube_options(AlgorithmicStress::DiscreteGradient),
        &cube_options(MDG),
        500,
    );
    println!("X2r (record) NH cube, max |u_MDG - u_DG|_inf / |u_DG|_inf: {d:e}");
}

#[test]
fn e2_strain_energy_of_the_two_energies_at_a_state_of_the_cube() {
    let s = cube(cube_neo_hookean());
    let mut c = Checks::new();
    let mut a = integrator(&s, &cube_options(AlgorithmicStress::DiscreteGradient));
    a.run(100).unwrap();
    let mut b = NlIntegrator::new(&s.model, &[], &cube_options(MDG)).unwrap();
    b.restore(a.state().unwrap()).unwrap();
    let (wa, wb) = (a.strain_energy().unwrap(), b.strain_energy().unwrap());
    println!("E2 strain energies: step-local {wa:e}, F-425 {wb:e}");
    c.at_most(
        "E2 |W_D2 - W_425|/|W_425| at step 100",
        (wa - wb).abs() / wb.abs(),
        1e-12,
    );
    c.finish();
}

#[test]
fn r1m_mooney_rivlin_save_and_restore() {
    let s = cube(cube_mooney_rivlin());
    let o = cube_options(MDG);
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
        "R1m 60 steps equal 30 + bytes + restore + 30",
        whole.state().unwrap().to_bytes() == second.state().unwrap().to_bytes(),
        "state bytes",
    );
    c.finish();
}

#[test]
fn t1m_mooney_rivlin_threads() {
    let s = cube(cube_mooney_rivlin());
    let mut c = Checks::new();
    let mut one = integrator(&s, &cube_options(MDG).threads(1));
    let mut four = integrator(&s, &cube_options(MDG).threads(4));
    one.run(50).unwrap();
    four.run(50).unwrap();
    c.holds(
        "T1m 1 and 4 threads",
        one.state().unwrap().to_bytes() == four.state().unwrap().to_bytes(),
        "state bytes",
    );
    c.finish();
}

#[test]
fn f1m_refusals() {
    let mut c = Checks::new();
    let with = |m: Material| {
        let mut model = SolidModel::new(slender_mesh());
        model.add_material("all", m).unwrap();
        model
    };
    let mr = with(Material::hyperelastic(HyperelasticEnergy::mooney_rivlin(3e5, 1e5, 1e9).unwrap(), 1100.0).unwrap());
    let nh = with(bar_neo_hookean());
    let r = NlIntegrator::new(&mr, &[], &bar_options(MDG));
    c.holds(
        "F1m MR accepted (MaterialDiscreteGradient)",
        r.is_ok(),
        &format!("{:?}", r.err()),
    );
    let r = NlIntegrator::new(&nh, &[], &bar_options(MDG));
    c.holds(
        "F1m NH accepted (MaterialDiscreteGradient)",
        r.is_ok(),
        &format!("{:?}", r.err()),
    );
    for stress in [AlgorithmicStress::DiscreteGradient, AlgorithmicStress::Average] {
        let (ok, d) = refused_with(
            NlIntegrator::new(&mr, &[], &bar_options(stress)),
            "nl_transient",
            "ADR 0523",
        );
        c.holds(&format!("F1m MR refused with {stress:?} (ADR 0523)"), ok, &d);
    }
    let r = NlIntegrator::new(&nh, &[], &NlTransientOptions::new(0.01).algorithmic_stress(MDG));
    c.holds(
        "F1m NH with the Method step and MaterialDiscreteGradient refused (NotYetSupported)",
        matches!(r, Err(FemError::NotYetSupported { .. })),
        &format!("{:?}", r.err()),
    );
    let mut mesh = slender_mesh();
    mesh.set_block_property(0, BlockProperty::solid(SolidFormulation::BBar))
        .unwrap();
    let mut model = SolidModel::new(mesh);
    model.add_material("all", bar_neo_hookean()).unwrap();
    let (ok, d) = refused_with(
        NlIntegrator::new(&model, &[], &bar_options(MDG)),
        "nl_transient",
        "ADR 0514",
    );
    c.holds("F1m NH in a BBar Hex8 block refused with ADR 0514", ok, &d);
    let inverted = [-0.6, 0.0, 0.0, 0.0, 0.0, 0.0];
    let energy = HyperelasticEnergy::mooney_rivlin(3.5e4, 1.5e4, 1e6).unwrap();
    let (ok, d) = refused_with(
        rcfd_fem::mat_hyper::energy_magnitude(&energy, &inverted),
        rcfd_fem::nonlinear::INVERSION_KEY,
        "det C",
    );
    c.holds("F1m energy_magnitude of an inverted strain refused", ok, &d);
    c.finish();
}
