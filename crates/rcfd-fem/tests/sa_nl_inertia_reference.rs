//! F-631 (ADR 0603): the opt-in inertia-term residual reference of the nonlinear transient. FT1, NC1, FT2, RF1, DB1, DB2
//! (fast) and NT3 (heavy, ignored, run once). Each registered run is made once; every value is printed. Every aggregated error
//! uses a NaN-propagating maximum, and a NaN misses its check.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::nl_transient::{Integrator as NlIntegrator, NlTransientOptions, ResidualReference};
use rcfd_fem::nonlinear::{solve, NewtonOptions};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, FemError, Load, Material, SolidMesh, SolidModel};
use std::f64::consts::PI;
use std::time::Instant;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed (a NaN misses).
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn record(&mut self, name: &str, value: f64) {
        println!("{name}: {value:.6e} (recorded)");
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

/// The larger of two values, NaN if either is NaN.
fn nmax(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

/// The largest absolute value, NaN if any value is NaN.
fn max_abs(v: impl IntoIterator<Item = f64>) -> f64 {
    v.into_iter().fold(0.0f64, |m, x| nmax(m, x.abs()))
}

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

/// The cantilever strip of ADR 0522 D6 cut into 32 Shell3 (ADR 0545 `strip3`), clamped at `x = 0`; the tip node of
/// `y = 0` is node `N_S`.
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
    m.add_material("all", Material::isotropic_elastic(E_S, 0.0, RHO_S, 0.0).unwrap()).unwrap();
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("root", comp, 0.0)).unwrap();
    }
    m
}

/// The modal options of ADR 0522 amendment 4.
fn strip_modal() -> ModalOptions {
    let mut mo = ModalOptions::new(1);
    mo.max_iterations = 2000;
    mo.tolerance = 1e-8;
    mo
}

/// The start of ADR 0545 NT1: the model, `f_1`, the tip `u_z` equation and `v0 = omega a phi/phi_z,tip`.
fn start() -> (SolidModel, f64, usize, Vec<f64>) {
    let m = strip3();
    let modal = modal_analysis(&m, &strip_modal()).unwrap();
    let f1 = modal.frequencies[0];
    let qz = m.dof_map().equation(N_S, 2).expect("tip u_z");
    let phi = &modal.modes[0];
    let omega = 2.0 * PI * f1;
    let v0: Vec<f64> = phi.iter().map(|x| omega * 1e-3 * L_S * x / phi[qz]).collect();
    (m, f1, qz, v0)
}

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

/// 40 steps of the strip at `h = 1/(40 f_1)` with the options; the hash of `u`, `v`, `a` after the run.
fn period_hash(options: impl Fn(f64) -> NlTransientOptions) -> u64 {
    let (m, f1, _, v0) = start();
    let n = v0.len();
    let mut it = NlIntegrator::new(&m, &[], &options(1.0 / (40.0 * f1))).unwrap();
    it.start(0.0, &vec![0.0; n], &v0).unwrap();
    it.run(40).unwrap();
    let (u, v, a) = (it.displacement().unwrap(), it.velocity().unwrap(), it.acceleration().unwrap());
    fnv(&[&u, &v, &a])
}

/// The hash of DB1 recorded on the unchanged library at 8333080a (`f631-scratch/db1-record.log`), before any library line
/// of F-631 changed.
const DB1_RECORD: u64 = 0xa1c3_afbc_fc5e_3fc6;

/// DB1: the default bits of 40 steps against the record of the unchanged library.
#[test]
fn db1_default_reference_bits() {
    let mut c = Checks::new();
    let h = period_hash(NlTransientOptions::new);
    c.holds(
        "DB1 default hash",
        h == DB1_RECORD,
        &format!("{h:#018x} against {DB1_RECORD:#018x}"),
    );
    c.finish();
}

/// DB2: the same 40 steps with the driver's reference given explicitly.
#[test]
fn db2_explicit_driver_reference_bits() {
    let mut c = Checks::new();
    let h = period_hash(|dt| NlTransientOptions::new(dt).residual_reference(ResidualReference::Driver));
    c.holds(
        "DB2 explicit Driver hash",
        h == DB1_RECORD,
        &format!("{h:#018x} against {DB1_RECORD:#018x}"),
    );
    c.finish();
}

