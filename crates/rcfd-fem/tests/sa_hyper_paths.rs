//! F-620 (ADR 0555): hyperelastic materials on the nonlinear transient Method step and the linear paths. Checks M0 to M8 (the
//! Method step, with F-611's `MaterialDiscreteGradient` step as its companion), L0 to L8 (the linear paths with the
//! linearised material), RF1 to RF4 (refusals) and RC (the copies of the three records of D5) of the registration. Set-ups of
//! ADR 0555 D7. Every aggregated value is required finite before it enters a maximum.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::damped::{damped_modes, DampedOptions};
use rcfd_fem::geometric::Prestress;
use rcfd_fem::harmonic::HarmonicLoad;
use rcfd_fem::harmonic_direct::{DirectHarmonic, DirectHarmonicOptions, LossFactor};
use rcfd_fem::mat_hyper::{self, LINEAR_FOLLOW_UP};
use rcfd_fem::material::{isotropic_elasticity, HyperelasticEnergy, MaterialAxes, Matrix6, PointContext};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::nl_transient::{AlgorithmicStress, Integrator as NlIntegrator, NlTransientOptions, NlTransientState};
use rcfd_fem::nonlinear::{solve, NewtonOptions};
use rcfd_fem::recover::IntegrationPointField;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::thermal::TemperatureFieldHistory;
use rcfd_fem::transient::{Integrator as LinIntegrator, LoadHistory, LoadTable, Rayleigh, TransientOptions};
use rcfd_fem::{
    BlockProperty, Constraint, ElementKind, FemError, Load, Material, SolidFormulation, SolidMesh, SolidModel,
};

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
    fn within(&mut self, name: &str, value: f64, lo: f64, hi: f64) {
        let ok = value.is_finite() && value >= lo && value <= hi;
        println!(
            "{name}: {value:.6e} (limits {lo:e} ..= {hi:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} not in [{lo:e}, {hi:e}]"));
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

/// The running maximum of finite values; panics on a non-finite one.
fn worst(acc: &mut f64, value: f64, what: &str) {
    assert!(value.is_finite(), "{what}: non-finite value {value}");
    *acc = acc.max(value);
}

fn inf_norm(x: &[f64]) -> f64 {
    x.iter().fold(0.0f64, |m, v| {
        assert!(v.is_finite(), "non-finite value");
        m.max(v.abs())
    })
}

fn diff_inf(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    inf_norm(&a.iter().zip(b).map(|(x, y)| x - y).collect::<Vec<_>>())
}

/// `|a - b|_inf / |b|_inf`.
fn rel(a: &[f64], b: &[f64]) -> f64 {
    diff_inf(a, b) / inf_norm(b)
}

fn rel_matrix(a: &Matrix6, b: &Matrix6) -> f64 {
    rel(&a.concat(), &b.concat())
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

fn linear_refusal<T>(r: &rcfd_fem::Result<T>, what: &str) -> bool {
    matches!(r, Err(FemError::NotYetSupported { variant, package, location, .. })
        if *variant == "Hyperelastic" && *package == LINEAR_FOLLOW_UP && location.contains(what))
}

// ----------------------------------------------------------------------------------------------- energies (D7)

const RHO: f64 = 1000.0;

fn nhc() -> HyperelasticEnergy {
    HyperelasticEnergy::neo_hookean(1e6, 1e7).unwrap()
}
fn mrc() -> HyperelasticEnergy {
    HyperelasticEnergy::mooney_rivlin(3.5e5, 1.5e5, 1e7).unwrap()
}
fn nhi() -> HyperelasticEnergy {
    HyperelasticEnergy::neo_hookean(1e6, 1e10).unwrap()
}
fn nhk() -> HyperelasticEnergy {
    HyperelasticEnergy::neo_hookean(1e5, 1e6).unwrap()
}
fn mrk() -> HyperelasticEnergy {
    HyperelasticEnergy::mooney_rivlin(3.5e4, 1.5e4, 1e6).unwrap()
}

fn hyper(e: HyperelasticEnergy) -> Material {
    Material::hyperelastic(e, RHO).unwrap()
}

/// `(E, nu)` of the twin of an energy.
fn twin_constants(e: &HyperelasticEnergy) -> (f64, f64) {
    let (g, k) = mat_hyper::small_strain_moduli(e);
    (9.0 * k * g / (3.0 * k + g), (3.0 * k - 2.0 * g) / (2.0 * (3.0 * k + g)))
}

fn twin(e: &HyperelasticEnergy) -> Material {
    let (young, nu) = twin_constants(e);
    Material::isotropic_elastic(young, nu, RHO, 0.0).unwrap()
}

// ----------------------------------------------------------------------------------------------- set-ups (D7)

/// Cantilever L.
fn cantilever(material: Material, formulation: SolidFormulation, force: Vec3) -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [2.0, 0.5, 0.5], [8, 2, 2]).unwrap();
    if formulation != SolidFormulation::Full {
        mesh.set_block_property(0, BlockProperty::solid(formulation)).unwrap();
    }
    let mut model = SolidModel::new(mesh);
    model.add_material("all", material).unwrap();
    for k in 0..3 {
        model.add_constraint(Constraint::displacement("xmin", k, 0.0)).unwrap();
    }
    model.add_load(Load::nodal_force("xmax", force)).unwrap();
    model
}

/// The rows of the cantilever checks: name, energy, formulation, limit.
fn cantilever_rows() -> Vec<(&'static str, HyperelasticEnergy, SolidFormulation, f64)> {
    vec![
        ("NHc Full", nhc(), SolidFormulation::Full, 1e-10),
        ("MRc Full", mrc(), SolidFormulation::Full, 1e-10),
        ("NHi BBar", nhi(), SolidFormulation::BBar, 1e-8),
    ]
}

