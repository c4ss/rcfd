//! F-578 (ADR 0490): the corotated Shell4 and Shell3 in the Newton driver. Checks T1, T1r, S1 (the tangent against central
//! differences), L1, L1f, L2 (the linear limit and rigid motions), RU4, RU4b, RU3, RU4a, RU4f (the tip-moment roll-up),
//! CY4, CY3 and the record CY4a (the cylinder under the follower internal pressure), R1 (the refusals kept) and the record
//! P2. Each registered run is made once; every value is printed. Every aggregated error uses a NaN-propagating maximum.

use rcfd_fem::arclength::{trace, ArcLengthOptions, PathPoint};
use rcfd_fem::assemble::stiffness;
use rcfd_fem::follower::FollowerPressure;
use rcfd_fem::nl_transient::{Integrator, NlTransientOptions};
use rcfd_fem::nonlinear::{
    initial_states, internal_force, solve, solve_from, solve_with, tangent, Contribution, DriverExtensions, NewtonOptions,
};
use rcfd_fem::shell::Shell4Membrane;
use rcfd_fem::shell3::Shell3Membrane;
use rcfd_fem::{BlockProperty, Constraint, ElementKind, FemError, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::CsrMatrix;
use std::cell::RefCell;
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
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!(
            "{name}: {value:.4e} (limit >= {limit:e}) {}",
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

fn max_abs(v: &[f64]) -> f64 {
    v.iter().fold(0.0, |m, x| nmax(m, x.abs()))
}

fn dense(a: &CsrMatrix) -> Vec<f64> {
    let n = a.n;
    let mut k = vec![0.0; n * n];
    for r in 0..n {
        for p in a.row_ptr[r]..a.row_ptr[r + 1] {
            k[r * n + a.col[p]] += a.val[p];
        }
    }
    k
}

/// SplitMix64, a deterministic source only.
fn uniform(state: &mut u64) -> f64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((z ^ (z >> 31)) >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

fn sym(state: &mut u64, r: f64) -> f64 {
    r * (2.0 * uniform(state) - 1.0)
}

/// The rotation of the Rodrigues vector `w` applied to `x`.
fn rotate(w: [f64; 3], x: [f64; 3]) -> [f64; 3] {
    let t = (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
    if t == 0.0 {
        return x;
    }
    let k = [w[0] / t, w[1] / t, w[2] / t];
    let kx = [k[1] * x[2] - k[2] * x[1], k[2] * x[0] - k[0] * x[2], k[0] * x[1] - k[1] * x[0]];
    let kd = k[0] * x[0] + k[1] * x[1] + k[2] * x[2];
    let (s, c) = t.sin_cos();
    [0, 1, 2].map(|i| x[i] * c + kx[i] * s + k[i] * kd * (1.0 - c))
}

// ---------------------------------------------------------------------------------------- T1, T1r, S1, L1, L1f, L2

const T_EL: f64 = 0.01;

fn steel(nu: f64) -> Material {
    Material::isotropic_elastic(2.0e11, nu, 7850.0, 0.0).unwrap()
}

#[derive(Clone, Copy, PartialEq)]
enum Membrane {
    Default,
    Andes,
    Allman,
}

fn shell_model(kind: ElementKind, nodes: Vec<[f64; 3]>, conn: Vec<usize>, membrane: Membrane, nu: f64) -> SolidModel {
    let ne = conn.len() / if kind == ElementKind::Shell4 { 4 } else { 3 };
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind, conn).unwrap();
    let property = match (kind, membrane) {
        (_, Membrane::Default) => BlockProperty::shell(T_EL).unwrap(),
        (ElementKind::Shell4, Membrane::Andes) => BlockProperty::shell4_with_membrane(T_EL, Shell4Membrane::AndesOpt).unwrap(),
        (_, Membrane::Andes) => BlockProperty::shell_with_membrane(T_EL, Shell3Membrane::AndesOpt).unwrap(),
        (_, Membrane::Allman) => BlockProperty::shell_with_membrane(T_EL, Shell3Membrane::Allman).unwrap(),
    };
    mesh.set_block_property(0, property).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel(nu)).unwrap();
    m
}

fn warped4() -> Vec<[f64; 3]> {
    vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.06], [1.1, 0.9, 0.0], [-0.1, 1.0, 0.08]]
}

