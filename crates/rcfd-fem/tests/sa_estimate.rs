//! F-432 (ADR 0478): stress recovery by element patches and an energy-norm error estimate. Checks L1, L2, M1 to M5, F1, P1,
//! P2, B1, T1, S1 to S3, R1 and N1 of the registration (D1 is in `sa_estimate_default_bits.rs`, X1 in `sa_scaffold.rs`); the
//! Tet10 and Hex20 sequences of M1, M2 and M5 are heavy records (ignored, run once).
//!
//! The manufactured field, the body force, the error rules and case D are those of F-378 (`sb_mms.rs`, ADR 0279); the plate
//! with a hole is the plate of ADR 0449 (`sa_fatigue.rs`) with two layers through the thickness (ADR 0478 reading a6).

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10, BOX_FACES};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::element::element_for;
use rcfd_fem::estimate::{self, energy_error, recover, PatchBasis, PatchOptions, Recovery, ShortPatch};
use rcfd_fem::hex20::{faces, upgrade_to_hex20, upgrade_to_hex20_mapped};
use rcfd_fem::loads::body_force_loads;
use rcfd_fem::material::{isotropic_elasticity, Matrix6};
use rcfd_fem::mesh::{ElementKind, SolidMesh};
use rcfd_fem::recover::{nodal_average, tensor_norm, IntegrationPointField};
use rcfd_fem::solve::{solve_static, solve_static_with, StaticOptions, StaticSolver};
use rcfd_fem::{BeamSection, BlockProperty, Constraint, FemError, Load, Material, SolidModel, SolidResult};
use std::collections::BTreeSet;
use std::f64::consts::PI;
use std::time::Instant;

const SEED: u64 = 0xF432_0478;
const E_MOD: f64 = 1.0;
const NU: f64 = 0.3;

/// SplitMix64, a deterministic source only.
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
        println!("{name}: {value:.4e} (limit <= {limit:e}) {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!("{name}: {value:.4} (limit >= {limit}) {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name} = {value} < {limit}"));
        }
    }
    fn within(&mut self, name: &str, value: f64, lo: f64, hi: f64) {
        let ok = (lo..=hi).contains(&value);
        println!("{name}: {value:.4} (limit in [{lo}, {hi}]) {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name} = {value} not in [{lo}, {hi}]"));
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

// ---------------------------------------------------------------------------------------------------------------------
// Fields (sb_mms.rs arithmetic 1 and X3)

fn lame(nu: f64) -> (f64, f64) {
    (E_MOD * nu / ((1.0 + nu) * (1.0 - 2.0 * nu)), E_MOD / (2.0 * (1.0 + nu)))
}

fn d_matrix() -> Matrix6 {
    isotropic_elasticity(E_MOD, NU)
}

type Mat3 = [[f64; 3]; 3];

trait Field {
    fn u(&self, x: Vec3) -> Vec3;
    fn grad(&self, x: Vec3) -> Mat3;
    fn hess(&self, x: Vec3) -> [Mat3; 3];
}

fn voigt_strain(g: &Mat3) -> [f64; 6] {
    [
        g[0][0],
        g[1][1],
        g[2][2],
        g[0][1] + g[1][0],
        g[1][2] + g[2][1],
        g[2][0] + g[0][2],
    ]
}

fn apply(d: &Matrix6, v: &[f64; 6]) -> [f64; 6] {
    std::array::from_fn(|r| (0..6).map(|c| d[r][c] * v[c]).sum())
}

fn energy(d: &Matrix6, v: &[f64; 6]) -> f64 {
    let mut s = 0.0;
    for r in 0..6 {
        for c in 0..6 {
            s += v[r] * d[r][c] * v[c];
        }
    }
    s
}

fn exact_stress(f: &dyn Field, x: Vec3) -> [f64; 6] {
    apply(&d_matrix(), &voigt_strain(&f.grad(x)))
}

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

