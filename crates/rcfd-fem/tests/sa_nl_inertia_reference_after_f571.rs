//! ADR 0604 amendment 1 (F-571 after F-631): the copy of ADR 0603's FT1 and NC1 with the rounding bound of ADR 0604 in the
//! driver. FT1 as registered by ADR 0603; NC1b: the default step at the zero crossing is accepted at the rounding bound at its
//! last iterate. Helpers copied from `sa_nl_inertia_reference.rs` (not edited apart from the ignore of its FT1/NC1 record).

use rcfd_core::vec3::Vec3;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::nl_transient::{Integrator as NlIntegrator, NlTransientOptions, ResidualReference};
use rcfd_fem::nonlinear::{initial_states, internal_force, tangent};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, Material, SolidMesh, SolidModel};
use std::f64::consts::PI;

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
    m.add_material("all", Material::isotropic_elastic(E_S, 0.0, RHO_S, 0.0).unwrap())
        .unwrap();
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

/// FT1 (as ADR 0603) and NC1b (ADR 0604 amendment 1): one step from the start (a zero crossing) at `h_f = h/1024`, with the
/// inertia-term reference and with the driver's reference.
#[test]
fn ft1_nc1b_a_step_at_the_zero_crossing_after_f571() {
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
    // NC1b.
    let opts = NlTransientOptions::new(hf);
    let max_iterations = opts.newton.max_iterations;
    let mut it = NlIntegrator::new(&m, &[], &opts).unwrap();
    it.start(0.0, &vec![0.0; n], &v0).unwrap();
    match it.step() {
        Ok(rep) => {
            println!(
                "NC1b step: {} halvings, {} iterations, residual {:e}",
                rep.halvings, rep.iterations, rep.residual
            );
            c.holds(
                "NC1b accepted with 0 halvings",
                rep.halvings == 0,
                &format!("{} halvings", rep.halvings),
            );
            c.holds(
                "NC1b accepted at the last iterate",
                rep.iterations == max_iterations,
                &format!("{} iterations, max_iterations {max_iterations}", rep.iterations),
            );
            c.holds(
                "NC1b final relative residual above the tolerance 1e-10",
                rep.residual > 1e-10,
                &format!("{:e}", rep.residual),
            );
            c.at_most("NC1b final relative residual", rep.residual, 1e-7);
            // Recorded: rn and the bound, recomputed at the accepted u (Newmark average acceleration, beta = 1/4).
            let u = it.displacement().unwrap();
            let a = it.acceleration().unwrap();
            let free = it.free().to_vec();
            let mass = it.mass();
            let cm = 1.0 / (0.25 * hf * hf);
            let st = initial_states(&m).unwrap();
            let fint = internal_force(&m, &u, &st).unwrap();
            let mut ma = vec![0.0; n];
            mass.matvec(&a, &mut ma);
            let rn = free.iter().map(|&i| (fint[i] + ma[i]).powi(2)).sum::<f64>().sqrt();
            c.record("NC1b |f_int + M a|_free at the accepted u, N", rn);
            match tangent(&m, &u, &st, Some(1)) {
                Ok(k) => {
                    let mut s = 0.0f64;
                    for &i in &free {
                        let mut row = 0.0;
                        for p in k.row_ptr[i]..k.row_ptr[i + 1] {
                            row += k.val[p].abs() * u[k.col[p]].abs();
                        }
                        for p in mass.row_ptr[i]..mass.row_ptr[i + 1] {
                            row += cm * mass.val[p].abs() * u[mass.col[p]].abs();
                        }
                        s += row * row;
                    }
                    c.record(
                        "NC1b bound eps |(|K_eff| |u|)|_free at the accepted u, N",
                        f64::EPSILON * s.sqrt(),
                    );
                }
                Err(e) => println!("NC1b bound: not available ({e})"),
            }
        }
        Err(e) => c.holds("NC1b accepted with 0 halvings", false, &format!("refused: {e}")),
    }
    c.finish();
}
