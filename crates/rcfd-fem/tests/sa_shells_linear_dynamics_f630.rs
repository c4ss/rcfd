//! F-630 (ADR 0798): Shell4 and Shell3 on the linear dynamic paths by an opt-in per path. Checks F1 to F5, H1 to H4, G1 to
//! G8, P1, P2, R1 to R5, X1 to X8, B2 and the negative controls NC1 to NC4 of the registration. B1 is
//! `sa_shells_linear_dynamics_bits_f630.rs`; B3, B4 and Q are existing tests and scripts.

use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::damped::{damped_modes, DampedModes, DampedOptions};
use rcfd_fem::geometric::{geometric_stiffness, Prestress};
use rcfd_fem::harmonic::{participation, HarmonicLoad, HarmonicOptions, HarmonicSolver, ModalBasis, ModalDamping};
use rcfd_fem::harmonic_direct::{BaseMotion, Dashpot, DirectHarmonic, DirectHarmonicOptions, LossFactor, ModulusTable};
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::mass::{consistent, mass_properties, rigid_motions};
use rcfd_fem::material::PronyTerm;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::model::CyclicOptions;
use rcfd_fem::mpc::{modal_analysis_mpc, transformation, uses_transformation, Transformation};
use rcfd_fem::random::RandomSolver;
use rcfd_fem::shell::Shell4Membrane;
use rcfd_fem::shell3::Shell3Membrane;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::transient::{Integrator, LoadHistory, LoadTable, Method, MotionHistory, Rayleigh, TransientOptions};
use rcfd_fem::{
    beam, laminate, shell, shell3, BlockProperty, Constraint, ElasticSupport, ElementKind, FemError, Load, Material,
    Ply, SolidMesh, SolidModel,
};
use rcfd_linalg::complex::C64;
use rcfd_linalg::eigen::{generalized_eigen_dense, ResidualTest};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

const KINDS: [ElementKind; 2] = [ElementKind::Shell4, ElementKind::Shell3];
/// The first root of `1 + cos s cosh s` (`docs/reference-data.d/0000-base.part`, line 2126).
const S1: f64 = 1.875104068711961;

// ------------------------------------------------------------------------------------------------------------- helpers

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
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value > limit;
        println!(
            "{name}: {value:.4e} (limit > {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} <= {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
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

fn max_abs(v: impl IntoIterator<Item = f64>) -> f64 {
    v.into_iter().fold(0.0, |m, x| nmax(m, x.abs()))
}

/// The unknowns of the two kinds (translations; rotations) of a model's equations.
fn kinds_of(model: &SolidModel) -> [Vec<usize>; 2] {
    let d = model.dof_map();
    let mut out = [Vec::new(), Vec::new()];
    for e in 0..d.n_equations() {
        out[usize::from(d.node_component(e).unwrap().1 >= 3)].push(e);
    }
    out
}

/// The registered "rel": per kind the largest absolute difference over the largest reference value, and the larger one.
fn rel(kinds: &[Vec<usize>; 2], a: &[C64], reference: &[C64]) -> f64 {
    let mut worst = 0.0;
    for k in kinds.iter().filter(|k| !k.is_empty()) {
        let d = max_abs(k.iter().map(|&e| (a[e] - reference[e]).abs()));
        let r = max_abs(k.iter().map(|&e| reference[e].abs()));
        worst = nmax(worst, d / r);
    }
    worst
}

/// "rel" over a run: the largest difference of any step over the largest reference value of the run, per kind.
fn rel_run(kinds: &[Vec<usize>; 2], a: &[Vec<f64>], reference: &[Vec<f64>]) -> f64 {
    assert_eq!(a.len(), reference.len());
    let mut worst = 0.0;
    for k in kinds.iter().filter(|k| !k.is_empty()) {
        let (mut d, mut r) = (0.0, 0.0);
        for (x, y) in a.iter().zip(reference) {
            d = nmax(d, max_abs(k.iter().map(|&e| x[e] - y[e])));
            r = nmax(r, max_abs(k.iter().map(|&e| y[e])));
        }
        worst = nmax(worst, d / r);
    }
    worst
}

fn real(v: &[f64]) -> Vec<C64> {
    v.iter().map(|&x| C64::new(x, 0.0)).collect()
}

fn complex(r: &rcfd_fem::harmonic::HarmonicResponse) -> Vec<C64> {
    r.re.iter().zip(&r.im).map(|(&a, &b)| C64::new(a, b)).collect()
}

fn matvec(a: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; a.n];
    a.matvec(x, &mut y);
    y
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Gaussian elimination with partial pivoting of a dense complex system.
fn solve_dense(mut a: Vec<C64>, mut b: Vec<C64>) -> Vec<C64> {
    let n = b.len();
    assert_eq!(a.len(), n * n);
    for k in 0..n {
        let mut p = k;
        for i in k + 1..n {
            if a[i * n + k].abs() > a[p * n + k].abs() {
                p = i;
            }
        }
        if p != k {
            for j in 0..n {
                a.swap(k * n + j, p * n + j);
            }
            b.swap(k, p);
        }
        for i in k + 1..n {
            let f = a[i * n + k] / a[k * n + k];
            for j in k..n {
                let v = a[k * n + j];
                a[i * n + j] -= f * v;
            }
            let v = b[k];
            b[i] -= f * v;
        }
    }
    for k in (0..n).rev() {
        let mut s = b[k];
        for j in k + 1..n {
            s -= a[k * n + j] * b[j];
        }
        b[k] = s / a[k * n + k];
    }
    b
}

/// The dense reduction of a model: its free equations, or the independent unknowns of the transformation.
enum Map {
    Plain(Vec<usize>, usize),
    Tr(Box<Transformation>),
}

impl Map {
    fn of(model: &SolidModel) -> Map {
        if uses_transformation(model) {
            return Map::Tr(Box::new(transformation(model, &[]).unwrap()));
        }
        let d = model.dof_map();
        let mut fixed = vec![false; d.n_equations()];
        for p in rcfd_fem::constraint::prescribed_equations(model, &[]).unwrap() {
            fixed[p.0] = true;
        }
        let free = (0..d.n_equations()).filter(|&e| !fixed[e]).collect();
        Map::Plain(free, d.n_equations())
    }
    fn matrix(&self, a: &CsrMatrix) -> Vec<f64> {
        let full;
        let (c, pick): (&CsrMatrix, Vec<usize>) = match self {
            Map::Plain(free, _) => (a, free.clone()),
            Map::Tr(tr) => {
                full = tr.reduce(a, &vec![0.0; tr.n_unknowns()]).unwrap().0;
                (&full, (0..full.n).collect())
            }
        };
        let n = pick.len();
        let mut at = vec![usize::MAX; c.n];
        for (i, &e) in pick.iter().enumerate() {
            at[e] = i;
        }
        let mut out = vec![0.0; n * n];
        for (i, &e) in pick.iter().enumerate() {
            for p in c.row_ptr[e]..c.row_ptr[e + 1] {
                if at[c.col[p]] != usize::MAX {
                    out[i * n + at[c.col[p]]] += c.val[p];
                }
            }
        }
        out
    }
    fn vector(&self, f: &[f64]) -> Vec<f64> {
        match self {
            Map::Plain(free, _) => free.iter().map(|&e| f[e]).collect(),
            Map::Tr(tr) => {
                let mut g = f.to_vec();
                g.resize(tr.n_unknowns(), 0.0);
                tr.transpose_times(&g)
            }
        }
    }
    /// The mesh part of the expansion.
    fn expand(&self, q: &[f64]) -> Vec<f64> {
        match self {
            Map::Plain(free, n) => {
                let mut x = vec![0.0; *n];
                for (&e, &v) in free.iter().zip(q) {
                    x[e] = v;
                }
                x
            }
            Map::Tr(tr) => {
                let mut x = tr.expand(q);
                x.truncate(tr.n_mesh());
                x
            }
        }
    }
    fn expand_c(&self, q: &[C64]) -> Vec<C64> {
        let re = self.expand(&q.iter().map(|z| z.re).collect::<Vec<_>>());
        let im = self.expand(&q.iter().map(|z| z.im).collect::<Vec<_>>());
        re.into_iter().zip(im).map(|(a, b)| C64::new(a, b)).collect()
    }
}

/// `K` with the elastic supports.
fn stiffness_ext(model: &SolidModel) -> CsrMatrix {
    let mut k = stiffness(model).unwrap();
    rcfd_fem::mpc::add_supports(&mut k, model).unwrap();
    k
}

/// Every mode of the dense reduced pencil of a model.
fn every_mode(model: &SolidModel, k: &CsrMatrix, m: &CsrMatrix) -> (Vec<f64>, Vec<Vec<f64>>) {
    let map = Map::of(model);
    let (kd, md) = (map.matrix(k), map.matrix(m));
    let n = (kd.len() as f64).sqrt().round() as usize;
    let e = generalized_eigen_dense(&kd, &md, n).unwrap();
    (e.values, e.vectors.iter().map(|v| map.expand(v)).collect())
}

/// `(sum_i c_i A_i) x = f` solved dense on the reduction of the model.
fn dense_response(model: &SolidModel, parts: &[(C64, &CsrMatrix)], f: &[C64]) -> Vec<C64> {
    let map = Map::of(model);
    let re = map.vector(&f.iter().map(|z| z.re).collect::<Vec<_>>());
    let im = map.vector(&f.iter().map(|z| z.im).collect::<Vec<_>>());
    let b: Vec<C64> = re.into_iter().zip(im).map(|(x, y)| C64::new(x, y)).collect();
    let n = b.len();
    let mut a = vec![C64::new(0.0, 0.0); n * n];
    for (c, m) in parts {
        for (s, v) in a.iter_mut().zip(map.matrix(m)) {
            *s += *c * v;
        }
    }
    map.expand_c(&solve_dense(a, b))
}

/// C4: the recurrence of one mode started from `q = 1`, `v = 0`, for `n` steps (`n + 1` values).
fn recurrence(lambda: f64, alpha: f64, beta: f64, h: f64, n: usize) -> Vec<f64> {
    let c = alpha + beta * lambda;
    let disc = c * c / 4.0 - lambda;
    if disc < 0.0 {
        let l = C64::new(-c / 2.0, (-disc).sqrt());
        let one = C64::new(1.0, 0.0);
        let mu = (one + l * (h / 2.0)) / (one - l * (h / 2.0));
        let amp = -(l.conj() / (l - l.conj()));
        let mut p = one;
        (0..=n)
            .map(|_| {
                let v = 2.0 * (amp * p).re;
                p *= mu;
                v
            })
            .collect()
    } else {
        let (l1, l2) = (-c / 2.0 + disc.sqrt(), -c / 2.0 - disc.sqrt());
        let mu = |l: f64| (1.0 + l * h / 2.0) / (1.0 - l * h / 2.0);
        let (a, b) = (-l2 / (l1 - l2), l1 / (l1 - l2));
        let (mut p1, mut p2) = (1.0, 1.0);
        (0..=n)
            .map(|_| {
                let v = a * p1 + b * p2;
                p1 *= mu(l1);
                p2 *= mu(l2);
                v
            })
            .collect()
    }
}

/// The run from rest under a constant load by C4 summed over the modes (`n + 1` states).
fn modal_run(lambdas: &[f64], modes: &[Vec<f64>], f: &[f64], ab: (f64, f64), h: f64, n: usize) -> Vec<Vec<f64>> {
    let mut out = vec![vec![0.0; f.len()]; n + 1];
    for (&l, phi) in lambdas.iter().zip(modes) {
        let s = recurrence(l, ab.0, ab.1, h, n);
        let fr = dot(phi, f) / l;
        for (x, q) in out.iter_mut().zip(s) {
            for (xi, p) in x.iter_mut().zip(phi) {
                *xi += fr * (1.0 - q) * p;
            }
        }
    }
    out
}

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap()
}