fn tri3() -> Vec<[f64; 3]> {
    vec![[0.0, 0.0, 0.0], [1.0, 0.1, 0.05], [0.2, 0.9, -0.03]]
}

fn flat4() -> Vec<[f64; 3]> {
    vec![[0.0, 0.0, 0.0], [1.2, 0.1, 0.0], [1.0, 0.9, 0.0], [-0.1, 1.1, 0.0]]
}

fn single(kind: ElementKind, nodes: Vec<[f64; 3]>, membrane: Membrane) -> SolidModel {
    let n = nodes.len();
    shell_model(kind, nodes, (0..n).collect(), membrane, 0.3)
}

/// Quads `(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)` of a grid of `(ni + 1) x (nj + 1)` nodes, node `j (ni + 1) + i`;
/// cut along the diagonal from the first to the third corner when `tri`.
fn grid_conn(ni: usize, nj: usize, tri: bool) -> Vec<usize> {
    let idx = |i: usize, j: usize| j * (ni + 1) + i;
    let mut conn = Vec::new();
    for j in 0..nj {
        for i in 0..ni {
            let (a, b, c, d) = (idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1));
            if tri {
                conn.extend_from_slice(&[a, b, c, a, c, d]);
            } else {
                conn.extend_from_slice(&[a, b, c, d]);
            }
        }
    }
    conn
}

fn kind_of(tri: bool) -> ElementKind {
    if tri {
        ElementKind::Shell3
    } else {
        ElementKind::Shell4
    }
}

/// The 2 x 2 patch of ADR 0487 on the cylinder `r = 2`, `theta` in `[0, 0.5]`, `z` in `[0, 1]`.
fn patch(tri: bool) -> SolidModel {
    let mut nodes = Vec::new();
    for j in 0..=2 {
        for i in 0..=2 {
            let th = 0.25 * i as f64;
            nodes.push([2.0 * th.cos(), 2.0 * th.sin(), 0.5 * j as f64]);
        }
    }
    shell_model(kind_of(tri), nodes, grid_conn(2, 2, tri), Membrane::Default, 0.3)
}

/// A rigid motion of the model's nodes: the rotation `w` about the centroid and the translation `t`, every nodal rotation
/// `w`; one value per equation.
fn rigid(model: &SolidModel, w: [f64; 3], t: [f64; 3]) -> Vec<f64> {
    let nodes = model.mesh().nodes();
    let nn = nodes.len() as f64;
    let mut c = [0.0; 3];
    for x in nodes {
        for k in 0..3 {
            c[k] += x[k] / nn;
        }
    }
    let dofs = model.dof_map();
    let mut u = vec![0.0; dofs.n_equations()];
    for (a, x) in nodes.iter().enumerate() {
        let r = rotate(w, [x[0] - c[0], x[1] - c[1], x[2] - c[2]]);
        for k in 0..3 {
            u[dofs.equation(a, k).unwrap()] = c[k] + r[k] + t[k] - x[k];
            u[dofs.equation(a, 3 + k).unwrap()] = w[k];
        }
    }
    u
}

fn size(model: &SolidModel) -> f64 {
    let nodes = model.mesh().nodes();
    let nn = nodes.len() as f64;
    let mut c = [0.0; 3];
    for x in nodes {
        for k in 0..3 {
            c[k] += x[k] / nn;
        }
    }
    nodes.iter().fold(0.0, |m, x| {
        nmax(m, ((x[0] - c[0]).powi(2) + (x[1] - c[1]).powi(2) + (x[2] - c[2]).powi(2)).sqrt())
    })
}

