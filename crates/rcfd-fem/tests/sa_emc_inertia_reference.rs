//! F-681 (ADR 0609): the inertia-term residual reference on the energy-momentum step of `nl_transient`. RT1, FZ, NZ, S, NS,
//! V1, DB1, DB2 (fast) and V (heavy, ignored, run once). Each registered run is made once; every value is printed. Every
//! aggregated value is required finite before it enters a maximum (an `f64::max` would hide a `NaN`).

use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::mass::consistent;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::nl_transient::{Integrator as NlIntegrator, NlTransientOptions, ResidualReference};
use rcfd_fem::nonlinear::{solve_with, Contribution, DriverExtensions, NewtonOptions};
use rcfd_fem::{Constraint, FemError, Material, SolidModel};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;
use std::time::Instant;

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

fn soft() -> Material {
    Material::isotropic_elastic(2.0e9, 0.3, 7850.0, 0.0).unwrap()
}

fn is_step_refusal(e: &FemError) -> bool {
    matches!(e, FemError::Solver { solver, .. } if *solver == "nl_transient")
}

// ------------------------------------------------------------------------------------------- the cantilever: FZ, V

/// The clamped cantilever of ADR 0609, `f_1`, and `v_0 = omega_1 a phi/phi_max` with `a = 0.05 L`.
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

/// RT1: the energy-momentum step takes the inertia-term reference (the refusal of ADR 0603 D2 lifted), and the driver's.
#[test]
fn rt1_inertia_terms_taken_by_the_energy_momentum_step() {
    let mut c = Checks::new();
    let (m, f1, _) = cantilever();
    let h = 1.0 / (40.0 * f1);
    for (name, r) in [
        ("RT1 taken with InertiaTerms", ResidualReference::InertiaTerms),
        ("RT1 taken with Driver", ResidualReference::Driver),
    ] {
        match NlIntegrator::new(&m, &[], &emc(h, r)) {
            Ok(_) => c.holds(name, true, "Ok"),
            Err(e) => c.holds(name, false, &format!("refused: {e}")),
        }
    }
    c.finish();
}

/// FZ and NZ: one step from the start (a zero crossing) at `h_f = 1/(40 f_1 1024)` with the inertia-term reference and with
/// the driver's (ADR 0609).
#[test]
fn fz_nz_a_step_at_the_zero_crossing() {
    let mut c = Checks::new();
    let (m, f1, v0) = cantilever();
    let n = v0.len();
    let hf = 1.0 / (40.0 * f1) / 1024.0;
    let omega = 2.0 * PI * f1;
    // FZ.
    let mut it = NlIntegrator::new(&m, &[], &emc(hf, ResidualReference::InertiaTerms)).unwrap();
    it.start(0.0, &vec![0.0; n], &v0).unwrap();
    match it.step() {
        Ok(rep) => {
            println!(
                "FZ step: {} halvings, {} iterations, residual {:e}",
                rep.halvings, rep.iterations, rep.residual
            );
            c.holds(
                "FZ converges with 0 halvings",
                rep.halvings == 0,
                &format!("{} halvings", rep.halvings),
            );
            c.at_most("FZ final relative residual", rep.residual, 1e-10);
            let u = it.displacement().unwrap();
            let scale = max_abs(v0.iter().map(|x| hf * x));
            let k = 1.0 + (omega * hf).powi(2) / 4.0;
            let e = max_abs(u.iter().zip(&v0).map(|(ui, vi)| ui - hf * vi / k)) / scale;
            c.at_most("FZ |u_1 - h_f v0/(1 + (omega h_f)^2/4)| / |h_f v0|", e, 1e-8);
        }
        Err(e) => c.holds("FZ converges with 0 halvings", false, &format!("refused: {e}")),
    }
    // NZ.
    let mut it = NlIntegrator::new(&m, &[], &emc(hf, ResidualReference::Driver)).unwrap();
    it.start(0.0, &vec![0.0; n], &v0).unwrap();
    match it.step() {
        Ok(rep) => c.holds(
            "NZ Driver refused",
            false,
            &format!("converged: {} halvings, residual {:e}", rep.halvings, rep.residual),
        ),
        Err(e) => c.holds("NZ Driver refused", is_step_refusal(&e), &format!("{e}")),
    }
    c.finish();
}

/// What a run of the cantilever measured.
struct CantRun {
    steps: u64,
    error: Option<String>,
    halvings: usize,
    iterations: usize,
    residual: f64,
    energy: f64,
}