/// FT1 and NC1: one step from the start (a zero crossing) at `h_f = h/1024`, with the inertia-term reference and with the
/// driver's reference (ADR 0603).
#[test]
fn ft1_nc1_a_step_at_the_zero_crossing() {
    let mut c = Checks::new();
    let (m, f1, _, v0) = start();
    let n = v0.len();
    let hf = 1.0 / (40.0 * f1) / 1024.0;
    let omega = 2.0 * PI * f1;
    // FT1.
    let opts = NlTransientOptions::new(hf).residual_reference(ResidualReference::InertiaTerms);
    let mut it = NlIntegrator::new(&m, &[], &opts).unwrap();
    it.start(0.0, &vec![0.0; n], &v0).unwrap();
    match it.step() {
        Ok(rep) => {
            println!(
                "FT1 step: {} halvings, {} iterations, residual {:e}",
                rep.halvings, rep.iterations, rep.residual
            );
            c.holds(
                "FT1 converges with 0 halvings",
                rep.halvings == 0,
                &format!("{} halvings", rep.halvings),
            );
            c.at_most("FT1 final relative residual", rep.residual, 1e-10);
            let u = it.displacement().unwrap();
            let scale = max_abs(v0.iter().map(|x| hf * x));
            let k = 1.0 + (omega * hf).powi(2) / 4.0;
            let e = max_abs(u.iter().zip(&v0).map(|(ui, vi)| ui - hf * vi / k)) / scale;
            c.at_most("FT1 |u_1 - h_f v0/(1 + (omega h_f)^2/4)| / |h_f v0|", e, 1e-8);
        }
        Err(e) => c.holds("FT1 converges with 0 halvings", false, &format!("refused: {e}")),
    }
    // NC1.
    let mut it = NlIntegrator::new(&m, &[], &NlTransientOptions::new(hf)).unwrap();
    it.start(0.0, &vec![0.0; n], &v0).unwrap();
    match it.step() {
        Ok(rep) => c.holds(
            "NC1 Driver refused",
            false,
            &format!("converged: {} halvings, residual {:e}", rep.halvings, rep.residual),
        ),
        Err(e) => {
            let ok = matches!(&e, FemError::Solver { solver, .. } if *solver == "nl_transient");
            c.holds("NC1 Driver refused", ok, &format!("{e}"));
        }
    }
    c.finish();
}

/// FT2: one period at `h` with the inertia-term reference (the fast companion of NT3).
#[test]
fn ft2_one_period_with_the_inertia_term_reference() {
    let mut c = Checks::new();
    let (m, f1, _, v0) = start();
    let n = v0.len();
    let h = 1.0 / (40.0 * f1);
    let opts = NlTransientOptions::new(h).residual_reference(ResidualReference::InertiaTerms);
    let mut it = NlIntegrator::new(&m, &[], &opts).unwrap();
    it.start(0.0, &vec![0.0; n], &v0).unwrap();
    let h0 = it.kinetic_energy().unwrap() + it.strain_energy().unwrap();
    let (mut halvings, mut iterations, mut worst) = (0, 0, 0.0f64);
    let mut energy_error = 0.0;
    let mut done = true;
    for _ in 0..40 {
        match it.step() {
            Ok(rep) => {
                halvings += rep.halvings;
                iterations += rep.iterations;
                worst = nmax(worst, rep.residual);
            }
            Err(e) => {
                println!("FT2 refused: {e}");
                done = false;
                break;
            }
        }
        let hn = it.kinetic_energy().unwrap() + it.strain_energy().unwrap();
        energy_error = nmax(energy_error, ((hn - h0) / h0).abs());
    }
    println!("FT2: {halvings} halvings, {iterations} iterations, worst residual {worst:e}");
    c.holds(
        "FT2 completes with 0 halvings",
        done && halvings == 0,
        &format!("completed {done}, {halvings} halvings"),
    );
    c.at_most(
        "FT2 max |H_n - H_0|/H_0",
        if done { energy_error } else { f64::NAN },
        1e-5,
    );
    c.finish();
}

