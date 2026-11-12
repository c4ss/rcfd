//! F-461 (ADR 0358): the constant-strain patch test of Hex8 on a box whose interior nodes are moved by the seed
//! 0xF461_0358. Checks V1, V2, P1, P2 and N1 of the registration. The one-point stiffness rule (check M1) is a library
//! mutation run on a scratch copy; this file only measures and prints.
//!
//! The node-moving code, the field type, the error rule and the error norms are copied from `sb_mms_distorted.rs`
//! (ADR 0355), which is not edited.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, BOX_FACES};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::element::element_for;
use rcfd_fem::loads::body_force_loads;
use rcfd_fem::material::{isotropic_elasticity, Matrix6};
use rcfd_fem::mesh::{ElementKind, SolidMesh};
use rcfd_fem::solve::{solve_static_with, StaticOptions};
use rcfd_fem::{Material, SolidModel, SolidResult};
use std::collections::BTreeSet;
use std::f64::consts::PI;
use std::time::Instant;

const SEED: u64 = 0xF461_0358;
const E_MOD: f64 = 1.0;
const NU: f64 = 0.3;

// D2: the registered configurations.
const NS: [usize; 3] = [3, 4, 6];
const ALPHAS: [f64; 2] = [0.15, 0.05];
const DRAWS: [u64; 3] = [0, 1, 2];

// Limits (checks table of the registration).
const PATCH_LIMIT: f64 = 1e-9;
const MOVE_FRACTION: f64 = 0.3;
const QUADRATIC_FLOOR: f64 = 1e-3;

/// SplitMix64, a deterministic source only (as in sb_mms_distorted.rs).
struct Rng(u64);

impl Rng {
    fn new(tag: u64) -> Rng {
        Rng(SEED.wrapping_add(tag))
    }
    fn uniform(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
}

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
            "{name}: {value:.4e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} < {limit:e}"));
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

/// `u_i = a_i + G_i . x + x^T Q_i x` (Q symmetric) with its gradient `grad[i][m] = d_m u_i` and body force.
struct Poly {
    a: Vec3,
    g: [[f64; 3]; 3],
    q: [[[f64; 3]; 3]; 3],
}

