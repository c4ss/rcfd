//! F-691 (ADR 0625): a negative control of the energy-momentum inertia-term reference off rest. One step of the clamped
//! cantilever of ADR 0609 centred on a zero crossing (`u_0 = -(h_c/2) v_0`) at `h_c = T_1/(40 x 1024 x 64)`: C1 (the
//! inertia-term reference converges without halving), N1 and N1b (the driver reference refused at the rounding floor), DZ (the
//! diagnostic step from rest at `h_c`). Each registered run is made once; every value is printed. Every aggregated value is
//! required finite before it enters a maximum (an `f64::max` would hide a `NaN`).

use rcfd_fem::builders::hex_box;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::nl_transient::{Integrator as NlIntegrator, NlTransientOptions, ResidualReference};
use rcfd_fem::{Constraint, FemError, Material, SolidModel};
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
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value > limit;
        println!(
            "{name}: {value:.6e} (limit > {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} not > {limit:e}"));
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

/// The running maximum of finite values; a non-finite value makes the maximum `NaN` (and so misses its check).
fn worst(acc: &mut f64, value: f64) {
    *acc = if value.is_finite() && acc.is_finite() {
        acc.max(value)
    } else {
        f64::NAN
    };
}

fn max_abs(v: impl IntoIterator<Item = f64>) -> f64 {
    let mut m = 0.0;
    for x in v {
        worst(&mut m, x.abs());
    }
    m
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn soft() -> Material {
    Material::isotropic_elastic(2.0e9, 0.3, 7850.0, 0.0).unwrap()
}

fn is_step_refusal(e: &FemError) -> bool {
    matches!(e, FemError::Solver { solver, .. } if *solver == "nl_transient")
}

/// The clamped cantilever of ADR 0609, `f_1`, and `v_0 = omega_1 a phi/phi_max` with `a = 0.05 L` (copied from
/// `sa_emc_inertia_reference.rs`).
fn cantilever() -> (SolidModel, f64, Vec<f64>) {
    let mut mesh = hex_box([0.0, -0.01, -0.02], [1.0, 0.02, 0.04], [10, 2, 2]).unwrap();
    let nodes = mesh.nodes().to_vec();
    let root: Vec<usize> = (0..nodes.len()).filter(|&i| near(nodes[i][0], 0.0)).collect();
    mesh.add_node_set("root", root).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", soft()).unwrap();
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("root", comp, 0.0)).unwrap();
    }
    let mut mo = ModalOptions::new(1);
    mo.max_iterations = 2000;
    mo.tolerance = 1e-8;
    let modal = modal_analysis(&m, &mo).unwrap();
    let f1 = modal.frequencies[0];
    let phi = &modal.modes[0];
    let pmax = phi.iter().fold(0.0f64, |a, x| if x.abs() > a.abs() { *x } else { a });
    let omega = 2.0 * PI * f1;
    let v0: Vec<f64> = phi.iter().map(|x| omega * 0.05 * x / pmax).collect();
    println!("cantilever: f_1 {f1:.9e} Hz, phi_max {pmax:e}");
    (m, f1, v0)
}

fn emc(dt: f64, reference: ResidualReference) -> NlTransientOptions {
    NlTransientOptions::new(dt)
        .energy_momentum()
        .residual_reference(reference)
}

/// The registered step `h_c = T_1/(40 x 1024 x 64)` (ADR 0625, c1).
fn h_c(f1: f64) -> f64 {
    1.0 / (40.0 * f1) / 1024.0 / 64.0
}

/// The start centred on the zero crossing: `u_0 = -(h/2) v_0` (ADR 0625, c1).
fn centred_start(h: f64, v0: &[f64]) -> Vec<f64> {
    v0.iter().map(|x| -0.5 * (h * x)).collect()
}

/// The number after `key` in an error text, up to the next `,` or `)`.
fn number_after(text: &str, key: &str) -> f64 {
    match text.rfind(key) {
        Some(p) => {
            let rest = &text[p + key.len()..];
            let end = rest.find([',', ')']).unwrap_or(rest.len());
            rest[..end].trim().parse().unwrap_or(f64::NAN)
        }
        None => f64::NAN,
    }
}

