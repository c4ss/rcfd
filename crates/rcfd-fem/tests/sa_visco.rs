//! F-410 (ADR 0485): linear viscoelasticity, a Prony series with the recursive update of the internal stresses, the complex
//! modulus and the time-temperature shift. Checks R1, R1m, R2 (relaxation), C1, C2, C1m, C2m (creep), M1 to M3 (complex
//! modulus, table), D1, D2 (dissipation), S1, S1m, S2 (shift), V1, V2 (free vibration), B1 (no branches), H1 (save and
//! restore), RF1 (refusals), T1 (tangent), N2, N3 (negative controls) of the registration.

use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::element::{element_for, ElementState};
use rcfd_fem::harmonic_direct::ModulusTable;
use rcfd_fem::mass::consistent;
use rcfd_fem::mat_visco::{self, complex_moduli, modulus_table, shift_factor, GAS_CONSTANT};
use rcfd_fem::material::{MaterialAxes, PointContext, PronyTerm, TimeTemperatureShift};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::nl_transient::{Integrator as NlIntegrator, NlTransientOptions, NlTransientState};
use rcfd_fem::nonlinear::{
    element_second_piola_kirchhoff, initial_states, solve, solve_from, solve_from_at, Equilibrium, NewtonOptions,
};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::transient::{Integrator, TransientOptions};
use rcfd_fem::{Constraint, FemError, Material, SolidModel};
use rcfd_linalg::complex::C64;
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

/// A list of measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!(
            "{name}: {value:.6e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} < {limit:e}"));
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

fn key_of(e: &FemError) -> Option<&'static str> {
    match e {
        FemError::InvalidValue { key, .. }
        | FemError::FieldLength { key, .. }
        | FemError::InvalidMaterial { key, .. } => Some(key),
        _ => None,
    }
}

fn refused_with<T>(r: rcfd_fem::Result<T>, key: &str) -> (bool, String) {
    match r {
        Ok(_) => (false, "accepted".to_string()),
        Err(e) => (key_of(&e) == Some(key), format!("{e}")),
    }
}

fn not_yet<T>(r: &rcfd_fem::Result<T>, location: &str) -> (bool, String) {
    match r {
        Err(
            e @ FemError::NotYetSupported {
                item,
                variant,
                location: l,
                package,
            },
        ) => (
            *item == "material"
                && *variant == "Viscoelastic"
                && l.contains(location)
                && *package == mat_visco::FOLLOW_UP,
            format!("{e}"),
        ),
        Err(e) => (false, format!("{e}")),
        Ok(_) => (false, "accepted".to_string()),
    }
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn mv(m: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; x.len()];
    m.matvec(x, &mut y);
    y
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

// ------------------------------------------------------------------------------------------------- materials

const E0: f64 = 2.0e9;
const NU: f64 = 0.35;
const W3: [f64; 3] = [0.3, 0.2, 0.1];
const TAU3: [f64; 3] = [0.01, 0.1, 1.0];

fn base(nu: f64) -> Material {
    Material::isotropic_elastic(E0, nu, 1200.0, 0.0).unwrap()
}

/// Equal shear and bulk weights `w`, times `tau`.
fn equal(w: &[f64], tau: &[f64], nu: f64, shift: Option<TimeTemperatureShift>) -> Material {
    let terms = w
        .iter()
        .zip(tau)
        .map(|(&wi, &ti)| PronyTerm::new(wi, wi, ti).unwrap())
        .collect();
    Material::viscoelastic(base(nu), terms, shift).unwrap()
}

/// The three-branch material of R1.
fn three() -> Material {
    equal(&W3, &TAU3, NU, None)
}

/// The unequal weights of the scaffold (R2, M2, T1).
fn unequal_terms() -> Vec<(f64, f64, f64)> {
    vec![(0.3, 0.1, 1e-3), (0.2, 0.0, 1e-1), (0.1, 0.05, 10.0)]
}

fn unequal() -> Material {
    let terms = unequal_terms()
        .into_iter()
        .map(|(g, k, t)| PronyTerm::new(g, k, t).unwrap())
        .collect();
    Material::viscoelastic(base(NU), terms, None).unwrap()
}

/// `E(t) = E_inf + sum E_i exp(-t/tau_i)`, `E_i = w_i E_0`.
fn relaxation(w: &[f64], tau: &[f64], t: f64) -> f64 {
    let einf = E0 * (1.0 - w.iter().sum::<f64>());
    einf + w.iter().zip(tau).map(|(wi, ti)| E0 * wi * (-t / ti).exp()).sum::<f64>()
}

fn uniaxial(eps: f64, nu: f64) -> [f64; 6] {
    [eps, -nu * eps, -nu * eps, 0.0, 0.0, 0.0]
}

fn ctx(t: f64, dt: f64) -> PointContext {
    PointContext::new().with_time(t, dt).unwrap()
}

// ------------------------------------------------------------------------------------------------- the cube

/// The unit cube of one Hex8 with three symmetry planes.
fn cube(material: Material) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0; 3], [1, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m
}

