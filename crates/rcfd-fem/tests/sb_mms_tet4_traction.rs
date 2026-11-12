//! F-458 (ADR 0365): the L2 and energy order of Tet4 in case T of the manufactured solution of ADR 0279 (F-378) on the pair
//! 16 -> 32 cells per edge (98 304 free unknowns at the finest, direct solver), registered as a check. The code is copied from
//! `sb_mms.rs` (ADR 0279), reduced to Tet4 case T; ADR 0279 ran this pair once as a diagnostic (L2 order 1.9176).

use rcfd_core::vec3::{cross, norm, Vec3};
use rcfd_fem::builders::tet_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::element::element_for;
use rcfd_fem::loads::{body_force_loads, face_rule, face_shape_functions, NodalForceAt};
use rcfd_fem::material::{isotropic_elasticity, Matrix6};
use rcfd_fem::mesh::SolidMesh;
use rcfd_fem::solve::{solve_static_with, StaticOptions};
use rcfd_fem::{Material, SolidModel, SolidResult};
use std::collections::BTreeSet;
use std::f64::consts::PI;
use std::time::Instant;

const E_MOD: f64 = 1.0;
const NU: f64 = 0.3;
const AMPLITUDE: f64 = 1.0;

// Named constants that the mutations of the registration change (values of ADR 0279 in the committed file).
const TRACTION_SCALE: f64 = 1.0;
const LAMBDA_SIGN: f64 = 1.0;
const LOADED: [(&str, usize); 3] = [("xmax", 0), ("ymax", 1), ("zmax", 2)];
const LEVELS: [usize; 3] = [8, 16, 32];
const FAST_LEVELS: [usize; 3] = [2, 4, 8];

// Arithmetic 1 of ADR 0279: wavenumbers in units of pi and phases, row = component, column = coordinate.
const K: [[f64; 3]; 3] = [[1.0, 0.5, 0.75], [0.75, 1.0, 0.5], [0.5, 0.75, 1.0]];
const PHI: [[f64; 3]; 3] = [[0.3, 0.7, 0.5], [0.9, 0.2, 0.6], [0.4, 0.8, 0.1]];

/// A list of measured values against their limits; every value is printed, the test fails at the end if any missed.
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
            "{name}: {value:.4} (limit >= {limit}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value} < {limit}"));
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

fn lame(nu: f64) -> (f64, f64) {
    (E_MOD * nu / ((1.0 + nu) * (1.0 - 2.0 * nu)), E_MOD / (2.0 * (1.0 + nu)))
}

fn d_matrix() -> Matrix6 {
    isotropic_elasticity(E_MOD, NU)
}

/// A displacement field with its first and second derivatives: `grad[i][m] = d_m u_i`, `hess[i][m][n] = d_m d_n u_i`.
trait Field {
    fn u(&self, x: Vec3) -> Vec3;
    fn grad(&self, x: Vec3) -> [[f64; 3]; 3];
    fn hess(&self, x: Vec3) -> [[[f64; 3]; 3]; 3];
}

/// Voigt strain (engineering shear) from a displacement gradient.
fn voigt_strain(g: &[[f64; 3]; 3]) -> [f64; 6] {
    [
        g[0][0],
        g[1][1],
        g[2][2],
        g[0][1] + g[1][0],
        g[1][2] + g[2][1],
        g[2][0] + g[0][2],
    ]
}

/// The stress tensor `lambda tr(eps) I + 2 mu eps` of a field at a point.
fn stress_tensor(f: &dyn Field, x: Vec3) -> [[f64; 3]; 3] {
    let (lambda, mu) = lame(NU);
    let g = f.grad(x);
    let div = g[0][0] + g[1][1] + g[2][2];
    let mut s = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            s[i][j] = mu * (g[i][j] + g[j][i]) + if i == j { lambda * div } else { 0.0 };
        }
    }
    s
}

/// The body force `b_i = -[(s lambda + mu) d_i div u + mu Laplace u_i]` (arithmetic 3); `lambda_sign = -1` is the negative
/// control of arithmetic 6.
fn body_force(f: &dyn Field, x: Vec3, lambda_sign: f64) -> Vec3 {
    let (lambda, mu) = lame(NU);
    let h = f.hess(x);
    let mut b = [0.0; 3];
    for (i, bi) in b.iter_mut().enumerate() {
        let grad_div: f64 = (0..3).map(|k| h[k][i][k]).sum();
        let lap: f64 = (0..3).map(|j| h[i][j][j]).sum();
        *bi = -((lambda_sign * lambda + mu) * grad_div + mu * lap);
    }
    b
}

type Mat3 = [[f64; 3]; 3];

/// The manufactured field of arithmetic 1.
struct Trig;