/// C1: the centred step at `h_c` with the inertia-term reference converges without halving, and its state is the centred
/// step's.
#[test]
fn c1_the_centred_step_converges_with_the_inertia_term_reference() {
    let mut c = Checks::new();
    let (m, f1, v0) = cantilever();
    let h = h_c(f1);
    let u0 = centred_start(h, &v0);
    println!("h_c {h:e} s, omega h_c {:e}", 2.0 * PI * f1 * h);
    let mut it = NlIntegrator::new(&m, &[], &emc(h, ResidualReference::InertiaTerms)).unwrap();
    it.start(0.0, &u0, &v0).unwrap();
    let h0 = it.kinetic_energy().unwrap() + it.strain_energy().unwrap();
    match it.step() {
        Ok(rep) => {
            println!(
                "C1 step: {} halvings, {} iterations, residual {:e}",
                rep.halvings, rep.iterations, rep.residual
            );
            c.holds("C1 converges", true, "Ok");
            c.holds(
                "C1 halvings 0",
                rep.halvings == 0,
                &format!("{} halvings", rep.halvings),
            );
            c.at_most("C1 final relative residual", rep.residual, 1e-10);
            let u1 = it.displacement().unwrap();
            let v1 = it.velocity().unwrap();
            let su = max_abs(u1.iter().zip(&u0).map(|(a, b)| a + b)) / max_abs(v0.iter().map(|x| h * x));
            c.at_most("C1 max |u_1 + u_0| / max |h_c v_0|", su, 1e-8);
            let sv = max_abs(v1.iter().zip(&v0).map(|(a, b)| a - b)) / max_abs(v0.iter().copied());
            c.at_most("C1 max |v_1 - v_0| / max |v_0|", sv, 1e-8);
            let h1 = it.kinetic_energy().unwrap() + it.strain_energy().unwrap();
            c.at_most("C1 |H_1 - H_0|/H_0", ((h1 - h0) / h0).abs(), 1e-8);
        }
        Err(e) => c.holds("C1 converges", false, &format!("refused: {e}")),
    }
    c.finish();
}

/// N1 and N1b: the same step with the driver reference is refused after the 5 halvings, the last level at the rounding floor
/// (its relative residual above the tolerance with the corrections shrunk to rounding).
#[test]
fn n1_the_centred_step_refused_with_the_driver_reference() {
    let mut c = Checks::new();
    let (m, f1, v0) = cantilever();
    let h = h_c(f1);
    let u0 = centred_start(h, &v0);
    let mut it = NlIntegrator::new(&m, &[], &emc(h, ResidualReference::Driver)).unwrap();
    it.start(0.0, &u0, &v0).unwrap();
    match it.step() {
        Ok(rep) => {
            c.holds(
                "N1 Driver refused",
                false,
                &format!(
                    "converged: {} halvings, {} iterations, residual {:e}",
                    rep.halvings, rep.iterations, rep.residual
                ),
            );
            c.holds("N1b last level at the floor", false, "no refusal to read");
        }
        Err(e) => {
            let text = e.to_string();
            c.holds(
                "N1 Driver refused",
                is_step_refusal(&e) && text.contains("after 5 halvings"),
                &text,
            );
            c.above(
                "N1b last level relative residual",
                number_after(&text, "relative residual "),
                1e-10,
            );
            c.at_most(
                "N1b last level energy ratio",
                number_after(&text, "energy ratio "),
                1e-20,
            );
        }
    }
    c.finish();
}

/// DZ (diagnostic, not in the decision rule of ADR 0625): the step from rest at `h_c` with the driver reference; predicted
/// refused. Recorded either way: the test asserts nothing.
#[test]
fn dz_the_step_from_rest_at_h_c_with_the_driver_reference_diagnostic() {
    let (m, f1, v0) = cantilever();
    let h = h_c(f1);
    let n = v0.len();
    let mut it = NlIntegrator::new(&m, &[], &emc(h, ResidualReference::Driver)).unwrap();
    it.start(0.0, &vec![0.0; n], &v0).unwrap();
    match it.step() {
        Ok(rep) => println!(
            "DZ (predicted refused): converged, {} halvings, {} iterations, residual {:e}",
            rep.halvings, rep.iterations, rep.residual
        ),
        Err(e) => println!(
            "DZ (predicted refused): refused ({}): {e}",
            if is_step_refusal(&e) { "nl_transient" } else { "other" }
        ),
    }
}