fn run_cantilever(steps: u64) -> CantRun {
    let (m, f1, v0) = cantilever();
    let n = v0.len();
    let mut it = NlIntegrator::new(&m, &[], &emc(1.0 / (40.0 * f1), ResidualReference::InertiaTerms)).unwrap();
    it.start(0.0, &vec![0.0; n], &v0).unwrap();
    let h0 = it.kinetic_energy().unwrap() + it.strain_energy().unwrap();
    let mut r = CantRun {
        steps: 0,
        error: None,
        halvings: 0,
        iterations: 0,
        residual: 0.0,
        energy: 0.0,
    };
    for _ in 0..steps {
        match it.step() {
            Ok(rep) => {
                r.halvings += rep.halvings;
                r.iterations += rep.iterations;
                worst(&mut r.residual, rep.residual);
            }
            Err(e) => {
                r.error = Some(e.to_string());
                break;
            }
        }
        r.steps += 1;
        let hn = it.kinetic_energy().unwrap() + it.strain_energy().unwrap();
        worst(&mut r.energy, ((hn - h0) / h0).abs());
    }
    println!(
        "cantilever, {} of {steps} steps: {} halvings, {} iterations, largest final residual {:e}, energy {:e}, error {:?}",
        r.steps, r.halvings, r.iterations, r.residual, r.energy, r.error
    );
    r
}

/// V1: one period of the cantilever with the inertia-term reference (the fast companion of V).
#[test]
fn v1_one_period_of_the_cantilever() {
    let mut c = Checks::new();
    let r = run_cantilever(40);
    c.holds(
        "V1 40 steps converged",
        r.steps == 40 && r.error.is_none(),
        &format!("{} steps, {:?}", r.steps, r.error),
    );
    c.holds("V1 halvings 0", r.halvings == 0, &format!("{}", r.halvings));
    c.at_most(
        "V1 max |H_n - H_0|/H_0",
        if r.steps == 40 { r.energy } else { f64::NAN },
        1e-6,
    );
    c.finish();
}

