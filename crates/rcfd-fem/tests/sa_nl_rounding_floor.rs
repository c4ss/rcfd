//! F-571 (ADR 0604): the rounding floor of the Newton residual of the total Lagrangian step under large translation and
//! rotation. Checks A1, A1h, A1r (S3 of ADR 0479 amendment 2 with the driver defaults), A2 (against the relative test at a
//! looser tolerance) and P1 (the pendulum of ADR 0479 P0 to P2 with the driver defaults). Helpers copied from
//! `sa_nl_transient.rs` (not edited).

use rcfd_fem::builders::hex_box;
use rcfd_fem::nl_transient::{Integrator as NlIntegrator, NlTransientOptions};
use rcfd_fem::nonlinear::NewtonOptions;
use rcfd_fem::transient::{LoadHistory, LoadTable};
use rcfd_fem::{Constraint, Load, Material, SolidModel};
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

const E_STEEL: f64 = 2.0e11;
const RHO: f64 = 7850.0;

fn t_lin() -> f64 {
    2.0 * PI * (RHO / (3.0 * E_STEEL)).sqrt()
}

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

/// What a run of S3 gives: the error of the first failed step, the displacement history (step 0 included), the largest
/// `|P_n - P_0|/|P_0|`, the total halvings and the largest final relative residual of a step.
struct S3Run {
    error: Option<String>,
    history: Vec<Vec<f64>>,
    dp: f64,
    halvings: usize,
    residual: f64,
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
        residual: 0.0,
    };
    for k in 0..50 {
        match it.step() {
            Ok(r) => {
                run.halvings += r.halvings;
                run.residual = run.residual.max(r.residual);
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
fn a1_a1h_a1r_a2_s3_free_hex8_with_the_driver_defaults() {
    let mut c = Checks::new();
    let run = s3_run(NlTransientOptions::new(t_lin() / 200.0));
    match &run.error {
        None => c.holds(
            "A1 S3 every step converges with the driver defaults",
            true,
            "50 of 50 steps",
        ),
        Some(e) => c.holds("A1 S3 every step converges with the driver defaults", false, e),
    }
    c.at_most("A1 S3 max |P_n - P_0|/|P_0|", run.dp, 1e-11);
    c.at_most("A1h S3 total halvings", run.halvings as f64, 0.0);
    c.at_least("A1r S3 largest final relative residual of a step", run.residual, 1e-10);
    let mut newton = NewtonOptions::new(1);
    newton.residual_tolerance = 1e-6;
    let loose = s3_run(NlTransientOptions::new(t_lin() / 200.0).newton(newton));
    c.holds(
        "A2 (set-up) the run at residual tolerance 1e-6 completes",
        loose.error.is_none(),
        loose.error.as_deref().unwrap_or("50 of 50 steps"),
    );
    if run.history.len() == loose.history.len() {
        let scale = loose.history.iter().fold(0.0f64, |m, u| m.max(inf_norm(u)));
        let d = run.history.iter().zip(&loose.history).fold(0.0f64, |m, (x, y)| {
            m.max(x.iter().zip(y).fold(0.0f64, |mm, (p, q)| mm.max((p - q).abs())))
        });
        c.at_most(
            "A2 S3 defaults against tolerance 1e-6, max |du|_inf / max |u|_inf",
            d / scale,
            1e-9,
        );
    } else {
        c.holds("A2 S3 histories of equal length", false, "a run stopped");
    }
    c.finish();
}

// --------------------------------------------------------------------------- the pendulum of ADR 0479 (P1 of ADR 0604)

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
#[ignore = "heavy record of ADR 0604 P1 (1940 steps, most at the rounding floor with 20 corrections each); run once with --ignored; the fast test on the same code path is a1_a1h_a1r_a2_s3_free_hex8_with_the_driver_defaults"]
fn p1_pendulum_released_from_the_horizontal_with_the_driver_defaults() {
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
    let mut it = NlIntegrator::new(&model, &loads, &NlTransientOptions::new(dt)).unwrap();
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
    let mut halvings = 0;
    let mut largest = 0.0f64;
    let mut c = Checks::new();
    let mut failed = None;
    for k in 1..=steps {
        match it.step() {
            Ok(r) => {
                halvings += r.halvings;
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
        None => c.holds(
            "P1 every step converges with the driver defaults",
            true,
            "1940 of 1940 steps",
        ),
        Some(e) => c.holds("P1 every step converges with the driver defaults", false, e),
    }
    println!("P1 largest final relative residual of a step: {largest:e}");
    let t_closed = 2.0 * PI * (2.0 * L_BAR / (3.0 * G)).sqrt() * 1.180340599016096;
    let n = crossings.len();
    c.at_least("P1 crossings of psi = 0", n as f64, 9.0);
    if n >= 2 {
        let t_fe = 2.0 * (crossings[n - 1] - crossings[0]) / (n - 1) as f64;
        c.at_most("P1 period |T_FE - T|/T", (t_fe - t_closed).abs() / t_closed, 1e-2);
    }
    c.at_most("P1 energy max |E - E_0|/(m g L/2)", e_dev / swing, 1e-3);
    c.at_most("P1 total halvings", halvings as f64, 19.0);
    c.finish();
}