fn set_x_equations(model: &SolidModel, set: &str) -> Vec<(usize, usize)> {
    let dofs = model.dof_map();
    model
        .mesh()
        .node_set(set)
        .unwrap()
        .iter()
        .map(|&n| (n, dofs.equation(n, 0).unwrap()))
        .collect()
}

/// The prescribed `u_x = value` at the `xmax` nodes, written into `u`.
fn pull(model: &SolidModel, value: f64, u: &mut [f64]) -> Vec<Prescribed> {
    set_x_equations(model, "xmax")
        .into_iter()
        .map(|(n, q)| {
            u[q] = value;
            Prescribed::new(n, 0, value)
        })
        .collect()
}

/// The second Piola-Kirchhoff stresses at the points of element 0 for `u` and the committed `state` (default context: the
/// committed stress).
fn stresses(model: &SolidModel, u: &[f64], state: &ElementState) -> Vec<[f64; 6]> {
    let mesh = model.mesh();
    let dofs = model.dof_map();
    let nodes = mesh.element_nodes(0);
    let coords: Vec<[f64; 3]> = nodes.iter().map(|&n| mesh.nodes()[n]).collect();
    let ue: Vec<f64> = nodes
        .iter()
        .flat_map(|&n| (0..3).map(move |c| (n, c)))
        .map(|(n, c)| u[dofs.equation(n, c).unwrap()])
        .collect();
    let material = &model.materials()[0].1;
    element_second_piola_kirchhoff(element_for(mesh.element_kind(0)), &coords, material, &ue, state).unwrap()
}

/// The Green-Lagrange `E_xx` of the homogeneous stretch of the unit cube.
fn exx(model: &SolidModel, u: &[f64]) -> f64 {
    let (_, q) = set_x_equations(model, "xmax")[0];
    let a = u[q];
    a + 0.5 * a * a
}

fn tight() -> NewtonOptions {
    let mut o = NewtonOptions::new(1);
    o.residual_tolerance = 1e-13;
    o.energy_tolerance = 1.0;
    o
}

/// A quasi-static history of the cube under the prescribed `u_x(t)` at `xmax`: an increment of zero length to `value(0)`,
/// then `steps` increments of `dt` at the point contexts of `context(t, dt)`; `measure(t, model, u, states)` after every
/// increment.
#[allow(clippy::too_many_arguments)]
fn cube_history(
    model: &SolidModel,
    value: impl Fn(f64) -> f64,
    steps: usize,
    dt: f64,
    options: &NewtonOptions,
    context: impl Fn(f64, f64) -> PointContext,
    mut measure: impl FnMut(f64, &[f64], &[ElementState], &Equilibrium),
) {
    let n = model.dof_map().n_equations();
    let mut u = vec![0.0; n];
    let mut states = initial_states(model).unwrap();
    let load = vec![0.0; n];
    for k in 0..=steps {
        let (t, h) = if k == 0 { (0.0, 0.0) } else { (k as f64 * dt, dt) };
        let pres = pull(model, value(t), &mut u);
        let eq = solve_from_at(model, &pres, &load, &u, &states, options, &[], &context(t, h)).unwrap();
        u = eq.u.clone();
        states = eq.states.clone();
        measure(t, &u, &states, &eq);
    }
}

// ------------------------------------------------------------------------------------------------- R1, R1m, R2, N3