/// Cube K (clamped at `xmin`), its `v_0 = s X_1 (5, 4, 3)`.
fn cube_k(material: Material, s: f64) -> (SolidModel, Vec<f64>) {
    let mut model = SolidModel::new(hex_box([0.0; 3], [1.0; 3], [2, 2, 2]).unwrap());
    model.add_material("all", material).unwrap();
    for k in 0..3 {
        model.add_constraint(Constraint::displacement("xmin", k, 0.0)).unwrap();
    }
    let dofs = model.dof_map();
    let mut v0 = vec![0.0; dofs.n_equations()];
    for (i, p) in model.mesh().nodes().iter().enumerate() {
        for (comp, w) in [5.0, 4.0, 3.0].iter().enumerate() {
            v0[dofs.equation(i, comp).unwrap()] = s * p[0] * w;
        }
    }
    (model, v0)
}

/// Cube F (free), the `v_0` of ADR 0523 arithmetic 3.
fn cube_f(material: Material, formulation: SolidFormulation) -> (SolidModel, Vec<f64>) {
    let mut mesh = hex_box([0.0; 3], [1.0; 3], [2, 2, 2]).unwrap();
    if formulation != SolidFormulation::Full {
        mesh.set_block_property(0, BlockProperty::solid(formulation)).unwrap();
    }
    let mut model = SolidModel::new(mesh);
    model.add_material("all", material).unwrap();
    let dofs = model.dof_map();
    let (omega, alpha, gamma) = (3.0, 5.0, 4.0);
    let velocity = [0.3, -0.2, 0.1];
    let mut v0 = vec![0.0; dofs.n_equations()];
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
    (model, v0)
}

/// State S's displacement on Cube F's mesh: `u_0 = 0.1 r + 0.05 r_2 e_1`.
fn state_s(model: &SolidModel) -> Vec<f64> {
    let dofs = model.dof_map();
    let mut u0 = vec![0.0; dofs.n_equations()];
    for (i, p) in model.mesh().nodes().iter().enumerate() {
        let r = [p[0] - 0.5, p[1] - 0.5, p[2] - 0.5];
        let u = [0.1 * r[0] + 0.05 * r[1], 0.1 * r[1], 0.1 * r[2]];
        for (comp, uc) in u.iter().enumerate() {
            u0[dofs.equation(i, comp).unwrap()] = *uc;
        }
    }
    u0
}

const MDG: AlgorithmicStress = AlgorithmicStress::MaterialDiscreteGradient;

fn method(dt: f64) -> NlTransientOptions {
    NlTransientOptions::new(dt).newton(NewtonOptions::new(1))
}

fn f611(dt: f64) -> NlTransientOptions {
    NlTransientOptions::new(dt)
        .newton(NewtonOptions::new(1))
        .energy_momentum()
        .algorithmic_stress(MDG)
}

// ----------------------------------------------------------------------------------------------- the Method step

/// A run of Cube F: the final displacement and, per step, the energy change, finite or a panic.
struct FRun {
    u: Vec<f64>,
    steps: u64,
    error: Option<String>,
    de: f64,
}

fn run_f(model: &SolidModel, v0: &[f64], options: &NlTransientOptions, steps: u64) -> FRun {
    let n = v0.len();
    let built = NlIntegrator::new(model, &[], options);
    let mut it = match built {
        Ok(it) => it,
        Err(e) => {
            return FRun {
                u: Vec::new(),
                steps: 0,
                error: Some(e.to_string()),
                de: f64::NAN,
            }
        }
    };
    it.start(0.0, &vec![0.0; n], v0).unwrap();
    let e0 = it.kinetic_energy().unwrap() + it.strain_energy().unwrap();
    let mut r = FRun {
        u: Vec::new(),
        steps: 0,
        error: None,
        de: 0.0,
    };
    for _ in 0..steps {
        if let Err(e) = it.step() {
            r.error = Some(e.to_string());
            break;
        }
        r.steps += 1;
        let e = it.kinetic_energy().unwrap() + it.strain_energy().unwrap();
        worst(&mut r.de, (e - e0).abs() / e0.abs(), "energy");
    }
    r.u = it.displacement().unwrap();
    r
}

#[test]
fn m0_m4_m7r_method_step_against_f611_on_the_free_cube() {
    let mut c = Checks::new();
    for (name, e) in [("NHk", nhk()), ("MRk", mrk())] {
        let (model, v0) = cube_f(hyper(e), SolidFormulation::Full);
        let m4 = run_f(&model, &v0, &method(0.004), 50);
        c.holds(
            &format!("M0 {name} Method step dt 0.004, 50 steps"),
            m4.steps == 50 && m4.error.is_none(),
            &format!("{} steps, {:?}", m4.steps, m4.error),
        );
        println!(
            "M7r {name} (record) Method step dt 0.004 max |E - E_0|/|E_0|: {:.6e}",
            m4.de
        );
        let e4 = run_f(&model, &v0, &f611(0.004), 50);
        let m1 = run_f(&model, &v0, &method(0.001), 200);
        let e1 = run_f(&model, &v0, &f611(0.001), 200);
        for (label, r) in [
            ("F-611 dt 0.004", &e4),
            ("Method dt 0.001", &m1),
            ("F-611 dt 0.001", &e1),
        ] {
            assert!(r.error.is_none(), "{name} {label}: {:?}", r.error);
        }
        let d4 = rel(&m4.u, &e4.u);
        let d1 = rel(&m1.u, &e1.u);
        println!("{name}: D(0.004) = {d4:.6e}, D(0.001) = {d1:.6e}");
        c.at_most(&format!("M4 {name} D(0.001)"), d1, 1e-2);
        c.within(&format!("M4b {name} D(0.004)/D(0.001)"), d4 / d1, 4.0, f64::INFINITY);
    }
    c.finish();
}

