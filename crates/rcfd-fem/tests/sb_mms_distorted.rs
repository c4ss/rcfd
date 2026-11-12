//! F-459 (ADR 0355): the manufactured solution of ADR 0279, case D, on boxes whose interior nodes are moved by the seed
//! 0xF459_0355 and the amplitude `alpha h` (0.15 for Hex8, 0.075 for the tetrahedra). Checks J1, J2, X3, A1 to A3, N1 and
//! the heavy record H1 of the registration.
//!
//! The field, body force, error rules and error norms are copied from `sb_mms.rs` (ADR 0279), which is not edited.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10, BOX_FACES};
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

const SEED: u64 = 0xF459_0355;
const E_MOD: f64 = 1.0;
const NU: f64 = 0.3;
const AMPLITUDE: f64 = 1.0;

// Arithmetic 2: the amplitudes of the interior moves, relative to h.
const ALPHA_HEX: f64 = 0.15;
const ALPHA_TET: f64 = 0.075;
// Arithmetic 3: the guaranteed lower bound of det J relative to the regular element.
const J1_LIMIT: f64 = 1e-3;

// ADR 0279 arithmetic 1: wavenumbers in units of pi and phases, row = component, column = coordinate.
const K: [[f64; 3]; 3] = [[1.0, 0.5, 0.75], [0.75, 1.0, 0.5], [0.5, 0.75, 1.0]];
const PHI: [[f64; 3]; 3] = [[0.3, 0.7, 0.5], [0.9, 0.2, 0.6], [0.4, 0.8, 0.1]];

// Arithmetic 7: the fast (acceptance) sequences and the heavy family.
const FAST_LINEAR: [usize; 3] = [4, 8, 16];
const FAST_TET10: [usize; 3] = [2, 4, 8];
const HEAVY_LINEAR: [usize; 3] = [6, 12, 24];
const HEAVY_TET10: [usize; 3] = [3, 6, 12];

/// SplitMix64, a deterministic source only (as in sb_mms.rs).
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
            "{name}: {value:.4} (limit >= {limit}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value} < {limit}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value > limit;
        println!(
            "{name}: {value:.4} (limit > {limit}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value} <= {limit}"));
        }
    }
    fn below(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value < limit;
        println!(
            "{name}: {value:.4} (limit < {limit}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value} >= {limit}"));
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

/// The body force `b_i = -[(s lambda + mu) d_i div u + mu Laplace u_i]` (ADR 0279 arithmetic 3); `lambda_sign = -1` is the
/// negative control.
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

/// The manufactured field of ADR 0279 arithmetic 1.
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

/// `u_i = a_i + G_i . x + x^T Q_i x` (Q symmetric); X3 (the fields of ADR 0279's X3).
struct Poly {
    a: Vec3,
    g: [[f64; 3]; 3],
    q: [[[f64; 3]; 3]; 3],
}

impl Poly {
    fn linear() -> Poly {
        Poly {
            a: [0.1, -0.2, 0.3],
            g: [[0.5, 0.2, -0.1], [0.3, -0.4, 0.25], [-0.15, 0.35, 0.6]],
            q: [[[0.0; 3]; 3]; 3],
        }
    }
    fn quadratic() -> Poly {
        Poly {
            q: [
                [[0.2, 0.1, -0.05], [0.1, -0.3, 0.15], [-0.05, 0.15, 0.25]],
                [[-0.1, 0.2, 0.05], [0.2, 0.15, -0.1], [0.05, -0.1, 0.3]],
                [[0.3, -0.05, 0.1], [-0.05, 0.2, 0.2], [0.1, 0.2, -0.25]],
            ],
            ..Poly::linear()
        }
    }
}