#[test]
fn r1_r1m_r2_n3_relaxation() {
    let mut c = Checks::new();
    let eps0 = 1e-3;
    // R1: one Hex8, the strain step, 100 increments of 0.02 s.
    let model = cube(three());
    let mut worst: f64 = 0.0;
    let mut count = 0;
    cube_history(
        &model,
        |_| eps0,
        100,
        0.02,
        &tight(),
        ctx,
        |t, u, states, _| {
            let e = exx(&model, u);
            let want = e * relaxation(&W3, &TAU3, t);
            for s in stresses(&model, u, &states[0]) {
                worst = worst.max((s[0] - want).abs() / (E0 * e));
                count += 1;
            }
        },
    );
    c.holds("R1 set-up", count == 101 * 8, &format!("{count} point values"));
    c.at_most("R1 relaxation of one element, three branches", worst, 1e-10);

    // R1m: the material point.
    let m = three();
    let mut state = vec![0.0; m.state_size()];
    let strain = uniaxial(eps0, NU);
    let mut worst: f64 = 0.0;
    for k in 0..=100 {
        let (t, h) = if k == 0 { (0.0, 0.0) } else { (k as f64 * 0.02, 0.02) };
        let up = mat_visco::update(&m, &strain, &state, &ctx(t, h)).unwrap();
        state = up.state;
        worst = worst.max((up.stress[0] - eps0 * relaxation(&W3, &TAU3, t)).abs() / (E0 * eps0));
    }
    c.at_most("R1m relaxation at the material point", worst, 1e-10);

    // R2: unequal weights, an arbitrary strain step.
    let m = unequal();
    let terms = unequal_terms();
    let g0 = E0 / (2.0 * (1.0 + NU));
    let k0 = E0 / (3.0 * (1.0 - 2.0 * NU));
    let eps = [1e-3, -2e-4, 3e-4, 5e-4, -1e-4, 2e-4];
    let tr = eps[0] + eps[1] + eps[2];
    let mut state = vec![0.0; m.state_size()];
    let mut worst: f64 = 0.0;
    let mut scale: f64 = 0.0;
    for k in 0..=100 {
        let (t, h) = if k == 0 { (0.0, 0.0) } else { (k as f64 * 0.05, 0.05) };
        let up = mat_visco::update(&m, &eps, &state, &ctx(t, h)).unwrap();
        state = up.state;
        let gt = g0
            * (1.0 - terms.iter().map(|x| x.0).sum::<f64>()
                + terms.iter().map(|x| x.0 * (-t / x.2).exp()).sum::<f64>());
        let kt = k0
            * (1.0 - terms.iter().map(|x| x.1).sum::<f64>()
                + terms.iter().map(|x| x.1 * (-t / x.2).exp()).sum::<f64>());
        let mut want = [0.0; 6];
        for i in 0..3 {
            want[i] = 2.0 * gt * (eps[i] - tr / 3.0) + kt * tr;
        }
        for i in 3..6 {
            want[i] = gt * eps[i];
        }
        if k == 0 {
            scale = want.iter().fold(0.0, |a: f64, b| a.max(b.abs()));
        }
        for i in 0..6 {
            worst = worst.max((up.stress[i] - want[i]).abs());
        }
    }
    c.at_most("R2 unequal weights, strain step", worst / scale, 1e-10);

    // N3: through solve_from (the default context, no time) the stress does not relax.
    let n = model.dof_map().n_equations();
    let mut u = vec![0.0; n];
    let pres = pull(&model, eps0, &mut u);
    let load = vec![0.0; n];
    let mut states = initial_states(&model).unwrap();
    let mut first = Vec::new();
    let mut last = Vec::new();
    for k in 0..6 {
        let eq = solve_from(&model, &pres, &load, &u, &states, &tight(), &[]).unwrap();
        u = eq.u;
        states = eq.states;
        let s: Vec<f64> = stresses(&model, &u, &states[0]).iter().map(|s| s[0]).collect();
        if k == 0 {
            first = s;
        } else {
            last = s;
        }
    }
    c.holds(
        "N3 no time, no relaxation",
        bits(&first) == bits(&last),
        &format!(
            "S_xx {:e} after the step, {:e} after five increments",
            first[0], last[0]
        ),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- C1, C2, C1m, C2m

const W_SLS: f64 = 0.5;
const TAU_SLS: f64 = 1.0;

fn creep_closed(sig0: f64, t: f64) -> f64 {
    let einf = E0 * (1.0 - W_SLS);
    let tauc = TAU_SLS * E0 / einf;
    sig0 * (1.0 / einf - (1.0 / einf - 1.0 / E0) * (-t / tauc).exp())
}

/// The element creep: largest relative error of `u_x(xmax)` over the step ends at `per` steps per `tau` to `5 tau_c`.
fn creep_element(per: usize) -> f64 {
    let model = cube(equal(&[W_SLS], &[TAU_SLS], NU, None));
    let n = model.dof_map().n_equations();
    let sig0 = 1e-8 * E0 * (1.0 - W_SLS);
    let xmax = set_x_equations(&model, "xmax");
    let mut load = vec![0.0; n];
    for &(_, q) in &xmax {
        load[q] = sig0 / 4.0;
    }
    let dt = TAU_SLS / per as f64;
    let steps = (5.0 * 2.0 * TAU_SLS / dt).round() as usize;
    let mut u = vec![0.0; n];
    let mut states = initial_states(&model).unwrap();
    let options = NewtonOptions::new(1);
    let mut worst: f64 = 0.0;
    for k in 0..=steps {
        let (t, h) = if k == 0 { (0.0, 0.0) } else { (k as f64 * dt, dt) };
        let eq = solve_from_at(&model, &[], &load, &u, &states, &options, &[], &ctx(t, h)).unwrap();
        u = eq.u;
        states = eq.states;
        if k > 0 {
            let eps = xmax.iter().map(|&(_, q)| u[q]).sum::<f64>() / 4.0;
            let want = creep_closed(sig0, t);
            worst = worst.max((eps - want).abs() / want);
        }
    }
    worst
}

/// The material-point creep under stress control.
fn creep_point(per: usize) -> f64 {
    let m = equal(&[W_SLS], &[TAU_SLS], NU, None);
    let sig0 = 1e-3 * E0 * (1.0 - W_SLS);
    let dir = uniaxial(1.0, NU);
    let dt = TAU_SLS / per as f64;
    let steps = (5.0 * 2.0 * TAU_SLS / dt).round() as usize;
    let mut state = vec![0.0; m.state_size()];
    let mut eps = 0.0;
    let mut worst: f64 = 0.0;
    for k in 0..=steps {
        let (t, h) = if k == 0 { (0.0, 0.0) } else { (k as f64 * dt, dt) };
        let trial = mat_visco::update(&m, &uniaxial(eps, NU), &state, &ctx(t, h)).unwrap();
        let slope: f64 = (0..6).map(|j| trial.tangent[0][j] * dir[j]).sum();
        eps += (sig0 - trial.stress[0]) / slope;
        let up = mat_visco::update(&m, &uniaxial(eps, NU), &state, &ctx(t, h)).unwrap();
        state = up.state;
        if k > 0 {
            let want = creep_closed(sig0, t);
            worst = worst.max((eps - want).abs() / want);
        }
    }
    worst
}

#[test]
fn c1_c2_creep_of_the_standard_solid() {
    let mut c = Checks::new();
    let (e20, e40) = (creep_element(20), creep_element(40));
    c.at_most("C1 creep of one element, 20 steps per tau", e20, 1e-3);
    println!("C2 creep of one element, 40 steps per tau: {e40:.6e}");
    c.at_least("C2 observed order", (e20 / e40).log2(), 1.9);
    let (m20, m40) = (creep_point(20), creep_point(40));
    c.at_most("C1m creep at the material point, 20 steps per tau", m20, 1e-3);
    println!("C2m creep at the material point, 40 steps per tau: {m40:.6e}");
    c.at_least("C2m observed order", (m20 / m40).log2(), 1.9);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- M1, M2, M3

#[test]
fn m1_m2_m3_complex_modulus() {
    let mut c = Checks::new();
    let m = three();
    let einf = E0 * (1.0 - W3.iter().sum::<f64>());
    let mut worst: f64 = 0.0;
    for k in 0..=60 {
        let om = 10f64.powf(-3.0 + k as f64 / 10.0) / TAU3[1];
        let y = complex_moduli(&m, om, None).unwrap().young;
        let mut ep = einf;
        let mut epp = 0.0;
        for (wi, ti) in W3.iter().zip(&TAU3) {
            let ei = wi * E0;
            ep += ei * om * om * ti * ti / (1.0 + om * om * ti * ti);
            epp += ei * om * ti / (1.0 + om * om * ti * ti);
        }
        let mag = (ep * ep + epp * epp).sqrt();
        worst = worst.max((y.re - ep).abs().max((y.im - epp).abs()) / mag);
    }
    c.at_most(
        "M1 E' and E'' of the three-branch material, 61 frequencies",
        worst,
        1e-13,
    );

    let m = unequal();
    let terms = unequal_terms();
    let g0 = E0 / (2.0 * (1.0 + NU));
    let k0 = E0 / (3.0 * (1.0 - 2.0 * NU));
    let mut worst: f64 = 0.0;
    for k in 0..=60 {
        let om = 10f64.powf(-3.0 + k as f64 / 10.0) / 0.1;
        let cm = complex_moduli(&m, om, None).unwrap();
        for (modulus, m0, pick) in [(cm.shear, g0, 0), (cm.bulk, k0, 1)] {
            let w = |t: &(f64, f64, f64)| if pick == 0 { t.0 } else { t.1 };
            let mut s = m0 * (1.0 - terms.iter().map(w).sum::<f64>());
            let mut l = 0.0;
            for t in &terms {
                let x = om * t.2;
                s += m0 * w(t) * x * x / (1.0 + x * x);
                l += m0 * w(t) * x / (1.0 + x * x);
            }
            let mag = (s * s + l * l).sqrt();
            worst = worst.max((modulus.re - s).abs().max((modulus.im - l).abs()) / mag);
        }
    }
    c.at_most("M2 shear and bulk moduli, unequal weights", worst, 1e-13);

    // M3: the table for the direct harmonic solve.
    let freqs: Vec<f64> = (0..8).map(|k| 0.05 * 2f64.powi(k)).collect();
    let table = modulus_table(&three(), &freqs, None).unwrap();
    let (mut st, mut lo) = (Vec::new(), Vec::new());
    for &f in &freqs {
        let y = complex_moduli(&three(), 2.0 * PI * f, None).unwrap().young;
        st.push(y.re);
        lo.push(y.im / y.re);
    }
    let want = ModulusTable::new(freqs.clone(), st, lo).unwrap();
    let (refused, msg) = refused_with(modulus_table(&unequal(), &freqs, None), "terms");
    c.holds("M3 table equals the complex modulus", table == want, "bit for bit");
    c.holds("M3 table refused for unequal weights", refused, &msg);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- D1, D2

fn loss_modulus(om: f64) -> f64 {
    W3.iter()
        .zip(&TAU3)
        .map(|(wi, ti)| wi * E0 * om * ti / (1.0 + om * om * ti * ti))
        .sum()
}

#[test]
fn d1_d2_dissipation_per_cycle() {
    let mut c = Checks::new();
    let eps0 = 1e-4;
    let om = 10.0;
    let per = 200;
    let dt = 2.0 * PI / om / per as f64;
    let want = PI * eps0 * eps0 * loss_modulus(om);
    // D1: the material point.
    let m = three();
    let mut state = vec![0.0; m.state_size()];
    let (mut s_prev, mut e_prev) = ([0.0; 6], [0.0; 6]);
    let mut work = 0.0;
    let mut cycle = 0.0;
    for k in 0..=(10 * per) {
        let (t, h) = if k == 0 { (0.0, 0.0) } else { (k as f64 * dt, dt) };
        let e = uniaxial(eps0 * (om * t).sin(), NU);
        let up = mat_visco::update(&m, &e, &state, &ctx(t, h)).unwrap();
        state = up.state;
        if k > 0 {
            work += (0..6)
                .map(|i| 0.5 * (up.stress[i] + s_prev[i]) * (e[i] - e_prev[i]))
                .sum::<f64>();
        }
        s_prev = up.stress;
        e_prev = e;
        if k > 0 && k % per == 0 {
            cycle = work;
            work = 0.0;
        }
    }
    c.at_most(
        "D1 dissipation of the tenth cycle, material point",
        (cycle - want).abs() / want,
        1e-2,
    );

    // D2: one Hex8, the prescribed cycled u_x, the work of the reactions.
    let model = cube(three());
    let xmax = set_x_equations(&model, "xmax");
    let mut prev: Option<(Vec<f64>, Vec<f64>)> = None;
    let mut work = 0.0;
    let mut cycle = 0.0;
    let mut k = 0usize;
    cube_history(
        &model,
        |t| eps0 * (om * t).sin(),
        10 * per,
        dt,
        &NewtonOptions::new(1),
        ctx,
        |_, u, _, eq| {
            let r: Vec<f64> = xmax.iter().map(|&(_, q)| eq.internal_force[q]).collect();
            let x: Vec<f64> = xmax.iter().map(|&(_, q)| u[q]).collect();
            if let Some((r0, x0)) = &prev {
                work += (0..4).map(|i| 0.5 * (r[i] + r0[i]) * (x[i] - x0[i])).sum::<f64>();
            }
            prev = Some((r, x));
            if k > 0 && k % per == 0 {
                cycle = work;
                work = 0.0;
            }
            k += 1;
        },
    );
    c.at_most(
        "D2 dissipation of the tenth cycle, one element",
        (cycle - want).abs() / want,
        1e-2,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- S1, S1m, S2

const T_REF: f64 = 373.0;

fn wlf() -> TimeTemperatureShift {
    TimeTemperatureShift::wlf(T_REF, 17.44, 51.6).unwrap()
}

fn arrhenius() -> TimeTemperatureShift {
    TimeTemperatureShift::arrhenius(T_REF, 1e5).unwrap()
}

fn wlf_formula(t: f64) -> f64 {
    10f64.powf(-17.44 * (t - T_REF) / (51.6 + t - T_REF))
}

fn arrhenius_formula(t: f64) -> f64 {
    ((1e5 / GAS_CONSTANT) * (1.0 / t - 1.0 / T_REF)).exp()
}

#[test]
fn s1_s1m_s2_time_temperature_shift() {
    let mut c = Checks::new();
    let eps0 = 1e-3;
    let temp = T_REF + 10.0;
    let a = wlf_formula(temp);
    println!("S1 a_T = {a:.9e}");
    // S1: one Hex8 at T, WLF.
    let model = cube(equal(&W3, &TAU3, NU, Some(wlf())));
    let mut worst: f64 = 0.0;
    cube_history(
        &model,
        |_| eps0,
        100,
        0.02 * a,
        &tight(),
        |t, h| ctx(t, h).with_temperature(temp).unwrap(),
        |t, u, states, _| {
            let e = exx(&model, u);
            let want = e * relaxation(&W3, &TAU3, t / a);
            for s in stresses(&model, u, &states[0]) {
                worst = worst.max((s[0] - want).abs() / (E0 * e));
            }
        },
    );
    c.at_most("S1 shifted relaxation of one element, WLF", worst, 1e-10);

    // S1m: the material point, WLF and Arrhenius, uneven steps.
    for (name, shift, temp, a) in [
        ("WLF", wlf(), T_REF + 10.0, wlf_formula(T_REF + 10.0)),
        ("Arrhenius", arrhenius(), T_REF + 20.0, arrhenius_formula(T_REF + 20.0)),
    ] {
        let m = equal(&W3, &TAU3, NU, Some(shift));
        let strain = uniaxial(eps0, NU);
        let mut state = vec![0.0; m.state_size()];
        let mut t = 0.0;
        let mut worst: f64 = 0.0;
        for k in 0..=200 {
            let h = if k == 0 {
                0.0
            } else {
                0.013 * a * (1.0 + 0.5 * (k as f64).sin())
            };
            t += h;
            let p = ctx(t, h).with_temperature(temp).unwrap();
            let up = mat_visco::update(&m, &strain, &state, &p).unwrap();
            state = up.state;
            worst = worst.max((up.stress[0] - eps0 * relaxation(&W3, &TAU3, t / a)).abs() / (E0 * eps0));
        }
        c.at_most(
            &format!("S1m shifted relaxation at the material point, {name}"),
            worst,
            1e-10,
        );
    }

    // S2: the shift factor against the formulas.
    let mut worst: f64 = 0.0;
    for d in [-20.0, -5.0, 0.0, 10.0, 40.0] {
        let t = T_REF + d;
        worst = worst.max((shift_factor(&wlf(), t).unwrap() / wlf_formula(t) - 1.0).abs());
        worst = worst.max((shift_factor(&arrhenius(), t).unwrap() / arrhenius_formula(t) - 1.0).abs());
    }
    c.at_most("S2 shift factor, WLF and Arrhenius", worst, 1e-13);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- V1, V2, B1, H1

/// The rod of V1: 20 Hex8 along x, nu = 0, the lateral motion held.
fn rod(material: Material) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.05, 0.05], [20, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m
}

struct Rod {
    lambda: f64,
    phi: Vec<f64>,
    mass: CsrMatrix,
    v0: Vec<f64>,
}

fn rod_modes() -> Rod {
    let m = rod(base(0.0));
    let modal = modal_analysis(&m, &ModalOptions::new(2)).unwrap();
    let mass = consistent(&m).unwrap();
    let mut phi = modal.modes[0].clone();
    let norm = dot(&phi, &mv(&mass, &phi)).sqrt();
    for x in phi.iter_mut() {
        *x /= norm;
    }
    let lambda = modal.eigenvalues[0];
    let omega = lambda.sqrt();
    let peak = phi.iter().fold(0.0, |a: f64, b| a.max(b.abs()));
    // The largest strain of the quarter wave is about (pi/2) u_tip/L with u_tip = (A/omega) peak.
    let amp = 1e-6 * omega / (peak * PI / 2.0);
    let v0 = phi.iter().map(|x| amp * x).collect();
    Rod { lambda, phi, mass, v0 }
}

#[test]
fn v1_v2_free_vibration_of_a_bar() {
    let mut c = Checks::new();
    let r = rod_modes();
    let w = 2.0 / 21.0;
    let omega1 = (20.0 / 21.0 * r.lambda).sqrt();
    let tau = 1.0 / omega1;
    c.holds(
        "V set-up omega_1 tau = 1",
        (omega1 * tau - 1.0).abs() <= 1e-12,
        &format!("{:e}", omega1 * tau - 1.0),
    );
    let model = rod(equal(&[w], &[tau], 0.0, None));
    let period = 2.0 * PI / omega1;
    let dt = period / 200.0;
    let mut it = NlIntegrator::new(&model, &[], &NlTransientOptions::new(dt)).unwrap();
    let n = r.phi.len();
    it.start(0.0, &vec![0.0; n], &r.v0).unwrap();
    let mphi = mv(&r.mass, &r.phi);
    let mut q = vec![0.0];
    for _ in 0..1600 {
        it.step().unwrap();
        q.push(dot(&mphi, &it.displacement().unwrap()));
    }
    let mut peaks = Vec::new();
    for i in 1..q.len() - 1 {
        if q[i] > q[i - 1] && q[i] >= q[i + 1] {
            let (y0, y1, y2) = (q[i - 1], q[i], q[i + 1]);
            let p = 0.5 * (y0 - y2) / (y0 - 2.0 * y1 + y2);
            peaks.push(y1 - 0.25 * (y0 - y2) * p);
        }
    }
    c.holds("V set-up maxima", peaks.len() >= 7, &format!("{} maxima", peaks.len()));
    let delta = (peaks[1] / peaks[6]).ln() / 5.0;
    let eta = 0.05;
    println!("V1 delta = {delta:.9}, pi eta = {:.9}", PI * eta);
    c.at_most(
        "V1 logarithmic decrement against pi E''/E'",
        (delta / (PI * eta) - 1.0).abs(),
        3e-2,
    );
    // V2: the root of s^2 + lambda_1 (w_inf + w s tau/(1 + s tau)) = 0.
    let lam = r.lambda;
    let mut s = C64::new(0.0, (20.0 / 21.0 * lam).sqrt());
    for _ in 0..50 {
        let one_st = C64::ONE + s.scale(tau);
        let f = s * s + (C64::from_real(1.0 - w) + (s.scale(w * tau) / one_st)).scale(lam);
        let df = s.scale(2.0) + (C64::from_real(w * tau) / (one_st * one_st)).scale(lam);
        s -= f / df;
    }
    let d_root = 2.0 * PI * s.re.abs() / s.im;
    println!("V2 decrement of the root {d_root:.9}");
    c.at_most(
        "V2 decrement against the exact root",
        (delta / d_root - 1.0).abs(),
        5e-3,
    );
    c.finish();
}

#[test]
fn b1_no_branches_gives_the_elastic_bits() {
    let mut c = Checks::new();
    let plain = Material::viscoelastic(base(NU), Vec::new(), None).unwrap();
    c.holds(
        "B1 state size",
        plain.state_size() == 0,
        &format!("{}", plain.state_size()),
    );
    let mut seed: u64 = 0xF410;
    let mut rnd = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((seed >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * 2e-3
    };
    let mut same = true;
    for _ in 0..20 {
        let e = [rnd(), rnd(), rnd(), rnd(), rnd(), rnd()];
        let (s1, d1) = plain.stress(&e, &ctx(1.0, 0.1), &[]).unwrap();
        let (s2, d2) = base(NU).stress(&e, &ctx(1.0, 0.1), &[]).unwrap();
        same &= bits(&s1) == bits(&s2) && bits(&d1.concat()) == bits(&d2.concat());
    }
    c.holds("B1 Material::stress, 20 strains", same, "bits");
    // solve_from_at of the cube: the step and two increments.
    let run = |m: Material| {
        let model = cube(m);
        let mut v = Vec::new();
        cube_history(
            &model,
            |_| 1e-3,
            2,
            0.05,
            &tight(),
            ctx,
            |_, u, states, eq| {
                v.extend_from_slice(u);
                v.extend_from_slice(&eq.internal_force);
                for s in states {
                    v.extend_from_slice(s.values());
                }
                v.push(states.iter().map(|s| s.values().len()).sum::<usize>() as f64);
            },
        );
        v
    };
    c.holds(
        "B1 solve_from_at",
        bits(&run(plain.clone())) == bits(&run(base(NU))),
        "bits",
    );
    // nl_transient of the rod, 100 steps.
    let r = rod_modes();
    let period = 2.0 * PI / r.lambda.sqrt();
    let run = |m: Material| {
        let model = rod(m);
        let mut it = NlIntegrator::new(&model, &[], &NlTransientOptions::new(period / 200.0)).unwrap();
        it.start(0.0, &vec![0.0; r.phi.len()], &r.v0).unwrap();
        let mut v = Vec::new();
        for _ in 0..100 {
            it.step().unwrap();
            let s = it.state().unwrap();
            v.extend_from_slice(&s.u);
            v.extend_from_slice(&s.v);
            v.extend_from_slice(&s.a);
            v.extend_from_slice(&s.internal_force);
        }
        v
    };
    let plain0 = Material::viscoelastic(base(0.0), Vec::new(), None).unwrap();
    c.holds(
        "B1 nl_transient, 100 steps",
        bits(&run(plain0)) == bits(&run(base(0.0))),
        "bits",
    );
    c.finish();
}

#[test]
fn h1_save_and_restore_with_the_internal_stresses() {
    let mut c = Checks::new();
    let r = rod_modes();
    let omega1 = (20.0 / 21.0 * r.lambda).sqrt();
    let tau: Vec<f64> = TAU3.iter().map(|t| t / 0.1 / omega1).collect();
    let model = rod(equal(&W3, &tau, 0.0, None));
    let dt = 2.0 * PI / omega1 / 200.0;
    let fresh = || {
        let mut it = NlIntegrator::new(&model, &[], &NlTransientOptions::new(dt)).unwrap();
        it.start(0.0, &vec![0.0; r.phi.len()], &r.v0).unwrap();
        it
    };
    let mut a = fresh();
    a.run(40).unwrap();
    let mut b = fresh();
    b.run(17).unwrap();
    let bytes = b.state().unwrap().to_bytes();
    let loaded = NlTransientState::from_bytes(&bytes).unwrap();
    c.holds("H1 set-up bytes", loaded.to_bytes() == bytes, "save, load, save");
    let nonzero = loaded.states.iter().any(|s| s.values().iter().any(|&x| x != 0.0));
    c.holds("H1 set-up states non-zero", nonzero, "");
    let mut b2 = NlIntegrator::new(&model, &[], &NlTransientOptions::new(dt)).unwrap();
    b2.restore(loaded).unwrap();
    b2.run(23).unwrap();
    let (sa, sb) = (a.state().unwrap(), b2.state().unwrap());
    let flat = |s: &NlTransientState| {
        let mut v = s.u.clone();
        v.extend_from_slice(&s.v);
        v.extend_from_slice(&s.a);
        v.extend_from_slice(&s.internal_force);
        for e in &s.states {
            v.extend_from_slice(e.values());
        }
        v
    };
    c.holds(
        "H1 split run equals the unsplit run",
        bits(&flat(&sa)) == bits(&flat(&sb)) && sa.step == sb.step,
        &format!("{} values", flat(&sa).len()),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- RF1, T1, N2

#[test]
fn rf1_t1_n2_refusals_tangent_and_negative_controls() {
    let mut c = Checks::new();
    // RF1.
    let term = |g: f64, k: f64, t: f64| PronyTerm::new(g, k, t);
    let rows: Vec<(&str, (bool, String))> = vec![
        ("tau 0", refused_with(term(0.1, 0.1, 0.0), "tau")),
        ("tau -1", refused_with(term(0.1, 0.1, -1.0), "tau")),
        ("tau NaN", refused_with(term(0.1, 0.1, f64::NAN), "tau")),
        ("both weights 0", refused_with(term(0.0, 0.0, 1.0), "shear")),
        ("shear weight -0.1", refused_with(term(-0.1, 0.1, 1.0), "shear")),
        ("bulk weight -0.1", refused_with(term(0.1, -0.1, 1.0), "bulk")),
        (
            "base E 0",
            refused_with(Material::isotropic_elastic(0.0, NU, 1200.0, 0.0), "e"),
        ),
        (
            "base E -1",
            refused_with(Material::isotropic_elastic(-1.0, NU, 1200.0, 0.0), "e"),
        ),
        (
            "shear weights sum 1",
            refused_with(
                Material::viscoelastic(
                    base(NU),
                    vec![term(0.6, 0.1, 1.0).unwrap(), term(0.4, 0.1, 2.0).unwrap()],
                    None,
                ),
                "shear",
            ),
        ),
        (
            "bulk weights sum 1",
            refused_with(
                Material::viscoelastic(
                    base(NU),
                    vec![term(0.1, 0.5, 1.0).unwrap(), term(0.1, 0.5, 2.0).unwrap()],
                    None,
                ),
                "bulk",
            ),
        ),
    ];
    for (name, (ok, msg)) in rows {
        c.holds(&format!("RF1 {name}"), ok, &msg);
    }

    // T1: the tangent against central differences, unequal weights, a non-zero committed state.
    let m = unequal();
    let e1 = [1e-3, -4e-4, 2e-4, 3e-4, -2e-4, 1e-4];
    let s1 = mat_visco::update(&m, &e1, &vec![0.0; m.state_size()], &ctx(0.0, 0.0))
        .unwrap()
        .state;
    let s2 = mat_visco::update(&m, &[1.5e-3, -5e-4, 1e-4, 4e-4, -1e-4, 2e-4], &s1, &ctx(0.02, 0.02))
        .unwrap()
        .state;
    let e = [1.7e-3, -6e-4, 3e-4, 2e-4, -3e-4, 1.5e-4];
    let p = ctx(0.05, 0.03);
    let up = mat_visco::update(&m, &e, &s2, &p).unwrap();
    let h = 1e-7;
    let mut diff: f64 = 0.0;
    let mut big: f64 = 0.0;
    for j in 0..6 {
        let (mut ep, mut em) = (e, e);
        ep[j] += h;
        em[j] -= h;
        let sp = mat_visco::update(&m, &ep, &s2, &p).unwrap().stress;
        let sm = mat_visco::update(&m, &em, &s2, &p).unwrap().stress;
        for i in 0..6 {
            let fd = (sp[i] - sm[i]) / (2.0 * h);
            diff = diff.max((up.tangent[i][j] - fd).abs());
            big = big.max(up.tangent[i][j].abs());
        }
    }
    c.at_most("T1 tangent against central differences", diff / big, 1e-6);

    // N2: the linear paths refuse the material.
    let vm = cube(three());
    let rows: Vec<(&str, (bool, String))> = vec![
        (
            "solve_static",
            not_yet(&solve_static(&vm, &StaticOptions::default()), "element set 'all'"),
        ),
        (
            "modal_analysis",
            not_yet(&modal_analysis(&vm, &ModalOptions::new(1)), "element set 'all'"),
        ),
        (
            "transient::Integrator::new",
            not_yet(
                &Integrator::new(&vm, &[], &TransientOptions::new(1e-3)),
                "element set 'all'",
            ),
        ),
        ("stiffness", not_yet(&stiffness(&vm), "element set 'all'")),
        (
            "elasticity_matrix",
            not_yet(&three().elasticity_matrix(), "the element-constant elasticity matrix"),
        ),
        (
            "nonlinear::solve",
            not_yet(
                &solve(&vm, &[], &[], &NewtonOptions::new(1)),
                "the nonlinear static solve (no time)",
            ),
        ),
    ];
    for (name, (ok, msg)) in rows {
        c.holds(&format!("N2 {name} refused"), ok, &msg);
    }
    let ortho = Material::orthotropic(
        [1.4e11, 1.0e10, 1.0e10],
        [0.3, 0.3, 0.45],
        [5.0e9, 5.0e9, 3.5e9],
        1600.0,
        [0.0; 3],
        MaterialAxes::fixed([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]).unwrap(),
    )
    .unwrap();
    let vo = Material::viscoelastic(ortho, vec![PronyTerm::new(0.2, 0.2, 1.0).unwrap()], None).unwrap();
    let (ok, msg) = not_yet(&mat_visco::update(&vo, &e, &[0.0; 12], &p), "an orthotropic base");
    c.holds("N2 orthotropic base refused", ok, &msg);
    let shifted = equal(&W3, &TAU3, NU, Some(wlf()));
    let (ok, msg) = refused_with(mat_visco::update(&shifted, &e, &[0.0; 24], &p), "temperature");
    c.holds("N2 shift without a temperature refused", ok, &msg);
    let cold = p.clone().with_temperature(T_REF - 51.6).unwrap();
    let (ok, msg) = refused_with(mat_visco::update(&shifted, &e, &[0.0; 24], &cold), "temperature");
    c.holds("N2 WLF with c2 + T - T_ref = 0 refused", ok, &msg);
    c.finish();
}