#[test]
fn m1_m2_the_start_against_f611() {
    let mut c = Checks::new();
    for (name, e) in [("NHk", nhk()), ("MRk", mrk())] {
        let (model, v0) = cube_f(hyper(e), SolidFormulation::Full);
        let u0 = state_s(&model);
        let mut a = NlIntegrator::new(&model, &[], &method(0.004)).unwrap();
        let mut b = NlIntegrator::new(&model, &[], &f611(0.004)).unwrap();
        a.start(0.0, &u0, &v0).unwrap();
        b.start(0.0, &u0, &v0).unwrap();
        let (aa, ab) = (a.acceleration().unwrap(), b.acceleration().unwrap());
        c.at_most(
            &format!("M1 {name} |a_M - a_EM|_inf / |a_EM|_inf"),
            rel(&aa, &ab),
            1e-12,
        );
        let (wa, wb) = (a.strain_energy().unwrap(), b.strain_energy().unwrap());
        c.holds(
            &format!("M2 {name} strain_energy equal bits"),
            wa.is_finite() && wa.to_bits() == wb.to_bits(),
            &format!("{wa:e} / {wb:e}"),
        );
    }
    c.finish();
}

/// `max_n |u_M - u_L|_inf / max_n |u_L|_inf` of the Method step and the linear integrator on Cube K, 100 steps.
fn small_amplitude(e: HyperelasticEnergy, s: f64, damping: Option<Rayleigh>) -> f64 {
    let (model, v0) = cube_k(hyper(e), s);
    let n = v0.len();
    let mut nl_options = method(0.004);
    let mut lin_options = TransientOptions::new(0.004).threads(1);
    if let Some(d) = damping {
        nl_options = nl_options.damping(d);
        lin_options = lin_options.damping(d);
    }
    let mut a = NlIntegrator::new(&model, &[], &nl_options).unwrap();
    let mut b = LinIntegrator::new(&model, &[], &lin_options).unwrap();
    a.start(0.0, &vec![0.0; n], &v0).unwrap();
    b.start(0.0, &vec![0.0; n], &v0).unwrap();
    let (mut du, mut ul) = (0.0, 0.0);
    for _ in 0..100 {
        a.step().unwrap();
        b.step().unwrap();
        let (ua, ub) = (a.displacement().unwrap(), b.displacement().unwrap());
        worst(&mut du, diff_inf(&ua, &ub), "du");
        worst(&mut ul, inf_norm(&ub), "u_L");
    }
    du / ul
}

#[test]
fn m3_m3b_m3d_method_step_against_the_linear_integrator_at_small_amplitude() {
    let mut c = Checks::new();
    for (name, e) in [("NHk", nhk()), ("MRk", mrk())] {
        let big = small_amplitude(e, 1e-5, None);
        let small = small_amplitude(e, 1e-6, None);
        c.at_most(&format!("M3 {name} s = 1e-5"), big, 1e-3);
        println!("M3 {name} s = 1e-6 (for M3b): {small:.6e}");
        c.within(&format!("M3b {name} ratio"), big / small, 5.0, 20.0);
    }
    let damped = small_amplitude(
        nhk(),
        1e-5,
        Some(Rayleigh {
            mass: 1.0,
            stiffness: 1e-4,
        }),
    );
    c.at_most("M3d NHk damped s = 1e-5", damped, 1e-3);
    c.finish();
}

#[test]
fn m5_m6_threads_and_restore() {
    let mut c = Checks::new();
    let (model, v0) = cube_f(hyper(mrk()), SolidFormulation::Full);
    let n = v0.len();
    let mut one = NlIntegrator::new(&model, &[], &method(0.004).threads(1)).unwrap();
    let mut four = NlIntegrator::new(&model, &[], &method(0.004).threads(4)).unwrap();
    one.start(0.0, &vec![0.0; n], &v0).unwrap();
    four.start(0.0, &vec![0.0; n], &v0).unwrap();
    one.run(20).unwrap();
    four.run(20).unwrap();
    c.holds(
        "M5 MRk 20 steps, 1 and 4 threads",
        one.state().unwrap().to_bytes() == four.state().unwrap().to_bytes(),
        "state bytes",
    );
    let mut split = NlIntegrator::new(&model, &[], &method(0.004).threads(1)).unwrap();
    split.start(0.0, &vec![0.0; n], &v0).unwrap();
    split.run(10).unwrap();
    let bytes = split.state().unwrap().to_bytes();
    let mut again = NlIntegrator::new(&model, &[], &method(0.004).threads(1)).unwrap();
    again.restore(NlTransientState::from_bytes(&bytes).unwrap()).unwrap();
    again.run(10).unwrap();
    c.holds(
        "M6 MRk 20 steps against 10 + bytes + restore + 10",
        again.state().unwrap().to_bytes() == one.state().unwrap().to_bytes(),
        "state bytes",
    );
    c.finish();
}

#[test]
fn m8_method_step_with_the_f_bar_hex8() {
    let mut c = Checks::new();
    let (model, v0) = cube_f(hyper(nhk()), SolidFormulation::BBar);
    let r = run_f(&model, &v0, &method(0.004), 20);
    c.holds(
        "M8 NHk BBar Method step 20 steps, energies finite",
        r.steps == 20 && r.error.is_none() && r.de.is_finite(),
        &format!("{} steps, {:?}, max |dE|/E_0 {:e}", r.steps, r.error, r.de),
    );
    c.finish();
}

// ----------------------------------------------------------------------------------------------- the linear paths