/// V: 100 periods of the cantilever with the inertia-term reference at the default tolerance (ADR 0609).
#[test]
#[ignore = "heavy registered record of ADR 0609 V (100 periods of the cantilever, 4000 steps); run once with --ignored"]
fn v_cantilever_100_periods_inertia_term_reference() {
    let mut c = Checks::new();
    let t = Instant::now();
    let r = run_cantilever(4000);
    println!("V time {:.1} s", t.elapsed().as_secs_f64());
    c.holds(
        "V 4000 steps converged",
        r.steps == 4000 && r.error.is_none(),
        &format!("{} steps, {:?}", r.steps, r.error),
    );
    c.holds("V halvings 0", r.halvings == 0, &format!("{}", r.halvings));
    c.at_most("V largest final relative residual", r.residual, 1e-10);
    c.at_most(
        "V max |H_n - H_0|/H_0",
        if r.steps == 4000 { r.energy } else { f64::NAN },
        1e-6,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------- the spinning bar: S, NS

/// The centrifugal force of the rotating frame, `-omega^2 M P (X + u - c)`, with its tangent (ADR 0479 D5; copied from
/// `sa_emc.rs`, used only for the steady start).
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

/// The free spinning bar of ADR 0479 D5 and ADR 0514 c1 (`spin()` of `sa_emc.rs`): the model, `u_0`, `v_0`.
fn spin() -> (SolidModel, Vec<f64>, Vec<f64>) {
    let omega = 2.0 * PI;
    let centre = [0.5, 0.0, 0.0];
    let velocity = [0.5, 0.25, 0.1];
    let mut model = SolidModel::new(hex_box([0.0, -0.01, -0.01], [1.0, 0.02, 0.02], [10, 2, 2]).unwrap());
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

/// S and NS: the spinning bar over ten turns at the default tolerance with the inertia-term reference, and the first step
/// with the driver's (ADR 0609).
#[test]
fn s_ns_spinning_bar_ten_turns_at_the_default_tolerance() {
    let mut c = Checks::new();
    let (model, u0, v0) = spin();
    // S.
    let mut it = NlIntegrator::new(&model, &[], &emc(0.01, ResidualReference::InertiaTerms)).unwrap();
    it.start(0.0, &u0, &v0).unwrap();
    let energy = |it: &NlIntegrator| it.kinetic_energy().unwrap() + it.strain_energy().unwrap();
    let (p0, j0) = it.momentum().unwrap();
    let e0 = energy(&it);
    let (mut steps, mut halvings, mut iterations) = (0u64, 0usize, 0usize);
    let (mut residual, mut dp, mut dj, mut de) = (0.0, 0.0, 0.0, 0.0);
    let mut error = None;
    for _ in 0..1000 {
        match it.step() {
            Ok(rep) => {
                halvings += rep.halvings;
                iterations += rep.iterations;
                worst(&mut residual, rep.residual);
            }
            Err(e) => {
                error = Some(e.to_string());
                break;
            }
        }
        steps += 1;
        let (p, j) = it.momentum().unwrap();
        worst(&mut dp, norm3(sub3(p, p0)) / norm3(p0));
        worst(&mut dj, norm3(sub3(j, j0)) / norm3(j0));
        worst(&mut de, (energy(&it) - e0).abs() / e0.abs());
    }
    println!("S: {steps} steps, {halvings} halvings, {iterations} iterations, largest final residual {residual:e}, {error:?}");
    let done = steps == 1000 && error.is_none();
    c.holds("S 1000 steps converged", done, &format!("{steps} steps, {error:?}"));
    c.holds("S halvings 0", halvings == 0, &format!("{halvings}"));
    c.at_most("S largest final relative residual", residual, 1e-10);
    let gate = |x: f64| if done { x } else { f64::NAN };
    c.at_most("S linear momentum max |P - P_0|/|P_0|", gate(dp), 1e-10);
    c.at_most("S angular momentum max |J - J_0|/|J_0|", gate(dj), 1e-6);
    c.at_most("S energy max |E - E_0|/|E_0|", gate(de), 1e-6);
    // NS.
    let mut it = NlIntegrator::new(&model, &[], &emc(0.01, ResidualReference::Driver)).unwrap();
    it.start(0.0, &u0, &v0).unwrap();
    match it.step() {
        Ok(rep) => c.holds(
            "NS Driver refused at step 1",
            false,
            &format!("converged: {} halvings, residual {:e}", rep.halvings, rep.residual),
        ),
        Err(e) => c.holds("NS Driver refused at step 1", is_step_refusal(&e), &format!("{e}")),
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------- the default bits: DB1, DB2

/// FNV-1a 64 of the bit patterns of the vectors.
fn fnv(vs: &[&[f64]]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for v in vs {
        for x in *v {
            for b in x.to_bits().to_le_bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        }
    }
    h
}

fn state_hash(it: &NlIntegrator) -> u64 {
    let (u, v, a) = (
        it.displacement().unwrap(),
        it.velocity().unwrap(),
        it.acceleration().unwrap(),
    );
    fnv(&[&u, &v, &a])
}

/// The two default energy-momentum runs of DB1 with an option transform: (i) the spinning bar at residual `1e-7`, 20 steps,
/// (ii) the cantilever at `h = 1/(40 f_1)`, 40 steps.
fn default_hashes(with: impl Fn(NlTransientOptions) -> NlTransientOptions) -> [u64; 2] {
    let (model, u0, v0) = spin();
    let mut newton = NewtonOptions::new(1);
    newton.residual_tolerance = 1e-7;
    let o = with(NlTransientOptions::new(0.01).newton(newton).energy_momentum());
    let mut it = NlIntegrator::new(&model, &[], &o).unwrap();
    it.start(0.0, &u0, &v0).unwrap();
    it.run(20).unwrap();
    let a = state_hash(&it);
    let (m, f1, v0) = cantilever();
    let n = v0.len();
    let o = with(NlTransientOptions::new(1.0 / (40.0 * f1)).energy_momentum());
    let mut it = NlIntegrator::new(&m, &[], &o).unwrap();
    it.start(0.0, &vec![0.0; n], &v0).unwrap();
    it.run(40).unwrap();
    [a, state_hash(&it)]
}

/// The hashes of DB1 recorded on the unchanged library at 731ceef5 (`f681-scratch/db1-record.log`), before any library line
/// of F-681 changed.
const DB1_RECORD: Option<[u64; 2]> = Some([0xfe09_4c8e_3a14_5311, 0xcd91_0d4f_0b77_ec56]);

fn check_hashes(c: &mut Checks, name: &str, h: [u64; 2]) {
    println!("{name}: {:#018x} {:#018x}", h[0], h[1]);
    match DB1_RECORD {
        Some(r) => {
            c.holds(
                &format!("{name} (i) spinning bar"),
                h[0] == r[0],
                &format!("{:#018x} against {:#018x}", h[0], r[0]),
            );
            c.holds(
                &format!("{name} (ii) cantilever"),
                h[1] == r[1],
                &format!("{:#018x} against {:#018x}", h[1], r[1]),
            );
        }
        None => c.holds(name, false, "no record yet"),
    }
}

/// DB1: the default bits of the energy-momentum step against the record of the unchanged library.
#[test]
fn db1_default_energy_momentum_bits() {
    let mut c = Checks::new();
    check_hashes(&mut c, "DB1 default hashes", default_hashes(|o| o));
    c.finish();
}

/// DB2: the same runs with the driver's reference given explicitly.
#[test]
fn db2_explicit_driver_reference_bits() {
    let mut c = Checks::new();
    check_hashes(
        &mut c,
        "DB2 explicit Driver hashes",
        default_hashes(|o| o.residual_reference(ResidualReference::Driver)),
    );
    c.finish();
}