/// RF1: the inertia-term reference with the energy-momentum step is refused (ADR 0603 D2).
#[test]
fn rf1_inertia_terms_refused_with_the_energy_momentum_step() {
    let mut c = Checks::new();
    let mut model = SolidModel::new(hex_box([0.0; 3], [1.0, 0.1, 0.1], [4, 1, 1]).unwrap());
    model
        .add_material("all", Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    let opts = NlTransientOptions::new(1e-4)
        .energy_momentum()
        .residual_reference(ResidualReference::InertiaTerms);
    match NlIntegrator::new(&model, &[], &opts) {
        Ok(_) => c.holds("RF1 refused", false, "taken"),
        Err(e) => {
            let ok = matches!(&e, FemError::InvalidValue { key, reason }
                if *key == "nl_transient" && reason.contains("ADR 0603"));
            c.holds("RF1 refused", ok, &format!("{e}"));
        }
    }
    // The same model is taken by the energy-momentum step with the driver's reference (the refusal is the option's).
    let taken = NlIntegrator::new(&model, &[], &NlTransientOptions::new(1e-4).energy_momentum()).is_ok();
    c.holds("RF1 control: taken with Driver", taken, "");
    c.finish();
}

const BETA_1: f64 = 1.875_104_068_711_961;
const TIP_FORCE_A2: f64 = -1.042e-3;

fn f_eb() -> f64 {
    let ei = E_S * B_S * T_S.powi(3) / 12.0;
    BETA_1 * BETA_1 / (2.0 * PI * L_S * L_S) * (ei / (RHO_S * B_S * T_S)).sqrt()
}

/// EZ of ADR 0522 amendment 4 on the Shell3 strip (ADR 0545 `ez_clapeyron`).
fn ez_clapeyron() -> f64 {
    let mut m = strip3();
    m.add_load(Load::nodal_force("tip", [0.0, 0.0, TIP_FORCE_A2])).unwrap();
    let r = solve(&m, &[], &[], &NewtonOptions::new(1)).unwrap();
    let us = r.result.displacement_values().to_vec();
    let n = us.len();
    let mut it = NlIntegrator::new(&m, &[], &NlTransientOptions::new(1.0)).unwrap();
    it.start(0.0, &us, &vec![0.0; n]).unwrap();
    let pi = it.strain_energy().unwrap();
    let f = nodal_load_vector(&m, &[]).unwrap();
    let work = 0.5 * f.iter().zip(&us).map(|(x, y)| x * y).sum::<f64>();
    println!("EZ Shell3: Pi {pi:.10e}, f . u/2 {work:.10e}");
    (pi / work - 1.0).abs()
}

/// NT3: ADR 0545 NT1 (100 periods of the Shell3 strip) at the default tolerance with the inertia-term reference. Heavy.
#[test]
#[ignore = "heavy registered record of ADR 0603 NT3 (100 periods of the Shell3 strip, 4000 steps); run once with --ignored"]
fn nt3_shell3_strip_100_periods_inertia_term_reference() {
    let mut c = Checks::new();
    let t_start = Instant::now();
    let (m, f1, qz, v0) = start();
    let n = v0.len();
    let omega = 2.0 * PI * f1;
    let h = 1.0 / (40.0 * f1);
    let opts = NlTransientOptions::new(h).residual_reference(ResidualReference::InertiaTerms);
    let mut it = NlIntegrator::new(&m, &[], &opts).unwrap();
    it.start(0.0, &vec![0.0; n], &v0).unwrap();
    let h0 = it.kinetic_energy().unwrap() + it.strain_energy().unwrap();
    let mut energy_error = 0.0;
    let mut prev = (0.0, 0.0);
    let mut cross_t = vec![0.0];
    let (mut halvings, mut iterations, mut worst) = (0, 0, 0.0f64);
    let mut completed = 0u64;
    for step in 1..=40 * 100u64 {
        let rep = match it.step() {
            Ok(r) => r,
            Err(e) => {
                println!("NT3 stopped at step {step}: {e}");
                break;
            }
        };
        completed = step;
        halvings += rep.halvings;
        iterations += rep.iterations;
        worst = nmax(worst, rep.residual);
        let t = step as f64 * h;
        let uz = it.displacement().unwrap()[qz];
        let hn = it.kinetic_energy().unwrap() + it.strain_energy().unwrap();
        energy_error = nmax(energy_error, ((hn - h0) / h0).abs());
        if step > 1 && (prev.1 > 0.0) != (uz > 0.0) {
            cross_t.push(prev.0 + h * prev.1 / (prev.1 - uz));
        }
        prev = (t, uz);
    }
    let done = completed == 4000;
    let kc = cross_t.len();
    let fm = if done && kc >= 2 {
        (kc - 1) as f64 / (2.0 * (cross_t[kc - 1] - cross_t[0]))
    } else {
        f64::NAN
    };
    let r = 2.0 * (0.5 * omega * h).atan() / (omega * h);
    let ez = ez_clapeyron();
    println!(
        "NT3 Shell3 strip 100 periods: f_1 {f1:.10e} Hz, f_m {fm:.10e} Hz, r {r:.8}, {kc} crossings, H_0 {h0:.6e}, \
         max |dH|/H_0 {energy_error:.4e}, EZ {ez:.4e}, {completed} steps, {halvings} halvings, {iterations} iterations, \
         worst residual {worst:e}, {:.1} s",
        t_start.elapsed().as_secs_f64()
    );
    c.holds("NT3 completes (4000 steps)", done, &format!("{completed} steps"));
    c.holds("NT3 total halvings 0", halvings == 0, &format!("{halvings}"));
    c.at_most("NT3 (F1) |f_m/f_1 - 1|", (fm / f1 - 1.0).abs(), 1e-2);
    c.at_most("NT3 (F1b) |f_m/(f_1 r) - 1|", (fm / (f1 * r) - 1.0).abs(), 1e-3);
    c.at_most("NT3 (F1c) |f_1/f_EB - 1|", (f1 / f_eb() - 1.0).abs(), 1e-2);
    c.at_most(
        "NT3 (E1) max |H_n - H_0|/H_0",
        if done { energy_error } else { f64::NAN },
        1e-5,
    );
    c.at_most("NT3 (EZ) |Pi(u_s)/(f . u_s/2) - 1|", ez, 1e-4);
    c.record("NT3 signed f_m/f_1 - 1", fm / f1 - 1.0);
    c.record("NT3 wall time s", t_start.elapsed().as_secs_f64());
    c.finish();
}