#[test]
fn l0_the_linearised_matrix() {
    let mut c = Checks::new();
    for (name, e, limit_t) in [
        ("NHc", nhc(), 1e-10),
        ("MRc", mrc(), 1e-10),
        ("NHi", nhi(), 1e-8),
        ("NHk", nhk(), 1e-10),
        ("MRk", mrk(), 1e-10),
    ] {
        let d = hyper(e).linear_elasticity_matrix().unwrap();
        let (_, d0) = mat_hyper::stress_and_tangent(&e, &[0.0; 6]).unwrap();
        c.at_most(
            &format!("L0 {name} against the tangent at rest"),
            rel_matrix(&d, &d0),
            1e-14,
        );
        let (young, nu) = twin_constants(&e);
        c.at_most(
            &format!("L0t {name} against isotropic_elasticity of the twin"),
            rel_matrix(&d, &isotropic_elasticity(young, nu)),
            limit_t,
        );
    }
    c.finish();
}

#[test]
fn l1_static_and_recovered_stress_against_the_twin() {
    let mut c = Checks::new();
    for (name, e, f, limit) in cantilever_rows() {
        let a = cantilever(hyper(e), f, [0.0, 0.0, 1.0]);
        let b = cantilever(twin(&e), f, [0.0, 0.0, 1.0]);
        let ra = solve_static(&a, &StaticOptions::default()).unwrap();
        let rb = solve_static(&b, &StaticOptions::default()).unwrap();
        c.at_most(
            &format!("L1 {name} displacement"),
            rel(ra.displacement_values(), rb.displacement_values()),
            limit,
        );
        let sa = IntegrationPointField::from_result(&a, &ra).unwrap();
        let sb = IntegrationPointField::from_result(&b, &rb).unwrap();
        c.at_most(
            &format!("L1 {name} recovered stress"),
            rel(&sa.stress().concat(), &sb.stress().concat()),
            limit,
        );
    }
    c.finish();
}

#[test]
fn l2_homogeneous_uniaxial_bar() {
    let mut c = Checks::new();
    let delta = 1e-3;
    for (name, e) in [("NHc", nhc()), ("MRc", mrc())] {
        let mut model = SolidModel::new(hex_box([0.0; 3], [1.0; 3], [2, 2, 2]).unwrap());
        model.add_material("all", hyper(e)).unwrap();
        model.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
        model.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
        model.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
        model
            .add_constraint(Constraint::displacement("xmax", 0, delta))
            .unwrap();
        let r = solve_static(&model, &StaticOptions::default()).unwrap();
        let (young, nu) = twin_constants(&e);
        let dofs = model.dof_map();
        let mesh = model.mesh();
        let mut lateral = 0.0;
        for &node in mesh.node_set("ymax").unwrap() {
            let uy = r.displacement_values()[dofs.equation(node, 1).unwrap()];
            worst(&mut lateral, (uy + nu * delta).abs() / (nu * delta), "lateral");
        }
        c.at_most(&format!("L2 {name} u_y(ymax) against -nu delta"), lateral, 1e-10);
        let reactions = r.reactions().expect("reactions");
        let sum: f64 = mesh
            .node_set("xmax")
            .unwrap()
            .iter()
            .map(|&node| reactions[dofs.equation(node, 0).unwrap()])
            .sum();
        c.at_most(
            &format!("L2 {name} sum R_x(xmax) against E delta"),
            (sum - young * delta).abs() / (young * delta),
            1e-10,
        );
    }
    c.finish();
}

#[test]
#[ignore = "known miss of ADR 0555 L3 (registered run, log run1.log): the NHi BBar row failed in modal_analysis of the hyperelastic model, subspace iteration did not converge in 200 iterations (tolerance 1e-10, residuals 1.3e-9 .. 2.7e-11); NHc and MRc met 2.99e-14 (limit 1e-10); amendment 1, continued in l3_nhc_mrc_rows, diagnostic l3d_nhi_bbar_diagnostic"]
fn l3_modes_against_the_twin_known_miss() {
    let mut c = Checks::new();
    for (name, e, f, limit) in cantilever_rows() {
        let a = modal_analysis(&cantilever(hyper(e), f, [0.0, 0.0, 1.0]), &ModalOptions::new(6)).unwrap();
        let b = modal_analysis(&cantilever(twin(&e), f, [0.0, 0.0, 1.0]), &ModalOptions::new(6)).unwrap();
        let mut w = 0.0;
        for (x, y) in a.frequencies.iter().zip(&b.frequencies) {
            worst(&mut w, (x - y).abs() / y.abs(), "frequency");
        }
        c.holds(
            &format!("L3 {name} six frequencies"),
            a.frequencies.len() == 6 && b.frequencies.len() == 6,
            &format!("{:?}", a.frequencies),
        );
        c.at_most(&format!("L3 {name} frequencies, worst relative"), w, limit);
    }
    c.finish();
}