impl Poly {
    /// Field A: the linear field of ADR 0355's X3.
    fn linear_a() -> Poly {
        Poly {
            a: [0.1, -0.2, 0.3],
            g: [[0.5, 0.2, -0.1], [0.3, -0.4, 0.25], [-0.15, 0.35, 0.6]],
            q: [[[0.0; 3]; 3]; 3],
        }
    }
    /// Field B: a different constant strain state.
    fn linear_b() -> Poly {
        Poly {
            a: [-0.05, 0.4, 0.1],
            g: [[-0.3, 0.6, 0.2], [0.1, 0.45, -0.5], [0.7, -0.25, 0.15]],
            q: [[[0.0; 3]; 3]; 3],
        }
    }
    /// The quadratic field of ADR 0355's X3 (the negative control N1).
    fn quadratic() -> Poly {
        Poly {
            q: [
                [[0.2, 0.1, -0.05], [0.1, -0.3, 0.15], [-0.05, 0.15, 0.25]],
                [[-0.1, 0.2, 0.05], [0.2, 0.15, -0.1], [0.05, -0.1, 0.3]],
                [[0.3, -0.05, 0.1], [-0.05, 0.2, 0.2], [0.1, 0.2, -0.25]],
            ],
            ..Poly::linear_a()
        }
    }
    fn u(&self, x: Vec3) -> Vec3 {
        [0, 1, 2].map(|i| {
            let mut v = self.a[i];
            for m in 0..3 {
                v += self.g[i][m] * x[m];
                for n in 0..3 {
                    v += self.q[i][m][n] * x[m] * x[n];
                }
            }
            v
        })
    }
    fn grad(&self, x: Vec3) -> [[f64; 3]; 3] {
        let mut g = self.g;
        for (i, gi) in g.iter_mut().enumerate() {
            for (m, gim) in gi.iter_mut().enumerate() {
                for n in 0..3 {
                    *gim += 2.0 * self.q[i][m][n] * x[n];
                }
            }
        }
        g
    }
    /// The body force `b_i = -[(lambda + mu) d_i div u + mu Laplace u_i]`; exactly zero for a linear field.
    fn body_force(&self) -> Vec3 {
        let (lambda, mu) = lame(NU);
        let mut b = [0.0; 3];
        for (i, bi) in b.iter_mut().enumerate() {
            let grad_div: f64 = (0..3).map(|k| 2.0 * self.q[k][i][k]).sum();
            let lap: f64 = (0..3).map(|j| 2.0 * self.q[i][j][j]).sum();
            *bi = -((lambda + mu) * grad_div + mu * lap);
        }
        b
    }
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

/// Gauss-Legendre nodes and weights on [-1, 1] by Newton's method on the three-term recurrence of `P_n`.
fn gauss_legendre(n: usize) -> (Vec<f64>, Vec<f64>) {
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

/// The 125-point error rule of the Hex8 as (reference point, weight).
fn error_rule() -> Vec<([f64; 3], f64)> {
    let (g, gw) = gauss_legendre(5);
    let mut out = Vec::with_capacity(125);
    for k in 0..5 {
        for j in 0..5 {
            for i in 0..5 {
                out.push(([g[i], g[j], g[k]], gw[i] * gw[j] * gw[k]));
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------------
// Moved meshes (D2)

/// A moved box with the regular one it came from.
struct Cube {
    mesh: SolidMesh,
    regular: SolidMesh,
    n: usize,
    alpha: f64,
    /// The largest offset component relative to h.
    max_offset: f64,
    /// The rms of the offset components of the interior nodes relative to h.
    rms_offset: f64,
}

/// The builder mesh with its interior nodes (in no face node set) moved by `alpha h (2 r - 1)` per component, the uniforms
/// `r` drawn from `Rng::new(tag)` in ascending node order; blocks and sets copied. Returns the moved mesh, the largest offset
/// component relative to h and the rms of the offset components of the moved nodes relative to h.
fn moved(mesh: &SolidMesh, alpha: f64, n: usize, tag: u64) -> (SolidMesh, f64, f64) {
    let h = 1.0 / n as f64;
    let mut boundary = BTreeSet::new();
    for name in BOX_FACES {
        boundary.extend(mesh.node_set(name).unwrap().iter().copied());
    }
    let mut rng = Rng::new(tag);
    let mut nodes = mesh.nodes().to_vec();
    let mut max_offset = 0.0f64;
    let (mut sq, mut count) = (0.0f64, 0usize);
    for (v, x) in nodes.iter_mut().enumerate() {
        if boundary.contains(&v) {
            continue;
        }
        for xc in x.iter_mut() {
            let d = alpha * h * (2.0 * rng.uniform() - 1.0);
            *xc += d;
            max_offset = max_offset.max(d.abs() / h);
            sq += (d / h) * (d / h);
            count += 1;
        }
    }
    let mut out = SolidMesh::new(nodes).unwrap();
    for b in mesh.blocks() {
        out.add_block(b.kind(), b.connectivity().to_vec()).unwrap();
    }
    for name in mesh.node_set_names() {
        out.add_node_set(name, mesh.node_set(name).unwrap().to_vec()).unwrap();
    }
    for name in mesh.element_set_names() {
        out.add_element_set(name, mesh.element_set(name).unwrap().to_vec())
            .unwrap();
    }
    for name in mesh.surface_set_names() {
        out.add_surface_set(name, mesh.surface_set(name).unwrap().to_vec())
            .unwrap();
    }
    (out, max_offset, (sq / count as f64).sqrt())
}

fn cube(n: usize, alpha: f64, draw: u64) -> Cube {
    let regular = hex_box([0.0; 3], [1.0; 3], [n; 3]).unwrap();
    let (mesh, max_offset, rms_offset) = moved(&regular, alpha, n, 1000 * draw + n as u64);
    Cube {
        mesh,
        regular,
        n,
        alpha,
        max_offset,
        rms_offset,
    }
}

/// V1: the minimum over elements and nodes of `det J / (h^3/8)`.
fn min_jacobian_ratio(c: &Cube) -> f64 {
    let regular_det = (1.0 / c.n as f64).powi(3) / 8.0;
    let mut min = f64::INFINITY;
    for e in 0..c.mesh.n_elements() {
        for j in c.mesh.jacobians_at_nodes(e) {
            min = min.min(j / regular_det);
        }
    }
    min
}

// ---------------------------------------------------------------------------------------------------------------------
// Model, solve, errors

fn model_of(mesh: SolidMesh) -> SolidModel {
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", Material::isotropic_elastic(E_MOD, NU, 0.0, 0.0).unwrap())
        .unwrap();
    model
}

/// Every node of the six face node sets prescribed at the exact field, the body force of the field (zero for a linear one)
/// by `body_force_loads`.
fn solve_case_d(field: &Poly, c: &Cube) -> (SolidModel, SolidResult) {
    let model = model_of(c.mesh.clone());
    let mesh = model.mesh();
    let mut nodes = BTreeSet::new();
    for name in BOX_FACES {
        nodes.extend(mesh.node_set(name).unwrap().iter().copied());
    }
    let mut prescribed = Vec::new();
    for node in nodes {
        let v = field.u(mesh.nodes()[node]);
        for (k, &vk) in v.iter().enumerate() {
            prescribed.push(Prescribed::new(node, k, vk));
        }
    }
    let b = field.body_force();
    let forces = body_force_loads(&model, |_| b).unwrap();
    let result = solve_static_with(&model, &prescribed, &forces, &StaticOptions::default()).unwrap();
    (model, result)
}

/// The relative L2 and energy errors of a solution against the exact field (125-point rule, the exact field evaluated at the
/// physical point of the element).
fn relative_errors(field: &Poly, model: &SolidModel, result: &SolidResult) -> (f64, f64) {
    let mesh = model.mesh();
    let d = d_matrix();
    let rule = error_rule();
    let element = element_for(ElementKind::Hex8);
    let mut acc = [0.0; 4];
    for e in 0..mesh.n_elements() {
        let nodes = mesh.element_nodes(e);
        let coords: Vec<Vec3> = nodes.iter().map(|&a| mesh.nodes()[a]).collect();
        let ue: Vec<&[f64]> = nodes.iter().map(|&a| result.displacement(a)).collect();
        for &(xi, wq) in &rule {
            let p = element.point(&coords, xi).unwrap();
            let w = wq * p.det_j;
            let mut uh = [0.0; 3];
            let mut gh = [[0.0; 3]; 3];
            let mut x = [0.0; 3];
            for (a, ua) in ue.iter().enumerate() {
                for i in 0..3 {
                    x[i] += p.n[a] * coords[a][i];
                    uh[i] += p.n[a] * ua[i];
                    for m in 0..3 {
                        gh[i][m] += ua[i] * p.grad[a][m];
                    }
                }
            }
            let u = field.u(x);
            let eps = voigt_strain(&field.grad(x));
            let eps_h = voigt_strain(&gh);
            let de: [f64; 6] = std::array::from_fn(|r| eps[r] - eps_h[r]);
            let energy = |v: &[f64; 6]| -> f64 {
                let mut s = 0.0;
                for r in 0..6 {
                    for c in 0..6 {
                        s += v[r] * d[r][c] * v[c];
                    }
                }
                s
            };
            let du2: f64 = (0..3).map(|i| (u[i] - uh[i]) * (u[i] - uh[i])).sum();
            let u2: f64 = u.iter().map(|v| v * v).sum();
            acc[0] += w * du2;
            acc[1] += w * energy(&de);
            acc[2] += w * u2;
            acc[3] += w * energy(&eps);
        }
    }
    let [e_l2, e_en, n_l2, n_en] = acc.map(f64::sqrt);
    (e_l2 / n_l2, e_en / n_en)
}

/// The 18 registered meshes as (cells per edge, amplitude, draw).
fn configs() -> Vec<(usize, f64, u64)> {
    let mut out = Vec::new();
    for &draw in &DRAWS {
        for &alpha in &ALPHAS {
            for &n in &NS {
                out.push((n, alpha, draw));
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------------
// Tests (D3)

#[test]
fn v1_moved_boxes_valid_and_really_moved() {
    let mut checks = Checks::new();
    for (n, alpha, draw) in configs() {
        let c = cube(n, alpha, draw);
        let tag = format!("n = {n} alpha = {alpha} draw {draw}");
        let bound = (1.0 - 6.0 * alpha).powi(3);
        checks.at_least(
            &format!("V1 {tag}: min det J / regular, every node"),
            min_jacobian_ratio(&c),
            bound,
        );
        checks.at_most(&format!("V1 {tag}: largest offset / h"), c.max_offset, c.alpha);
        checks.at_least(
            &format!("V2 {tag}: rms interior offset component / h"),
            c.rms_offset,
            MOVE_FRACTION * alpha,
        );
        assert_eq!(c.mesh.nodes().len(), c.regular.nodes().len());
    }
    checks.finish();
}

#[test]
fn p1_constant_strain_patch_test_on_moved_hex8() {
    let mut checks = Checks::new();
    let t0 = Instant::now();
    let (mut max_l2, mut max_en, mut min_en) = (0.0f64, 0.0f64, f64::INFINITY);
    for (n, alpha, draw) in configs() {
        let c = cube(n, alpha, draw);
        for (name, field) in [("A", Poly::linear_a()), ("B", Poly::linear_b())] {
            let (model, result) = solve_case_d(&field, &c);
            let (l2, en) = relative_errors(&field, &model, &result);
            let tag = format!("n = {n} alpha = {alpha} draw {draw} field {name}");
            checks.at_most(&format!("P1 {tag}: relative L2 error"), l2, PATCH_LIMIT);
            checks.at_most(&format!("P2 {tag}: relative energy error"), en, PATCH_LIMIT);
            max_l2 = max_l2.max(l2);
            max_en = max_en.max(en);
            min_en = min_en.min(en);
        }
    }
    println!(
        "P summary over 36 solves: max relative L2 {max_l2:.3e}, relative energy min {min_en:.3e} max {max_en:.3e}, \
         {:.1} s",
        t0.elapsed().as_secs_f64()
    );
    checks.finish();
}

#[test]
fn n1_quadratic_field_not_reproduced_on_moved_hex8() {
    let mut checks = Checks::new();
    let field = Poly::quadratic();
    for (n, alpha, draw) in configs() {
        let c = cube(n, alpha, draw);
        let (model, result) = solve_case_d(&field, &c);
        let (l2, en) = relative_errors(&field, &model, &result);
        let tag = format!("n = {n} alpha = {alpha} draw {draw}");
        checks.at_least(&format!("N1 {tag}: relative L2 error"), l2, QUADRATIC_FLOOR);
        checks.at_least(&format!("N1 {tag}: relative energy error"), en, QUADRATIC_FLOOR);
    }
    checks.finish();
}