impl Trig {
    fn parts(x: Vec3) -> (Mat3, Mat3, Mat3) {
        let mut s = [[0.0; 3]; 3];
        let mut c = [[0.0; 3]; 3];
        let mut k = [[0.0; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                k[i][j] = PI * K[i][j];
                let a = k[i][j] * x[j] + PHI[i][j];
                s[i][j] = a.sin();
                c[i][j] = a.cos();
            }
        }
        (s, c, k)
    }
}

impl Field for Trig {
    fn u(&self, x: Vec3) -> Vec3 {
        let (s, _, _) = Trig::parts(x);
        [0, 1, 2].map(|i| AMPLITUDE * s[i][0] * s[i][1] * s[i][2])
    }
    fn grad(&self, x: Vec3) -> [[f64; 3]; 3] {
        let (s, c, k) = Trig::parts(x);
        let mut g = [[0.0; 3]; 3];
        for i in 0..3 {
            for m in 0..3 {
                let mut p = AMPLITUDE * k[i][m] * c[i][m];
                for j in (0..3).filter(|&j| j != m) {
                    p *= s[i][j];
                }
                g[i][m] = p;
            }
        }
        g
    }
    fn hess(&self, x: Vec3) -> [[[f64; 3]; 3]; 3] {
        let (s, c, k) = Trig::parts(x);
        let mut h = [[[0.0; 3]; 3]; 3];
        for i in 0..3 {
            for m in 0..3 {
                for n in 0..3 {
                    let mut p = AMPLITUDE;
                    if m == n {
                        p *= -k[i][m] * k[i][m];
                        for j in 0..3 {
                            p *= s[i][j];
                        }
                    } else {
                        p *= k[i][m] * c[i][m] * k[i][n] * c[i][n];
                        for j in (0..3).filter(|&j| j != m && j != n) {
                            p *= s[i][j];
                        }
                    }
                    h[i][m][n] = p;
                }
            }
        }
        h
    }
}

/// Gauss-Legendre nodes and weights on [-1, 1] by Newton's method on the three-term recurrence of `P_n`.
fn gauss_legendre(n: usize) -> (Vec<f64>, Vec<f64>) {
    // P_n(t) and P_n'(t) from the recurrence k P_k = (2k - 1) t P_{k-1} - (k - 1) P_{k-2}.
    let legendre = |t: f64| -> (f64, f64) {
        let (mut p0, mut p1) = (1.0, t);
        for k in 2..=n {
            let p2 = ((2 * k - 1) as f64 * t * p1 - (k - 1) as f64 * p0) / k as f64;
            p0 = p1;
            p1 = p2;
        }
        (p1, n as f64 * (t * p1 - p0) / (t * t - 1.0))
    };
    let mut x = Vec::with_capacity(n);
    let mut w = Vec::with_capacity(n);
    for i in 1..=n {
        let mut t = (PI * (i as f64 - 0.25) / (n as f64 + 0.5)).cos();
        for _ in 0..100 {
            let (p, dp) = legendre(t);
            let dt = p / dp;
            t -= dt;
            if dt.abs() < 1e-16 {
                break;
            }
        }
        let (_, dp) = legendre(t);
        x.push(t);
        w.push(2.0 / ((1.0 - t * t) * dp * dp));
    }
    (x, w)
}

/// The 125-point error rule of the tetrahedron (degree 7) as (reference point, weight).
fn error_rule() -> Vec<([f64; 3], f64)> {
    let (g, gw) = gauss_legendre(5);
    let mut out = Vec::with_capacity(125);
    let s: Vec<f64> = g.iter().map(|t| 0.5 * (1.0 + t)).collect();
    let sw: Vec<f64> = gw.iter().map(|w| 0.5 * w).collect();
    for k in 0..5 {
        for j in 0..5 {
            for i in 0..5 {
                let (u, v, w) = (s[i], s[j], s[k]);
                let p = [u, (1.0 - u) * v, (1.0 - u) * (1.0 - v) * w];
                let jac = (1.0 - u) * (1.0 - u) * (1.0 - v);
                out.push((p, sw[i] * sw[j] * sw[k] * jac));
            }
        }
    }
    out
}