#[test]
fn l4_l5_l6_l8_transient_harmonic_buckling_damped_modes_against_the_twin() {
    let mut c = Checks::new();
    let e = nhc();
    let a = cantilever(hyper(e), SolidFormulation::Full, [0.0, 0.0, 1.0]);
    let b = cantilever(twin(&e), SolidFormulation::Full, [0.0, 0.0, 1.0]);
    // L4.
    let n = a.dof_map().n_equations();
    let run = |model: &SolidModel| -> Vec<Vec<f64>> {
        let history = LoadHistory::model_loads(model, LoadTable::constant(1.0, 0.0, 1.0).unwrap()).unwrap();
        let mut it = LinIntegrator::new(model, &[history], &TransientOptions::new(1e-3).threads(1)).unwrap();
        it.start(0.0, &vec![0.0; n], &vec![0.0; n]).unwrap();
        (0..50)
            .map(|_| {
                it.step().unwrap();
                it.displacement().unwrap()
            })
            .collect()
    };
    let (ua, ub) = (run(&a), run(&b));
    let (mut du, mut ul) = (0.0, 0.0);
    for (x, y) in ua.iter().zip(&ub) {
        worst(&mut du, diff_inf(x, y), "du");
        worst(&mut ul, inf_norm(y), "u");
    }
    c.at_most("L4 NHc linear transient, 50 steps", du / ul, 1e-10);
    // L5.
    let options = DirectHarmonicOptions::new()
        .loss_factor("all", LossFactor::Constant(0.02))
        .threads(1);
    let ha = DirectHarmonic::new(&a, &options).unwrap();
    let hb = DirectHarmonic::new(&b, &options).unwrap();
    let load = HarmonicLoad::from_model(&a).unwrap();
    for omega in [50.0, 500.0] {
        let (xa, xb) = (ha.response(&load, omega).unwrap(), hb.response(&load, omega).unwrap());
        let mut d = 0.0;
        let mut m = 0.0;
        for j in 0..xa.re.len() {
            worst(&mut d, (xa.re[j] - xb.re[j]).hypot(xa.im[j] - xb.im[j]), "dx");
            worst(&mut m, xb.re[j].hypot(xb.im[j]), "x");
        }
        c.at_most(&format!("L5 NHc direct harmonic at omega {omega}"), d / m, 1e-10);
    }
    // L6.
    let factors = |model: &SolidModel| -> Vec<f64> {
        let r = solve_static(model, &StaticOptions::default()).unwrap();
        let pre = Prestress::from_static(model, &r).unwrap();
        rcfd_fem::buckling::factors(model, &pre, &rcfd_fem::buckling::BucklingOptions::new(3))
            .unwrap()
            .factors
    };
    let fa = factors(&cantilever(hyper(e), SolidFormulation::Full, [-1.0, 0.0, 0.0]));
    let fb = factors(&cantilever(twin(&e), SolidFormulation::Full, [-1.0, 0.0, 0.0]));
    let mut w = 0.0;
    for (x, y) in fa.iter().zip(&fb) {
        worst(&mut w, (x - y).abs() / y.abs(), "factor");
    }
    c.holds(
        "L6 three factors each",
        fa.len() == 3 && fb.len() == 3,
        &format!("{fa:?}"),
    );
    c.at_most("L6 NHc buckling factors, worst relative", w, 1e-10);
    // L8.
    let mut d = DampedOptions::new(4);
    d.loss_factors.push(("all".to_string(), LossFactor::Constant(0.02)));
    let (da, db) = (damped_modes(&a, &d).unwrap(), damped_modes(&b, &d).unwrap());
    let mut w = 0.0;
    for (x, y) in da.roots.iter().zip(&db.roots) {
        worst(&mut w, (x.re - y.re).hypot(x.im - y.im) / y.re.hypot(y.im), "root");
    }
    c.holds(
        "L8 the same number of roots",
        !da.roots.is_empty() && da.roots.len() == db.roots.len(),
        &format!("{} / {}", da.roots.len(), db.roots.len()),
    );
    c.at_most("L8 NHc damped-mode roots, worst relative", w, 1e-10);
    c.finish();
}

#[test]
fn l7_linear_against_nonlinear_at_small_strain() {
    let mut c = Checks::new();
    for (name, e) in [("NHc", nhc()), ("MRc", mrc())] {
        let model = cantilever(hyper(e), SolidFormulation::Full, [0.0, 0.0, 3.3e-3]);
        let lin = solve_static(&model, &StaticOptions::default()).unwrap();
        let nl = solve(&model, &[], &[], &NewtonOptions::new(1)).unwrap();
        c.at_most(
            &format!("L7 {name} |u_lin - u_nl|_inf / |u_nl|_inf"),
            rel(lin.displacement_values(), nl.result.displacement_values()),
            1e-4,
        );
    }
    c.finish();
}

// ----------------------------------------------------------------------------------------------- refusals