fn young(e: f64) -> Material {
    Material::isotropic_elastic(e, 0.3, 7850.0, 0.0).unwrap()
}

fn mopts(n: usize) -> ModalOptions {
    let mut o = ModalOptions::new(n)
        .residual_test(ResidualTest::Backward)
        .tolerance(1e-12);
    o.subspace_size = Some(24);
    o.max_iterations = 1000;
    o
}

fn fix(m: &mut SolidModel, set: &str, components: usize) {
    for c in 0..components {
        m.add_constraint(Constraint::displacement(set, c, 0.0)).unwrap();
    }
}

fn rayleigh(zeta: f64, w1: f64, w2: f64) -> Rayleigh {
    Rayleigh::from_ratio(zeta, w1, w2).unwrap()
}

fn constant() -> LoadTable {
    LoadTable::constant(1.0, -1.0, 1.0e3).unwrap()
}

fn shells_only(on: bool) -> HarmonicOptions {
    HarmonicOptions::default().shells(on)
}

// ------------------------------------------------------------------------------------------------------------ strip S(n)

const SL: f64 = 1.0;
const SB: f64 = 0.125;
const ST: f64 = 0.01;
const SE: f64 = 2.0e11;
const SRHO: f64 = 7850.0;
const MESHES: [usize; 3] = [16, 32, 64];

fn s_ei() -> f64 {
    SE * SB * ST.powi(3) / 12.0
}

fn s_omega1() -> f64 {
    S1 * S1 * (s_ei() / (SRHO * SB * ST)).sqrt() / (SL * SL)
}