#[test]
fn t1_t1r_s1_tangent_against_central_differences() {
    let mut ck = Checks::new();
    let configurations = [
        ("warped Shell4 element", single(ElementKind::Shell4, warped4(), Membrane::Default)),
        ("Shell3 element", single(ElementKind::Shell3, tri3(), Membrane::Default)),
        ("Shell4 patch", patch(false)),
        ("Shell3 patch", patch(true)),
        ("Shell4 ANDES element", single(ElementKind::Shell4, warped4(), Membrane::Andes)),
        ("Shell3 ANDES element", single(ElementKind::Shell3, tri3(), Membrane::Andes)),
    ];
    let mut rng: u64 = 0xF578_0490;
    let w: [f64; 3] = [0.6, -0.9, 0.4];
    let wl = (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
    for (name, model) in &configurations {
        let states = initial_states(model).unwrap();
        let dofs = model.dof_map();
        let n = dofs.n_equations();
        for state in ["a", "b"] {
            let (wr, rot, tr) = if state == "a" {
                (w, 0.02, 0.02)
            } else {
                (w.map(|x| 0.05 * x / wl), 0.2, 0.02)
            };
            let mut u = rigid(model, wr, [0.0; 3]);
            for (i, ui) in u.iter_mut().enumerate() {
                let (_, c) = dofs.node_component(i).unwrap();
                *ui += sym(&mut rng, if c >= 3 { rot } else { tr });
            }
            let k = dense(&tangent(model, &u, &states, Some(1)).unwrap());
            let h = 1e-6;
            let mut kcd = vec![0.0; n * n];
            for j in 0..n {
                let mut up = u.clone();
                let mut um = u.clone();
                up[j] += h;
                um[j] -= h;
                let fp = internal_force(model, &up, &states).unwrap();
                let fm = internal_force(model, &um, &states).unwrap();
                for i in 0..n {
                    kcd[i * n + j] = (fp[i] - fm[i]) / (2.0 * h);
                }
            }
            let mut err = 0.0;
            let mut err_rot = 0.0;
            let mut max_rot = 0.0;
            let mut symmetric = true;
            for i in 0..n {
                for j in 0..n {
                    let e = (k[i * n + j] - kcd[i * n + j]).abs();
                    err = nmax(err, e);
                    let (_, ci) = dofs.node_component(i).unwrap();
                    let (_, cj) = dofs.node_component(j).unwrap();
                    if ci >= 3 && cj >= 3 {
                        err_rot = nmax(err_rot, e);
                        max_rot = nmax(max_rot, k[i * n + j].abs());
                    }
                    symmetric &= k[i * n + j].to_bits() == k[j * n + i].to_bits();
                }
            }
            ck.at_most(
                &format!("T1 {name} state {state}: max |K - K_cd| / max |K|"),
                err / max_abs(&k),
                1e-6,
            );
            ck.record(
                &format!("T1r {name} state {state}: rotational block relative"),
                err_rot / max_rot,
            );
            ck.holds(&format!("S1 {name} state {state}: K == K^T bits"), symmetric, "");
        }
    }
    ck.finish();
}

#[test]
fn l1_l1f_l2_linear_limit_and_rigid_motions() {
    let mut ck = Checks::new();
    let elements = [
        ("warped Shell4", single(ElementKind::Shell4, warped4(), Membrane::Default)),
        ("Shell3", single(ElementKind::Shell3, tri3(), Membrane::Default)),
        ("flat Shell4", single(ElementKind::Shell4, flat4(), Membrane::Default)),
        ("Shell4 ANDES", single(ElementKind::Shell4, warped4(), Membrane::Andes)),
        ("Shell3 ANDES", single(ElementKind::Shell3, tri3(), Membrane::Andes)),
        ("Shell3 Allman", single(ElementKind::Shell3, tri3(), Membrane::Allman)),
    ];
    let w2: [f64; 3] = [0.6, -1.0, 1.4];
    let w2l = (w2[0] * w2[0] + w2[1] * w2[1] + w2[2] * w2[2]).sqrt();
    for (name, model) in &elements {
        let states = initial_states(model).unwrap();
        let n = model.dof_map().n_equations();
        let zero = vec![0.0; n];
        let k0 = dense(&tangent(model, &zero, &states, Some(1)).unwrap());
        let klin = dense(&stiffness(model).unwrap());
        let mut d = 0.0;
        for (a, b) in k0.iter().zip(&klin) {
            d = nmax(d, (a - b).abs());
        }
        let scale = max_abs(&klin);
        ck.at_most(&format!("L1 {name}: max |K(0) - K_lin| / max |K_lin|"), d / scale, 1e-10);
        let l = size(model);
        let f0 = internal_force(model, &zero, &states).unwrap();
        ck.at_most(
            &format!("L1f {name}: max |f(0)| / (max |K_lin| L)"),
            max_abs(&f0) / (scale * l),
            1e-11,
        );
        for (label, w) in [("large", w2), ("small", w2.map(|x| 0.01 * x / w2l))] {
            let u = rigid(model, w, [0.2, -0.1, 0.3]);
            let f = internal_force(model, &u, &states).unwrap();
            ck.at_most(
                &format!("L2 {name} rigid {label}: max |f| / (max |K_lin| L)"),
                max_abs(&f) / (scale * l),
                1e-11,
            );
        }
    }
    ck.finish();
}

// ------------------------------------------------------------------------------------------- RU4, RU4b, RU3, RU4a, RU4f

const L_S: f64 = 12.0;
const B_S: f64 = 1.0;
const T_S: f64 = 0.1;
const E_S: f64 = 1.2e6;
const N_S: usize = 16;

fn ei() -> f64 {
    E_S * B_S * T_S.powi(3) / 12.0
}

/// The cantilever strip of D5, the dead tip moment `scale M` about `-y`; with `planar`, `u_y, theta_x, theta_z` held on
/// every node.
fn roll_strip(tri: bool, membrane: Membrane, planar: bool, scale: f64) -> SolidModel {
    let mut nodes = Vec::new();
    for j in 0..=1 {
        for i in 0..=N_S {
            nodes.push([L_S * i as f64 / N_S as f64, B_S * j as f64, 0.0]);
        }
    }
    let kind = kind_of(tri);
    let conn = grid_conn(N_S, 1, tri);
    let ne = conn.len() / if tri { 3 } else { 4 };
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind, conn).unwrap();
    let property = match membrane {
        Membrane::Andes => BlockProperty::shell4_with_membrane(T_S, Shell4Membrane::AndesOpt).unwrap(),
        _ => BlockProperty::shell(T_S).unwrap(),
    };
    mesh.set_block_property(0, property).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("root", vec![0, N_S + 1]).unwrap();
    mesh.add_node_set("tip", vec![N_S, 2 * N_S + 1]).unwrap();
    mesh.add_node_set("nodes", (0..2 * (N_S + 1)).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E_S, 0.0, 1.0, 0.0).unwrap())
        .unwrap();
    for c in 0..6 {
        m.add_constraint(Constraint::displacement("root", c, 0.0)).unwrap();
    }
    if planar {
        for c in [1, 3, 5] {
            m.add_constraint(Constraint::displacement("nodes", c, 0.0)).unwrap();
        }
    }
    let moment = scale * 2.0 * PI * ei() / L_S;
    m.add_load(Load::nodal_moment("tip", [0.0, -0.5 * moment, 0.0])).unwrap();
    m
}