/// The consistent nodal load `int N_a t(x) dS` of a traction given by a function on a surface set, accumulated into a
/// node-major vector (three entries per node) in face order and point order, as the library does for a uniform traction.
fn add_traction<T>(f: &mut [f64], mesh: &SolidMesh, set: &str, traction: T)
where
    T: Fn(Vec3) -> Vec3,
{
    for &face in mesh.surface_set(set).unwrap() {
        let nodes = mesh.face_nodes(face);
        let x: Vec<Vec3> = nodes.iter().map(|&a| mesh.nodes()[a]).collect();
        let mut fe = vec![[0.0; 3]; nodes.len()];
        for (uv, w) in face_rule(nodes.len()).unwrap() {
            let (nf, d) = face_shape_functions(nodes.len(), uv).unwrap();
            let mut xu = [0.0; 3];
            let mut xv = [0.0; 3];
            let mut xq = [0.0; 3];
            for ((xa, da), na) in x.iter().zip(&d).zip(&nf) {
                for i in 0..3 {
                    xu[i] += xa[i] * da[0];
                    xv[i] += xa[i] * da[1];
                    xq[i] += xa[i] * na;
                }
            }
            let s = norm(cross(xu, xv));
            let t0 = traction(xq);
            let t = [t0[0] * s, t0[1] * s, t0[2] * s];
            for (fa, na) in fe.iter_mut().zip(&nf) {
                for i in 0..3 {
                    fa[i] += w * na * t[i];
                }
            }
        }
        for (&node, fa) in nodes.iter().zip(&fe) {
            for i in 0..3 {
                f[3 * node + i] += fa[i];
            }
        }
    }
}

/// The traction `sigma n` of a field on a box face with outward normal `+e_axis`.
fn face_traction(field: &dyn Field, x: Vec3, axis: usize) -> Vec3 {
    let s = stress_tensor(field, x);
    [s[0][axis], s[1][axis], s[2][axis]]
}

/// Case T: `xmin`, `ymin`, `zmin` prescribed at the exact nodal value, tractions on `xmax`, `ymax`, `zmax`.
fn prescribed_nodes(mesh: &SolidMesh) -> BTreeSet<usize> {
    let mut set = BTreeSet::new();
    for name in ["xmin", "ymin", "zmin"] {
        set.extend(mesh.node_set(name).unwrap().iter().copied());
    }
    set
}

fn solve_case_t(field: &dyn Field, n: usize) -> (SolidModel, SolidResult) {
    let mesh = tet_box([0.0; 3], [1.0; 3], [n; 3]).unwrap();
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", Material::isotropic_elastic(E_MOD, NU, 0.0, 0.0).unwrap())
        .unwrap();
    let mesh = model.mesh();
    let mut prescribed = Vec::new();
    for node in prescribed_nodes(mesh) {
        let v = field.u(mesh.nodes()[node]);
        for (k, &vk) in v.iter().enumerate() {
            prescribed.push(Prescribed::new(node, k, vk));
        }
    }
    let mut forces = body_force_loads(&model, |x| body_force(field, x, LAMBDA_SIGN)).unwrap();
    let mut f = vec![0.0; 3 * mesh.nodes().len()];
    for (set, axis) in LOADED {
        add_traction(&mut f, mesh, set, |x| {
            let t = face_traction(field, x, axis);
            t.map(|v| v * TRACTION_SCALE)
        });
    }
    forces.extend(
        f.chunks(3)
            .enumerate()
            .map(|(node, v)| NodalForceAt::new(node, [v[0], v[1], v[2]])),
    );
    let result = solve_static_with(&model, &prescribed, &forces, &StaticOptions::default()).unwrap();
    (model, result)
}

/// L2 and energy errors against the exact field with the 125-point rule (degree 7).
fn errors(field: &dyn Field, model: &SolidModel, result: &SolidResult) -> [f64; 2] {
    let mesh = model.mesh();
    let d = d_matrix();
    let rule = error_rule();
    let mut acc = [0.0; 2];
    for e in 0..mesh.n_elements() {
        let element = element_for(mesh.element_kind(e));
        let nodes = mesh.element_nodes(e);
        let coords: Vec<Vec3> = nodes.iter().map(|&a| mesh.nodes()[a]).collect();
        let ue: Vec<&[f64]> = nodes.iter().map(|&a| result.displacement(a)).collect();
        for &(xi, wq) in &rule {
            let p = element.point(&coords, xi).unwrap();
            let w = wq * p.det_j;
            let mut uh = [0.0; 3];
            let mut gh = [[0.0; 3]; 3];
            for (a, ua) in ue.iter().enumerate() {
                for i in 0..3 {
                    uh[i] += p.n[a] * ua[i];
                    for m in 0..3 {
                        gh[i][m] += ua[i] * p.grad[a][m];
                    }
                }
            }
            let u = field.u(p.x);
            let eps = voigt_strain(&field.grad(p.x));
            let eps_h = voigt_strain(&gh);
            let de: [f64; 6] = std::array::from_fn(|r| eps[r] - eps_h[r]);
            let mut energy = 0.0;
            for r in 0..6 {
                for c in 0..6 {
                    energy += de[r] * d[r][c] * de[c];
                }
            }
            acc[0] += w * (0..3).map(|i| (u[i] - uh[i]) * (u[i] - uh[i])).sum::<f64>();
            acc[1] += w * energy;
        }
    }
    acc.map(f64::sqrt)
}