const K: Mat3 = [[1.0, 0.5, 0.75], [0.75, 1.0, 0.5], [0.5, 0.75, 1.0]];
const PHI: Mat3 = [[0.3, 0.7, 0.5], [0.9, 0.2, 0.6], [0.4, 0.8, 0.1]];

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
        [0, 1, 2].map(|i| s[i][0] * s[i][1] * s[i][2])
    }
    fn grad(&self, x: Vec3) -> Mat3 {
        let (s, c, k) = Trig::parts(x);
        let mut g = [[0.0; 3]; 3];
        for i in 0..3 {
            for m in 0..3 {
                let mut p = k[i][m] * c[i][m];
                for j in (0..3).filter(|&j| j != m) {
                    p *= s[i][j];
                }
                g[i][m] = p;
            }
        }
        g
    }
    fn hess(&self, x: Vec3) -> [Mat3; 3] {
        let (s, c, k) = Trig::parts(x);
        let mut h = [[[0.0; 3]; 3]; 3];
        for i in 0..3 {
            for m in 0..3 {
                for n in 0..3 {
                    let mut p = 1.0;
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

/// `u_i = a_i + G_i . x + x^T Q_i x` (sb_mms.rs X3).
struct Poly {
    a: Vec3,
    g: Mat3,
    q: [Mat3; 3],
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
    fn grad(&self, x: Vec3) -> Mat3 {
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
    fn hess(&self, _x: Vec3) -> [Mat3; 3] {
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
// Error rules (sb_mms.rs D3)

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

fn error_rule(hex: bool) -> Vec<([f64; 3], f64)> {
    let (g, gw) = gauss_legendre(5);
    let mut out = Vec::with_capacity(125);
    if hex {
        for k in 0..5 {
            for j in 0..5 {
                for i in 0..5 {
                    out.push(([g[i], g[j], g[k]], gw[i] * gw[j] * gw[k]));
                }
            }
        }
    } else {
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
    out
}

// ---------------------------------------------------------------------------------------------------------------------
// Meshes and solves

fn cube(kind: ElementKind, n: usize) -> SolidMesh {
    let (o, l, d) = ([0.0; 3], [1.0; 3], [n; 3]);
    match kind {
        ElementKind::Hex8 => hex_box(o, l, d).unwrap(),
        ElementKind::Tet4 => tet_box(o, l, d).unwrap(),
        ElementKind::Tet10 => upgrade_to_tet10(&tet_box(o, l, d).unwrap()).unwrap().0,
        ElementKind::Hex20 => upgrade_to_hex20(&hex_box(o, l, d).unwrap()).unwrap().0,
        _ => unreachable!(),
    }
}

fn model_of(mesh: SolidMesh) -> SolidModel {
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", Material::isotropic_elastic(E_MOD, NU, 0.0, 0.0).unwrap())
        .unwrap();
    model
}

/// Case D of F-378: every face prescribed with the nodal values of the field, the body force of the field.
fn solve_case_d(field: &dyn Field, mesh: SolidMesh, lambda_sign: f64, solver: StaticSolver) -> (SolidModel, SolidResult) {
    let model = model_of(mesh);
    let mesh = model.mesh();
    let mut fixed = BTreeSet::new();
    for name in BOX_FACES {
        fixed.extend(mesh.node_set(name).unwrap().iter().copied());
    }
    let mut prescribed = Vec::new();
    for node in fixed {
        for (k, &v) in field.u(mesh.nodes()[node]).iter().enumerate() {
            prescribed.push(Prescribed::new(node, k, v));
        }
    }
    let forces = body_force_loads(&model, |x| body_force(field, x, lambda_sign)).unwrap();
    let result = solve_static_with(&model, &prescribed, &forces, &StaticOptions::with_solver(solver)).unwrap();
    (model, result)
}

/// The nodal values of a field as a displacement vector.
fn displacement_of(model: &SolidModel, field: &dyn Field) -> Vec<f64> {
    let dofs = model.dof_map();
    let mut u = vec![0.0; dofs.n_equations()];
    for (n, &x) in model.mesh().nodes().iter().enumerate() {
        for (c, v) in field.u(x).iter().enumerate() {
            u[dofs.equation(n, c).unwrap()] = *v;
        }
    }
    u
}

struct Measured {
    n: usize,
    theta: f64,
    eta: f64,
    true_energy: f64,
    raw_l2: f64,
    rec_l2: f64,
}

/// True energy error, raw and recovered stress L2 errors with the 125-point rules, and the estimate.
fn measure(field: &dyn Field, model: &SolidModel, result: &SolidResult, n: usize, threads: Option<usize>) -> Measured {
    let mesh = model.mesh();
    let d = d_matrix();
    let (rec, est) = energy_error(model, result.displacement_values(), &PatchOptions::new().threads(threads)).unwrap();
    let tet_rule = error_rule(false);
    let hex_rule = error_rule(true);
    let (mut en, mut raw, mut recl2) = (0.0, 0.0, 0.0);
    for e in 0..mesh.n_elements() {
        let kind = mesh.element_kind(e);
        let element = element_for(kind);
        let nodes = mesh.element_nodes(e);
        let coords: Vec<Vec3> = nodes.iter().map(|&a| mesh.nodes()[a]).collect();
        let rule = if matches!(kind, ElementKind::Hex8 | ElementKind::Hex20) {
            &hex_rule
        } else {
            &tet_rule
        };
        for &(xi, wq) in rule {
            let p = element.point(&coords, xi).unwrap();
            let w = wq * p.det_j;
            let mut gh = [[0.0; 3]; 3];
            for (a, &node) in nodes.iter().enumerate() {
                let ua = result.displacement(node);
                for i in 0..3 {
                    for m in 0..3 {
                        gh[i][m] += ua[i] * p.grad[a][m];
                    }
                }
            }
            let eps = voigt_strain(&field.grad(p.x));
            let eps_h = voigt_strain(&gh);
            let de: [f64; 6] = std::array::from_fn(|r| eps[r] - eps_h[r]);
            let s = apply(&d, &eps);
            let sh = apply(&d, &eps_h);
            let ss = rec.stress_at(model, e, xi).unwrap();
            let dr: [f64; 6] = std::array::from_fn(|r| s[r] - sh[r]);
            let ds: [f64; 6] = std::array::from_fn(|r| s[r] - ss[r]);
            en += w * energy(&d, &de);
            raw += w * tensor_norm(&dr).powi(2);
            recl2 += w * tensor_norm(&ds).powi(2);
        }
    }
    let true_energy = en.sqrt();
    let m = Measured {
        n,
        theta: est.estimate() / true_energy,
        eta: est.estimate(),
        true_energy,
        raw_l2: raw.sqrt(),
        rec_l2: recl2.sqrt(),
    };
    println!(
        "  {} n = {n:2}: eta {:.5e}, true {:.5e}, theta {:.5}, raw L2 {:.5e}, recovered L2 {:.5e}, relative estimate {:.4e}, \
         {} patches, {} short",
        mesh.element_kind(0).name(),
        m.eta,
        m.true_energy,
        m.theta,
        m.raw_l2,
        m.rec_l2,
        est.relative(),
        rec.patches().len(),
        rec.short_patches().len()
    );
    m
}

fn run(kind: ElementKind, ns: &[usize], lambda_sign: f64, solver: StaticSolver) -> Vec<Measured> {
    ns.iter()
        .map(|&n| {
            let t0 = Instant::now();
            let (model, result) = solve_case_d(&Trig, cube(kind, n), lambda_sign, solver.clone());
            let m = measure(&Trig, &model, &result, n, None);
            println!("    ({:.2} s)", t0.elapsed().as_secs_f64());
            m
        })
        .collect()
}

fn order(a: f64, b: f64) -> f64 {
    (a / b).log2()
}

/// M1 and M2 of one element (and M5 recorded).
fn check_effectivity(c: &mut Checks, kind: ElementKind, levels: &[Measured]) {
    let name = kind.name();
    for m in levels {
        c.within(&format!("M1 {name} n = {} theta", m.n), m.theta, 0.8, 1.2);
    }
    for w in levels.windows(2) {
        let (a, b) = ((w[0].theta - 1.0).abs(), (w[1].theta - 1.0).abs());
        c.holds(
            &format!("M2 {name} |theta - 1| does not grow {} -> {}", w[0].n, w[1].n),
            b <= a,
            &format!("{a:.5} -> {b:.5}"),
        );
    }
    let (a, b) = (&levels[levels.len() - 2], &levels[levels.len() - 1]);
    println!(
        "M5 {name}: recovered L2 order {} -> {}: {:.4}, raw {:.4}; energy order {:.4}",
        a.n,
        b.n,
        order(a.rec_l2, b.rec_l2),
        order(a.raw_l2, b.raw_l2),
        order(a.true_energy, b.true_energy)
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// L1, L2: polynomial fields

/// The 3 x 3 x 3 box of a kind with the 8 interior grid nodes moved by uniform +-0.15 h per coordinate.
fn moved_box(kind: ElementKind, tag: u64) -> SolidMesh {
    let base_kind = match kind {
        ElementKind::Tet4 | ElementKind::Tet10 => ElementKind::Tet4,
        _ => ElementKind::Hex8,
    };
    let base = cube(base_kind, 3);
    let h = 1.0 / 3.0;
    let mut rng = Rng::new(tag);
    let nodes: Vec<Vec3> = base
        .nodes()
        .iter()
        .map(|&x| {
            if x.iter().all(|&v| v > 1e-9 && v < 1.0 - 1e-9) {
                [0, 1, 2].map(|i| x[i] + 0.15 * h * (2.0 * rng.uniform() - 1.0))
            } else {
                x
            }
        })
        .collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(base_kind, base.blocks()[0].connectivity().to_vec()).unwrap();
    mesh.add_element_set("all", (0..mesh.n_elements()).collect()).unwrap();
    match kind {
        ElementKind::Tet10 => upgrade_to_tet10(&mesh).unwrap().0,
        ElementKind::Hex20 => upgrade_to_hex20(&mesh).unwrap().0,
        _ => mesh,
    }
}

/// (max nodal recovered-stress error over the largest nodal stress, estimate over the exact energy norm).
fn polynomial_errors(field: &dyn Field, mesh: SolidMesh) -> (f64, f64) {
    let model = model_of(mesh);
    let u = displacement_of(&model, field);
    let (rec, est) = energy_error(&model, &u, &PatchOptions::new()).unwrap();
    let mesh = model.mesh();
    let mut worst: f64 = 0.0;
    let mut largest: f64 = 0.0;
    for (n, &x) in mesh.nodes().iter().enumerate() {
        let s = exact_stress(field, x);
        let r = rec.nodal()[n].unwrap();
        let diff: [f64; 6] = std::array::from_fn(|k| r[k] - s[k]);
        worst = worst.max(tensor_norm(&diff));
        largest = largest.max(tensor_norm(&s));
    }
    // The exact energy norm with the error rules.
    let d = d_matrix();
    let mut en = 0.0;
    for e in 0..mesh.n_elements() {
        let kind = mesh.element_kind(e);
        let element = element_for(kind);
        let coords: Vec<Vec3> = mesh.element_nodes(e).iter().map(|&a| mesh.nodes()[a]).collect();
        for (xi, wq) in error_rule(matches!(kind, ElementKind::Hex8 | ElementKind::Hex20)) {
            let p = element.point(&coords, xi).unwrap();
            en += wq * p.det_j * energy(&d, &voigt_strain(&field.grad(p.x)));
        }
    }
    (worst / largest, est.estimate() / en.sqrt())
}

#[test]
fn l1_l2_polynomial_fields() {
    let mut c = Checks::new();
    for (tag, kind) in [ElementKind::Hex8, ElementKind::Tet4, ElementKind::Tet10, ElementKind::Hex20]
        .into_iter()
        .enumerate()
    {
        let (s, e) = polynomial_errors(&Poly::linear(), moved_box(kind, tag as u64));
        c.at_most(&format!("L1 {} linear field: nodal stress", kind.name()), s, 1e-10);
        c.at_most(&format!("L1 {} linear field: estimate", kind.name()), e, 1e-10);
    }
    let (s, e) = polynomial_errors(&Poly::quadratic(), moved_box(ElementKind::Tet10, 2));
    c.at_most("L2 Tet10 quadratic field: nodal stress", s, 1e-10);
    c.at_most("L2 Tet10 quadratic field: estimate", e, 1e-10);
    let (s, e) = polynomial_errors(&Poly::quadratic(), cube(ElementKind::Hex20, 3));
    c.at_most("L2 Hex20 quadratic field: nodal stress", s, 1e-10);
    c.at_most("L2 Hex20 quadratic field: estimate", e, 1e-10);
    c.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// M1 to M5, N1, F1

#[test]
fn m1_to_m4_hex8_and_n1() {
    let mut c = Checks::new();
    let levels = run(ElementKind::Hex8, &[4, 8, 16], 1.0, StaticSolver::Direct);
    check_effectivity(&mut c, ElementKind::Hex8, &levels);
    let (a, b) = (&levels[1], &levels[2]);
    c.at_least("M3 Hex8 recovered stress L2 order 8 -> 16", order(a.rec_l2, b.rec_l2), 1.7);
    c.at_least("M4 Hex8 raw stress L2 order 8 -> 16", order(a.raw_l2, b.raw_l2), 0.9);
    let neg = run(ElementKind::Hex8, &[16], -1.0, StaticSolver::Direct);
    let ok = neg[0].theta < 0.5;
    c.holds("N1 Hex8 n = 16, lambda reversed: theta < 0.5", ok, &format!("{:.4}", neg[0].theta));
    c.finish();
}

#[test]
fn f1_tet10_hex20_fast() {
    let mut c = Checks::new();
    for kind in [ElementKind::Tet10, ElementKind::Hex20] {
        for m in run(kind, &[2, 4], 1.0, StaticSolver::Direct) {
            let ok = m.theta > 0.5 && m.theta < 2.0;
            c.holds(&format!("F1 {} n = {} theta in (0.5, 2.0)", kind.name(), m.n), ok, &format!("{:.4}", m.theta));
        }
    }
    c.finish();
}

#[test]
#[ignore = "heavy record of ADR 0478 (Tet10 n = 4, 8, 16; 89 373 unknowns at 16), run once"]
fn m1_m2_m5_tet10_heavy() {
    let mut c = Checks::new();
    let levels = run(ElementKind::Tet10, &[4, 8, 16], 1.0, StaticSolver::SparseDirect);
    check_effectivity(&mut c, ElementKind::Tet10, &levels);
    c.finish();
}

#[test]
#[ignore = "heavy record of ADR 0478 (Hex20 n = 4, 8, 16; 42 525 unknowns at 16), run once"]
fn m1_m2_m5_hex20_heavy() {
    let mut c = Checks::new();
    let levels = run(ElementKind::Hex20, &[4, 8, 16], 1.0, StaticSolver::SparseDirect);
    check_effectivity(&mut c, ElementKind::Hex20, &levels);
    c.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// The plate with a hole (ADR 0449) with two layers

const HR: f64 = 0.005;
const HA: f64 = 0.1;
const HB: f64 = 0.05;
const HT: f64 = 0.001;
const S0: f64 = 50e6;

fn plate_map(p: Vec3) -> Vec3 {
    let (s, q) = (p[0], p[1]);
    let (circle, outer) = if q == 1.0 {
        ([0.0, HR], [0.0, HB])
    } else {
        let th = 0.5 * PI * q;
        let ell = q * (HA + HB);
        let o = if ell <= HB { [HA, ell] } else { [HA - (ell - HB), HB] };
        ([HR * th.cos(), HR * th.sin()], o)
    };
    let g = ((4.0 * s).exp() - 1.0) / (4f64.exp() - 1.0);
    [
        circle[0] + g * (outer[0] - circle[0]),
        circle[1] + g * (outer[1] - circle[1]),
        p[2],
    ]
}

fn plate(layers: usize) -> SolidModel {
    let (mut mesh, _) =
        upgrade_to_hex20_mapped(&hex_box([0.0; 3], [1.0, 1.0, HT], [24, 24, layers]).unwrap(), plate_map).unwrap();
    let right: Vec<_> = mesh
        .surface_set("xmax")
        .unwrap()
        .iter()
        .copied()
        .filter(|f| {
            let nodes = mesh.element_nodes(f.element);
            faces()[f.face]
                .iter()
                .all(|&a| (mesh.nodes()[nodes[a]][0] - HA).abs() <= 1e-12)
        })
        .collect();
    mesh.add_surface_set("right", right).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2e11, 0.0, 7850.0, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymax", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m.add_load(Load::traction("right", [S0, 0.0, 0.0])).unwrap();
    m
}

fn solved_plate() -> (SolidModel, Vec<f64>) {
    let model = plate(2);
    let r = solve_static(&model, &StaticOptions::with_solver(StaticSolver::SparseDirect)).unwrap();
    let u = r.displacement_values().to_vec();
    (model, u)
}

fn on_hole(x: Vec3) -> bool {
    (x[0].hypot(x[1]) - HR).abs() <= 1e-9
}

#[test]
fn p1_p2_plate_with_a_hole() {
    let mut c = Checks::new();
    let t0 = Instant::now();
    let (model, u) = solved_plate();
    let mesh = model.mesh();
    println!(
        "plate: {} elements, {} nodes, {} equations, solved in {:.2} s",
        mesh.n_elements(),
        mesh.nodes().len(),
        u.len(),
        t0.elapsed().as_secs_f64()
    );
    let (rec, est) = energy_error(&model, &u, &PatchOptions::new()).unwrap();
    println!(
        "plate: {} patches, {} short, estimate {:.4e}, relative {:.4e}",
        rec.patches().len(),
        rec.short_patches().len(),
        est.estimate(),
        est.relative()
    );
    let field = IntegrationPointField::compute(&model, &u).unwrap();
    let avg = nodal_average(&model, &field, field.stress(), None).unwrap();
    let peak = |v: &dyn Fn(usize) -> f64| -> (usize, f64) {
        let mut best = (0, f64::NEG_INFINITY);
        for n in 0..mesh.nodes().len() {
            let s = v(n);
            if s > best.1 {
                best = (n, s);
            }
        }
        best
    };
    let (nr, sr) = peak(&|n| rec.nodal()[n].unwrap()[0]);
    let (na, sa) = peak(&|n| avg[n].unwrap()[0]);
    println!(
        "P1 record: nodal average (F-376) peak sigma_xx {:.6e} at node {na} {:?}: {:+.4e} of 3 sigma_0",
        sa,
        mesh.nodes()[na],
        sa / (3.0 * S0) - 1.0
    );
    println!("P1 record: recovered peak at node {nr} {:?}, sources {:?}", mesh.nodes()[nr], rec.sources(nr));
    c.at_most("P1 recovered peak sigma_xx against 3 sigma_0", (sr / (3.0 * S0) - 1.0).abs(), 2e-2);
    println!("P1 signed: {:+.4e}", sr / (3.0 * S0) - 1.0);
    // P2: the hole-edge elements among the tenth of largest indicator.
    let edge: Vec<usize> = (0..mesh.n_elements())
        .filter(|&e| {
            let nodes = mesh.element_nodes(e);
            mesh.element_kind(e)
                .faces()
                .iter()
                .any(|f| f.iter().all(|&a| on_hole(mesh.nodes()[nodes[a]])))
        })
        .collect();
    let ranked = est.ranked();
    let tenth = mesh.n_elements().div_ceil(10);
    let top: BTreeSet<usize> = ranked[..tenth].iter().copied().collect();
    let rank_of = |e: usize| ranked.iter().position(|&r| r == e).unwrap();
    let mut ranks: Vec<usize> = edge.iter().map(|&e| rank_of(e)).collect();
    ranks.sort_unstable();
    println!("P2 record: {} hole-edge elements, tenth = {tenth}; their ranks {ranks:?}", edge.len());
    let ee = est.element_errors();
    let edge_max = edge.iter().map(|&e| ee[e]).fold(0.0, f64::max);
    println!(
        "P2 record: largest indicator {:.4e} (element {}), largest at the hole edge {:.4e}, indicator of rank {tenth} {:.4e}",
        ee[ranked[0]],
        ranked[0],
        edge_max,
        ee[ranked[tenth - 1]]
    );
    let missing = edge.iter().filter(|e| !top.contains(e)).count();
    c.holds(
        "P2 the tenth of largest indicator contains the hole-edge elements",
        edge.len() == 48 && missing == 0,
        &format!("{} of {} hole-edge elements outside the tenth", missing, edge.len()),
    );
    c.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// B1, T1

fn distance(a: Vec3, b: Vec3) -> f64 {
    let (dx, dy, dz) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// B1 on one model: every node's sources by a search over all solved centres, and its value bit for bit.
fn check_rule(c: &mut Checks, tag: &str, model: &SolidModel, rec: &Recovery) {
    let mesh = model.mesh();
    let centres: Vec<(usize, Vec3)> = rec.patches().iter().map(|p| (p.centre(), mesh.nodes()[p.centre()])).collect();
    let (mut bad_sources, mut bad_values, mut ties) = (0, 0, 0);
    for (n, &x) in mesh.nodes().iter().enumerate() {
        let expect: Vec<usize> = if rec.patch(n).is_some() {
            vec![n]
        } else {
            let best = centres.iter().map(|&(_, y)| distance(x, y)).fold(f64::INFINITY, f64::min);
            centres
                .iter()
                .filter(|&&(_, y)| distance(x, y) <= best * (1.0 + estimate::TIE_TOLERANCE))
                .map(|&(k, _)| k)
                .collect()
        };
        if expect.len() > 1 {
            ties += 1;
        }
        if rec.sources(n) != expect.as_slice() {
            bad_sources += 1;
            continue;
        }
        let mut s = [0.0; 6];
        for &k in &expect {
            let v = rec.patch(k).unwrap().evaluate(x);
            for (sc, vc) in s.iter_mut().zip(&v) {
                *sc += vc;
            }
        }
        let count = expect.len() as f64;
        let s = s.map(|v| v / count);
        let got = rec.nodal()[n].unwrap();
        if s.map(f64::to_bits) != got.map(f64::to_bits) {
            bad_values += 1;
        }
    }
    c.holds(
        &format!("B1 {tag}: sources are the nearest solved centres and values their mean, bit for bit"),
        bad_sources == 0 && bad_values == 0,
        &format!(
            "{} nodes, {} centres, {ties} with ties; {bad_sources} wrong sources, {bad_values} wrong values",
            mesh.nodes().len(),
            centres.len()
        ),
    );
}

fn check_threads(c: &mut Checks, tag: &str, model: &SolidModel, u: &[f64]) -> Recovery {
    let (r1, e1) = energy_error(model, u, &PatchOptions::new().threads(Some(1))).unwrap();
    let (r8, e8) = energy_error(model, u, &PatchOptions::new().threads(Some(8))).unwrap();
    let nodal_equal = r1
        .nodal()
        .iter()
        .zip(r8.nodal())
        .all(|(a, b)| a.map(|v| v.map(f64::to_bits)) == b.map(|v| v.map(f64::to_bits)));
    let elements_equal = e1
        .element_errors()
        .iter()
        .zip(e8.element_errors())
        .all(|(a, b)| a.to_bits() == b.to_bits());
    let global = e1.estimate().to_bits() == e8.estimate().to_bits();
    c.holds(
        &format!("T1 {tag}: 1 and 8 threads bit-identical"),
        nodal_equal && elements_equal && global && r1 == r8,
        &format!("nodal {nodal_equal}, elements {elements_equal}, estimate {global}"),
    );
    r8
}

#[test]
fn b1_t1_rule_and_threads() {
    let mut c = Checks::new();
    for kind in [ElementKind::Tet10, ElementKind::Hex20] {
        let (model, result) = solve_case_d(&Trig, cube(kind, 4), 1.0, StaticSolver::Direct);
        let rec = check_threads(&mut c, kind.name(), &model, result.displacement_values());
        check_rule(&mut c, kind.name(), &model, &rec);
    }
    let (model, u) = solved_plate();
    let rec = check_threads(&mut c, "plate", &model, &u);
    check_rule(&mut c, "plate", &model, &rec);
    c.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// S1 to S3, R1

fn refused(r: &rcfd_fem::Result<impl std::fmt::Debug>, key: &str, texts: &[&str]) -> (bool, String) {
    match r {
        Err(e @ FemError::InvalidValue { key: k, .. }) => {
            let msg = e.to_string();
            (*k == key && texts.iter().all(|t| msg.contains(t)), msg)
        }
        Err(e) => (false, format!("other error: {e}")),
        Ok(v) => (false, format!("not refused: {v:?}")),
    }
}

#[test]
fn s1_to_s3_short_patches() {
    let mut c = Checks::new();
    // S1: tet_box 2 x 2 x 2 with element 0 split by its centroid.
    let base = tet_box([0.0; 3], [1.0; 3], [2; 3]).unwrap();
    let conn = base.blocks()[0].connectivity().to_vec();
    let v: Vec<usize> = conn[..4].to_vec();
    let mut nodes = base.nodes().to_vec();
    let p = nodes.len();
    let centroid: Vec3 = [0, 1, 2].map(|i| v.iter().map(|&a| nodes[a][i]).sum::<f64>() / 4.0);
    nodes.push(centroid);
    let mut new_conn = conn[4..].to_vec();
    for k in 0..4 {
        let mut t = v.clone();
        t[k] = p;
        new_conn.extend_from_slice(&t);
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Tet4, new_conn).unwrap();
    mesh.add_element_set("all", (0..mesh.n_elements()).collect()).unwrap();
    let centre = mesh.nodes().iter().position(|&x| x == [0.5, 0.5, 0.5]).unwrap();
    let model = model_of(mesh);
    let u = displacement_of(&model, &Poly::linear());
    let rec = recover(&model, &u, &PatchOptions::new().basis(PatchBasis::Quadratic)).unwrap();
    let want = vec![ShortPatch {
        node: p,
        points: 4,
        terms: 10,
        reason: "too few points",
    }];
    c.holds("S1 the split node is reported short", rec.short_patches() == want.as_slice(), &format!("{:?}", rec.short_patches()));
    c.holds("S1 the split node has no fit", rec.patch(p).is_none(), "");
    let fit = rec.patch(centre);
    c.holds(
        "S1 the box centre is solved with 26 points",
        fit.map(|f| (f.n_points(), f.elements().len())) == Some((26, 26)),
        &format!("{:?}", fit.map(|f| (f.n_points(), f.elements().len()))),
    );
    c.holds("S1 the split node takes the centre's fit", rec.sources(p) == [centre], &format!("{:?}", rec.sources(p)));
    // S2: Hex8 3 x 3 x 3 with the quadratic basis.
    let model = model_of(cube(ElementKind::Hex8, 3));
    let u = displacement_of(&model, &Poly::linear());
    let r = recover(&model, &u, &PatchOptions::new().basis(PatchBasis::Quadratic));
    let mesh = model.mesh();
    let interior: Vec<String> = (0..mesh.nodes().len())
        .filter(|&n| mesh.nodes()[n].iter().all(|&v| v > 1e-9 && v < 1.0 - 1e-9))
        .map(|n| format!("node {n}: 8 points for 10 terms, too few points"))
        .collect();
    let mut texts: Vec<&str> = interior.iter().map(String::as_str).collect();
    texts.push("8 short patches");
    let (ok, msg) = refused(&r, "patch recovery", &texts);
    c.holds("S2 Hex8 quadratic basis refused, the 8 short patches named", ok && interior.len() == 8, &msg);
    // S3: the single-layer plate.
    let model = plate(1);
    let u = vec![0.0; model.dof_map().n_equations()];
    let (ok, msg) = refused(&recover(&model, &u, &PatchOptions::new()), "patch recovery", &["0 interior corner nodes"]);
    c.holds("S3 single-layer plate refused (no interior corner node)", ok, &msg);
    c.finish();
}

#[test]
fn r1_refusals() {
    let mut c = Checks::new();
    let model = model_of(cube(ElementKind::Hex8, 2));
    let r = recover(&model, &[0.0; 5], &PatchOptions::new());
    let ok = matches!(r, Err(FemError::FieldLength { key: "displacement", .. }));
    c.holds("R1 displacement of the wrong length", ok, &format!("{r:?}"));
    // A Hex8 block with a beam.
    let hex = hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap();
    let mut nodes = hex.nodes().to_vec();
    let on_block = nodes.iter().position(|y| *y == [2.0, 0.0, 0.0]).unwrap();
    nodes.push([3.0, 0.0, 0.0]);
    let free = nodes.len() - 1;
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, hex.blocks()[0].connectivity().to_vec()).unwrap();
    mesh.add_block(ElementKind::Beam2, vec![free, on_block]).unwrap();
    let section = BeamSection::new(0.012, 3.0e-6, 5.0e-6, 4.0e-6, 0.85, 0.75).unwrap();
    mesh.set_block_property(1, BlockProperty::beam(section, [0.0, 0.0, 1.0]).unwrap()).unwrap();
    mesh.add_element_set("all", vec![0, 1, 2]).unwrap();
    let model = model_of(mesh);
    let u = vec![0.0; model.dof_map().n_equations()];
    let r = recover(&model, &u, &PatchOptions::new());
    let ok = matches!(r, Err(FemError::UnsupportedKind { block: 1, kind: "Beam2", .. }));
    c.holds("R1 a Beam2 block", ok, &format!("{:?}", r.err()));
    // A temperature field.
    let mut model = model_of(cube(ElementKind::Hex8, 2));
    let nn = model.mesh().nodes().len();
    model.add_load(Load::temperature(vec![1.0; nn], 0.0)).unwrap();
    let u = vec![0.0; model.dof_map().n_equations()];
    let r = energy_error(&model, &u, &PatchOptions::new());
    let ok = matches!(&r, Err(FemError::InvalidValue { key: "temperature", .. }));
    c.holds("R1 a temperature field", ok, &format!("{:?}", r.err()));
    // Two materials meeting at the centre node of a 2 x 2 x 2 box.
    let mut mesh = cube(ElementKind::Hex8, 2);
    let (left, right): (Vec<usize>, Vec<usize>) = (0..mesh.n_elements()).partition(|&e| {
        let nodes = mesh.element_nodes(e);
        nodes.iter().map(|&a| mesh.nodes()[a][0]).sum::<f64>() / 8.0 < 0.5
    });
    mesh.add_element_set("soft", left).unwrap();
    mesh.add_element_set("hard", right).unwrap();
    let centre = mesh.nodes().iter().position(|&x| x == [0.5, 0.5, 0.5]).unwrap();
    let mut model = SolidModel::new(mesh);
    model.add_material("soft", Material::isotropic_elastic(1.0, NU, 0.0, 0.0).unwrap()).unwrap();
    model.add_material("hard", Material::isotropic_elastic(2.0, NU, 0.0, 0.0).unwrap()).unwrap();
    let u = vec![0.0; model.dof_map().n_equations()];
    let node_text = format!("node {centre}");
    let (ok, msg) = refused(&recover(&model, &u, &PatchOptions::new()), "patch recovery", &[&node_text, "'soft'", "'hard'"]);
    c.holds("R1 a patch across a material interface", ok, &msg);
    c.finish();
}

#[test]
fn rule_tables() {
    // The registered tables of D1 (not a numbered check; guards the tables the checks rely on).
    assert_eq!(estimate::sampling_rule(ElementKind::Hex8).unwrap().len(), 1);
    assert_eq!(estimate::sampling_rule(ElementKind::Hex20).unwrap().len(), 8);
    assert_eq!(estimate::sampling_rule(ElementKind::Tet4).unwrap().len(), 1);
    assert_eq!(estimate::sampling_rule(ElementKind::Tet10).unwrap().len(), 4);
    assert_eq!(estimate::default_basis(ElementKind::Hex8), Some(PatchBasis::Linear));
    assert_eq!(estimate::default_basis(ElementKind::Tet10), Some(PatchBasis::Quadratic));
    assert!(estimate::sampling_rule(ElementKind::Shell4).is_none());
}