/// Records the displacement of every call with its load factor (a zero contribution).
struct Recorder<'a> {
    inner: Option<&'a FollowerPressure>,
    calls: RefCell<Vec<(f64, Vec<f64>)>>,
}

impl Contribution for Recorder<'_> {
    fn name(&self) -> &str {
        "recorder"
    }
    fn add(
        &self,
        u: &[f64],
        load_factor: f64,
        force: &mut [f64],
        tangent: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::Result<()> {
        self.calls.borrow_mut().push((load_factor, u.to_vec()));
        match self.inner {
            Some(fp) => fp.add(u, load_factor, force, tangent),
            None => Ok(()),
        }
    }
}

fn recorder(inner: Option<&FollowerPressure>) -> Recorder<'_> {
    Recorder {
        inner,
        calls: RefCell::new(Vec::new()),
    }
}

/// The displacement of the last call at the load factor `lf`.
fn at_factor(rec: &Recorder, lf: f64) -> Vec<f64> {
    rec.calls
        .borrow()
        .iter()
        .rev()
        .find(|(l, _)| l.to_bits() == lf.to_bits())
        .expect("a call at the converged load factor")
        .1
        .clone()
}

/// RU of D5 on one strip: `e` and the tip-rotation error at every step, the out-of-plane largest value; the run of `solve`
/// gives the final state, equal to the recorder run's (amendment 1).
fn roll_up(ck: &mut Checks, label: &str, tri: bool, membrane: Membrane, planar: bool, scale: f64, steps: usize) {
    let m = roll_strip(tri, membrane, planar, scale);
    let rec = recorder(None);
    let t0 = Instant::now();
    let ext = DriverExtensions {
        contributions: vec![&rec],
        step_control: None,
    };
    let r = match solve_with(&m, &[], &[], &NewtonOptions::new(steps), ext) {
        Ok(r) => r,
        Err(e) => {
            ck.holds(&format!("{label} the Newton driver rolls the strip up"), false, &format!("{e:?}"));
            return;
        }
    };
    ck.record(&format!("P2 {label} wall time, s"), t0.elapsed().as_secs_f64());
    let plain = solve(&m, &[], &[], &NewtonOptions::new(steps)).unwrap();
    let same = plain
        .result
        .displacement_values()
        .iter()
        .zip(r.result.displacement_values())
        .all(|(a, b)| a == b);
    ck.holds(&format!("{label} solve gives the final state of the recorder run"), same, "");
    let dofs = m.dof_map();
    let tips = [N_S, 2 * N_S + 1];
    let (mut e_max, mut rot_max, mut out_max) = (0.0, 0.0, 0.0);
    let mut iterations = Vec::new();
    for s in &r.steps {
        let u = at_factor(&rec, s.load_factor);
        let phi = scale * 2.0 * PI * s.load_factor;
        let rho = L_S / phi;
        let (xc, zc) = (rho * phi.sin(), rho * (1.0 - phi.cos()));
        let mut e = 0.0;
        let mut er = 0.0;
        for &t in &tips {
            let x = L_S + u[dofs.equation(t, 0).unwrap()];
            let z = u[dofs.equation(t, 2).unwrap()];
            e = nmax(e, ((x - xc).powi(2) + (z - zc).powi(2)).sqrt() / L_S);
            er = nmax(er, (u[dofs.equation(t, 4).unwrap()] / -phi - 1.0).abs());
        }
        for node in 0..2 * (N_S + 1) {
            out_max = nmax(out_max, u[dofs.equation(node, 1).unwrap()].abs() / L_S);
            out_max = nmax(out_max, u[dofs.equation(node, 3).unwrap()].abs());
            out_max = nmax(out_max, u[dofs.equation(node, 5).unwrap()].abs());
        }
        println!(
            "  {label} step {} lambda {:.4}: e {e:.4e}, tip rotation error {er:.3e}, iterations {}",
            s.step,
            s.load_factor,
            s.residuals.len() - 1
        );
        iterations.push(s.residuals.len() - 1);
        e_max = nmax(e_max, e);
        rot_max = nmax(rot_max, er);
    }
    let last = r.steps.last().map_or(f64::NAN, |s| s.load_factor);
    ck.at_most(&format!("{label}: max over the steps of the tip error e"), e_max, 1e-2);
    ck.holds(&format!("{label}: last load factor 1"), last == 1.0, &format!("{last}"));
    ck.record(&format!("{label}: increments"), r.steps.len() as f64);
    ck.record(&format!("{label}: total iterations"), iterations.iter().sum::<usize>() as f64);
    if planar {
        ck.at_most(&format!("{label}b: max |theta_y,tip/(-phi) - 1|"), rot_max, 1e-6);
    } else {
        ck.record(&format!("{label}: max |theta_y,tip/(-phi) - 1|"), rot_max);
    }
    if !planar {
        ck.at_most(&format!("{label}: out-of-plane max |u_y|/L, |theta_x|, |theta_z|"), out_max, 1e-6);
    }
}