struct Level {
    n: usize,
    unknowns: usize,
    l2: f64,
    energy: f64,
}

fn run_levels(ns: &[usize]) -> Vec<Level> {
    ns.iter()
        .map(|&n| {
            let t0 = Instant::now();
            let (model, result) = solve_case_t(&Trig, n);
            let [l2, energy] = errors(&Trig, &model, &result);
            let unknowns = result.stats().map(|s| s.unknowns()).unwrap_or(0);
            println!(
                "  Tet4 case T n = {n:2}: free unknowns {unknowns}, e_L2 {l2:.4e}, e_E {energy:.4e}, {:.2} s",
                t0.elapsed().as_secs_f64()
            );
            Level {
                n,
                unknowns,
                l2,
                energy,
            }
        })
        .collect()
}

fn order(coarse: f64, fine: f64) -> f64 {
    (coarse / fine).log2()
}

fn print_orders(levels: &[Level]) {
    for w in levels.windows(2) {
        println!(
            "  pair {} -> {}: L2 order {:.4}, energy order {:.4}",
            w[0].n,
            w[1].n,
            order(w[0].l2, w[1].l2),
            order(w[0].energy, w[1].energy)
        );
    }
}

fn monotone(checks: &mut Checks, name: &str, levels: &[Level]) {
    let mono_l2 = levels.windows(2).all(|w| w[1].l2 < w[0].l2);
    let mono_e = levels.windows(2).all(|w| w[1].energy < w[0].energy);
    let l2s: Vec<String> = levels.iter().map(|l| format!("{:.3e}", l.l2)).collect();
    let es: Vec<String> = levels.iter().map(|l| format!("{:.3e}", l.energy)).collect();
    checks.holds(name, mono_l2 && mono_e, &format!("L2 {l2s:?}, energy {es:?}"));
}

/// H2 (heavy record, ADR 0365): checks S1, R1, A1 to A3, T1. Run singly:
/// `cargo test -p rcfd-fem --release --test sb_mms_tet4_traction -- --ignored h2 --nocapture`.
#[test]
#[ignore = "heavy record (ADR 0365 H2): Tet4 case T up to 98 304 unknowns, direct solver, minutes in release"]
fn h2_tet4_case_t_order_16_to_32() {
    let mut checks = Checks::new();
    let t0 = Instant::now();
    let levels = run_levels(&LEVELS);
    print_orders(&levels);
    // S1: the free unknowns are 3 n^3 (1 536, 12 288, 98 304).
    for l in &levels {
        checks.holds(
            &format!("S1 free unknowns at n = {}", l.n),
            l.unknowns == 3 * l.n * l.n * l.n,
            &format!("{} (expected {})", l.unknowns, 3 * l.n * l.n * l.n),
        );
    }
    // R1: copy fidelity against the L2 errors printed in ADR 0279.
    for (n, reference) in [(8usize, 2.731e-2), (16, 7.883e-3)] {
        if let Some(l) = levels.iter().find(|l| l.n == n) {
            checks.at_most(
                &format!("R1 L2 error at n = {n} against ADR 0279 (relative difference)"),
                (l.l2 - reference).abs() / reference,
                5e-3,
            );
        }
    }
    let (a, b) = (&levels[levels.len() - 2], &levels[levels.len() - 1]);
    checks.at_least(
        &format!("A1 L2 order of the pair {} -> {}", a.n, b.n),
        order(a.l2, b.l2),
        1.8,
    );
    checks.at_least(
        &format!("A2 energy order of the pair {} -> {}", a.n, b.n),
        order(a.energy, b.energy),
        0.9,
    );
    monotone(&mut checks, "A3 monotone fall of both errors", &levels);
    checks.at_most("T1 total wall time, s (budget)", t0.elapsed().as_secs_f64(), 1800.0);
    checks.finish();
}

/// The same code path on 2, 4, 8 cells per edge (seconds): F1 to F3 of ADR 0365.
#[test]
fn f2_tet4_case_t_fast_pipeline() {
    let mut checks = Checks::new();
    let levels = run_levels(&FAST_LEVELS);
    print_orders(&levels);
    monotone(&mut checks, "F1 monotone fall of both errors", &levels);
    let (a, b) = (&levels[1], &levels[2]);
    checks.at_least("F2 L2 order of the pair 4 -> 8", order(a.l2, b.l2), 1.5);
    checks.at_least("F3 energy order of the pair 4 -> 8", order(a.energy, b.energy), 0.85);
    checks.finish();
}