impl Field for Poly {
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
    fn hess(&self, _x: Vec3) -> [[[f64; 3]; 3]; 3] {
        let mut h = self.q;
        for hi in h.iter_mut() {
            for row in hi.iter_mut() {
                for v in row.iter_mut() {
                    *v *= 2.0;
                }
            }
        }
        h
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Error rules (ADR 0279 D3, arithmetic 8)

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

/// The 125-point error rule of an element kind as (reference point, weight).
fn error_rule(kind: ElementKind) -> Vec<([f64; 3], f64)> {
    let (g, gw) = gauss_legendre(5);
    let mut out = Vec::with_capacity(125);
    match kind {
        ElementKind::Hex8 => {
            for k in 0..5 {
                for j in 0..5 {
                    for i in 0..5 {
                        out.push(([g[i], g[j], g[k]], gw[i] * gw[j] * gw[k]));
                    }
                }
            }
        }
        _ => {
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
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------------
// Moved meshes (D2)

/// A moved mesh with the regular one it came from.
struct Cube {
    mesh: SolidMesh,
    regular: SolidMesh,
    n: usize,
    alpha: f64,
    /// The largest offset component relative to h.
    max_offset: f64,
}

/// The builder mesh with its interior nodes (in no face node set) moved by `alpha h (2 r - 1)` per component, the uniforms
/// `r` drawn from `Rng::new(n)` in ascending node order (arithmetic 2); blocks and sets copied. Returns the moved mesh and
/// the largest offset component relative to h.
fn moved(mesh: &SolidMesh, alpha: f64, n: usize) -> (SolidMesh, f64) {
    let h = 1.0 / n as f64;
    let mut boundary = BTreeSet::new();
    for name in BOX_FACES {
        boundary.extend(mesh.node_set(name).unwrap().iter().copied());
    }
    let mut rng = Rng::new(n as u64);
    let mut nodes = mesh.nodes().to_vec();
    let mut max_offset = 0.0f64;
    for (v, x) in nodes.iter_mut().enumerate() {
        if boundary.contains(&v) {
            continue;
        }
        for xc in x.iter_mut() {
            let d = alpha * h * (2.0 * rng.uniform() - 1.0);
            *xc += d;
            max_offset = max_offset.max(d.abs() / h);
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
    (out, max_offset)
}

fn cube(kind: ElementKind, n: usize) -> Cube {
    let (o, l, d) = ([0.0; 3], [1.0; 3], [n; 3]);
    match kind {
        ElementKind::Hex8 => {
            let regular = hex_box(o, l, d).unwrap();
            let (mesh, max_offset) = moved(&regular, ALPHA_HEX, n);
            Cube {
                mesh,
                regular,
                n,
                alpha: ALPHA_HEX,
                max_offset,
            }
        }
        ElementKind::Tet4 => {
            let regular = tet_box(o, l, d).unwrap();
            let (mesh, max_offset) = moved(&regular, ALPHA_TET, n);
            Cube {
                mesh,
                regular,
                n,
                alpha: ALPHA_TET,
                max_offset,
            }
        }
        _ => {
            let base = tet_box(o, l, d).unwrap();
            let (moved4, max_offset) = moved(&base, ALPHA_TET, n);
            let (mesh, _) = upgrade_to_tet10(&moved4).unwrap();
            let (regular, _) = upgrade_to_tet10(&base).unwrap();
            Cube {
                mesh,
                regular,
                n,
                alpha: ALPHA_TET,
                max_offset,
            }
        }
    }
}

/// The regular `det J` of an element of the box with `n` cells per edge: `h^3/8` (Hex8, reference cube `[-1, 1]^3`), `h^3`
/// (the Kuhn tetrahedra on the unit reference tetrahedron).
fn regular_det(kind: ElementKind, n: usize) -> f64 {
    let h = 1.0 / n as f64;
    match kind {
        ElementKind::Hex8 => h * h * h / 8.0,
        _ => h * h * h,
    }
}

/// J1: the minimum over elements and nodes of `det J / det J_regular`.
fn min_jacobian_ratio(c: &Cube) -> f64 {
    let mut min = f64::INFINITY;
    for e in 0..c.mesh.n_elements() {
        let r = regular_det(c.mesh.element_kind(e), c.n);
        for j in c.mesh.jacobians_at_nodes(e) {
            min = min.min(j / r);
        }
    }
    min
}

/// J2: Hex8, the minimum of `det J` over the 125 error points of every element, relative to `h^3/8`.
fn min_error_point_jacobian_ratio(c: &Cube) -> f64 {
    let rule = error_rule(ElementKind::Hex8);
    let element = element_for(ElementKind::Hex8);
    let r = regular_det(ElementKind::Hex8, c.n);
    let mut min = f64::INFINITY;
    for e in 0..c.mesh.n_elements() {
        let coords: Vec<Vec3> = c.mesh.element_nodes(e).iter().map(|&a| c.mesh.nodes()[a]).collect();
        for &(xi, _) in &rule {
            // `point` refuses det J <= 0; a refusal is recorded as 0 (a miss of J2).
            let det = element.point(&coords, xi).map(|p| p.det_j).unwrap_or(0.0);
            min = min.min(det / r);
        }
    }
    min
}

// ---------------------------------------------------------------------------------------------------------------------
// Model, loads, solve, errors (ADR 0279 D2, D3; case D only)

fn model_of(mesh: SolidMesh) -> SolidModel {
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", Material::isotropic_elastic(E_MOD, NU, 0.0, 0.0).unwrap())
        .unwrap();
    model
}

/// Case D: every node of the six face node sets prescribed at the exact field (nodal interpolation), the body force by
/// `body_force_loads`.
fn solve_case_d(field: &dyn Field, c: &Cube, lambda_sign: f64) -> (SolidModel, SolidResult) {
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
    let forces = body_force_loads(&model, |x| body_force(field, x, lambda_sign)).unwrap();
    let result = solve_static_with(&model, &prescribed, &forces, &StaticOptions::default()).unwrap();
    (model, result)
}

/// Errors (L2, energy) and the norms of the exact field (L2, energy) with the error rule. The exact field is evaluated at
/// the point `sum_a N_a x_a` with the node coordinates of `at` (the model's mesh; mutation M3 passes the regular mesh).
fn errors(field: &dyn Field, model: &SolidModel, result: &SolidResult, at: &SolidMesh) -> [f64; 4] {
    let mesh = model.mesh();
    let d = d_matrix();
    let tet_rule = error_rule(ElementKind::Tet4);
    let hex_rule = error_rule(ElementKind::Hex8);
    let mut acc = [0.0; 4];
    for e in 0..mesh.n_elements() {
        let kind = mesh.element_kind(e);
        let element = element_for(kind);
        let nodes = mesh.element_nodes(e);
        let coords: Vec<Vec3> = nodes.iter().map(|&a| mesh.nodes()[a]).collect();
        let at_coords: Vec<Vec3> = nodes.iter().map(|&a| at.nodes()[a]).collect();
        let ue: Vec<&[f64]> = nodes.iter().map(|&a| result.displacement(a)).collect();
        let rule = if kind == ElementKind::Hex8 {
            &hex_rule
        } else {
            &tet_rule
        };
        for &(xi, wq) in rule {
            let p = element.point(&coords, xi).unwrap();
            let w = wq * p.det_j;
            let mut uh = [0.0; 3];
            let mut gh = [[0.0; 3]; 3];
            let mut x = [0.0; 3];
            for (a, ua) in ue.iter().enumerate() {
                for i in 0..3 {
                    x[i] += p.n[a] * at_coords[a][i];
                    uh[i] += p.n[a] * ua[i];
                    for m in 0..3 {
                        gh[i][m] += ua[i] * p.grad[a][m];
                    }
                }
            }
            let u = field.u(x);
            let g = field.grad(x);
            let eps = voigt_strain(&g);
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
    acc.map(f64::sqrt)
}

struct Level {
    n: usize,
    unknowns: usize,
    l2: f64,
    energy: f64,
    seconds: f64,
}

fn run_sequence(field: &dyn Field, kind: ElementKind, ns: &[usize], lambda_sign: f64) -> Vec<Level> {
    ns.iter()
        .map(|&n| {
            let t0 = Instant::now();
            let c = cube(kind, n);
            let (model, result) = solve_case_d(field, &c, lambda_sign);
            let [l2, energy, norm_l2, norm_energy] = errors(field, &model, &result, model.mesh());
            let seconds = t0.elapsed().as_secs_f64();
            let unknowns = result.stats().map(|s| s.unknowns()).unwrap_or(0);
            println!(
                "  {} moved (alpha {}) sign {lambda_sign:+} n = {n:2}: free unknowns {unknowns}, e_L2 {l2:.4e} \
                 (rel {:.3e}), e_E {energy:.4e} (rel {:.3e}), {seconds:.2} s",
                kind.name(),
                c.alpha,
                l2 / norm_l2,
                energy / norm_energy
            );
            Level {
                n,
                unknowns,
                l2,
                energy,
                seconds,
            }
        })
        .collect()
}

fn order(coarse: f64, fine: f64) -> f64 {
    (coarse / fine).log2()
}

fn orders(levels: &[Level]) -> (f64, f64) {
    let (a, b) = (&levels[levels.len() - 2], &levels[levels.len() - 1]);
    assert_eq!(b.n, 2 * a.n, "each level halves h");
    (order(a.l2, b.l2), order(a.energy, b.energy))
}

fn limits(kind: ElementKind) -> (f64, f64) {
    match kind {
        ElementKind::Tet10 => (2.7, 1.8),
        _ => (1.8, 0.9),
    }
}

/// A1 to A3 of one element on one sequence.
fn check_orders(checks: &mut Checks, tag: &str, kind: ElementKind, levels: &[Level]) {
    let (l2_limit, e_limit) = limits(kind);
    let (p_l2, p_e) = orders(levels);
    for w in levels.windows(2) {
        println!(
            "  {tag} {} pair {} -> {}: L2 order {:.4}, energy order {:.4}",
            kind.name(),
            w[0].n,
            w[1].n,
            order(w[0].l2, w[1].l2),
            order(w[0].energy, w[1].energy)
        );
    }
    let name = kind.name();
    checks.at_least(&format!("A1-{tag} {name} L2 order (finest pair)"), p_l2, l2_limit);
    checks.at_least(&format!("A2-{tag} {name} energy order (finest pair)"), p_e, e_limit);
    let mono_l2 = levels.windows(2).all(|w| w[1].l2 < w[0].l2);
    let mono_e = levels.windows(2).all(|w| w[1].energy < w[0].energy);
    let l2s: Vec<String> = levels.iter().map(|l| format!("{:.3e}", l.l2)).collect();
    let es: Vec<String> = levels.iter().map(|l| format!("{:.3e}", l.energy)).collect();
    checks.holds(
        &format!("A3-{tag} {name} monotone fall"),
        mono_l2 && mono_e,
        &format!("L2 {l2s:?}, energy {es:?}"),
    );
    let total: f64 = levels.iter().map(|l| l.seconds).sum();
    let max_unknowns = levels.iter().map(|l| l.unknowns).max().unwrap_or(0);
    println!("  {tag} {name}: wall time {total:.1} s, largest solve {max_unknowns} free unknowns");
}

/// J1 (and J2 for Hex8) of one mesh.
fn check_mesh(checks: &mut Checks, tag: &str, kind: ElementKind, n: usize) {
    let c = cube(kind, n);
    let name = kind.name();
    let h = 1.0 / n as f64;
    let moved_nodes = c.mesh.nodes().len();
    let sq: f64 = c
        .mesh
        .nodes()
        .iter()
        .zip(c.regular.nodes())
        .map(|(a, b)| (0..3).map(|i| (a[i] - b[i]) * (a[i] - b[i])).sum::<f64>())
        .sum();
    println!(
        "  {tag} {name} n = {n}: {moved_nodes} nodes, rms node move / h {:.4} (all nodes, mid-edge included)",
        (sq / moved_nodes as f64).sqrt() / h
    );
    checks.at_least(
        &format!("J1-{tag} {name} n = {n}: min det J / regular, every node"),
        min_jacobian_ratio(&c),
        J1_LIMIT,
    );
    checks.at_most(
        &format!("J1-{tag} {name} n = {n}: largest offset / h"),
        c.max_offset,
        c.alpha,
    );
    if kind == ElementKind::Hex8 {
        checks.above(
            &format!("J2-{tag} {name} n = {n}: min det J at the error points / regular"),
            min_error_point_jacobian_ratio(&c),
            0.0,
        );
    }
}

fn element_orders(kind: ElementKind, ns: &[usize]) {
    let field = Trig;
    let mut checks = Checks::new();
    let t0 = Instant::now();
    let d = run_sequence(&field, kind, ns, 1.0);
    check_orders(&mut checks, "D", kind, &d);
    // N1: the body force with the lambda term reversed, case D, the same moved meshes.
    let neg = run_sequence(&field, kind, ns, -1.0);
    let (p_l2, p_e) = orders(&neg);
    checks.below(&format!("N1 {} L2 order, lambda term reversed", kind.name()), p_l2, 0.5);
    checks.below(
        &format!("N1 {} energy order, lambda term reversed", kind.name()),
        p_e,
        0.5,
    );
    println!("{}: total wall time {:.1} s", kind.name(), t0.elapsed().as_secs_f64());
    checks.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// Tests (D3)

#[test]
fn j1_moved_meshes_valid_at_every_node() {
    let mut checks = Checks::new();
    for (kind, fast, heavy) in [
        (ElementKind::Tet4, FAST_LINEAR, HEAVY_LINEAR),
        (ElementKind::Hex8, FAST_LINEAR, HEAVY_LINEAR),
        (ElementKind::Tet10, FAST_TET10, HEAVY_TET10),
    ] {
        for n in fast {
            check_mesh(&mut checks, "fast", kind, n);
        }
        for n in heavy {
            check_mesh(&mut checks, "heavy", kind, n);
        }
    }
    checks.finish();
}

#[test]
fn x3_polynomial_fields_reproduced_on_moved_meshes() {
    let mut checks = Checks::new();
    for (kind, ns) in [
        (ElementKind::Tet4, &[4][..]),
        (ElementKind::Hex8, &[4][..]),
        (ElementKind::Tet10, &[2, 4][..]),
    ] {
        let field = if kind == ElementKind::Tet10 {
            Poly::quadratic()
        } else {
            Poly::linear()
        };
        for &n in ns {
            let c = cube(kind, n);
            let (model, result) = solve_case_d(&field, &c, 1.0);
            let [l2, energy, nl2, ne] = errors(&field, &model, &result, model.mesh());
            checks.at_most(
                &format!("X3 {} n = {n} moved, relative L2 error", kind.name()),
                l2 / nl2,
                1e-9,
            );
            checks.at_most(
                &format!("X3 {} n = {n} moved, relative energy error", kind.name()),
                energy / ne,
                1e-9,
            );
        }
    }
    checks.finish();
}

#[test]
fn a_tet4_orders_on_moved_meshes_and_negative_control() {
    element_orders(ElementKind::Tet4, &FAST_LINEAR);
}

#[test]
fn a_hex8_orders_on_moved_meshes_and_negative_control() {
    element_orders(ElementKind::Hex8, &FAST_LINEAR);
}

#[test]
fn a_tet10_orders_on_moved_meshes_and_negative_control() {
    element_orders(ElementKind::Tet10, &FAST_TET10);
}

/// H1 (heavy record): the finer family on moved meshes, case D, the limits of A1 to A3 (J1 of its meshes is in
/// `j1_moved_meshes_valid_at_every_node`). Run singly:
/// `cargo test -p rcfd-fem --release --test sb_mms_distorted -- --ignored h1 --nocapture`.
#[test]
#[ignore = "heavy record (ADR 0355 H1): solves of up to 36 501 unknowns, minutes in release"]
fn h1_heavy_finer_family_on_moved_meshes() {
    let field = Trig;
    let mut checks = Checks::new();
    let t0 = Instant::now();
    for (kind, ns) in [
        (ElementKind::Tet4, HEAVY_LINEAR),
        (ElementKind::Hex8, HEAVY_LINEAR),
        (ElementKind::Tet10, HEAVY_TET10),
    ] {
        let levels = run_sequence(&field, kind, &ns, 1.0);
        check_orders(&mut checks, "H1", kind, &levels);
    }
    let total = t0.elapsed().as_secs_f64();
    checks.at_most("H1 total wall time, s (budget)", total, 1800.0);
    checks.finish();
}