/// Nodes across the width first; node sets `root`, `tip`.
fn strip(kind: ElementKind, n: usize) -> SolidModel {
    let w = n / 8;
    let id = |i: usize, j: usize| i * (w + 1) + j;
    let mut nodes = Vec::new();
    for i in 0..=n {
        for j in 0..=w {
            nodes.push([SL * i as f64 / n as f64, SB * j as f64 / w as f64, 0.0]);
        }
    }
    let mut conn = Vec::new();
    for i in 0..n {
        for j in 0..w {
            push(
                &mut conn,
                kind,
                [id(i, j), id(i + 1, j), id(i + 1, j + 1), id(i, j + 1)],
            );
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(ST).unwrap()).unwrap();
    let ne = mesh.n_elements();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("root", (0..=w).map(|j| id(0, j)).collect()).unwrap();
    mesh.add_node_set("tip", (0..=w).map(|j| id(n, j)).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(SE, 0.0, SRHO, 0.0).unwrap())
        .unwrap();
    fix(&mut m, "root", 6);
    m
}

fn push(conn: &mut Vec<usize>, kind: ElementKind, q: [usize; 4]) {
    match kind {
        ElementKind::Shell4 => conn.extend(q),
        _ => conn.extend([q[0], q[1], q[2], q[0], q[2], q[3]]),
    }
}

/// The tip force `p` shared over the tip edge in the ratio 1/2, 1, ..., 1, 1/2.
fn tip_force(m: &SolidModel, p: f64) -> Vec<f64> {
    let d = m.dof_map();
    let tip = m.mesh().node_set("tip").unwrap();
    let w = tip.len() - 1;
    let mut f = vec![0.0; d.n_equations()];
    for (j, &n) in tip.iter().enumerate() {
        f[d.equation(n, 2).unwrap()] = p / w as f64 * if j == 0 || j == w { 0.5 } else { 1.0 };
    }
    f
}

/// The mean deflection of the tip edge.
fn tip_mean(m: &SolidModel, u: &[C64]) -> C64 {
    let d = m.dof_map();
    let tip = m.mesh().node_set("tip").unwrap();
    let mut s = C64::new(0.0, 0.0);
    for &n in tip {
        s += u[d.equation(n, 2).unwrap()];
    }
    s / tip.len() as f64
}

/// A complex number's sine, cosine, hyperbolic sine and cosine.
fn trig(z: C64) -> [C64; 4] {
    let i = C64::new(0.0, 1.0);
    let exp = |z: C64| C64::exp_i(z.im).scale(z.re.exp());
    let (p, q) = (exp(i * z), exp(-(i * z)));
    let (a, b) = (exp(z), exp(-z));
    [
        (p - q) / C64::new(0.0, 2.0),
        (p + q) / 2.0,
        (a - b) / 2.0,
        (a + b) / 2.0,
    ]
}

/// The principal fourth root.
fn root4(z: C64) -> C64 {
    C64::exp_i(z.arg() / 4.0).scale(z.abs().powf(0.25))
}

/// C2 with `E I*` and `rho A* omega^2`.
fn c2(ei: C64, mass: C64, p: f64) -> C64 {
    let s = root4(mass / ei * SL.powi(4));
    let [sin, cos, sinh, cosh] = trig(s);
    let one = C64::new(1.0, 0.0);
    C64::new(p * SL.powi(3), 0.0) / (ei * s * s * s) * (sin * cosh - cos * sinh) / (one + cos * cosh)
}

/// C3 at the tip, undamped.
fn c3(ei: f64, rho_a: f64, omega: f64, q: f64) -> f64 {
    let beta = (rho_a * omega * omega / ei).powf(0.25);
    let s = beta * SL;
    let w0 = q / (ei * beta.powi(4));
    let (p, r, m) = (s.cos() + s.cosh(), s.sin() + s.sinh(), s.sin() - s.sinh());
    let det = p * p + r * m;
    let a = w0 * (p * s.cosh() - r * s.sinh()) / det;
    let b = w0 * (p * s.sinh() + m * s.cosh()) / det;
    a * s.cos() + b * s.sin() + (w0 - a) * s.cosh() - b * s.sinh() - w0
}

#[test]
fn f1_f2_modal_analysis_of_the_strip() {
    let mut c = Checks::new();
    for kind in KINDS {
        let mut f = Vec::new();
        for n in MESHES {
            let r = modal_analysis(&strip(kind, n), &mopts(1)).unwrap();
            let w = r.eigenvalues[0].sqrt();
            f.push(w);
            let e = w / s_omega1() - 1.0;
            c.at_most(&format!("F1 {kind:?} n={n} (signed {e:.3e})"), e.abs(), 1e-3);
        }
        let order = ((f[0] - f[1]) / (f[1] - f[2])).log2();
        c.at_most(
            &format!("F2 {kind:?} |order - 2|, order {order:.4}"),
            (order - 2.0).abs(),
            0.5,
        );
    }
    c.finish();
}

/// The initial deflection: `1e-3 L` times the shape of C1, `theta_y = -W'`.
fn first_shape(m: &SolidModel) -> Vec<f64> {
    let d = m.dof_map();
    let mut u = vec![0.0; d.n_equations()];
    let k = (S1.cosh() + S1.cos()) / (S1.sinh() + S1.sin());
    for (n, x) in m.mesh().nodes().iter().enumerate() {
        let y = S1 * x[0] / SL;
        let w = y.cosh() - y.cos() - k * (y.sinh() - y.sin());
        let dw = S1 / SL * (y.sinh() + y.sin() - k * (y.cosh() - y.cos()));
        u[d.equation(n, 2).unwrap()] = 1e-3 * SL * w;
        u[d.equation(n, 4).unwrap()] = -1e-3 * SL * dw;
    }
    let root = m.mesh().node_set("root").unwrap();
    for &n in root {
        for comp in 0..6 {
            assert_eq!(u[d.equation(n, comp).unwrap()], 0.0);
        }
    }
    u
}

/// The tip over 10 periods of 200 steps, and the largest relative change of the energy.
fn free_run(m: &SolidModel, damping: Option<Rayleigh>) -> (Vec<f64>, f64, f64) {
    let h = 2.0 * PI / s_omega1() / 200.0;
    let mut o = TransientOptions::new(h).shells(true);
    if let Some(r) = damping {
        o = o.damping(r);
    }
    let mut it = Integrator::new(m, &[], &o).unwrap();
    let u0 = first_shape(m);
    it.start(0.0, &u0, &vec![0.0; u0.len()]).unwrap();
    let energy = |it: &Integrator| it.kinetic_energy().unwrap() + it.strain_energy().unwrap();
    let e0 = energy(&it);
    let mut tip = vec![tip_mean(m, &real(&it.displacement().unwrap())).re];
    let mut change = 0.0;
    for _ in 0..2000 {
        it.step().unwrap();
        tip.push(tip_mean(m, &real(&it.displacement().unwrap())).re);
        change = nmax(change, (energy(&it) / e0 - 1.0).abs());
    }
    (tip, change, h)
}

#[test]
fn f3_f4_f5_free_vibration_of_the_strip() {
    let mut c = Checks::new();
    let w1 = s_omega1();
    for kind in KINDS {
        for n in MESHES {
            let m = strip(kind, n);
            let (tip, change, h) = free_run(&m, None);
            let crossings: Vec<f64> = (0..tip.len() - 1)
                .filter(|&i| tip[i] > 0.0 && tip[i + 1] <= 0.0)
                .map(|i| (i as f64 + tip[i] / (tip[i] - tip[i + 1])) * h)
                .collect();
            assert!(crossings.len() >= 2);
            let period = (crossings[crossings.len() - 1] - crossings[0]) / (crossings.len() - 1) as f64;
            let e = 2.0 * PI / period / w1 - 1.0;
            c.at_most(
                &format!("F3 {kind:?} n={n} ({} crossings, signed {e:.3e})", crossings.len()),
                e.abs(),
                1e-3,
            );
            c.at_most(&format!("F4 {kind:?} n={n}"), change, 1e-8);
            let (tip, _, _) = free_run(&m, Some(rayleigh(0.02, w1, 6.27 * w1)));
            // the start is the first positive peak
            let mut peaks = vec![tip[0]];
            for i in 1..tip.len() - 1 {
                if tip[i] > 0.0 && tip[i] >= tip[i - 1] && tip[i] > tip[i + 1] {
                    let (a, b, d) = (tip[i - 1], tip[i], tip[i + 1]);
                    peaks.push(b - (d - a) * (d - a) / (8.0 * (a - 2.0 * b + d)));
                }
            }
            assert!(peaks.len() >= 10, "{} peaks", peaks.len());
            let delta = (peaks[2] / peaks[9]).ln() / 7.0;
            let zeta = delta / (4.0 * PI * PI + delta * delta).sqrt();
            let e = zeta / 0.02 - 1.0;
            c.at_most(
                &format!("F5 {kind:?} n={n} ({} peaks, signed {e:.3e})", peaks.len()),
                e.abs(),
                1e-3,
            );
        }
    }
    c.finish();
}

fn h_limit(n: usize) -> f64 {
    match n {
        16 => 1e-2,
        32 => 3e-3,
        _ => 1e-3,
    }
}

#[test]
fn h1_h2_h3_h4_harmonic_tip_response_of_the_strip_nc1() {
    let mut c = Checks::new();
    let w1 = s_omega1();
    let (ei, rho_a) = (s_ei(), SRHO * SB * ST);
    let re = |x: f64| C64::new(x, 0.0);
    for kind in KINDS {
        for n in MESHES {
            let m = strip(kind, n);
            let load = HarmonicLoad::real(tip_force(&m, 10.0)).unwrap();
            let lim = h_limit(n);
            let err = |x: C64, y: C64| (x - y).abs() / y.abs();
            for factor in [0.5, 2.5] {
                let w = factor * w1;
                // H1
                for eta in [0.0, 0.05] {
                    let o = DirectHarmonicOptions::new()
                        .shells(true)
                        .loss_factor("all", LossFactor::Constant(eta));
                    let x = complex(&DirectHarmonic::new(&m, &o).unwrap().response(&load, w).unwrap());
                    let tip = tip_mean(&m, &x);
                    let exact = c2(C64::new(ei, ei * eta), re(rho_a * w * w), 10.0);
                    c.at_most(
                        &format!("H1 {kind:?} n={n} {factor} w1 eta={eta}"),
                        err(tip, exact),
                        lim,
                    );
                    if n == 64 && factor == 0.5 && eta == 0.0 {
                        let wrong = c2(re(ei * 1.1f64.powi(3)), re(rho_a * w * w), 10.0);
                        c.above(&format!("NC1 {kind:?}"), err(tip, wrong), 0.1);
                    }
                }
                // H2
                let r = rayleigh(0.02, w1, 6.27 * w1);
                let o = DirectHarmonicOptions::new().shells(true).rayleigh(r.mass, r.stiffness);
                let x = complex(&DirectHarmonic::new(&m, &o).unwrap().response(&load, w).unwrap());
                let exact = c2(
                    C64::new(ei, ei * w * r.stiffness),
                    C64::new(rho_a * w * w, -rho_a * w * r.mass),
                    10.0,
                );
                c.at_most(
                    &format!("H2 {kind:?} n={n} {factor} w1"),
                    err(tip_mean(&m, &x), exact),
                    lim,
                );
                // H3
                let mut mp = strip(kind, n);
                mp.add_load(Load::shell_pressure("all", 100.0)).unwrap();
                let pressure = HarmonicLoad::from_model(&mp).unwrap();
                let o = DirectHarmonicOptions::new().shells(true);
                let x = complex(&DirectHarmonic::new(&mp, &o).unwrap().response(&pressure, w).unwrap());
                let exact = re(c3(ei, rho_a, w, 100.0 * SB));
                let total: f64 = (0..pressure.re().len() / 6).map(|i| pressure.re()[6 * i + 2]).sum();
                let sign = total.signum();
                c.at_most(
                    &format!("H3 {kind:?} n={n} {factor} w1 (resultant {total:.6})"),
                    err(tip_mean(&m, &x) * sign, exact),
                    lim,
                );
                // H4
                let basis = ModalBasis::from_result(&modal_analysis(&m, &mopts(6)).unwrap()).unwrap();
                let s = HarmonicSolver::new(&m, basis, &HarmonicOptions::new(ModalDamping::Ratio(0.0)).shells(true));
                let x = complex(&s.unwrap().response(&load, w).unwrap());
                let exact = c2(re(ei), re(rho_a * w * w), 10.0);
                c.at_most(
                    &format!("H4 {kind:?} n={n} {factor} w1"),
                    err(tip_mean(&m, &x), exact),
                    lim,
                );
            }
        }
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------- block B

const BL: f64 = 1.0;
const BB: f64 = 0.2;
const BT: f64 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq)]
enum V {
    V1,
    V2,
    V3,
    V4,
    V5,
    V6,
    V7,
}

const ALL_V: [V; 7] = [V::V1, V::V2, V::V3, V::V4, V::V5, V::V6, V::V7];

fn node_b(i: usize, j: usize) -> usize {
    j * 7 + i
}

/// The mesh of block B: node `(i, j)` at `7 j + i`; node sets `root`, `tip`, `corner`, `p<node>`; element sets `all`, and
/// `one` (cells 0 to 2), `two` (cells 3 to 5).
fn b_mesh(v: V) -> SolidMesh {
    let mut nodes = Vec::new();
    for j in 0..2 {
        for i in 0..7 {
            let x = BL * i as f64 / 6.0;
            let z = if v == V::V5 {
                0.08 * (x / BL) * (2.0 * j as f64 - 1.0)
            } else {
                0.0
            };
            nodes.push([x, BB * j as f64, z]);
        }
    }
    let kind = match v {
        V::V2 | V::V4 => ElementKind::Shell3,
        _ => ElementKind::Shell4,
    };
    let cell = |i: usize| [node_b(i, 0), node_b(i + 1, 0), node_b(i + 1, 1), node_b(i, 1)];
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let mut conn = Vec::new();
    let cells = if v == V::V7 { 3 } else { 6 };
    for i in 0..cells {
        push(&mut conn, kind, cell(i));
    }
    mesh.add_block(kind, conn).unwrap();
    if v == V::V7 {
        let mut conn = Vec::new();
        for i in 3..6 {
            push(&mut conn, ElementKind::Shell3, cell(i));
        }
        mesh.add_block(ElementKind::Shell3, conn).unwrap();
        mesh.set_block_property(1, BlockProperty::shell(BT).unwrap()).unwrap();
    }
    let property = match v {
        V::V3 => BlockProperty::shell4_with_membrane(BT, Shell4Membrane::AndesOpt).unwrap(),
        V::V4 => BlockProperty::shell_with_membrane(BT, Shell3Membrane::AndesOpt).unwrap(),
        V::V6 => BlockProperty::layup(vec![
            Ply::new(steel(), 0.06, 0.0).unwrap(),
            Ply::new(
                Material::isotropic_elastic(7.0e10, 0.33, 2700.0, 0.0).unwrap(),
                0.04,
                0.0,
            )
            .unwrap(),
        ])
        .unwrap(),
        _ => BlockProperty::shell(BT).unwrap(),
    };
    mesh.set_block_property(0, property).unwrap();
    let ne = mesh.n_elements();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    let left = |e: usize| {
        let en = mesh.element_nodes(e);
        en.iter().map(|&n| mesh.nodes()[n][0]).sum::<f64>() / (en.len() as f64) < 0.5
    };
    let one: Vec<usize> = (0..ne).filter(|&e| left(e)).collect();
    let two: Vec<usize> = (0..ne).filter(|&e| !left(e)).collect();
    mesh.add_element_set("one", one).unwrap();
    mesh.add_element_set("two", two).unwrap();
    mesh.add_node_set("root", vec![node_b(0, 0), node_b(0, 1)]).unwrap();
    mesh.add_node_set("tip", vec![node_b(6, 0), node_b(6, 1)]).unwrap();
    mesh.add_node_set("corner", vec![node_b(0, 0)]).unwrap();
    for n in 0..14 {
        mesh.add_node_set(&format!("p{n}"), vec![n]).unwrap();
    }
    mesh
}

fn b_free(v: V) -> SolidModel {
    let mut m = SolidModel::new(b_mesh(v));
    m.add_material("all", steel()).unwrap();
    m
}

fn b(v: V) -> SolidModel {
    let mut m = b_free(v);
    fix(&mut m, "root", 6);
    m
}

fn v12(kind: ElementKind) -> V {
    if kind == ElementKind::Shell4 {
        V::V1
    } else {
        V::V2
    }
}

/// Block B in two materials: `E = 2e11` on cells 0 to 2, `e2` on 3 to 5.
fn b_two(kind: ElementKind, e2: f64) -> SolidModel {
    let mut m = SolidModel::new(b_mesh(v12(kind)));
    m.add_material("one", young(2.0e11)).unwrap();
    m.add_material("two", young(e2)).unwrap();
    fix(&mut m, "root", 6);
    m
}

/// The load `f_j = sin(1.3 j + 0.2)` on every free unknown (0 at the prescribed ones).
fn full_load(m: &SolidModel) -> Vec<f64> {
    let d = m.dof_map();
    let mut f: Vec<f64> = (0..d.n_equations()).map(|j| (1.3 * j as f64 + 0.2).sin()).collect();
    for p in rcfd_fem::constraint::prescribed_equations(m, &[]).unwrap() {
        f[p.0] = 0.0;
    }
    f
}

fn omegas(l: &[f64]) -> [f64; 3] {
    [
        0.5 * l[0].sqrt(),
        ((l[0] + l[1]) / 2.0).sqrt(),
        ((l[3] + l[4]) / 2.0).sqrt(),
    ]
}

#[test]
fn g1_g2_direct_harmonic_against_every_mode_nc3() {
    let mut c = Checks::new();
    for v in ALL_V {
        let m = b(v);
        let kinds = kinds_of(&m);
        let basis = ModalBasis::all_modes_with_shells(&m, None).unwrap();
        // G2
        let free = kinds[0].len() + kinds[1].len() - 12;
        let l = basis.eigenvalues().to_vec();
        c.holds(
            &format!("G2 {v:?} count and signs"),
            basis.len() == free && free == 72 && l.iter().all(|x| x.is_finite() && *x > 0.0),
            &format!(
                "{} modes of {free}, lowest {:.4e}, highest {:.4e}",
                basis.len(),
                l[0],
                l[l.len() - 1]
            ),
        );
        let mm = consistent(&m).unwrap();
        let mut worst = 0.0;
        for (r, p) in basis.modes().iter().enumerate() {
            let mp = matvec(&mm, p);
            for (s, q) in basis.modes().iter().enumerate() {
                worst = nmax(worst, (dot(q, &mp) - if r == s { 1.0 } else { 0.0 }).abs());
            }
        }
        c.at_most(&format!("G2 {v:?} Phi^T M Phi - I"), worst, 1e-8);
        // G1
        let f = full_load(&m);
        let load = HarmonicLoad::real(f.clone()).unwrap();
        let r = rayleigh(0.02, l[0].sqrt(), l[1].sqrt());
        for (dname, ab) in [("undamped", (0.0, 0.0)), ("Rayleigh", (r.mass, r.stiffness))] {
            let direct =
                DirectHarmonic::new(&m, &DirectHarmonicOptions::new().shells(true).rayleigh(ab.0, ab.1)).unwrap();
            let damping = ModalDamping::Rayleigh {
                mass: ab.0,
                stiffness: ab.1,
            };
            let s = HarmonicSolver::new(&m, basis.clone(), &HarmonicOptions::new(damping).shells(true)).unwrap();
            for (w, name) in omegas(&l).into_iter().zip(["w_a", "w_b", "w_c"]) {
                let x = complex(&direct.response(&load, w).unwrap());
                let y = complex(&s.response(&load, w).unwrap());
                c.at_most(&format!("G1 {v:?} {dname} {name}"), rel(&kinds, &x, &y), 1e-8);
            }
        }
        if v == V::V1 {
            let w = omegas(&l)[0];
            let out = (0..basis.len())
                .max_by(|&i, &j| {
                    dot(&basis.modes()[i], &f)
                        .abs()
                        .total_cmp(&dot(&basis.modes()[j], &f).abs())
                })
                .unwrap();
            let keep = |r: usize| r != out;
            let cut = ModalBasis::new(
                (0..basis.len()).filter(|&r| keep(r)).map(|r| l[r]).collect(),
                (0..basis.len())
                    .filter(|&r| keep(r))
                    .map(|r| basis.modes()[r].clone())
                    .collect(),
                m.dof_map(),
            )
            .unwrap();
            let s = HarmonicSolver::new(&m, cut, &HarmonicOptions::default().static_correction(false)).unwrap();
            let y = complex(&s.response(&load, w).unwrap());
            let x = DirectHarmonic::new(&m, &DirectHarmonicOptions::new().shells(true)).unwrap();
            let x = complex(&x.response(&load, w).unwrap());
            c.above(&format!("NC3 (mode {out} left out)"), rel(&kinds, &x, &y), 1e-6);
        }
    }
    c.finish();
}

/// The states of a run from rest (`n + 1` of them, mesh part).
fn rest_run(m: &SolidModel, f: &[f64], ab: Option<Rayleigh>, h: f64, n: usize, shells: bool) -> Vec<Vec<f64>> {
    let mut o = TransientOptions::new(h).shells(shells);
    if let Some(r) = ab {
        o = o.damping(r);
    }
    let mut it = Integrator::new(m, &[LoadHistory::new(f.to_vec(), constant())], &o).unwrap();
    let z = vec![0.0; it.n_unknowns()];
    it.start(0.0, &z, &z).unwrap();
    let neq = m.dof_map().n_equations();
    let state = |it: &Integrator| {
        let mut u = it.displacement().unwrap();
        u.truncate(neq);
        u
    };
    let mut out = vec![state(&it)];
    for _ in 0..n {
        it.step().unwrap();
        out.push(state(&it));
    }
    out
}

#[test]
fn g3_transient_against_the_modal_recurrence_nc2() {
    let mut c = Checks::new();
    for v in ALL_V {
        let m = b(v);
        let kinds = kinds_of(&m);
        let basis = ModalBasis::all_modes_with_shells(&m, None).unwrap();
        let l = basis.eigenvalues().to_vec();
        let f = full_load(&m);
        let h = 2.0 * PI / l[0].sqrt() / 40.0;
        let r = rayleigh(0.02, l[0].sqrt(), l[1].sqrt());
        for (name, d) in [("undamped", None), ("Rayleigh", Some(r))] {
            let ab = d.map_or((0.0, 0.0), |r: Rayleigh| (r.mass, r.stiffness));
            let x = rest_run(&m, &f, d, h, 80, true);
            let y = modal_run(&l, basis.modes(), &f, ab, h, 80);
            c.at_most(&format!("G3 {v:?} {name}"), rel_run(&kinds, &x, &y), 1e-8);
            if v == V::V1 && d.is_none() {
                let mut mm = consistent(&m).unwrap();
                let dofs = m.dof_map();
                let rot = |e: usize| dofs.node_component(e).unwrap().1 >= 3;
                for i in 0..mm.n {
                    for p in mm.row_ptr[i]..mm.row_ptr[i + 1] {
                        if rot(i) && rot(mm.col[p]) {
                            mm.val[p] *= 0.5;
                        }
                    }
                }
                let (lw, _) = every_mode(&m, &stiffness(&m).unwrap(), &mm);
                let y = modal_run(&lw, basis.modes(), &f, ab, h, 80);
                c.above("NC2", rel_run(&kinds, &x, &y), 1e-6);
            }
        }
    }
    c.finish();
}

/// The roots of a damped analysis that oscillate, by ascending imaginary part.
fn oscillating(r: &DampedModes) -> Vec<C64> {
    r.modes.iter().filter(|m| m.oscillating).map(|m| m.eigenvalue).collect()
}

#[test]
fn g4_damped_modes_against_the_closed_roots() {
    let mut c = Checks::new();
    for v in ALL_V {
        let m = b(v);
        let basis = ModalBasis::all_modes_with_shells(&m, None).unwrap();
        let l = &basis.eigenvalues()[..8];
        let r = rayleigh(0.02, l[0].sqrt(), l[1].sqrt());
        let mut exact: Vec<C64> = l
            .iter()
            .map(|&l| {
                let g = (r.mass + r.stiffness * l) / 2.0;
                C64::new(-g, (l - g * g).sqrt())
            })
            .collect();
        let check = |c: &mut Checks, name: &str, o: &DampedOptions, exact: &mut Vec<C64>| {
            exact.retain(|z| z.im.is_finite());
            exact.sort_by(|a, b| a.im.total_cmp(&b.im));
            let got = oscillating(&damped_modes(&m, o).unwrap());
            assert!(got.len() >= 6 && exact.len() >= 6);
            let worst = (0..6).fold(0.0, |w, i| nmax(w, (got[i] - exact[i]).abs() / exact[i].abs()));
            c.at_most(&format!("G4 {v:?} {name}"), worst, 1e-8);
        };
        let o = DampedOptions::with_modal(mopts(8)).shells(true);
        check(&mut c, "Rayleigh", &o.clone().rayleigh(r.mass, r.stiffness), &mut exact);
        let z = C64::new(1.0, 0.05);
        let half = C64::exp_i(z.arg() / 2.0).scale(z.abs().sqrt());
        let mut exact: Vec<C64> = l.iter().map(|&l| C64::new(0.0, l.sqrt()) * half).collect();
        check(
            &mut c,
            "loss factor",
            &o.loss_factor("all", LossFactor::Constant(0.05)),
            &mut exact,
        );
    }
    c.finish();
}

fn difference(a: &CsrMatrix, b: &CsrMatrix) -> CsrMatrix {
    let mut d = a.clone();
    assert!(a.col == b.col && a.row_ptr == b.row_ptr);
    for (x, y) in d.val.iter_mut().zip(&b.val) {
        *x -= y;
    }
    d
}

#[test]
fn g5_g6_loss_factor_on_one_material_nc4() {
    let mut c = Checks::new();
    let eta = 0.1;
    for kind in KINDS {
        let m = b_two(kind, 7.0e10);
        let kinds = kinds_of(&m);
        let k = stiffness(&m).unwrap();
        let k2 = difference(&stiffness(&b_two(kind, 14.0e10)).unwrap(), &k);
        let mm = consistent(&m).unwrap();
        let (l, _) = every_mode(&m, &k, &mm);
        let f = full_load(&m);
        let load = HarmonicLoad::real(f.clone()).unwrap();
        let o = DirectHarmonicOptions::new().shells(true);
        let one = o.clone().loss_factor("two", LossFactor::Constant(eta));
        let both = one.clone().loss_factor("one", LossFactor::Constant(eta));
        for (w, name) in omegas(&l).into_iter().zip(["w_a", "w_b", "w_c"]) {
            let parts = [
                (C64::new(1.0, 0.0), &k),
                (C64::new(0.0, eta), &k2),
                (C64::new(-w * w, 0.0), &mm),
            ];
            let y = dense_response(&m, &parts, &real(&f));
            let run =
                |o: &DirectHarmonicOptions| complex(&DirectHarmonic::new(&m, o).unwrap().response(&load, w).unwrap());
            c.at_most(&format!("G5 {kind:?} {name}"), rel(&kinds, &run(&one), &y), 1e-8);
            c.above(&format!("NC4 {kind:?} {name}"), rel(&kinds, &run(&both), &y), 1e-3);
        }
        // G6
        let phi = modal_analysis(&m, &mopts(8)).unwrap().modes;
        let d = DampedOptions::with_modal(mopts(8))
            .shells(true)
            .loss_factor("two", LossFactor::Constant(eta));
        let r = damped_modes(&m, &d).unwrap();
        let apply = |a: &CsrMatrix, x: &[C64]| -> Vec<C64> {
            let re = matvec(a, &x.iter().map(|z| z.re).collect::<Vec<_>>());
            let im = matvec(a, &x.iter().map(|z| z.im).collect::<Vec<_>>());
            phi.iter().map(|p| C64::new(dot(p, &re), dot(p, &im))).collect()
        };
        let norm = |v: &[C64]| v.iter().map(|z| z.norm_sqr()).sum::<f64>().sqrt();
        let modes: Vec<_> = r.modes.iter().filter(|x| x.oscillating).collect();
        assert!(modes.len() >= 6);
        let mut worst = 0.0;
        for x in &modes[..6] {
            let z = x.eigenvalue;
            let a: Vec<C64> = apply(&mm, &x.shape).into_iter().map(|v| z * z * v).collect();
            let b1 = apply(&k, &x.shape);
            let b2: Vec<C64> = apply(&k2, &x.shape)
                .into_iter()
                .map(|v| C64::new(0.0, eta) * v)
                .collect();
            let sum: Vec<C64> = (0..a.len()).map(|i| a[i] + b1[i] + b2[i]).collect();
            worst = nmax(worst, norm(&sum) / (norm(&a) + norm(&b1) + norm(&b2)));
        }
        c.at_most(&format!("G6 {kind:?}"), worst, 1e-8);
    }
    c.finish();
}

/// Block B with the load `full_load` as nodal forces and moments of the model.
fn b_loaded(v: V) -> SolidModel {
    let mut m = b(v);
    let f = full_load(&m);
    for n in 0..14 {
        let g = &f[6 * n..6 * n + 6];
        m.add_load(Load::nodal_force(&format!("p{n}"), [g[0], g[1], g[2]]))
            .unwrap();
        m.add_load(Load::nodal_moment(&format!("p{n}"), [g[3], g[4], g[5]]))
            .unwrap();
    }
    m
}

#[test]
fn g7_static_correction_at_zero_frequency() {
    let mut c = Checks::new();
    for v in [V::V1, V::V2] {
        let m = b_loaded(v);
        let kinds = kinds_of(&m);
        let basis = ModalBasis::from_result(&modal_analysis(&m, &mopts(4)).unwrap()).unwrap();
        let load = HarmonicLoad::from_model(&m).unwrap();
        let s = HarmonicSolver::new(&m, basis, &shells_only(true)).unwrap();
        let x = complex(&s.response(&load, 0.0).unwrap());
        let u = solve_static(&m, &StaticOptions::default()).unwrap();
        c.at_most(
            &format!("G7 {v:?} static"),
            rel(&kinds, &x, &real(u.displacement_values())),
            1e-8,
        );
        let d = DirectHarmonic::new(&m, &DirectHarmonicOptions::new().shells(true)).unwrap();
        let y = complex(&d.response(&load, 0.0).unwrap());
        c.at_most(&format!("G7 {v:?} direct"), rel(&kinds, &x, &y), 1e-8);
    }
    c.finish();
}

#[test]
fn g8_free_body_with_inertia_relief() {
    let mut c = Checks::new();
    for v in [V::V1, V::V2] {
        let m = b_free(v);
        let kinds = kinds_of(&m);
        let basis = ModalBasis::all_modes_with_shells(&m, None).unwrap();
        let l = basis.eigenvalues().to_vec();
        let w = ((l[6] + l[7]) / 2.0).sqrt();
        let load = HarmonicLoad::real(full_load(&m)).unwrap();
        let s = HarmonicSolver::new(&m, basis, &shells_only(true)).unwrap();
        let x = complex(&s.response(&load, w).unwrap());
        let d = DirectHarmonic::new(&m, &DirectHarmonicOptions::new().shells(true)).unwrap();
        let y = complex(&d.response(&load, w).unwrap());
        c.at_most(
            &format!("G8 {v:?} (lambda_6 {:.2e}, lambda_7 {:.2e})", l[5], l[6]),
            rel(&kinds, &x, &y),
            1e-8,
        );
    }
    c.finish();
}

#[test]
fn p1_p2_prestress() {
    let mut c = Checks::new();
    for v in [V::V1, V::V2] {
        let m = b(v);
        let kinds = kinds_of(&m);
        let p = Prestress::uniform(&m, [1.0e8, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
        let k = stiffness(&m).unwrap();
        let kg = geometric_stiffness(&m, &p, None).unwrap();
        assert!(k.col == kg.col && k.row_ptr == kg.row_ptr);
        let mut ks = k.clone();
        for (x, y) in ks.val.iter_mut().zip(&kg.val) {
            *x += y;
        }
        let mm = consistent(&m).unwrap();
        let (l, modes) = every_mode(&m, &ks, &mm);
        let basis = ModalBasis::new(l.clone(), modes, m.dof_map()).unwrap();
        let f = full_load(&m);
        let load = HarmonicLoad::real(f.clone()).unwrap();
        let d = DirectHarmonic::new(&m, &DirectHarmonicOptions::new().shells(true).prestress(p.clone())).unwrap();
        let s = HarmonicSolver::new(&m, basis, &shells_only(true).prestress(p)).unwrap();
        for (w, name) in omegas(&l).into_iter().take(2).zip(["w_a", "w_b"]) {
            let x = complex(&d.response(&load, w).unwrap());
            let parts = [(C64::new(1.0, 0.0), &ks), (C64::new(-w * w, 0.0), &mm)];
            let y = dense_response(&m, &parts, &real(&f));
            c.at_most(&format!("P1 {v:?} {name}"), rel(&kinds, &x, &y), 1e-8);
            let z = complex(&s.response(&load, w).unwrap());
            c.at_most(&format!("P2 {v:?} {name}"), rel(&kinds, &z, &x), 1e-8);
        }
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------- transformation path

#[test]
fn r1_rigid_root_against_the_clamp() {
    let mut c = Checks::new();
    for v in [V::V1, V::V2] {
        let clamped = b(v);
        let mut rigid = b_free(v);
        rigid
            .add_constraint(Constraint::rigid_set("root", [0.0, 0.1, 0.0]))
            .unwrap();
        fix(&mut rigid, "corner", 6);
        let kinds = kinds_of(&clamped);
        let f = full_load(&clamped);
        let load = HarmonicLoad::real(f.clone()).unwrap();
        let l = ModalBasis::all_modes_with_shells(&clamped, None).unwrap();
        let l = l.eigenvalues().to_vec();
        let o = DirectHarmonicOptions::new().shells(true);
        let superposed = |m: &SolidModel, w: f64| {
            let basis = ModalBasis::from_result(&modal_analysis(m, &mopts(4)).unwrap()).unwrap();
            let s = HarmonicSolver::new(m, basis, &shells_only(true)).unwrap();
            complex(&s.response(&load, w).unwrap())
        };
        for (w, name) in omegas(&l).into_iter().take(2).zip(["w_a", "w_b"]) {
            let x = complex(&DirectHarmonic::new(&rigid, &o).unwrap().response(&load, w).unwrap());
            let y = complex(&DirectHarmonic::new(&clamped, &o).unwrap().response(&load, w).unwrap());
            c.at_most(&format!("R1 {v:?} direct {name}"), rel(&kinds, &x, &y), 1e-8);
            c.at_most(
                &format!("R1 {v:?} superposition {name}"),
                rel(&kinds, &superposed(&rigid, w), &superposed(&clamped, w)),
                1e-8,
            );
        }
        let h = 2.0 * PI / l[0].sqrt() / 40.0;
        let x = rest_run(&rigid, &f, None, h, 20, true);
        let y = rest_run(&clamped, &f, None, h, 20, true);
        c.at_most(&format!("R1 {v:?} transient"), rel_run(&kinds, &x, &y), 1e-8);
    }
    c.finish();
}

fn b_spring(v: V) -> SolidModel {
    let mut m = b(v);
    for comp in 0..6 {
        let k = if comp < 3 { 1.0e8 } else { 1.0e6 };
        m.add_support(ElasticSupport::nodal_spring("tip", comp, k)).unwrap();
    }
    m
}

/// The direct harmonic against every reduced mode with the static correction.
fn direct_against_reduced(c: &mut Checks, name: &str, m: &SolidModel, dampings: &[bool]) {
    let kinds = kinds_of(m);
    let (l, modes) = every_mode(m, &stiffness_ext(m), &consistent(m).unwrap());
    let basis = ModalBasis::new(l.clone(), modes, m.dof_map()).unwrap();
    let load = HarmonicLoad::real(full_load(m)).unwrap();
    let r = rayleigh(0.02, l[0].sqrt(), l[1].sqrt());
    for &damped in dampings {
        let ab = if damped { (r.mass, r.stiffness) } else { (0.0, 0.0) };
        let o = DirectHarmonicOptions::new().shells(true).rayleigh(ab.0, ab.1);
        let d = DirectHarmonic::new(m, &o).unwrap();
        let damping = ModalDamping::Rayleigh {
            mass: ab.0,
            stiffness: ab.1,
        };
        let s = HarmonicSolver::new(m, basis.clone(), &HarmonicOptions::new(damping).shells(true)).unwrap();
        for (w, wn) in omegas(&l).into_iter().take(2).zip(["w_a", "w_b"]) {
            let x = complex(&d.response(&load, w).unwrap());
            let y = complex(&s.response(&load, w).unwrap());
            c.at_most(&format!("{name} {wn} damped={damped}"), rel(&kinds, &x, &y), 1e-8);
        }
    }
}

/// The transient from rest against C4 over every reduced mode.
fn transient_against_reduced(c: &mut Checks, name: &str, m: &SolidModel, dampings: &[bool]) {
    let kinds = kinds_of(m);
    let (l, modes) = every_mode(m, &stiffness_ext(m), &consistent(m).unwrap());
    let f = full_load(m);
    let h = 2.0 * PI / l[0].sqrt() / 40.0;
    let r = rayleigh(0.02, l[0].sqrt(), l[1].sqrt());
    for &damped in dampings {
        let ab = if damped { (r.mass, r.stiffness) } else { (0.0, 0.0) };
        let x = rest_run(m, &f, if damped { Some(r) } else { None }, h, 80, true);
        let y = modal_run(&l, &modes, &f, ab, h, 80);
        c.at_most(&format!("{name} damped={damped}"), rel_run(&kinds, &x, &y), 1e-8);
    }
}

#[test]
fn r2_r3_springs_at_the_tip() {
    let mut c = Checks::new();
    for v in [V::V1, V::V2] {
        let m = b_spring(v);
        direct_against_reduced(&mut c, &format!("R2 {v:?}"), &m, &[false, true]);
        transient_against_reduced(&mut c, &format!("R3 {v:?}"), &m, &[false, true]);
    }
    c.finish();
}

#[test]
fn r4_base_motion() {
    let mut c = Checks::new();
    for v in [V::V1, V::V2] {
        let m = b(v);
        let kinds = kinds_of(&m);
        let k = stiffness(&m).unwrap();
        let mm = consistent(&m).unwrap();
        let (l, _) = every_mode(&m, &k, &mm);
        let rz = &rigid_motions(&m, [0.0; 3])[2];
        let d = DirectHarmonic::new(&m, &DirectHarmonicOptions::new().shells(true)).unwrap();
        let mut base = BaseMotion::new();
        base.add_set(&m, "root", 2, C64::new(1.0, 0.0)).unwrap();
        let none = HarmonicLoad::real(vec![0.0; rz.len()]).unwrap();
        for (w, name) in omegas(&l).into_iter().take(2).zip(["w_a", "w_b"]) {
            let x = complex(&d.response_with_base(&none, &base, w).unwrap());
            let f: Vec<C64> = matvec(&mm, rz).iter().map(|&v| C64::new(w * w * v, 0.0)).collect();
            let parts = [(C64::new(1.0, 0.0), &k), (C64::new(-w * w, 0.0), &mm)];
            let y: Vec<C64> = dense_response(&m, &parts, &f)
                .into_iter()
                .zip(rz)
                .map(|(a, &r)| a + C64::new(r, 0.0))
                .collect();
            c.at_most(&format!("R4 {v:?} {name}"), rel(&kinds, &x, &y), 1e-8);
        }
    }
    c.finish();
}

#[test]
fn r5_motion_history() {
    let mut c = Checks::new();
    for v in [V::V1, V::V2] {
        let clamped = b(v);
        let l = ModalBasis::all_modes_with_shells(&clamped, None).unwrap();
        let h = 2.0 * PI / l.eigenvalues()[0].sqrt() / 40.0;
        let mut m = b_free(v);
        for comp in [0, 1, 3, 4, 5] {
            m.add_constraint(Constraint::displacement("root", comp, 0.0)).unwrap();
        }
        let moved: Vec<Prescribed> = m
            .mesh()
            .node_set("root")
            .unwrap()
            .iter()
            .map(|&n| Prescribed::new(n, 2, 1.0))
            .collect();
        let table = LoadTable::new(vec![(-1.0, -1.0), (1.0, 1.0)]).unwrap();
        let o = TransientOptions::new(h).shells(true);
        let mut it = Integrator::with_motion(&m, &[], &[MotionHistory::new(moved, table)], &o).unwrap();
        let rz = rigid_motions(&m, [0.0; 3])[2].clone();
        assert_eq!(it.n_unknowns(), rz.len());
        it.start(0.0, &vec![0.0; rz.len()], &rz).unwrap();
        it.run(40).unwrap();
        let x = it.displacement().unwrap();
        let t = 40.0 * h;
        // the exact rotations are zero: both kinds are measured against the translation t
        let worst = max_abs(x.iter().zip(&rz).map(|(a, r)| a - t * r)) / t;
        c.at_most(&format!("R5 {v:?}"), worst, 1e-8);
    }
    c.finish();
}

// --------------------------------------------------------------------------------------------------- mixed models, names

/// Mixed X: the Hex8 block on `x < 0`, its far face clamped, joined to the root edge of B by a rigid set.
fn mixed(v: V, e_shell: f64) -> SolidModel {
    let hex = hex_box([-0.2, 0.0, -0.1], [0.2, 0.2, 0.2], [1, 1, 1]).unwrap();
    let shell_mesh = b_mesh(v);
    let mut nodes = hex.nodes().to_vec();
    let nh = nodes.len();
    nodes.extend_from_slice(shell_mesh.nodes());
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, hex.element_nodes(0).to_vec())
        .unwrap();
    let kind = shell_mesh.element_kind(0);
    let mut conn = Vec::new();
    for e in 0..shell_mesh.n_elements() {
        conn.extend(shell_mesh.element_nodes(e).iter().map(|&n| n + nh));
    }
    mesh.add_block(kind, conn).unwrap();
    mesh.set_block_property(1, BlockProperty::shell(BT).unwrap()).unwrap();
    let ne = mesh.n_elements();
    mesh.add_element_set("solid", vec![0]).unwrap();
    mesh.add_element_set("shell", (1..ne).collect()).unwrap();
    let far: Vec<usize> = (0..nh).filter(|&n| hex.nodes()[n][0] < -0.1).collect();
    let mut joint: Vec<usize> = (0..nh).filter(|&n| hex.nodes()[n][0] > -0.1).collect();
    assert!(far.len() == 4 && joint.len() == 4);
    joint.extend([nh + node_b(0, 0), nh + node_b(0, 1)]);
    mesh.add_node_set("far", far).unwrap();
    mesh.add_node_set("joint", joint).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("solid", Material::isotropic_elastic(7.0e10, 0.33, 2700.0, 0.0).unwrap())
        .unwrap();
    m.add_material("shell", young(e_shell)).unwrap();
    fix(&mut m, "far", 3);
    m.add_constraint(Constraint::rigid_set("joint", [0.0, 0.1, 0.0]))
        .unwrap();
    m
}

#[test]
fn x1_x2_mixed_solid_and_shell() {
    let mut c = Checks::new();
    let eta = 0.1;
    for v in [V::V1, V::V2] {
        let m = mixed(v, 2.0e11);
        direct_against_reduced(&mut c, &format!("X1 {v:?}"), &m, &[true]);
        let k = stiffness_ext(&m);
        let k2 = difference(&stiffness_ext(&mixed(v, 4.0e11)), &k);
        let mm = consistent(&m).unwrap();
        let (l, _) = every_mode(&m, &k, &mm);
        let f = full_load(&m);
        let o = DirectHarmonicOptions::new()
            .shells(true)
            .loss_factor("shell", LossFactor::Constant(eta));
        let d = DirectHarmonic::new(&m, &o).unwrap();
        let load = HarmonicLoad::real(f.clone()).unwrap();
        for (w, name) in omegas(&l).into_iter().take(2).zip(["w_a", "w_b"]) {
            let parts = [
                (C64::new(1.0, 0.0), &k),
                (C64::new(0.0, eta), &k2),
                (C64::new(-w * w, 0.0), &mm),
            ];
            let y = dense_response(&m, &parts, &real(&f));
            let x = complex(&d.response(&load, w).unwrap());
            c.at_most(&format!("X1 {v:?} loss {name}"), rel(&kinds_of(&m), &x, &y), 1e-8);
        }
        transient_against_reduced(&mut c, &format!("X2 {v:?}"), &m, &[false]);
    }
    c.finish();
}

/// The innermost error.
fn root(e: &FemError) -> &FemError {
    match e {
        FemError::Element { source, .. } => root(source),
        other => other,
    }
}

type Outcome = (&'static str, Result<(), FemError>);

/// The modal basis of one unit vector (the refusals come before the modes are used).
fn dummy_basis(m: &SolidModel) -> ModalBasis {
    let d = m.dof_map();
    ModalBasis::new(vec![1.0], vec![vec![1.0; d.n_equations()]], d).unwrap()
}

fn on_transformation(m: &SolidModel, set: &str) -> SolidModel {
    let mut t = m.clone();
    t.add_constraint(Constraint::rigid_set(set, [0.0, 0.1, 0.0])).unwrap();
    t
}

/// What each path of D3 says about a model; with `set`, the transformation branches too (a rigid set on `set`).
fn paths(m: &SolidModel, set: Option<&str>, shells: bool) -> Vec<Outcome> {
    let o = TransientOptions::new(1e-5).shells(shells);
    let h = HarmonicOptions::default().shells(shells);
    let d = DirectHarmonicOptions::new().shells(shells);
    let all = |m: &SolidModel| {
        if shells {
            ModalBasis::all_modes_with_shells(m, None)
        } else {
            ModalBasis::all_modes(m, None)
        }
    };
    let mut out: Vec<Outcome> = vec![
        ("transient plain", Integrator::new(m, &[], &o).map(|_| ())),
        (
            "damped",
            damped_modes(m, &DampedOptions::with_modal(mopts(4)).shells(shells)).map(|_| ()),
        ),
        ("all modes", all(m).map(|_| ())),
        (
            "static correction plain",
            HarmonicSolver::new(m, dummy_basis(m), &h).map(|_| ()),
        ),
        ("direct plain", DirectHarmonic::new(m, &d).map(|_| ())),
    ];
    if let Some(set) = set {
        let t = on_transformation(m, set);
        out.push(("transient transformation", Integrator::new(&t, &[], &o).map(|_| ())));
        out.push((
            "static correction transformation",
            HarmonicSolver::new(&t, dummy_basis(&t), &h).map(|_| ()),
        ));
        out.push(("direct transformation", DirectHarmonic::new(&t, &d).map(|_| ())));
    }
    out
}

fn expect(c: &mut Checks, name: &str, r: &Result<(), FemError>, ok: impl Fn(&FemError) -> bool) {
    match r {
        Ok(()) => c.holds(name, false, "taken"),
        Err(e) => {
            let text: String = format!("{e}").chars().take(90).collect();
            c.holds(name, ok(root(e)), &format!("refused: {text}"));
        }
    }
}

fn package(e: &FemError, p: &str) -> bool {
    matches!(e, FemError::NotYetSupported { package, .. } if *package == p)
}

fn key(e: &FemError, k: &str) -> bool {
    matches!(e, FemError::InvalidValue { key, .. } if *key == k)
}

fn frustum(kind: ElementKind, options: CyclicOptions) -> SolidModel {
    const N: usize = 6;
    const NC: usize = 2;
    const NA: usize = 3;
    let origin = [0.3, -0.2, 0.5];
    let n = [1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0];
    let l = 5.0f64.sqrt();
    let ea = [2.0 / l, -1.0 / l, 0.0];
    let eb = [
        n[1] * ea[2] - n[2] * ea[1],
        n[2] * ea[0] - n[0] * ea[2],
        n[0] * ea[1] - n[1] * ea[0],
    ];
    let id = |i: usize, a: usize| a * (NC + 1) + i;
    let mut nodes = Vec::new();
    for a in 0..=NA {
        for i in 0..=NC {
            let phi = 2.0 * PI * i as f64 / (N * NC) as f64;
            let s = a as f64 / NA as f64;
            let (r, z) = (1.0 - 0.4 * s, 0.8 * s);
            nodes.push([0, 1, 2].map(|k| origin[k] + r * (phi.cos() * ea[k] + phi.sin() * eb[k]) + z * n[k]));
        }
    }
    let mut conn = Vec::new();
    for a in 0..NA {
        for i in 0..NC {
            push(
                &mut conn,
                kind,
                [id(i, a), id(i + 1, a), id(i + 1, a + 1), id(i, a + 1)],
            );
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.05).unwrap()).unwrap();
    let ne = mesh.n_elements();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("clamp", (0..=NC).map(|i| id(i, 0)).collect())
        .unwrap();
    mesh.add_node_set("low", (0..=NA).map(|a| id(0, a)).collect()).unwrap();
    mesh.add_node_set("high", (0..=NA).map(|a| id(NC, a)).collect())
        .unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    fix(&mut m, "clamp", 6);
    m.add_constraint(Constraint::cyclic_with("low", "high", n, origin, N, options))
        .unwrap();
    m
}

#[test]
fn x3_refusals_with_the_option() {
    let mut c = Checks::new();
    // a shell and a beam
    for kind in KINDS {
        let mut mesh = b_mesh(v12(kind));
        mesh.add_block(ElementKind::Beam2, vec![node_b(6, 0), node_b(6, 1)])
            .unwrap();
        let section = beam::rectangle_section(0.02, 0.02, 2.0e-8, 5.0 / 6.0).unwrap();
        mesh.set_block_property(1, BlockProperty::beam(section, [0.0, 0.0, 1.0]).unwrap())
            .unwrap();
        mesh.add_element_set("beam", vec![mesh.n_elements() - 1]).unwrap();
        let mut m = SolidModel::new(mesh);
        m.add_material("all", steel()).unwrap();
        m.add_material("beam", steel()).unwrap();
        fix(&mut m, "root", 6);
        for (p, r) in paths(&m, Some("tip"), true) {
            expect(&mut c, &format!("X3 {kind:?} and beam: {p}"), &r, |e| {
                package(e, beam::FOLLOW_UP)
            });
        }
    }
    // a shell node shared with a solid
    for kind in KINDS {
        let hex = hex_box([0.0; 3], [1.0, 1.0, 1.0], [1, 1, 1]).unwrap();
        let mut nodes = hex.nodes().to_vec();
        let at = |p: [f64; 3]| hex.nodes().iter().position(|q| *q == p).unwrap();
        let (q0, q1) = (at([1.0, 0.0, 0.0]), at([1.0, 1.0, 0.0]));
        nodes.push([2.0, 0.0, 0.0]);
        nodes.push([2.0, 1.0, 0.0]);
        let mut conn = Vec::new();
        push(&mut conn, kind, [q0, 8, 9, q1]);
        let mut mesh = SolidMesh::new(nodes).unwrap();
        mesh.add_block(ElementKind::Hex8, hex.element_nodes(0).to_vec())
            .unwrap();
        mesh.add_block(kind, conn).unwrap();
        mesh.set_block_property(1, BlockProperty::shell(0.1).unwrap()).unwrap();
        let ne = mesh.n_elements();
        mesh.add_element_set("all", (0..ne).collect()).unwrap();
        mesh.add_node_set("tip", vec![8, 9]).unwrap();
        mesh.add_node_set("far", (0..8).filter(|&n| hex.nodes()[n][0] == 0.0).collect())
            .unwrap();
        let mut m = SolidModel::new(mesh);
        m.add_material("all", steel()).unwrap();
        fix(&mut m, "far", 3);
        for (p, r) in paths(&m, Some("tip"), true) {
            expect(&mut c, &format!("X3 {kind:?} node of a solid: {p}"), &r, |e| {
                key(e, "node")
            });
        }
    }
    // warp above the limit (Shell4 only: a triangle is flat)
    {
        let nodes = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.2], [1.0, 1.0, 0.0], [0.0, 1.0, 0.2]];
        let mut mesh = SolidMesh::new(nodes).unwrap();
        mesh.add_block(ElementKind::Shell4, vec![0, 1, 2, 3]).unwrap();
        mesh.set_block_property(0, BlockProperty::shell(0.1).unwrap()).unwrap();
        mesh.add_element_set("all", vec![0]).unwrap();
        mesh.add_node_set("root", vec![0, 3]).unwrap();
        mesh.add_node_set("tip", vec![1, 2]).unwrap();
        let mut m = SolidModel::new(mesh);
        m.add_material("all", steel()).unwrap();
        fix(&mut m, "root", 6);
        for (p, r) in paths(&m, Some("tip"), true) {
            expect(&mut c, &format!("X3 warp: {p}"), &r, |e| key(e, "warp"));
        }
    }
    // a Shell3 layup
    {
        let mut mesh = b_mesh(V::V2);
        let layup = BlockProperty::layup(vec![Ply::new(steel(), BT, 0.0).unwrap()]).unwrap();
        mesh.set_block_property(0, layup).unwrap();
        let mut m = SolidModel::new(mesh);
        m.add_material("all", steel()).unwrap();
        fix(&mut m, "root", 6);
        for (p, r) in paths(&m, Some("tip"), true) {
            expect(&mut c, &format!("X3 Shell3 layup: {p}"), &r, |e| {
                package(e, laminate::FOLLOW_UP)
            });
        }
    }
    // a viscoelastic material with a shell
    for kind in KINDS {
        let mut m = SolidModel::new(b_mesh(v12(kind)));
        let visco = Material::viscoelastic(steel(), vec![PronyTerm::new(0.2, 0.2, 1.0).unwrap()], None).unwrap();
        m.add_material("all", visco).unwrap();
        fix(&mut m, "root", 6);
        let nys = |e: &FemError| matches!(e, FemError::NotYetSupported { .. });
        for (p, r) in paths(&m, Some("tip"), true) {
            expect(&mut c, &format!("X3 {kind:?} viscoelastic: {p}"), &r, nys);
        }
        let o = DampedOptions::with_modal(mopts(4))
            .shells(true)
            .viscoelastic_reference(100.0, None);
        let r = damped_modes(&m, &o).map(|_| ());
        expect(
            &mut c,
            &format!("X3 {kind:?} viscoelastic: damped at a reference"),
            &r,
            |e| {
                matches!(
                    e,
                    FemError::NotYetSupported {
                        item: "element kind",
                        ..
                    }
                )
            },
        );
    }
    // a loss table on a layup
    {
        let table = ModulusTable::new(vec![1.0, 1000.0], vec![2.0e11, 2.0e11], vec![1.0e9, 1.0e9]).unwrap();
        let o = DirectHarmonicOptions::new()
            .shells(true)
            .loss_factor("all", LossFactor::Table(table));
        let r = DirectHarmonic::new(&b(V::V6), &o).map(|_| ());
        expect(&mut c, "X3 table on a layup: direct plain", &r, |e| {
            key(e, "loss factor")
        });
        let r = DirectHarmonic::new(&on_transformation(&b(V::V6), "tip"), &o).map(|_| ());
        expect(&mut c, "X3 table on a layup: direct transformation", &r, |e| {
            key(e, "loss factor")
        });
        let r = DirectHarmonic::new(&b(V::V1), &o).map(|_| ());
        c.holds("X3 table on a Shell block", r.is_ok(), &format!("{r:?}"));
    }
    // a cyclic constraint without its option
    for (kind, own) in [
        (ElementKind::Shell4, shell::FOLLOW_UP),
        (ElementKind::Shell3, shell3::FOLLOW_UP),
    ] {
        let m = frustum(kind, CyclicOptions::new());
        let r = Integrator::new(&m, &[], &TransientOptions::new(1e-5).shells(true)).map(|_| ());
        expect(&mut c, &format!("X3 {kind:?} cyclic: transient"), &r, |e| {
            package(e, own) && matches!(e, FemError::NotYetSupported { variant: "Cyclic", .. })
        });
        for (which, m) in [
            ("without", m),
            ("with", frustum(kind, CyclicOptions::new().shells(true))),
        ] {
            let r = DirectHarmonic::new(&m, &DirectHarmonicOptions::new().shells(true)).map(|_| ());
            expect(
                &mut c,
                &format!("X3 {kind:?} cyclic {which} its option: direct"),
                &r,
                |e| key(e, "constraint"),
            );
            let r = HarmonicSolver::new(&m, dummy_basis(&m), &shells_only(true)).map(|_| ());
            expect(
                &mut c,
                &format!("X3 {kind:?} cyclic {which} its option: static correction"),
                &r,
                |e| key(e, "static correction"),
            );
        }
    }
    // the random response, the stress of a response
    for v in [V::V1, V::V2] {
        let m = b(v);
        let r = RandomSolver::new(&m, dummy_basis(&m), &shells_only(true)).map(|_| ());
        expect(&mut c, &format!("X3 {v:?} random"), &r, |e| {
            package(e, "a follow-up of ADR 0798")
        });
        let d = DirectHarmonic::new(&m, &DirectHarmonicOptions::new().shells(true)).unwrap();
        let x = d.response(&HarmonicLoad::real(full_load(&m)).unwrap(), 100.0).unwrap();
        let any = |_: &FemError| true;
        expect(
            &mut c,
            &format!("X3 {v:?} response stress"),
            &x.stress(&m).map(|_| ()),
            any,
        );
        expect(
            &mut c,
            &format!("X3 {v:?} direct stress"),
            &d.stress(&m, &x).map(|_| ()),
            any,
        );
    }
    c.finish();
}

#[test]
fn x4_refusals_and_paths_without_the_option() {
    let mut c = Checks::new();
    for (v, own) in [(V::V1, shell::FOLLOW_UP), (V::V2, shell3::FOLLOW_UP)] {
        let m = b(v);
        let mut plain = paths(&m, None, false);
        let mut free = b_free(v);
        free.add_constraint(Constraint::rigid_set("root", [0.0, 0.1, 0.0]))
            .unwrap();
        fix(&mut free, "corner", 6);
        let o = TransientOptions::new(1e-5);
        plain.push(("transient transformation", Integrator::new(&free, &[], &o).map(|_| ())));
        let motion = MotionHistory::new(
            vec![Prescribed::new(6, 2, 1.0)],
            LoadTable::new(vec![(-1.0, -1.0), (1.0, 1.0)]).unwrap(),
        );
        plain.push((
            "transient motion",
            Integrator::with_motion(&m, &[], &[motion], &o).map(|_| ()),
        ));
        let h = HarmonicOptions::default();
        plain.push((
            "static correction transformation",
            HarmonicSolver::new(&free, dummy_basis(&free), &h).map(|_| ()),
        ));
        let body = b_free(v);
        plain.push((
            "static correction free body",
            HarmonicSolver::new(&body, dummy_basis(&body), &h).map(|_| ()),
        ));
        plain.push((
            "direct transformation",
            DirectHarmonic::new(&free, &DirectHarmonicOptions::new()).map(|_| ()),
        ));
        c.holds(
            &format!("X4 {v:?} paths"),
            plain.len() == 10,
            &format!("{}", plain.len()),
        );
        for (p, r) in plain {
            expect(&mut c, &format!("X4 {v:?} {p}"), &r, |e| package(e, own));
        }
        // taken
        let r = modal_analysis(&m, &mopts(4));
        c.holds(&format!("X4 {v:?} modal"), r.is_ok(), "taken");
        let t = modal_analysis(&free, &mopts(4));
        c.holds(&format!("X4 {v:?} modal transformation"), t.is_ok(), "taken");
        let basis = ModalBasis::from_result(&r.unwrap()).unwrap();
        let s = HarmonicSolver::new(&m, basis.clone(), &HarmonicOptions::default().static_correction(false));
        c.holds(
            &format!("X4 {v:?} superposition without correction"),
            s.is_ok(),
            "taken",
        );
        let p = participation(&m, &basis, [0.0, 0.0, 1.0], None);
        c.holds(&format!("X4 {v:?} participation"), p.is_ok(), "taken");
    }
    c.finish();
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

#[test]
fn x5_one_ply_layup_is_the_shell() {
    let mut c = Checks::new();
    let one = b(V::V1);
    let mut mesh = b_mesh(V::V1);
    mesh.set_block_property(
        0,
        BlockProperty::layup(vec![Ply::new(steel(), BT, 0.0).unwrap()]).unwrap(),
    )
    .unwrap();
    let mut ply = SolidModel::new(mesh);
    ply.add_material("all", steel()).unwrap();
    fix(&mut ply, "root", 6);
    let f = full_load(&one);
    let load = HarmonicLoad::real(f.clone()).unwrap();
    let l = ModalBasis::all_modes_with_shells(&one, None).unwrap().eigenvalues()[0];
    let h = 2.0 * PI / l.sqrt() / 40.0;
    let w = 0.5 * l.sqrt();
    let run = |m: &SolidModel| -> Vec<(&'static str, Vec<f64>)> {
        let d = DampedOptions::with_modal(mopts(6)).shells(true).rayleigh(10.0, 1e-6);
        let d = damped_modes(m, &d).unwrap();
        let basis = ModalBasis::all_modes_with_shells(m, None).unwrap();
        let s = HarmonicSolver::new(m, basis, &shells_only(true)).unwrap();
        let s = s.response(&load, w).unwrap();
        let x = DirectHarmonic::new(m, &DirectHarmonicOptions::new().shells(true)).unwrap();
        let x = x.response(&load, w).unwrap();
        vec![
            ("transient", rest_run(m, &f, None, h, 20, true).concat()),
            (
                "damped",
                d.modes
                    .iter()
                    .flat_map(|m| m.shape.iter().chain([&m.eigenvalue]).flat_map(|z| [z.re, z.im]))
                    .collect(),
            ),
            ("superposition", [s.re, s.im].concat()),
            ("direct", [x.re, x.im].concat()),
        ]
    };
    for ((name, a), (_, b)) in run(&one).into_iter().zip(run(&ply)) {
        let finite = a.iter().all(|x| x.is_finite()) && !a.is_empty();
        c.holds(
            &format!("X5 {name}"),
            finite && bits(&a) == bits(&b),
            &format!("{} values, equal bits", a.len()),
        );
    }
    c.finish();
}

#[test]
fn x6_frustum_sector_transient() {
    let mut c = Checks::new();
    for kind in KINDS {
        let m = frustum(kind, CyclicOptions::new().shells(true));
        let kinds = kinds_of(&m);
        let r = modal_analysis_mpc(&m, &[], &mopts(4)).unwrap();
        for mode in [0, 2] {
            let l = r.result.eigenvalues[mode];
            let phi = &r.expanded[mode];
            let h = 2.0 * PI / l.sqrt() / 40.0;
            let mut it = Integrator::new(&m, &[], &TransientOptions::new(h).shells(true)).unwrap();
            it.start(0.0, phi, &vec![0.0; phi.len()]).unwrap();
            let mut x = vec![it.displacement().unwrap()];
            for _ in 0..80 {
                it.step().unwrap();
                x.push(it.displacement().unwrap());
            }
            let y: Vec<Vec<f64>> = recurrence(l, 0.0, 0.0, h, 80)
                .into_iter()
                .map(|s| phi.iter().map(|p| s * p).collect())
                .collect();
            c.at_most(&format!("X6 {kind:?} mode {}", mode + 1), rel_run(&kinds, &x, &y), 1e-8);
        }
    }
    c.finish();
}

#[test]
fn x7_x8_mass_properties() {
    let mut c = Checks::new();
    for v in [V::V1, V::V2] {
        let m = b_free(v);
        let p = mass_properties(&m, &consistent(&m).unwrap()).unwrap();
        let mass = 7850.0 * BL * BB * BT;
        c.at_most(&format!("X7 {v:?} mass"), (p.mass / mass - 1.0).abs(), 1e-12);
        let exact = [
            mass * (BB * BB + BT * BT) / 12.0,
            mass * (BL * BL + BT * BT) / 12.0,
            mass * (BL * BL + BB * BB) / 12.0 + mass * BT * BT / 12.0,
        ];
        for i in 0..3 {
            c.at_most(
                &format!("X7 {v:?} I_{}", ["x", "y", "z"][i]),
                (p.inertia[i][i] / exact[i] - 1.0).abs(),
                1e-12,
            );
        }
        let off = max_abs([p.inertia[0][1], p.inertia[0][2], p.inertia[1][2]]);
        c.at_most(&format!("X7 {v:?} products of inertia"), off / exact[2], 1e-12);
    }
    let m = b_free(V::V6);
    let p = mass_properties(&m, &consistent(&m).unwrap()).unwrap();
    let exact = (0.06 * 7850.0 + 0.04 * 2700.0) * 0.2;
    c.at_most("X8 mass", (p.mass / exact - 1.0).abs(), 1e-12);
    c.finish();
}

// ------------------------------------------------------------------------------------- B2: models without shells, opted in

/// The Hex8 box `1 x 0.2 x 0.1`, 4 x 2 x 1 of B1; element sets `one` (x < 1/2) and `two`.
fn box_model(two_materials: bool) -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [1.0, 0.2, 0.1], [4, 2, 1]).unwrap();
    let left = |e: usize| {
        let x: f64 = mesh.element_nodes(e).iter().map(|&n| mesh.nodes()[n][0]).sum::<f64>() / 8.0;
        x < 0.5
    };
    let one: Vec<usize> = (0..mesh.n_elements()).filter(|&e| left(e)).collect();
    let two: Vec<usize> = (0..mesh.n_elements()).filter(|&e| !left(e)).collect();
    mesh.add_element_set("one", one).unwrap();
    mesh.add_element_set("two", two).unwrap();
    let mut m = SolidModel::new(mesh);
    if two_materials {
        m.add_material("one", steel()).unwrap();
        m.add_material("two", Material::isotropic_elastic(7.0e10, 0.33, 2700.0, 0.0).unwrap())
            .unwrap();
    } else {
        m.add_material("all", steel()).unwrap();
    }
    m.add_load(Load::nodal_force("xmax", [3.0, 5.0, 7.0])).unwrap();
    m
}

fn box_clamped(two_materials: bool) -> SolidModel {
    let mut m = box_model(two_materials);
    fix(&mut m, "xmin", 3);
    m
}

fn box_transformed() -> SolidModel {
    let mut m = box_model(false);
    m.add_constraint(Constraint::rigid_set("xmin", [0.0, 0.1, 0.05]))
        .unwrap();
    m.add_support(ElasticSupport::nodal_spring("xmax", 2, 1.0e7)).unwrap();
    for comp in 0..3 {
        m.add_support(ElasticSupport::nodal_spring("xmin", comp, 1.0e9))
            .unwrap();
    }
    m
}

/// Every Hex8 result of B1, with or without the option.
fn box_results(on: bool) -> Vec<(String, Vec<f64>)> {
    let mut out: Vec<(String, Vec<f64>)> = Vec::new();
    let (one, two, tr, free) = (
        box_clamped(false),
        box_clamped(true),
        box_transformed(),
        box_model(false),
    );
    let ray = Rayleigh {
        mass: 12.0,
        stiffness: 3.0e-6,
    };
    let w = 900.0;
    let t = TransientOptions::new(2.0e-5).shells(on);
    let td = t.clone().damping(ray).method(Method::GeneralisedAlpha { rho_inf: 0.8 });
    let history = |m: &SolidModel| {
        vec![LoadHistory::new(
            nodal_load_vector(m, &[]).unwrap(),
            LoadTable::constant(1.0, -1.0, 1.0).unwrap(),
        )]
    };
    let state = |it: &Integrator| {
        [
            it.displacement().unwrap(),
            it.velocity().unwrap(),
            it.acceleration().unwrap(),
        ]
        .concat()
    };
    for (name, m) in [("clamped", &one), ("two materials", &two), ("transformation", &tr)] {
        for (d, o) in [("transient", &t), ("transient damped", &td)] {
            let mut it = Integrator::new(m, &history(m), o).unwrap();
            let z = vec![0.0; it.n_unknowns()];
            it.start(0.0, &z, &z).unwrap();
            it.run(30).unwrap();
            out.push((format!("{d} {name}"), state(&it)));
        }
    }
    let table = LoadTable::new(vec![(-1.0, -1.0), (0.0, 0.0), (1.0, 1.0)]).unwrap();
    let moved: Vec<Prescribed> = free
        .mesh()
        .node_set("xmin")
        .unwrap()
        .iter()
        .flat_map(|&n| {
            [
                Prescribed::new(n, 0, 0.0),
                Prescribed::new(n, 1, 0.0),
                Prescribed::new(n, 2, 0.5),
            ]
        })
        .collect();
    let mut it = Integrator::with_motion(&free, &history(&free), &[MotionHistory::new(moved, table)], &td).unwrap();
    let nq = it.free().len();
    it.start_reduced(0.0, &vec![0.0; nq], &vec![0.0; nq]).unwrap();
    it.run(30).unwrap();
    out.push(("transient motion".into(), state(&it)));
    let tip = one.mesh().node_set("xmax").unwrap()[0];
    let damped = |m: &SolidModel, o: DampedOptions| -> Vec<f64> {
        let r = damped_modes(m, &o.shells(on)).unwrap();
        r.modes
            .iter()
            .flat_map(|m| m.shape.iter().chain([&m.eigenvalue]).flat_map(|z| [z.re, z.im]))
            .collect()
    };
    let o = DampedOptions::with_modal(mopts(6));
    let d = o.clone().rayleigh(12.0, 3.0e-6).dashpot(Dashpot::ground(tip, 2, 50.0));
    out.push(("damped clamped".into(), damped(&one, d)));
    let d = o
        .clone()
        .loss_factor("one", LossFactor::Constant(0.02))
        .loss_factor("two", LossFactor::Constant(0.07));
    out.push(("damped two materials".into(), damped(&two, d)));
    let d = o.residual_vectors(true).dashpot(Dashpot::ground(tip, 1, 80.0));
    out.push(("damped residual".into(), damped(&one, d)));
    let every = |m: &SolidModel| {
        if on {
            ModalBasis::all_modes_with_shells(m, Some(2)).unwrap()
        } else {
            ModalBasis::all_modes(m, Some(2)).unwrap()
        }
    };
    let all = every(&one);
    out.push(("all modes eigenvalues".into(), all.eigenvalues().to_vec()));
    out.push(("all modes".into(), all.modes().concat()));
    let load = HarmonicLoad::from_model(&one).unwrap();
    let h = HarmonicOptions::new(ModalDamping::Ratio(0.01)).shells(on);
    let pre = Prestress::uniform(&one, [2.0e7, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    let four = ModalBasis::from_result(&modal_analysis(&one, &mopts(4)).unwrap()).unwrap();
    let on_tr = ModalBasis::from_result(&modal_analysis(&tr, &mopts(4)).unwrap()).unwrap();
    for (name, m, basis, o) in [
        ("all", &one, all, h.clone()),
        ("four", &one, four.clone(), h.clone()),
        ("prestress", &one, four, h.clone().prestress(pre.clone())),
        ("free", &free, every(&free), h.clone()),
        ("transformation", &tr, on_tr, h),
    ] {
        let r = HarmonicSolver::new(m, basis, &o).unwrap().response(&load, w).unwrap();
        out.push((format!("superposition {name}"), [r.re, r.im].concat()));
    }
    let o = DirectHarmonicOptions::new().shells(on);
    for (name, m) in [("clamped", &one), ("two materials", &two), ("transformation", &tr)] {
        let s = DirectHarmonic::new(m, &o.clone().rayleigh(12.0, 3.0e-6)).unwrap();
        let r = s.response(&load, w).unwrap();
        out.push((format!("direct {name}"), [r.re, r.im].concat()));
    }
    let loss = o
        .clone()
        .loss_factor("one", LossFactor::Constant(0.02))
        .dashpot(Dashpot::ground(tip, 2, 50.0));
    let s = DirectHarmonic::new(&two, &loss).unwrap();
    let r = s.response(&load, w).unwrap();
    out.push(("direct loss".into(), [r.re, r.im].concat()));
    let mut base = BaseMotion::new();
    base.add_set(&two, "xmin", 2, C64::new(1.0e-3, 2.0e-4)).unwrap();
    let r = s.response_with_base(&load, &base, w).unwrap();
    out.push(("direct base".into(), [r.re, r.im].concat()));
    let r = DirectHarmonic::new(&one, &o.prestress(pre)).unwrap();
    let r = r.response(&load, w).unwrap();
    out.push(("direct prestress".into(), [r.re, r.im].concat()));
    out
}

#[test]
fn b2_models_without_shells_with_the_option() {
    let mut c = Checks::new();
    let (a, b) = (box_results(false), box_results(true));
    assert_eq!(a.len(), b.len());
    for ((name, x), (_, y)) in a.iter().zip(&b) {
        let finite = !x.is_empty() && x.iter().all(|v| v.is_finite());
        c.holds(
            &format!("B2 {name}"),
            finite && bits(x) == bits(y),
            &format!("{} values, equal bits", x.len()),
        );
    }
    c.finish();
}