#[test]
fn ru4_ru4b_shell4_strip_rolls_up_to_a_full_circle() {
    let mut ck = Checks::new();
    roll_up(&mut ck, "RU4", false, Membrane::Default, true, 1.0, 40);
    ck.finish();
}

#[test]
fn ru3_shell3_strip_rolls_up_to_a_full_circle() {
    let mut ck = Checks::new();
    roll_up(&mut ck, "RU3", true, Membrane::Default, true, 1.0, 40);
    ck.finish();
}

#[test]
fn ru4a_shell4_andes_strip_rolls_up() {
    let mut ck = Checks::new();
    roll_up(&mut ck, "RU4a", false, Membrane::Andes, true, 1.0, 40);
    ck.finish();
}

#[test]
fn ru4f_shell4_strip_without_the_planar_holds() {
    let mut ck = Checks::new();
    roll_up(&mut ck, "RU4f", false, Membrane::Default, false, 0.9, 36);
    ck.finish();
}

// ---------------------------------------------------------------------------------------------------- CY4, CY3, CY4a

const R_C: f64 = 1.0;
const T_C: f64 = 0.01;
const E_C: f64 = 2.0e11;
const BW: f64 = 0.05;
const NQ: usize = 24;

/// The quarter-ring strip of ADR 0487 D3: 24 elements over the quarter, 1 across the width, the plane and the symmetry
/// held.
fn ring(tri: bool, membrane: Membrane) -> SolidModel {
    let mut nodes = Vec::new();
    for k in 0..=1 {
        for i in 0..=NQ {
            let th = 0.5 * PI * i as f64 / NQ as f64;
            nodes.push([R_C * th.cos(), R_C * th.sin(), BW * k as f64]);
        }
    }
    let conn = grid_conn(NQ, 1, tri);
    let ne = conn.len() / if tri { 3 } else { 4 };
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind_of(tri), conn).unwrap();
    let property = match membrane {
        Membrane::Andes => BlockProperty::shell4_with_membrane(T_C, Shell4Membrane::AndesOpt).unwrap(),
        _ => BlockProperty::shell(T_C).unwrap(),
    };
    mesh.set_block_property(0, property).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("nodes", (0..2 * (NQ + 1)).collect()).unwrap();
    mesh.add_node_set("th0", vec![0, NQ + 1]).unwrap();
    mesh.add_node_set("th90", vec![NQ, 2 * NQ + 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E_C, 0.0, 7850.0, 0.0).unwrap())
        .unwrap();
    for c in [2, 3, 4] {
        m.add_constraint(Constraint::displacement("nodes", c, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::displacement("th0", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("th0", 5, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("th90", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("th90", 5, 0.0)).unwrap();
    m
}

fn p_thin(lambda: f64) -> f64 {
    E_C * T_C * (lambda * lambda - 1.0) / (2.0 * R_C)
}

fn mean_stretch(model: &SolidModel, u: &[f64]) -> f64 {
    let dofs = model.dof_map();
    let nodes = model.mesh().nodes();
    let mut s = 0.0;
    for (node, x) in nodes.iter().enumerate() {
        let a = x[0] + u[dofs.equation(node, 0).unwrap()];
        let b = x[1] + u[dofs.equation(node, 1).unwrap()];
        s += (a * a + b * b).sqrt() / R_C;
    }
    s / nodes.len() as f64
}

/// CY of D6 on one ring; with `record`, only the last `p/p_thin` is recorded.
fn cylinder(ck: &mut Checks, label: &str, tri: bool, membrane: Membrane, record: bool) {
    let m = ring(tri, membrane);
    let p_ref = p_thin(1.51);
    let fp = FollowerPressure::on_shells(&m, "all", p_ref).unwrap();
    let rec = recorder(Some(&fp));
    let ext = DriverExtensions {
        contributions: vec![&rec],
        step_control: None,
    };
    let t0 = Instant::now();
    let r = match solve_with(&m, &[], &[], &NewtonOptions::new(20), ext) {
        Ok(r) => r,
        Err(e) => {
            if record {
                println!("{label}: the step sequence failed: {e:?} (recorded)");
            } else {
                ck.holds(&format!("{label} the Newton driver expands the ring"), false, &format!("{e:?}"));
            }
            return;
        }
    };
    ck.record(&format!("P2 {label} wall time, s"), t0.elapsed().as_secs_f64());
    let mut worst = 0.0;
    let mut last = f64::NAN;
    let mut last_ratio = f64::NAN;
    for s in &r.steps {
        let u = at_factor(&rec, s.load_factor);
        let lam = mean_stretch(&m, &u);
        let ratio = s.load_factor * p_ref / p_thin(lam);
        println!(
            "  {label} step {} lambda {lam:.6}: p/p_thin - 1 = {:+.4e}, iterations {}",
            s.step,
            ratio - 1.0,
            s.residuals.len() - 1
        );
        worst = nmax(worst, (ratio - 1.0).abs());
        last = lam;
        last_ratio = ratio;
    }
    if record {
        ck.record(&format!("{label}: last p/p_thin"), last_ratio);
        ck.record(&format!("{label}: last lambda"), last);
    } else {
        ck.at_most(&format!("{label}: max over the steps of |p/p_thin - 1|"), worst, 1e-2);
        ck.at_least(&format!("{label}: last stretch"), last, 1.5);
    }
}

#[test]
fn cy4_cy3_cylinder_under_follower_internal_pressure() {
    let mut ck = Checks::new();
    cylinder(&mut ck, "CY4", false, Membrane::Default, false);
    cylinder(&mut ck, "CY3", true, Membrane::Default, false);
    ck.finish();
}

#[test]
fn cy4a_andes_cylinder_record() {
    let mut ck = Checks::new();
    cylinder(&mut ck, "CY4a", false, Membrane::Andes, true);
    ck.finish();
}

// ------------------------------------------------------------------------------------------------------------------- R1

fn is_shell4_refusal(r: &rcfd_fem::Result<impl Sized>) -> (bool, String) {
    match r {
        Ok(_) => (false, "Ok".to_string()),
        Err(e) => (
            matches!(e, FemError::NotYetSupported { item: "element kind", variant: "Shell4", .. }),
            format!("{e:?}"),
        ),
    }
}

#[test]
fn r1_refusals_kept() {
    let mut ck = Checks::new();
    let m = roll_strip(false, Membrane::Default, true, 1.0);
    let mut stop = |_: &PathPoint| true;
    let (ok, d) = is_shell4_refusal(&trace(&m, &[], &[], &ArcLengthOptions::new(0.1), &[], &mut stop));
    ck.holds("R1 arclength::trace refuses Shell4", ok, &d);
    let (ok, d) = is_shell4_refusal(&Integrator::new(&m, &[], &NlTransientOptions::new(1e-3)));
    ck.holds("R1 nl_transient::Integrator::new refuses Shell4", ok, &d);
    let n = m.dof_map().n_equations();
    let states = initial_states(&m).unwrap();
    let (ok, d) = is_shell4_refusal(&solve_from(
        &m,
        &[],
        &vec![0.0; n],
        &vec![0.0; n],
        &states,
        &NewtonOptions::new(1),
        &[],
    ));
    ck.holds("R1 solve_from refuses Shell4", ok, &d);
    let mut j2 = SolidModel::new(m.mesh().clone());
    j2.add_material("all", Material::j2(steel(0.3), 2.5e8, 1.0e9, 0.0).unwrap())
        .unwrap();
    for c in 0..6 {
        j2.add_constraint(Constraint::displacement("root", c, 0.0)).unwrap();
    }
    let r = solve(&j2, &[], &[], &NewtonOptions::new(1));
    let d = match &r {
        Ok(_) => "Ok".to_string(),
        Err(e) => format!("{e:?}"),
    };
    let ok = matches!(&r, Err(FemError::InvalidValue { key: "nonlinear", .. }));
    ck.holds("R1 solve refuses a Shell4 block with the J2 material", ok, &d);
    ck.finish();
}