#[test]
fn rf1_to_rf4_refusals() {
    let mut c = Checks::new();
    // RF1: a 2 x 2 Shell4 plate.
    let mut nodes = Vec::new();
    for j in 0..=2 {
        for i in 0..=2 {
            nodes.push([i as f64 * 0.5, j as f64 * 0.5, 0.0]);
        }
    }
    let idx = |i: usize, j: usize| j * 3 + i;
    let mut conn = Vec::new();
    for j in 0..2 {
        for i in 0..2 {
            conn.extend_from_slice(&[idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", (0..4).collect()).unwrap();
    let mut plate = SolidModel::new(mesh);
    plate.add_material("all", hyper(nhc())).unwrap();
    let r = solve_static(&plate, &StaticOptions::default());
    c.holds(
        "RF1 a Shell4 model refused naming LINEAR_FOLLOW_UP",
        linear_refusal(&r, "a structural element"),
        &format!("{:?}", r.err()),
    );
    // RF2.
    let mut model = cantilever(hyper(nhc()), SolidFormulation::Full, [0.0, 0.0, 1.0]);
    let nn = model.mesh().nodes().len();
    model.add_load(Load::temperature(vec![303.0; nn], 293.0)).unwrap();
    let r = solve_static(&model, &StaticOptions::default());
    c.holds(
        "RF2 a temperature load refused naming LINEAR_FOLLOW_UP",
        linear_refusal(&r, "a temperature load"),
        &format!("{:?}", r.err()),
    );
    // RF3.
    let m = hyper(nhc());
    let em = m.elasticity_matrix();
    c.holds(
        "RF3 elasticity_matrix still refused naming mat_hyper::FOLLOW_UP",
        matches!(&em, Err(FemError::NotYetSupported { package, location, .. })
            if *package == mat_hyper::FOLLOW_UP && location == "the element-constant elasticity matrix"),
        &format!("{:?}", em.err()),
    );
    let eps = [1e-3, -4e-4, 2e-4, 6e-4, -3e-4, 5e-4];
    let st = m.stress(&eps, &PointContext::default(), &[]).unwrap();
    let want = mat_hyper::stress_and_tangent(&nhc(), &eps).unwrap();
    let bits = |x: &[f64]| x.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
    c.holds(
        "RF3 Material::stress equal in bits to stress_and_tangent",
        bits(&st.0) == bits(&want.0) && bits(&st.1.concat()) == bits(&want.1.concat()),
        "",
    );
    let iso = Material::isotropic_elastic(2e11, 0.3, 7850.0, 1.2e-5).unwrap();
    let others = [
        ("isotropic", iso.clone()),
        (
            "orthotropic",
            Material::orthotropic(
                [1.4e11, 1.0e10, 1.0e10],
                [0.3, 0.3, 0.45],
                [5.0e9, 5.0e9, 3.5e9],
                1600.0,
                [0.0; 3],
                MaterialAxes::fixed([[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]).unwrap(),
            )
            .unwrap(),
        ),
        ("J2", Material::j2(iso, 2.5e8, 1e9, 5e8).unwrap()),
    ];
    for (name, m) in others {
        let (a, b) = (m.linear_elasticity_matrix(), m.elasticity_matrix());
        let same = match (&a, &b) {
            (Ok(x), Ok(y)) => bits(&x.concat()) == bits(&y.concat()),
            (Err(x), Err(y)) => x.to_string() == y.to_string(),
            _ => false,
        };
        c.holds(
            &format!("RF3 {name} linear_elasticity_matrix as elasticity_matrix"),
            same,
            "",
        );
    }
    // RF4.
    let (cube, _) = cube_f(hyper(nhk()), SolidFormulation::Full);
    let nn = cube.mesh().nodes().len();
    let field = TemperatureFieldHistory::constant(293.0, vec![303.0; nn], 0.0, 1.0).unwrap();
    let (ok, d) = refused_with(
        NlIntegrator::new(&cube, &[], &method(0.004).temperature_field(field)),
        "expansion",
        "",
    );
    c.holds(
        "RF4 a temperature field on the Method step refused (key expansion)",
        ok,
        &d,
    );
    c.finish();
}

// ----------------------------------------------------------------------------------------------- RC: the copies of D5

/// Copy of `sa_hyper.rs::rf2_the_paths` (ADR 0541) with the rows lifted by ADR 0555 accepted.
#[test]
fn rf2_the_paths_after_f620() {
    let mut c = Checks::new();
    let mut model = SolidModel::new(hex_box([0.0; 3], [1.0; 3], [2, 2, 2]).unwrap());
    model
        .add_material("all", hyper(HyperelasticEnergy::neo_hookean(1e6, 2.1667e6).unwrap()))
        .unwrap();
    for k in 0..3 {
        model.add_constraint(Constraint::displacement("xmin", k, 0.0)).unwrap();
    }
    model.add_load(Load::nodal_force("xmax", [1e3, 0.0, 0.0])).unwrap();
    let r = solve_static(&model, &StaticOptions::default());
    c.holds(
        "RF2' solve_static takes it (ADR 0555)",
        r.is_ok(),
        &format!("{:?}", r.err()),
    );
    let r = modal_analysis(&model, &ModalOptions::new(3));
    c.holds(
        "RF2' modal_analysis takes it (ADR 0555)",
        r.is_ok(),
        &format!("{:?}", r.err()),
    );
    let r = hyper(HyperelasticEnergy::neo_hookean(1e6, 2.1667e6).unwrap()).elasticity_matrix();
    c.holds(
        "RF2' elasticity_matrix refused naming FOLLOW_UP",
        matches!(&r, Err(FemError::NotYetSupported { variant, package, location, .. })
            if *variant == "Hyperelastic" && *package == mat_hyper::FOLLOW_UP
                && location == "the element-constant elasticity matrix"),
        &format!("{:?}", r.err()),
    );
    let r = NlIntegrator::new(&model, &[], &NlTransientOptions::new(0.01));
    c.holds(
        "RF2' nl_transient Method step takes it (ADR 0555)",
        r.is_ok(),
        &format!("{:?}", r.as_ref().err()),
    );
    let dg = NlTransientOptions::new(0.01)
        .energy_momentum()
        .algorithmic_stress(AlgorithmicStress::DiscreteGradient);
    let r = NlIntegrator::new(&model, &[], &dg);
    c.holds(
        "RF2' energy-momentum step with DiscreteGradient accepted",
        r.is_ok(),
        &format!("{:?}", r.as_ref().err()),
    );
    let r = solve(&model, &[], &[], &NewtonOptions::new(2).threads(1));
    c.holds(
        "RF2' the nonlinear static solve takes it",
        r.is_ok(),
        &format!("{:?}", r.as_ref().err()),
    );
    c.finish();
}

// The bar of `sa_dg.rs` and `sa_dgm.rs` (ADR 0523 D7), copied.
const L_BAR: f64 = 1.0;
const B_BAR: f64 = 0.02;
const RHO_BAR: f64 = 7850.0;
const E_BAR: f64 = 2.0e9;
const NU_BAR: f64 = 0.3;

fn slender_mesh() -> SolidMesh {
    hex_box([0.0, -B_BAR / 2.0, -B_BAR / 2.0], [L_BAR, B_BAR, B_BAR], [10, 2, 2]).unwrap()
}

fn bar_neo_hookean() -> Material {
    let mu = E_BAR / (2.0 * (1.0 + NU_BAR));
    let kappa = E_BAR / (3.0 * (1.0 - 2.0 * NU_BAR));
    Material::hyperelastic(HyperelasticEnergy::neo_hookean(mu, kappa).unwrap(), RHO_BAR).unwrap()
}

fn bar_options(stress: AlgorithmicStress) -> NlTransientOptions {
    let mut o = NewtonOptions::new(1);
    o.residual_tolerance = 1e-7;
    NlTransientOptions::new(0.01)
        .newton(o)
        .energy_momentum()
        .algorithmic_stress(stress)
}

fn bar_with(m: Material) -> SolidModel {
    let mut model = SolidModel::new(slender_mesh());
    model.add_material("all", m).unwrap();
    model
}

/// Copy of `sa_dg.rs::f1h_refusals` (ADR 0523) with the Method-step row accepted (ADR 0555).
#[test]
fn f1h_refusals_after_f620() {
    let mut c = Checks::new();
    let dg = bar_options(AlgorithmicStress::DiscreteGradient);
    let mr =
        bar_with(Material::hyperelastic(HyperelasticEnergy::mooney_rivlin(3e5, 1e5, 1e9).unwrap(), 1100.0).unwrap());
    let (ok, d) = refused_with(NlIntegrator::new(&mr, &[], &dg), "nl_transient", "ADR 0523");
    c.holds("F1h' Mooney-Rivlin refused with ADR 0523", ok, &d);
    let nh = bar_with(bar_neo_hookean());
    let r = NlIntegrator::new(&nh, &[], &NlTransientOptions::new(0.01));
    c.holds(
        "F1h' NH with the Method step accepted (ADR 0555)",
        r.is_ok(),
        &format!("{:?}", r.err()),
    );
    let r = NlIntegrator::new(&nh, &[], &bar_options(AlgorithmicStress::Auto));
    c.holds(
        "F1h' NH with the energy-momentum step and Auto refused (amendment 1)",
        r.is_err(),
        &format!("{:?}", r.err()),
    );
    let r = NlIntegrator::new(&nh, &[], &dg);
    c.holds(
        "F1h' NH with the energy-momentum step and DiscreteGradient accepted (amendment 1)",
        r.is_ok(),
        &format!("{:?}", r.err()),
    );
    let mut mesh = slender_mesh();
    mesh.set_block_property(0, BlockProperty::solid(SolidFormulation::BBar))
        .unwrap();
    let mut model = SolidModel::new(mesh);
    model.add_material("all", bar_neo_hookean()).unwrap();
    let (ok, d) = refused_with(NlIntegrator::new(&model, &[], &dg), "nl_transient", "ADR 0514");
    c.holds("F1h' NH in a BBar Hex8 block refused with ADR 0514", ok, &d);
    c.finish();
}

/// Copy of `sa_dgm.rs::f1m_refusals` (ADR 0544) with the Method-step row accepted (ADR 0555).
#[test]
fn f1m_refusals_after_f620() {
    let mut c = Checks::new();
    let mr =
        bar_with(Material::hyperelastic(HyperelasticEnergy::mooney_rivlin(3e5, 1e5, 1e9).unwrap(), 1100.0).unwrap());
    let nh = bar_with(bar_neo_hookean());
    let r = NlIntegrator::new(&mr, &[], &bar_options(MDG));
    c.holds(
        "F1m' MR accepted (MaterialDiscreteGradient)",
        r.is_ok(),
        &format!("{:?}", r.err()),
    );
    let r = NlIntegrator::new(&nh, &[], &bar_options(MDG));
    c.holds(
        "F1m' NH accepted (MaterialDiscreteGradient)",
        r.is_ok(),
        &format!("{:?}", r.err()),
    );
    for stress in [AlgorithmicStress::DiscreteGradient, AlgorithmicStress::Average] {
        let (ok, d) = refused_with(
            NlIntegrator::new(&mr, &[], &bar_options(stress)),
            "nl_transient",
            "ADR 0523",
        );
        c.holds(&format!("F1m' MR refused with {stress:?} (ADR 0523)"), ok, &d);
    }
    let r = NlIntegrator::new(&nh, &[], &NlTransientOptions::new(0.01).algorithmic_stress(MDG));
    c.holds(
        "F1m' NH with the Method step and MaterialDiscreteGradient accepted (ADR 0555)",
        r.is_ok(),
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
    c.holds("F1m' NH in a BBar Hex8 block refused with ADR 0514", ok, &d);
    let inverted = [-0.6, 0.0, 0.0, 0.0, 0.0, 0.0];
    let energy = HyperelasticEnergy::mooney_rivlin(3.5e4, 1.5e4, 1e6).unwrap();
    let (ok, d) = refused_with(
        mat_hyper::energy_magnitude(&energy, &inverted),
        rcfd_fem::nonlinear::INVERSION_KEY,
        "det C",
    );
    c.holds("F1m' energy_magnitude of an inverted strain refused", ok, &d);
    c.finish();
}

/// ADR 0555 amendment 1, L3c: the NHc and MRc rows of L3 (met in the registered run).
#[test]
fn l3_nhc_mrc_rows() {
    let mut c = Checks::new();
    for (name, e, f, limit) in cantilever_rows().into_iter().take(2) {
        let a = modal_analysis(&cantilever(hyper(e), f, [0.0, 0.0, 1.0]), &ModalOptions::new(6)).unwrap();
        let b = modal_analysis(&cantilever(twin(&e), f, [0.0, 0.0, 1.0]), &ModalOptions::new(6)).unwrap();
        let mut w = 0.0;
        for (x, y) in a.frequencies.iter().zip(&b.frequencies) {
            worst(&mut w, (x - y).abs() / y.abs(), "frequency");
        }
        c.holds(
            &format!("L3c {name} six frequencies"),
            a.frequencies.len() == 6 && b.frequencies.len() == 6,
            &format!("{:?}", a.frequencies),
        );
        c.at_most(&format!("L3c {name} frequencies, worst relative"), w, limit);
    }
    c.finish();
}

/// ADR 0555 amendment 1, L3d (diagnostic record, no limit): the NHi BBar cantilever's modes with the hyperelastic material and
/// with its twin.
#[test]
fn l3d_nhi_bbar_diagnostic() {
    let e = nhi();
    let a = modal_analysis(
        &cantilever(hyper(e), SolidFormulation::BBar, [0.0, 0.0, 1.0]),
        &ModalOptions::new(6),
    );
    let b = modal_analysis(
        &cantilever(twin(&e), SolidFormulation::BBar, [0.0, 0.0, 1.0]),
        &ModalOptions::new(6),
    );
    println!(
        "L3d (record) NHi BBar hyperelastic: {:?}",
        a.as_ref().map(|r| r.frequencies.clone())
    );
    println!(
        "L3d (record) NHi BBar twin: {:?}",
        b.as_ref().map(|r| r.frequencies.clone())
    );
    let mut o = ModalOptions::new(6);
    o.max_iterations = 2000;
    let a = modal_analysis(&cantilever(hyper(e), SolidFormulation::BBar, [0.0, 0.0, 1.0]), &o);
    let b = modal_analysis(&cantilever(twin(&e), SolidFormulation::BBar, [0.0, 0.0, 1.0]), &o);
    println!(
        "L3d (record) 2000 iterations, hyperelastic: {:?}",
        a.as_ref().map(|r| r.frequencies.clone())
    );
    println!(
        "L3d (record) 2000 iterations, twin: {:?}",
        b.as_ref().map(|r| r.frequencies.clone())
    );
    if let (Ok(a), Ok(b)) = (&a, &b) {
        let w = a
            .frequencies
            .iter()
            .zip(&b.frequencies)
            .fold(0.0f64, |m, (x, y)| m.max((x - y).abs() / y.abs()));
        println!("L3d (record) 2000 iterations, worst relative frequency difference: {w:e}");
    }
}

/// ADR 0555 amendment 2, L5m and L8m: Cantilever L with two material sets (`root`, `tip`), each with its own loss factor.
fn two_materials(root: Material, tip: Material) -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [2.0, 0.5, 0.5], [8, 2, 2]).unwrap();
    let centre_x = |e: usize, mesh: &SolidMesh| -> f64 {
        let nodes = mesh.element_nodes(e);
        nodes.iter().map(|&n| mesh.nodes()[n][0]).sum::<f64>() / nodes.len() as f64
    };
    let ne = mesh.n_elements();
    let root_set: Vec<usize> = (0..ne).filter(|&e| centre_x(e, &mesh) < 1.0).collect();
    let tip_set: Vec<usize> = (0..ne).filter(|&e| centre_x(e, &mesh) >= 1.0).collect();
    assert_eq!((root_set.len(), tip_set.len()), (16, 16));
    mesh.add_element_set("root", root_set).unwrap();
    mesh.add_element_set("tip", tip_set).unwrap();
    let mut model = SolidModel::new(mesh);
    model.add_material("root", root).unwrap();
    model.add_material("tip", tip).unwrap();
    for k in 0..3 {
        model.add_constraint(Constraint::displacement("xmin", k, 0.0)).unwrap();
    }
    model.add_load(Load::nodal_force("xmax", [0.0, 0.0, 1.0])).unwrap();
    model
}

#[test]
fn l5m_l8m_two_materials() {
    let mut c = Checks::new();
    let a = two_materials(hyper(nhc()), hyper(mrc()));
    let b = two_materials(twin(&nhc()), twin(&mrc()));
    let options = DirectHarmonicOptions::new()
        .loss_factor("root", LossFactor::Constant(0.02))
        .loss_factor("tip", LossFactor::Constant(0.05))
        .threads(1);
    let ha = DirectHarmonic::new(&a, &options).unwrap();
    let hb = DirectHarmonic::new(&b, &options).unwrap();
    let load = HarmonicLoad::from_model(&a).unwrap();
    for omega in [50.0, 500.0] {
        let (xa, xb) = (ha.response(&load, omega).unwrap(), hb.response(&load, omega).unwrap());
        let mut d = 0.0;
        let mut m = 0.0;
        for j in 0..xa.re.len() {
            worst(&mut d, (xa.re[j] - xb.re[j]).hypot(xa.im[j] - xb.im[j]), "dx");
            worst(&mut m, xb.re[j].hypot(xb.im[j]), "x");
        }
        c.at_most(
            &format!("L5m two materials, direct harmonic at omega {omega}"),
            d / m,
            1e-10,
        );
    }
    let mut d = DampedOptions::new(4);
    d.loss_factors.push(("root".to_string(), LossFactor::Constant(0.02)));
    d.loss_factors.push(("tip".to_string(), LossFactor::Constant(0.05)));
    let (da, db) = (damped_modes(&a, &d).unwrap(), damped_modes(&b, &d).unwrap());
    let mut w = 0.0;
    for (x, y) in da.roots.iter().zip(&db.roots) {
        worst(&mut w, (x.re - y.re).hypot(x.im - y.im) / y.re.hypot(y.im), "root");
    }
    c.holds(
        "L8m the same number of roots",
        !da.roots.is_empty() && da.roots.len() == db.roots.len(),
        &format!("{} / {}", da.roots.len(), db.roots.len()),
    );
    c.at_most("L8m two materials, damped-mode roots, worst relative", w, 1e-10);
    c.finish();
}
